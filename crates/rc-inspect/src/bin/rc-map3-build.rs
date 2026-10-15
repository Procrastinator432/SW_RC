use std::{env, fs, path::Path};
fn copy<T: serde::Serialize, U: serde::de::DeserializeOwned>(
    v: &T,
) -> Result<U, serde_json::Error> {
    serde_json::from_value(serde_json::to_value(v)?)
}
fn main() -> Result<(), Box<dyn std::error::Error>> {
    let a: Vec<_> = env::args().collect();
    if a.len() != 5 {
        return Err("usage: rc-map3-build GAME_DATA WORLD FIRST_MAP OUT".into());
    }
    let root = Path::new(&a[1]);
    let out = Path::new(&a[4]);
    fs::create_dir_all(out)?;
    let game = rc_inspect::play_scene::load_completed_cells(
        root,
        &root.join("Maps/geo_01c.ctm"),
        Path::new(&a[2]),
    )?;
    let original = rc_game::bundle::decode(&fs::read(&a[3])?)?;
    let visual = original
        .visuals
        .as_ref()
        .ok_or("Missing original visual assets")?;
    let mut manifest = rc_game::bundle::Manifest::capture(
        &game,
        serde_json::json!({"map":"geo_01c.ctm","entry":"Original PlayerStart0","scope":"Original static map and movement; encounters/scripts/movers pending"}),
    )?;
    manifest.name = "GEO_01C".into();
    let base = rc_game::bundle::encode(&manifest, &game.scene)?;
    let base = rc_game::bundle::encode_animated(
        &base,
        &rc_game::visual::Assets {
            enemy: copy(&visual.enemy.mesh)?,
            weapon: copy(&visual.weapon.mesh)?,
        },
    )?;
    let assets = rc_game::map_runtime::Assets {
        sev_mesh: None,
        opening: None,
        fixer_mesh: Some(serde_json::from_value({
            let b = fs::read(rc_inspect::build_inputs::input("map_b_scene")?)?;
            let n = u32::from_le_bytes(b[8..12].try_into()?) as usize;
            let metadata: serde_json::Value = serde_json::from_slice(&b[12..12 + n])?;
            metadata["gameplay"]["fixer_mesh"].clone()
        })?),
        gameplay: None,
        destination: "GEO_01C.ctm#playerstart".into(),
        start: game.scene.starts[0].actor.clone(),
        pitch: game.scene.starts[0].rotation[0] as f64 * std::f64::consts::TAU / 65536.,
        weapon_snapshot: original
            .tactics
            .as_ref()
            .unwrap()
            .assets
            .squad
            .weapon_snapshot
            .clone(),
        squad_mesh: Some(copy(
            &original
                .rendezvous
                .as_ref()
                .ok_or("Missing original Scorch")?
                .mesh
                .mesh,
        )?),
        sounds: vec![copy(&original.mission.as_ref().unwrap().sounds[4])?],
    };
    let metadata = serde_json::to_vec(&assets)?;
    let mut bytes = b"RCPLAY14".to_vec();
    bytes.extend((metadata.len() as u32).to_le_bytes());
    bytes.extend(metadata);
    bytes.extend(base);
    let decoded = rc_game::bundle::decode(&bytes)?;
    fs::write(out.join("geo_01c.rcplay"), &bytes)?;
    fs::write(
        out.join("build-receipt.json"),
        serde_json::to_vec_pretty(
            &serde_json::json!({"bytes":bytes.len(),"spawn":manifest.spawn,"starts":game.scene.starts,"colliders":manifest.colliders.len(),"triangles":game.scene.triangles.len(),"textures":game.scene.textures.len(),"entry_state":decoded.state,"scope":"Map import + original playerstart, current weapon/audio and carried bounded companion; original map3 objectives/encounters are not yet reconstructed"}),
        )?,
    )?;
    println!(
        "GEO_01C: {} bytes; {} triangles; {} colliders; spawn {:?}; carried {}HP",
        bytes.len(),
        game.scene.triangles.len(),
        manifest.colliders.len(),
        manifest.spawn,
        decoded.state.health
    );
    Ok(())
}
