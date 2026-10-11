//! Prepare the original crash-site droid mesh and clips without claiming a playable mission.
use rc_game::{crash, visual};
use std::{env, fs, path::Path};
fn main() -> Result<(), Box<dyn std::error::Error>> {
    let args: Vec<_> = env::args_os().collect();
    if args.len() != 4 {
        return Err("usage: rc-crash-visual-check GAME_DATA CRASH_ASSETS OUTPUT_DIR".into());
    }
    let root = Path::new(&args[1]);
    let out = Path::new(&args[3]);
    fs::create_dir_all(out)?;
    let flow: crash::Assets = serde_json::from_slice(&fs::read(&args[2])?)?;
    flow.validate()?;
    let catalog = rc_inspect::defaults::load(root)?;
    for factory in &flow.factories {
        match catalog.resolve(&factory.prototype, "Mesh", 0)?.value {
            rc_package::properties::Value::Object { path, .. } if path == "Droid.BattleDroid" => {}
            _ => return Err("Unsupported original crash-site mesh".into()),
        }
    }
    let mut resolver = rc_inspect::assets::Assets::new(&[
        &root.join("Textures"),
        &root.join("Animations"),
        &root.join("System"),
    ])?;
    let assets = visual::Assets {
        enemy: rc_inspect::visual_assets::load(
            root,
            "droid.ukx",
            "BattleDroid",
            &["ActionBreathe", "RunForward", "Fire", "Impact", "Death"],
            &mut resolver,
        )?,
        weapon: rc_inspect::visual_assets::load(
            root,
            "hudarms.ukx",
            "HudArmsDC15s",
            &["ActionBreathe", "Walk", "Fire", "Reload"],
            &mut resolver,
        )?,
    };
    let asset_bytes = serde_json::to_vec_pretty(&assets)?;
    let mut rows = vec![];
    for mesh in [assets.enemy, assets.weapon] {
        let p = mesh.prepare()?;
        let mesh = &p.mesh;
        for i in 0..mesh.clips.len() {
            for frame in [0., 0.25, 0.5, 0.75] {
                let scene = p.scene(&p.channel(i, frame)?)?;
                if scene.triangles.is_empty()
                    || scene
                        .triangles
                        .iter()
                        .flat_map(|t| t.points)
                        .flatten()
                        .any(|v| !v.is_finite())
                {
                    return Err("Invalid original clip geometry".into());
                }
            }
        }
        rows.push(serde_json::json!({"source":mesh.source,"bones":mesh.bones.len(),"rest_bounds":p.rest_bounds,
            "clips":mesh.clips.iter().map(|c|&c.source).collect::<Vec<_>>(),"materials":mesh.materials.iter().map(|m|(&m.source,&m.chain,&m.opacity)).collect::<Vec<_>>()}));
    }
    fs::write(out.join("visual-assets.json"), asset_bytes)?;
    fs::write(
        out.join("validation.json"),
        serde_json::to_vec_pretty(
            &serde_json::json!({"passed":true,"models":rows,"evaluated_clip_frames":36,
        "scope":"Original mesh, compressed animation and base material preparation only. No attachment to mission scene, AI, original shader parity or Android acceptance."}),
        )?,
    )?;
    println!("Original BattleDroid and DC15s: nine clips at four frames prepared");
    Ok(())
}
