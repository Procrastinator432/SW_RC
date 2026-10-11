//! Bake the selected original scene, collision/defaults and provisional encounter for both hosts.
use std::{env, fs, path::Path};
fn main() -> Result<(), Box<dyn std::error::Error>> {
    let args: Vec<_> = env::args_os().collect();
    if args.len() != 4 {
        return Err("Usage: rc-play-build <GameData> <world.rcscene> <output.rcplay>".into());
    }
    let game_data = Path::new(&args[1]);
    let mut game = rc_inspect::play_scene::load(
        game_data,
        &game_data.join("Maps/geo_01a.ctm"),
        Path::new(&args[2]),
    )?;
    let positions = rc_inspect::play_scene::place_encounter(&mut game)?;
    let provenance = serde_json::json!({
        "map":game_data.join("Maps/geo_01a.ctm"), "scene":args[2],
        "movement_defaults":game.profile.properties,
        "collision":"Level-referenced world and classified player-blocking static mesh actors at serialized pose",
        "combat":"Own bounded encounter, hitscan, provisional robot/blaster geometry; no original combat/AI/script parity",
    });
    let manifest = rc_game::bundle::Manifest::capture(&game, provenance)?;
    let bytes = rc_game::bundle::encode(&manifest, &game.scene)?;
    let roundtrip = rc_game::bundle::decode(&bytes)?;
    if serde_json::to_vec(&game.state)? != serde_json::to_vec(&roundtrip.state)? {
        return Err(format!(
            "Game bundle changed reset state: original {} restored {}",
            serde_json::to_string(&game.state)?,
            serde_json::to_string(&roundtrip.state)?
        )
        .into());
    }
    if let Some(parent) = Path::new(&args[3]).parent() {
        fs::create_dir_all(parent)?;
    }
    fs::write(&args[3], &bytes)?;
    let report = serde_json::json!({"bundle_bytes":bytes.len(), "triangles":game.scene.triangles.len(),
        "textures":game.scene.textures.len(), "colliders":manifest.colliders.len(),
        "hulls":manifest.colliders.iter().map(|c| c.hulls.len()).sum::<usize>(), "volumes":manifest.volumes.len(),
        "spawn":manifest.spawn, "yaw":manifest.yaw, "enemies":positions, "exit":manifest.exit,
        "provenance":manifest.provenance});
    fs::write(
        Path::new(&args[3]).with_extension("json"),
        serde_json::to_vec_pretty(&report)?,
    )?;
    println!("{}", serde_json::to_string_pretty(&report)?);
    Ok(())
}
