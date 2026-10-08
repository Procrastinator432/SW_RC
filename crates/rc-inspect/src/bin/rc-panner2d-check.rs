use rc_inspect::assets::Assets;
use rc_package::material_uv::TexPanner2D;
use std::{env, fs, path::Path};
fn main() -> Result<(), Box<dyn std::error::Error>> {
    let args: Vec<_> = env::args_os().collect();
    if args.len() != 4 {
        return Err("usage: rc-panner2d-check MATERIAL_REPORT GAME_DATA OUTPUT".into());
    }
    let old: serde_json::Value = serde_json::from_slice(&fs::read(&args[1])?)?;
    let game = Path::new(&args[2]);
    let dirs = [
        game.join("Textures"),
        game.join("StaticMeshes"),
        game.join("Animations"),
        game.join("System"),
    ];
    let mut assets = Assets::new(&dirs.iter().map(|p| p.as_path()).collect::<Vec<_>>())?;
    let mut paths = std::collections::BTreeSet::new();
    let mut slots = 0;
    for o in old["objects"].as_array().ok_or("objects")? {
        for b in o["bindings"].as_array().ok_or("bindings")? {
            if b["error"] == "Unsupported material class Engine.TexPanner2D" {
                paths.insert(b["material"].as_str().ok_or("material")?.to_owned());
                slots += 1;
            }
        }
    }
    let mut materials = vec![];
    for path in paths {
        if assets.diffuse_at_size(&path, 256).is_ok() {
            return Err("legacy scalar resolver unexpectedly accepted a panner".into());
        }
        let d = assets.diffuse_at_time(&path, 256, 0.)?;
        let p = d.panner.ok_or("panner not found")?;
        let samples = [-1.25, 0., 0.25, 1., 1.25, 1000.]
            .map(|time| Ok(serde_json::json!({"time":time,"transform":p.at(time)?})))
            .into_iter()
            .collect::<Result<Vec<_>, String>>()?;
        let mut scene = rc_render::Scene::new(vec![rc_package::geometry::Triangle {
            points: [[0., 0., 0.], [1., 0., 0.], [0., 1., 0.]],
            surface: 0,
        }])?;
        scene.appearance = vec![Some(rc_render::FaceTexture {
            uv: [[0., 0.], [1., 0.], [-0.25, 1.25]],
            texture: 0,
        })];
        scene.apply_skeletal_diffuse_slots(&[Some((d.texture.clone(), 1.))])?;
        scene.apply_skeletal_uv_transforms(&[d.uv_transform])?;
        let snapshot = Path::new(&args[3])
            .with_file_name(format!("panner2d-material-{}.rcscene", materials.len()));
        fs::write(&snapshot, scene.snapshot()?)?;
        materials.push(serde_json::json!({"material":path,"source":d.source,"chain":d.chain,"parameters":p,"samples":samples,"snapshot":snapshot,"width":d.texture.width,"height":d.texture.height,"pixels":d.texture.pixels,"format":d.format,"source_mip":d.source_mip}));
    }
    let mut probes = vec![];
    for i in 0..256 {
        let p = TexPanner2D {
            speed: [(i as f32 - 128.) / 7., (i % 17) as f32 / 3.],
            offset: [(i % 11) as f32 / 4., -(i as f32) / 37.],
            scale: [0.5, 2.],
            clamped_size: [
                if i % 2 == 0 { 1. } else { 4. },
                if i % 3 == 0 { 0.25 } else { 2. },
            ],
        };
        let time = (i as f32 - 100.) / 13.;
        probes.push(serde_json::json!({"parameters":p,"time":time,"transform":p.at(time)?}));
    }
    for time in [
        -1.00001, -1., -0.00001, -0.0, 0., 0.99998, 0.99999, 1., 1.00001,
    ] {
        let p = TexPanner2D {
            speed: [1., -1.],
            offset: [0.; 2],
            scale: [1.; 2],
            clamped_size: [1.; 2],
        };
        probes.push(serde_json::json!({"parameters":p,"time":time,"transform":p.at(time)?}));
    }
    fs::write(
        &args[3],
        serde_json::to_vec(
            &serde_json::json!({"resolved_slots":slots,"materials":materials,"probes":probes,"scope":"Time sampled TexPanner2D base diffuse only. Original serialized defaults and instance floats; no script-driven parameter updates, modifier composition, original shaders or Android verification."}),
        )?,
    )?;
    println!(
        "{slots} formerly omitted slots: {} material paths, {} numerical probes",
        materials.len(),
        probes.len()
    );
    Ok(())
}
