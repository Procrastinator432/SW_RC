//! Offline scene assembly from original map and mesh packages, with explicit omissions.
use rc_package::{
    geometry::Triangle,
    mesh::{read_static_mesh, transform_point},
    properties::{self, Value},
    read_package,
};
use std::{
    collections::HashMap,
    env, fs,
    path::{Path, PathBuf},
};
#[path = "../assets.rs"]
mod assets;
#[path = "../defaults.rs"]
mod defaults;
struct MeshAsset {
    triangles: Vec<Triangle>,
    uv: Vec<Option<[[f32; 2]; 3]>>,
    materials: Vec<Option<String>>,
    package: String,
}
fn mesh_asset(mesh: rc_package::mesh::StaticMesh, package: &str) -> Result<MeshAsset, String> {
    let triangles = mesh.triangles()?;
    let stream = mesh.uv_streams.first();
    let mut uv = Vec::new();
    for section in &mesh.sections {
        let first = section.first_index as usize;
        for face in mesh.indices[first..first + section.face_count as usize * 3].chunks_exact(3) {
            uv.push(stream.and_then(|s| {
                Some([
                    *s.get(face[0] as usize)?,
                    *s.get(face[1] as usize)?,
                    *s.get(face[2] as usize)?,
                ])
            }));
        }
    }
    Ok(MeshAsset {
        triangles,
        uv,
        materials: mesh.materials,
        package: package.into(),
    })
}
fn main() -> Result<(), Box<dyn std::error::Error>> {
    let args: Vec<_> = env::args_os().collect();
    if args.len() != 4 {
        return Err("Usage: rc-scene <map.ctm> <StaticMeshes-directory> <output-directory>".into());
    }
    let data = fs::read(&args[1])?;
    let pkg = read_package(&data)?;
    let out = Path::new(&args[3]);
    fs::create_dir_all(out)?;
    let packages: HashMap<String, PathBuf> = fs::read_dir(&args[2])?
        .filter_map(|e| e.ok())
        .map(|e| e.path())
        .filter(|p| p.extension().is_some_and(|e| e.eq_ignore_ascii_case("usx")))
        .filter_map(|p| Some((p.file_stem()?.to_str()?.to_lowercase(), p.clone())))
        .collect();
    let mut loaded = HashMap::new();
    let mut mesh_cache: HashMap<String, Result<MeshAsset, String>> = HashMap::new();
    let texture_dir = Path::new(&args[2])
        .parent()
        .ok_or("Mesh directory has no parent")?
        .join("Textures");
    let mut assets = assets::Assets::new(&[Path::new(&args[2]), &texture_dir])?;
    let mut texture_cache: HashMap<String, Option<(usize, f32)>> = HashMap::new();
    let mut texture_report = Vec::new();
    let mut scene = rc_render::Scene::from_map(&data)?;
    let game_data = Path::new(&args[2])
        .parent()
        .ok_or("Mesh directory has no GameData parent")?;
    let catalog = defaults::load(game_data)?;
    scene.starts = rc_package::level::player_starts_with_defaults(
        &pkg,
        &data,
        &catalog,
        &args[1].to_string_lossy(),
    )?;
    let bsp_triangles = scene.triangles.len();
    scene.appearance = vec![None; bsp_triangles];
    let mut instances = Vec::new();
    let mut omitted = Vec::new();
    for (i, e) in pkg.exports.iter().enumerate() {
        if e.class == 0 {
            continue;
        }
        let props = properties::read(&pkg, &data, e)?;
        let prop = |name: &str| {
            props
                .values
                .iter()
                .find(|p| p.name == name && p.array_index == 0)
                .map(|p| &p.value)
        };
        let Some(Value::Object { index, path }) = prop("StaticMesh") else {
            continue;
        };
        if *index == 0 {
            continue;
        }
        let actor = pkg.object_path(i as i32 + 1)?;
        let class = pkg.object_path(e.class)?;
        // Exact class only: subclass defaults must be resolved before including subclasses.
        if class != "Engine.StaticMeshActor" {
            omitted.push(serde_json::json!({"actor":actor,"class":class,"reason":"Subclass defaults not resolved"}));
            continue;
        }
        if matches!(prop("bHidden"), Some(Value::Bool(true))) {
            omitted.push(serde_json::json!({"actor":actor,"reason":"bHidden"}));
            continue;
        }
        let location = if let Some(Value::Vector(v)) = prop("Location") {
            *v
        } else {
            [0.0; 3]
        };
        let rotation = if let Some(Value::Rotator(v)) = prop("Rotation") {
            *v
        } else {
            [0; 3]
        };
        let pivot = if let Some(Value::Vector(v)) = prop("PrePivot") {
            *v
        } else {
            [0.0; 3]
        };
        let draw_scale = if let Some(Value::Float(v)) = prop("DrawScale") {
            *v
        } else {
            1.0
        };
        let scale = if let Some(Value::Vector(v)) = prop("DrawScale3D") {
            v.map(|x| x * draw_scale)
        } else {
            [draw_scale; 3]
        };
        if !mesh_cache.contains_key(path) {
            let result = (|| -> Result<MeshAsset, String> {
                if *index > 0 {
                    let mesh = pkg
                        .exports
                        .get(*index as usize - 1)
                        .ok_or("Invalid local mesh reference")?;
                    if pkg.object_path(mesh.class)? != "Engine.StaticMesh" {
                        return Err("Referenced object is not a StaticMesh".into());
                    }
                    return mesh_asset(
                        read_static_mesh(&pkg, &data, mesh)?,
                        Path::new(&args[1])
                            .file_stem()
                            .and_then(|p| p.to_str())
                            .ok_or("Invalid map name")?,
                    );
                }
                let (package, object) = path
                    .split_once('.')
                    .ok_or("Incomplete external mesh path")?;
                let key = package.to_lowercase();
                if !loaded.contains_key(&key) {
                    let file = packages
                        .get(&key)
                        .ok_or_else(|| format!("Missing mesh package {package}"))?;
                    let bytes = fs::read(file).map_err(|e| e.to_string())?;
                    let package = read_package(&bytes)?;
                    loaded.insert(key.clone(), (package, bytes));
                }
                let (mesh_pkg, bytes) = &loaded[&key];
                let mesh_export = mesh_pkg
                    .exports
                    .iter()
                    .enumerate()
                    .find_map(|(i, e)| {
                        (mesh_pkg
                            .object_path(i as i32 + 1)
                            .ok()?
                            .eq_ignore_ascii_case(object)
                            && mesh_pkg.object_path(e.class).ok()? == "Engine.StaticMesh")
                            .then_some(e)
                    })
                    .ok_or_else(|| format!("Missing StaticMesh {path}"))?;
                mesh_asset(read_static_mesh(mesh_pkg, bytes, mesh_export)?, package)
            })();
            mesh_cache.insert(path.clone(), result);
        }
        match &mesh_cache[path] {
            Ok(mesh) => {
                let triangles = &mesh.triangles;
                let mut section_textures = Vec::new();
                for material in &mesh.materials {
                    let Some(material) = material else {
                        section_textures.push(None);
                        continue;
                    };
                    let path = assets.qualify(&mesh.package, material);
                    if !texture_cache.contains_key(&path) {
                        match assets.diffuse(&path) {
                            Ok(diffuse) => {
                                let texture = diffuse.texture;
                                let index = scene.textures.len();
                                texture_report.push(serde_json::json!({"material":path,"texture":diffuse.source,"width":texture.width,"height":texture.height,"uv_scale":diffuse.scale,"chain":diffuse.chain,"mode":"base diffuse preview; no shader effects"}));
                                scene.textures.push(texture);
                                texture_cache.insert(path.clone(), Some((index, diffuse.scale)));
                            }
                            Err(error) => {
                                texture_report
                                    .push(serde_json::json!({"material":path,"error":error}));
                                texture_cache.insert(path.clone(), None);
                            }
                        }
                    }
                    section_textures.push(texture_cache[&path]);
                }
                scene
                    .appearance
                    .extend(triangles.iter().enumerate().map(|(i, t)| {
                        let (texture, scale) =
                            section_textures.get(t.surface).copied().flatten()?;
                        Some(rc_render::FaceTexture {
                            uv: mesh.uv[i]?.map(|uv| uv.map(|v| v * scale)),
                            texture,
                        })
                    }));
                let color_base = 200 + instances.len() * 16;
                scene.triangles.extend(triangles.iter().map(|t| {
                    Triangle {
                        points: t
                            .points
                            .map(|p| transform_point(p, location, rotation, scale, pivot)),
                        surface: color_base + t.surface,
                    }
                }));
                let collision_properties: Vec<_> = props
                    .values
                    .iter()
                    .filter(|p| {
                        [
                            "bCollideActors",
                            "bCollideWorld",
                            "bBlockActors",
                            "bBlockPlayers",
                            "bWorldGeometry",
                            "bUseCylinderCollision",
                            "CollisionRadius",
                            "CollisionHeight",
                        ]
                        .contains(&p.name.as_str())
                    })
                    .collect();
                instances.push(serde_json::json!({"actor":actor,"mesh":path,"triangles":triangles.len(),"location":location,"rotation":rotation,"scale":scale,"pre_pivot":pivot,"explicit_collision_properties":collision_properties}));
            }
            Err(error) => {
                omitted.push(serde_json::json!({"actor":actor,"mesh":path,"reason":error}))
            }
        }
    }
    // Recompute full scene bounds from all original world-space triangles.
    let snapshot = scene.snapshot()?;
    fs::write(out.join("world.rcscene"), &snapshot)?;
    let snapshot_scene = rc_render::Scene::from_snapshot(&snapshot)?;
    let pixels = snapshot_scene.render(960, 640, -0.7, 0.65, 1.0)?;
    let mut ppm = b"P6\n960 640\n255\n".to_vec();
    for p in pixels {
        ppm.extend_from_slice(&[(p >> 16) as u8, (p >> 8) as u8, p as u8]);
    }
    fs::write(out.join("world.ppm"), ppm)?;
    let mut probes = Vec::new();
    for (index, start) in snapshot_scene.starts.iter().enumerate() {
        if let Ok(camera) = snapshot_scene.start_view(index) {
            let pixels = snapshot_scene.render_view(960, 640, camera)?;
            let mut ppm = b"P6\n960 640\n255\n".to_vec();
            for p in pixels {
                ppm.extend_from_slice(&[(p >> 16) as u8, (p >> 8) as u8, p as u8]);
            }
            fs::write(out.join(format!("start-{index}.ppm")), ppm)?;
        }
        for (name, direction) in [
            ("+X", [1., 0., 0.]),
            ("-X", [-1., 0., 0.]),
            ("+Y", [0., 1., 0.]),
            ("-Y", [0., -1., 0.]),
            ("+Z", [0., 0., 1.]),
            ("-Z", [0., 0., -1.]),
        ] {
            let from = start.location.map(f64::from);
            let end = std::array::from_fn(|i| from[i] + direction[i] * 10000.0);
            let hit = snapshot_scene.trace_geometry(from, end)?;
            probes.push(serde_json::json!({"anchor":start.actor,"direction":name,"length":10000,"hit":hit.map(|h| serde_json::json!({"triangle":h.triangle,"distance":h.fraction*10000.0,"position":h.position})),"scope":"Visible triangles only; not original collision"}));
        }
    }
    fs::write(
        out.join("scene.json"),
        serde_json::to_vec_pretty(&serde_json::json!({
            "source_map":args[1].to_string_lossy(),"mesh_directory":args[2].to_string_lossy(),"bsp_triangles":bsp_triangles,"triangles":scene.triangles.len(),
            "snapshot_bytes":snapshot.len(),"instances":instances,"omitted":omitted,"materials":texture_report,"textures":scene.textures.len(),"textured_triangles":scene.appearance.iter().filter(|a|a.is_some()).count(),
            "player_starts":snapshot_scene.starts,"geometry_probes":probes,"player_start_defaults_resolved":true,"class_catalog_classes":catalog.len(),
            "scope":"World BSP + exact Engine.StaticMeshActor instances; basic mesh diffuse textures, no subclass defaults, terrain, shader effects, animation or gameplay"
        }))?,
    )?;
    println!(
        "{} instances; {} omissions; {} triangles; {} bytes",
        instances.len(),
        omitted.len(),
        scene.triangles.len(),
        snapshot.len()
    );
    Ok(())
}
