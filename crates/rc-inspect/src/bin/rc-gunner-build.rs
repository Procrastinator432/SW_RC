//! Bind the original bunker meshes, collision and explosion assets to the live mission.
use rc_game::{bundle, dynamic, gunner_runtime};
use rc_inspect::mission_assets::{collider, mesh_scene, sound};
use rc_package::{
    mesh_query::ActorTransform,
    properties::{self, Value},
    read_package,
};
use std::{collections::BTreeMap, env, fs, path::Path};
fn unwrap(b: &[u8], magic: &[u8]) -> Result<(Vec<u8>, Vec<u8>), Box<dyn std::error::Error>> {
    if !b.starts_with(magic) {
        return Err("Unexpected scene layer".into());
    }
    let n = u32::from_le_bytes(b[8..12].try_into()?) as usize;
    Ok((b[12..12 + n].to_vec(), b[12 + n..].to_vec()))
}
fn wrap(magic: &[u8], meta: &[u8], base: &[u8]) -> Vec<u8> {
    let mut b = magic.to_vec();
    b.extend((meta.len() as u32).to_le_bytes());
    b.extend(meta);
    b.extend(base);
    b
}
fn key(t: &rc_package::geometry::Triangle) -> [[u32; 3]; 3] {
    t.points.map(|p| p.map(f32::to_bits))
}
fn range_midpoint(p: &rc_package::Package, raw: &[u8]) -> Result<f32, Box<dyn std::error::Error>> {
    let fields = properties::tagged_struct(p, raw)?;
    let get = |name| {
        fields
            .values
            .iter()
            .find_map(|f| {
                if f.name == name {
                    if let Value::Float(v) = f.value {
                        Some(v)
                    } else {
                        None
                    }
                } else {
                    None
                }
            })
            .ok_or("Missing original emitter range")
    };
    Ok((get("Min")? + get("Max")?) * 0.5)
}
fn main() -> Result<(), Box<dyn std::error::Error>> {
    let args: Vec<_> = env::args_os().collect();
    if args.len() != 4 {
        return Err("usage: rc-gunner-build GAME_DATA LIVE_SCENE OUTPUT".into());
    }
    let root = Path::new(&args[1]);
    let out = Path::new(&args[3]);
    fs::create_dir_all(out)?;
    let (m5, b4) = unwrap(&fs::read(&args[2])?, b"RCPLAY05")?;
    let (m4, b2) = unwrap(&b4, b"RCPLAY04")?;
    let (m2, b1) = unwrap(&b2, b"RCPLAY02")?;
    let (m1, snapshot) = unwrap(&b1, b"RCPLAY01")?;
    let mut manifest: bundle::Manifest = serde_json::from_slice(&m1)?;
    let mut scene = rc_render::Scene::from_snapshot(&snapshot)?;
    let catalog = rc_inspect::defaults::load(root)?;
    let bytes = fs::read(root.join("Maps/geo_01a.ctm"))?;
    let pkg = read_package(&bytes)?;
    let mut objects = vec![];
    let mut receipts = vec![];
    let mut remove = BTreeMap::<_, usize>::new();
    let mut bunker_position = [0.; 3];
    for (id, mesh, name) in [
        (850, "Arena_HR.ArenaWalls.Bunker1", "Prop11"),
        (1093, "Arena_HR.ArenaWalls.BunkerHatch", "Prop4"),
    ] {
        let e = &pkg.exports[id - 1];
        let props = properties::read(&pkg, &bytes, e)?;
        let class = pkg.object_path(e.class)?;
        let value = |n| {
            catalog
                .instance(&class, &props, n, 0, "geo_01a")
                .map(|v| v.value)
        };
        let vector = |n| match value(n)? {
            Value::Vector(v) => Ok(v),
            _ => Err("Expected actor vector".to_string()),
        };
        let scale = match value("DrawScale")? {
            Value::Float(v) => v,
            _ => return Err("Expected scale".into()),
        };
        let t = ActorTransform {
            location: vector("Location")?,
            rotation: match value("Rotation")? {
                Value::Rotator(v) => v,
                _ => return Err("Expected rotation".into()),
            },
            scale: vector("DrawScale3D")?.map(|v| v * scale),
            pivot: vector("PrePivot")?,
        };
        let visible = mesh_scene(root, mesh, &t, &catalog, &mut receipts)?;
        for tri in &visible.triangles {
            *remove.entry(key(tri)).or_default() += 1;
        }
        let i = manifest
            .colliders
            .iter()
            .position(|c| c.name == name)
            .ok_or("Missing original bunker collider")?;
        let original = manifest.colliders.remove(i);
        let released = if id == 850 {
            bunker_position = t.location.map(f64::from);
            let path = "Arena_HR.ArenaWalls.BunkerBlown";
            let damaged = mesh_scene(root, path, &t, &catalog, &mut receipts)?;
            let data = fs::read(root.join("StaticMeshes/Arena_HR.usx"))?;
            let p = read_package(&data)?;
            let e = p
                .exports
                .iter()
                .enumerate()
                .find_map(|(i, e)| {
                    p.object_path(i as i32 + 1)
                        .is_ok_and(|p| p == "ArenaWalls.BunkerBlown")
                        .then_some(e)
                })
                .ok_or("Missing BunkerBlown")?;
            let mesh = rc_package::mesh::read_static_mesh(&p, &data, e)?;
            let collision = rc_package::mesh_collision::read(&p, &data, e, &mesh)?;
            if collision.simple_model <= 0 {
                return Err("BunkerBlown has no original simple model".into());
            }
            let e = &p.exports[collision.simple_model as usize - 1];
            let bsp = rc_package::geometry::read_bsp(&p, &data, e)?;
            let solid = rc_package::collision::read_model_collision(&p, &data, e, &bsp)?;
            let h = rc_package::hull::HullSet::read(&bsp, &solid)?.transformed(&t)?;
            Some(dynamic::Released {
                collider: collider("BunkerBlown".into(), &h),
                snapshot: damaged.snapshot()?,
            })
        } else {
            None
        };
        objects.push(dynamic::Object {
            original: id as u32,
            collider: original,
            snapshot: visible.snapshot()?,
            released,
        });
    }
    let mut triangles = vec![];
    let mut appearance = vec![];
    let mut removed = 0;
    for (i, tri) in scene.triangles.into_iter().enumerate() {
        if let Some(n) = remove.get_mut(&key(&tri)) {
            if *n > 0 {
                *n -= 1;
                removed += 1;
                continue;
            }
        }
        triangles.push(tri);
        appearance.push(scene.appearance[i]);
    }
    let missing: usize = remove.values().sum();
    eprintln!("Original bunker faces: removed={removed}, absent={missing}");
    if removed > 0 && missing > 0 {
        return Err("Only part of the original bunker matched static scene".into());
    }
    scene.triangles = triangles;
    scene.appearance = appearance;
    // Rebuild spatial bounds/acceleration after removing the original static faces.
    let mut rebuilt = rc_render::Scene::new(scene.triangles)?;
    rebuilt.textures = scene.textures;
    rebuilt.appearance = scene.appearance;
    rebuilt.starts = scene.starts;
    rebuilt.solid = scene.solid;
    let b1 = bundle::encode(&manifest, &rebuilt)?;
    let b2 = wrap(b"RCPLAY02", &m2, &b1);
    let b4 = wrap(b"RCPLAY04", &m4, &b2);
    let b5 = wrap(b"RCPLAY05", &m5, &b4);
    let mut resolver = rc_inspect::assets::Assets::new(&[
        &root.join("Textures"),
        &root.join("Properties"),
        &root.join("System"),
    ])?;
    let mut sprites = vec![];
    let effect_bytes = fs::read(root.join("Properties/cteffects.u"))?;
    let effects = read_package(&effect_bytes)?;
    for id in [65, 64] {
        let e = &effects.exports[id - 1];
        let props = properties::read(&effects, &effect_bytes, e)?;
        let payload = effects.payload(&effect_bytes, e)?;
        let raw = |name| {
            props
                .values
                .iter()
                .find(|f| f.name == name)
                .map(|f| &payload[f.payload_offset..f.payload_offset + f.bytes])
                .ok_or("Missing emitter property")
        };
        let seconds = range_midpoint(&effects, raw("LifetimeRange")?)? as f64;
        let sizes = raw("StartSizeRange")?;
        let size_props = properties::tagged_struct(&effects, sizes)?;
        let x = size_props
            .values
            .iter()
            .find(|f| f.name == "X")
            .ok_or("Missing emitter X size")?;
        let size = range_midpoint(
            &effects,
            &sizes[x.payload_offset..x.payload_offset + x.bytes],
        )?;
        let source = props
            .values
            .iter()
            .find_map(|f| {
                if f.name == "Texture" {
                    if let Value::Object { path, .. } = &f.value {
                        Some(path.clone())
                    } else {
                        None
                    }
                } else {
                    None
                }
            })
            .ok_or("Missing original emitter texture")?;
        let grid = ["TextureUSubdivisions", "TextureVSubdivisions"].map(|name| {
            props
                .values
                .iter()
                .find_map(|f| {
                    if f.name == name {
                        if let Value::Int(v) = f.value {
                            Some(v as u32)
                        } else {
                            None
                        }
                    } else {
                        None
                    }
                })
                .unwrap_or(1)
        });
        let mut d = resolver.diffuse_at_size(&source, 512)?;
        // Current shared renderer supports cutout. Preserve source colors and derive
        // a mask for the original additive black background; no invented textures.
        for pixel in &mut d.texture.pixels {
            let p = *pixel;
            let light = (p & 255).max((p >> 8) & 255).max((p >> 16) & 255);
            *pixel = (p & 0xffffff) | if light < 18 { 0 } else { 0xff000000 };
        }
        sprites.push(gunner_runtime::Sprite {
            source,
            width: d.texture.width,
            height: d.texture.height,
            pixels: d.texture.pixels,
            size,
            seconds,
            grid,
        });
    }
    let sounds = [
        "GEN_Sound.Impacts_Explos.exp_GEN_MedWhistle_01",
        "GEN_Sound.Impacts_Explos.exp_GEN_sw_large_02",
    ]
    .iter()
    .map(|p| sound(root, p).map(|s| s.0))
    .collect::<Result<Vec<_>, _>>()?;
    let a = gunner_runtime::Assets {
        objects,
        bunker_position,
        sounds,
        sprites,
        provenance: serde_json::json!({"map":"geo_01a","actors":[850,1093],"destroyed_mesh":"Arena_HR.ArenaWalls.BunkerBlown","effect":"CTEffects.Explosion_Bunker_Lrg","thermal_default":"CTEffects.Grenade_ThermDet_Explo_Metal","sprite_emitters":[65,64],"sprite_adapter":"Deterministic midpoint sizes/lifetimes from original emitter ranges, cutout instead of additive blending; no general particle interpreter","materials":receipts}),
    };
    let result = bundle::encode_gunner(&b5, &a)?;
    let game = bundle::decode(&result)?;
    fs::write(out.join("geo_01a-gunner.rcplay"), &result)?;
    fs::write(
        out.join("build-validation.json"),
        serde_json::to_vec_pretty(
            &serde_json::json!({"passed":true,"format":"RCPLAY06","bytes":result.len(),"removed_static_faces":removed,"dynamic_objects":game.dynamic.as_ref().unwrap().objects.iter().map(|o|serde_json::json!({"id":o.original,"faces":o.triangles.len(),"released_faces":o.released_triangles.len(),"released_hulls":o.released_hulls.as_ref().map(|h|h.hulls.len())})).collect::<Vec<_>>(),"provenance":a.provenance}),
        )?,
    )?;
    println!(
        "Built {} bytes, {removed} static faces replaced",
        result.len()
    );
    Ok(())
}
