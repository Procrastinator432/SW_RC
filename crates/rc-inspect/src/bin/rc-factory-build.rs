use std::{env, fs, path::Path};
fn copy<T: serde::Serialize, U: serde::de::DeserializeOwned>(
    v: &T,
) -> Result<U, serde_json::Error> {
    serde_json::from_value(serde_json::to_value(v)?)
}
fn main() -> Result<(), Box<dyn std::error::Error>> {
    let a: Vec<_> = env::args().collect();
    if a.len() != 5 {
        return Err("usage: rc-factory-build GAME_DATA WORLD FIRST_MAP OUT".into());
    }
    let root = Path::new(&a[1]);
    let out = Path::new(&a[4]);
    fs::create_dir_all(out)?;
    let game = rc_inspect::play_scene::load_static_draw_types(
        root,
        &root.join("Maps/geo_03a.ctm"),
        Path::new(&a[2]),
    )?;
    let original = rc_game::bundle::decode(&fs::read(&a[3])?)?;
    let visual = original
        .visuals
        .as_ref()
        .ok_or("Missing original visual assets")?;
    let mut manifest = rc_game::bundle::Manifest::capture(
        &game,
        serde_json::json!({"map":"geo_03a.ctm","entry":"Original PlayerStart0","scope":"First mission complete; original factory entry map, body collision, carry and restart; next mission scripts/encounters pending"}),
    )?;
    manifest.name = "GEO_03A".into();
    let base = rc_game::bundle::encode(&manifest, &game.scene)?;
    let base = rc_game::bundle::encode_animated(
        &base,
        &rc_game::visual::Assets {
            enemy: copy(&visual.enemy.mesh)?,
            weapon: copy(&visual.weapon.mesh)?,
        },
    )?;
    let previous = fs::read("analysis/mission27-map3-elevator/delivery/geo_01c.rcplay")?;
    let n = u32::from_le_bytes(previous[8..12].try_into()?) as usize;
    let mut previous: rc_game::map_runtime::Assets = serde_json::from_slice(&previous[12..12 + n])?;
    let mut sev = previous.opening.as_mut().unwrap().actors[2]
        .mesh
        .take()
        .unwrap();
    sev.clips
        .retain(|c| ["ActionBreathe", "RunForward"].contains(&c.sequence.name.name.as_str()));
    let assets = rc_game::map_runtime::Assets {
        opening: None,
        gameplay: None,
        destination: "GEO_03A.ctm#playerstart".into(),
        start: game.scene.starts[0].actor.clone(),
        pitch: game.scene.starts[0].rotation[0] as f64 * std::f64::consts::TAU / 65536.,
        squad_mesh: previous.squad_mesh.take(),
        fixer_mesh: previous.fixer_mesh.take(),
        sev_mesh: Some(sev),
        weapon_snapshot: previous.weapon_snapshot,
        sounds: vec![copy(&previous.sounds[0])?],
    };
    let metadata = serde_json::to_vec(&assets)?;
    let mut bytes = b"RCPLAY14".to_vec();
    bytes.extend((metadata.len() as u32).to_le_bytes());
    bytes.extend(metadata);
    bytes.extend(base);
    let decoded = rc_game::bundle::decode(&bytes)?;
    fs::write(out.join("geo_03a.rcplay"), &bytes)?;
    fs::write(
        out.join("build-receipt.json"),
        serde_json::to_vec_pretty(
            &serde_json::json!({"bytes":bytes.len(),"spawn":manifest.spawn,"starts":game.scene.starts,"colliders":manifest.colliders.len(),"triangles":game.scene.triangles.len(),"textures":game.scene.textures.len(),"entry_state":decoded.state,"scope":"Map import + original playerstart, Original factory entry with three-member carry; subsequent mission scripts/encounters are not yet integrated"}),
        )?,
    )?;
    println!(
        "GEO_03A: {} bytes; {} triangles; {} colliders; spawn {:?}; carried {}HP",
        bytes.len(),
        game.scene.triangles.len(),
        manifest.colliders.len(),
        manifest.spawn,
        decoded.state.health
    );
    Ok(())
}
