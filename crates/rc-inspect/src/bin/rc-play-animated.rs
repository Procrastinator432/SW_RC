//! Build a local RCPLAY02 bundle from original assets.
use rc_game::visual::Assets;
use rc_inspect::{assets, visual_assets::load};
use std::{env, fs, path::Path};
fn main() -> Result<(), Box<dyn std::error::Error>> {
    let args: Vec<_> = env::args_os().collect();
    if args.len() != 4 && args.len() != 5 {
        return Err("usage: rc-play-animated GAME_DATA BASE_SCENE OUTPUT_DIR".into());
    }
    let game = Path::new(&args[1]);
    let out = Path::new(&args[3]);
    fs::create_dir_all(out)?;
    let textures = game.join("Textures");
    let animations = game.join("Animations");
    let system = game.join("System");
    let mut resolver = assets::Assets::new(&[&textures, &animations, &system])?;
    let assets = Assets {
        enemy: load(
            game,
            "geonosian.ukx",
            "GeonosianWarrior",
            &["ActionBreathe", "RunForward", "Fire", "hit", "Death"],
            &mut resolver,
        )?,
        weapon: load(
            game,
            "hudarms.ukx",
            args.get(4)
                .and_then(|s| s.to_str())
                .unwrap_or("HudArmsDC17m"),
            &["ActionBreathe", "Walk", "Fire", "Reload"],
            &mut resolver,
        )?,
    };
    let receipt = serde_json::json!({"enemy":{"mesh":assets.enemy.source,"bones":assets.enemy.bones.len(),"clips":assets.enemy.clips.iter().map(|c|&c.source).collect::<Vec<_>>(),"materials":assets.enemy.materials.iter().map(|m|(&m.source,&m.chain,&m.opacity,m.width,m.height)).collect::<Vec<_>>(),"scale":assets.enemy.scale.map(f32::from_bits),"origin":assets.enemy.origin.map(f32::from_bits),"rotation":assets.enemy.rotation},
        "weapon":{"mesh":assets.weapon.source,"bones":assets.weapon.bones.len(),"clips":assets.weapon.clips.iter().map(|c|&c.source).collect::<Vec<_>>(),"materials":assets.weapon.materials.iter().map(|m|(&m.source,&m.chain,&m.opacity,m.width,m.height)).collect::<Vec<_>>(),"scale":assets.weapon.scale.map(f32::from_bits),"origin":assets.weapon.origin.map(f32::from_bits),"rotation":assets.weapon.rotation},
        "scope":"Original compressed clips, mesh transform, skin stream, sections and base diffuse; prototype encounter and clip-selection policy. Material effects and notify scripts are not executed."});
    let base = fs::read(&args[2])?;
    let bundle = rc_game::bundle::encode_animated(&base, &assets)?;
    let game = rc_game::bundle::decode(&bundle)?;
    let mut bounds = vec![];
    for mesh in [
        &game.visuals.as_ref().unwrap().enemy,
        &game.visuals.as_ref().unwrap().weapon,
    ] {
        bounds.push(serde_json::json!({"mesh":mesh.mesh.source,"rest_bounds":mesh.rest_bounds}));
        for i in 0..mesh.mesh.clips.len() {
            for frame in [0., 0.25, 0.5, 0.75] {
                mesh.scene(&mesh.channel(i, frame)?)?;
            }
        }
    }
    fs::write(
        out.join("mesh-bounds.json"),
        serde_json::to_vec_pretty(&bounds)?,
    )?;
    fs::write(out.join("geo_01a-animated.rcplay"), &bundle)?;
    fs::write(
        out.join("asset-receipt.json"),
        serde_json::to_vec_pretty(&receipt)?,
    )?;
    println!(
        "Built {} bytes; both original meshes and all 9 clips evaluated",
        bundle.len()
    );
    Ok(())
}
