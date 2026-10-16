//! Repair the existing mission's missing surface bindings and DC15s fire sample.
//! Collision, mission scripts and the prepared actor geometry remain unchanged.

use rc_inspect::assets::Assets;
use rc_package::{geometry, properties::Value, read_package};
use std::{collections::HashMap, env, fs, path::Path};
type MaterialBinding = (usize, f32, usize, [f32; 2]);
fn package_object(
    root: &Path,
    folder: &str,
    extension: &str,
    path: &str,
) -> Result<(Vec<u8>, String), String> {
    let (package, object) = path.split_once('.').ok_or("Unqualified asset")?;
    let file = fs::read_dir(root.join(folder))
        .map_err(|e| e.to_string())?
        .filter_map(Result::ok)
        .map(|e| e.path())
        .find(|p| {
            p.file_name().is_some_and(|n| {
                n.to_string_lossy()
                    .eq_ignore_ascii_case(&format!("{package}.{extension}"))
            })
        })
        .ok_or(format!("Missing asset package {package}"))?;
    Ok((fs::read(file).map_err(|e| e.to_string())?, object.into()))
}
fn texture_size(root: &Path, path: &str) -> Result<[f32; 2], String> {
    let (data, object) = package_object(root, "Textures", "utx", path)?;
    let p = read_package(&data)?;
    let e = p
        .exports
        .iter()
        .enumerate()
        .find_map(|(i, e)| {
            p.object_path(i as i32 + 1)
                .is_ok_and(|s| s.eq_ignore_ascii_case(&object))
                .then_some(e)
        })
        .ok_or("Missing texture")?;
    let t = rc_package::texture::read_texture(&p, &data, e)?;
    let m = t.mips.first().ok_or("Missing texture dimensions")?;
    Ok([m.width as f32, m.height as f32])
}
fn main() -> Result<(), Box<dyn std::error::Error>> {
    let args: Vec<_> = env::args_os().collect();
    if ![4, 5].contains(&args.len()) {
        return Err("usage: rc-map2-surfaces GAME_DATA SCENE_DIR OUTPUT_DIR".into());
    }
    let map_name = args
        .get(4)
        .map(|s| s.to_string_lossy().into_owned())
        .unwrap_or_else(|| "geo_01b.ctm".into());
    let root = Path::new(&args[1]);
    let out = Path::new(&args[3]);
    fs::create_dir_all(out)?;
    let mut scene =
        rc_render::Scene::from_snapshot(&fs::read(Path::new(&args[2]).join("world.rcscene"))?)?;
    let mut assets = Assets::new(&[
        &root.join("Textures"),
        &root.join("StaticMeshes"),
        &root.join("System"),
    ])?;
    let map = fs::read(root.join("Maps").join(&map_name))?;
    let pkg = read_package(&map)?;
    let binding = rc_package::level::world_binding(&pkg, &map)?;
    let bsp = geometry::read_bsp(&pkg, &map, &pkg.exports[binding.model_export])?;
    let bsp_triangles: Vec<_> = bsp
        .triangles()?
        .into_iter()
        .filter(|t| bsp.surfaces[t.surface].flags & 0x81 == 0)
        .collect();
    let mut actor_faces = serde_json::Map::new();
    let mut receipts = vec![];
    let mut errors = vec![];
    let mut cache: HashMap<String, Result<MaterialBinding, String>> = HashMap::new();
    let mut resolve = |path: &str| -> Result<(usize, f32, usize, [f32; 2]), String> {
        if let Some(result) = cache.get(path) {
            return result.clone();
        }
        let result = (|| {
            let mut channel = 0;
            let mut source = path.to_owned();
            let mut chain = vec![];
            let mut visited = std::collections::HashSet::new();
            for _ in 0..16 {
                if !visited.insert(source.to_lowercase()) {
                    return Err("Material cycle".into());
                }
                let (class, props) = assets.material_properties(&source)?;
                let field = match class.as_str() {
                    "Engine.TexCoordSource" => {
                        channel = match props
                            .values
                            .iter()
                            .find(|p| p.name == "SourceChannel")
                            .map(|p| &p.value)
                        {
                            Some(Value::Int(v)) if (0..8).contains(v) => *v as usize,
                            None => 0,
                            _ => return Err("Invalid UV source channel".into()),
                        };
                        "Material"
                    }
                    "Engine.Combiner" => {
                        let op = match props
                            .values
                            .iter()
                            .find(|p| p.name == "CombineOperation")
                            .map(|p| &p.value)
                        {
                            Some(Value::Byte(v)) => *v,
                            None => 0,
                            _ => return Err("Invalid combiner operation".into()),
                        };
                        match op {
                            1 | 2 => "Material2",
                            7 => "Mask",
                            _ => "Material1",
                        }
                    }
                    "Engine.Shader" => "Diffuse",
                    "Engine.FinalBlend" | "Engine.TexPanner" | "Engine.ColorModifier" => "Material",
                    _ => break,
                };
                chain.push(class);
                let next = props
                    .values
                    .iter()
                    .find(|p| p.name == field)
                    .ok_or("Missing material input")?;
                source = match &next.value {
                    Value::Object { index, path: next } if *index != 0 => {
                        assets.qualify(source.split('.').next().unwrap(), next)
                    }
                    _ => return Err("Invalid material input".into()),
                };
            }
            let d = assets.diffuse_layer_at_time(&source, path, 128, 0.)?;
            if d.uv_transform.is_some() {
                return Err("Animated UV transform needs separate host".into());
            }
            let size = texture_size(root, &d.source)?;
            let index = scene.textures.len();
            receipts.push(serde_json::json!({"material":path,"diffuse":d.source,"source_channel":channel,"original_size":size,"scale":d.scale,"chain":chain.into_iter().chain(d.chain).collect::<Vec<_>>()}));
            scene.textures.push(d.texture);
            Ok((index, d.scale, channel, size))
        })();
        cache.insert(path.to_owned(), result.clone());
        result
    };
    let mut bsp_bound = 0;
    for (i, t) in bsp_triangles.iter().enumerate() {
        if t.points != scene.triangles[i].points || t.surface != scene.triangles[i].surface {
            return Err("BSP prefix differs from original mission".into());
        }
        let s = &bsp.surfaces[t.surface];
        let path = if s.material > 0 {
            format!("{}.{}", map_name.trim_end_matches(".ctm"), s.material_path)
        } else {
            s.material_path.clone()
        };
        match resolve(&path) {
            Ok((texture, scale, channel, size)) => {
                if channel != 0 {
                    return Err("BSP material requires unsupported UV stream".into());
                }
                let base = bsp.points[s.base as usize];
                let u = bsp.vectors[s.texture_u as usize];
                let v = bsp.vectors[s.texture_v as usize];
                let uv = t.points.map(|p| {
                    let d = std::array::from_fn::<_, 3, _>(|j| p[j] - base[j]);
                    [dot(d, u) / size[0] * scale, dot(d, v) / size[1] * scale]
                });
                scene.appearance[i] = Some(rc_render::FaceTexture { texture, uv });
                bsp_bound += 1;
            }
            Err(e) => errors.push(serde_json::json!({"kind":"bsp","material":path,"error":e})),
        }
    }
    let report: serde_json::Value =
        serde_json::from_slice(&fs::read(Path::new(&args[2]).join("scene.json"))?)?;
    let mut offset = bsp_triangles.len();
    let mut mesh_bound = 0;
    let mut skipped_fx = vec![];
    for instance in report["instances"]
        .as_array()
        .ok_or("Missing instance inventory")?
    {
        let count = instance["triangles"].as_u64().ok_or("No triangle count")? as usize;
        {
            let path = instance["mesh"].as_str().ok_or("No mesh")?;
            let (data, object) = package_object(root, "StaticMeshes", "usx", path)?;
            let p = read_package(&data)?;
            let e = p
                .exports
                .iter()
                .enumerate()
                .find_map(|(i, e)| {
                    p.object_path(i as i32 + 1)
                        .is_ok_and(|s| s.eq_ignore_ascii_case(&object))
                        .then_some(e)
                })
                .ok_or("No mesh export")?;
            let m = rc_package::mesh::read_static_mesh(&p, &data, e)?;
            let mut local = 0;
            for (section_index, section) in m.sections.iter().enumerate() {
                let material = m.materials[section_index]
                    .as_ref()
                    .ok_or("Missing section material")?;
                let material = assets_path(root, material, path.split('.').next().unwrap());
                let start = section.first_index as usize;
                for face in
                    m.indices[start..start + section.face_count as usize * 3].chunks_exact(3)
                {
                    let at = offset + local;
                    local += 1;
                    match resolve(&material) {
                        Ok((texture, scale, channel, _)) => {
                            let stream = m
                                .uv_streams
                                .get(channel)
                                .ok_or("Material UV channel missing on mesh")?;
                            let uv = face.try_into().map(|indices: [u16; 3]| {
                                indices.map(|i| stream[i as usize].map(|v| v * scale))
                            })?;
                            scene.appearance[at] = Some(rc_render::FaceTexture { texture, uv });
                            mesh_bound += 1;
                        }
                        Err(e) => {
                            if e == "Unsupported material class Engine.HsFalloff"
                                || e == "Unsupported material class Engine.ConstantColor"
                            {
                                skipped_fx.push(at);
                            } else {
                                errors.push(serde_json::json!({"kind":"mesh","actor":instance["actor"],"material":material,"error":e}));
                            }
                        }
                    }
                }
            }
            if local != count {
                return Err("Mesh triangle inventory changed".into());
            }
        }
        actor_faces.insert(
            instance["actor"].as_str().ok_or("Actor name")?.into(),
            serde_json::json!((offset..offset + count).collect::<Vec<_>>()),
        );
        offset += count;
    }
    let skipped: std::collections::BTreeSet<_> = skipped_fx.iter().copied().collect();
    for faces in actor_faces.values_mut() {
        let indices: Vec<usize> = serde_json::from_value(faces.clone())?;
        *faces = serde_json::json!(indices
            .into_iter()
            .filter(|i| !skipped.contains(i))
            .map(|i| i - skipped.range(..i).count())
            .collect::<Vec<_>>());
    }
    fs::write(
        out.join("actor-faces.json"),
        serde_json::to_vec(&actor_faces)?,
    )?;
    // Volumetric falloff effects have no opaque diffuse surface. Never draw them
    // as solid diagnostic geometry while their shader is unsupported.
    for &i in skipped_fx.iter().rev() {
        scene.triangles.remove(i);
        scene.appearance.remove(i);
    }
    fs::write(out.join("world.rcscene"), scene.snapshot()?)?;
    fs::write(
        out.join("surface-receipt.json"),
        serde_json::to_vec_pretty(
            &serde_json::json!({"bsp_textured":bsp_bound,"mesh_textured":mesh_bound,"fx_omitted":skipped_fx.len(),"untextured":scene.appearance.iter().filter(|a|a.is_none()).count(),"errors":errors,"materials":receipts,"scope":"Original base diffuse/UV stream; static panner sample, no full shader or mover parity"}),
        )?,
    )?;
    println!(
        "BSP {bsp_bound}, mesh {mesh_bound}, omitted FX {}, errors {}",
        skipped_fx.len(),
        errors.len()
    );
    Ok(())
}

fn assets_path(root: &Path, path: &str, package: &str) -> String {
    let first = path.split('.').next().unwrap();
    let qualified = [
        ("Textures", "utx"),
        ("StaticMeshes", "usx"),
        ("System", "u"),
    ]
    .iter()
    .any(|(folder, ext)| {
        fs::read_dir(root.join(folder)).is_ok_and(|entries| {
            entries.filter_map(Result::ok).any(|e| {
                e.file_name()
                    .to_string_lossy()
                    .eq_ignore_ascii_case(&format!("{first}.{ext}"))
            })
        })
    });
    if qualified {
        path.into()
    } else {
        format!("{package}.{path}")
    }
}
fn dot(a: [f32; 3], b: [f32; 3]) -> f32 {
    (0..3).map(|i| a[i] * b[i]).sum()
}
