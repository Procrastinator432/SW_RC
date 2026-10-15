//! Add the selected original intro and an explicit end-of-map handoff boundary.
use rc_game::{intro, opening};
use std::{env, fs, path::Path};
fn main() -> Result<(), Box<dyn std::error::Error>> {
    let args: Vec<_> = env::args().collect();
    if args.len() != 4 {
        return Err("usage: rc-first-mission-build INTRO_ASSETS RAPPEL_SCENE OUT".into());
    }
    let dir = Path::new(&args[1]);
    let out = Path::new(&args[3]);
    fs::create_dir_all(out)?;
    let bytes = fs::read(&args[2])?;
    if !bytes.starts_with(b"RCPLAY13") {
        return Err("Need opening scene".into());
    }
    let size = u32::from_le_bytes(bytes[8..12].try_into()?) as usize;
    let mut a: opening::Assets = serde_json::from_slice(&bytes[12..12 + size])?;
    if a.intro.is_some() {
        return Err("Already contains intro".into());
    }
    let binding: serde_json::Value = serde_json::from_slice(&fs::read(dir.join("bindings.json"))?)?;
    let flow = serde_json::from_slice(&fs::read(dir.join("flow.json"))?)?;
    let base = rc_game::bundle::decode(&bytes)?
        .mission
        .as_ref()
        .unwrap()
        .sounds
        .len();
    let sounds: Vec<rc_game::combat::Sound> =
        serde_json::from_slice(&fs::read(dir.join("sounds.json"))?)?;
    if sounds.len() != 6 {
        return Err("Need six original intro sounds".into());
    }
    let mut actors = vec![];
    for (id, extent) in [(452, [40., 40., 84.]), (530, [50., 50., 115.])] {
        let b = binding["actors"]
            .as_array()
            .unwrap()
            .iter()
            .find(|v| v["original"] == id)
            .ok_or("Missing original actor")?;
        actors.push(intro::Actor {
            original: id,
            mesh: Some(serde_json::from_slice(&fs::read(
                dir.join(format!("actor-{id}-mesh.json")),
            )?)?),
            location: serde_json::from_value(b["resolved"]["Location"]["value"].clone())?,
            rotation: serde_json::from_value(b["resolved"]["Rotation"]["value"].clone())?,
            extent,
        });
    }
    let destination = binding["actors"]
        .as_array()
        .unwrap()
        .iter()
        .find(|v| v["original"] == 413)
        .ok_or("Missing Goto node")?;
    a.intro = Some(intro::Assets {
        flow,
        actors,
        teleport: serde_json::from_value(
            binding["selected_teleport"]["location"]["value"].clone(),
        )?,
        rotation: serde_json::from_value(
            binding["selected_teleport"]["rotation"]["value"].clone(),
        )?,
        destination: serde_json::from_value(destination["resolved"]["Location"]["value"].clone())?,
        sounds: std::array::from_fn(|i| base + i),
    });
    a.stop_at_transition = true;
    a.sounds.extend(sounds);
    let metadata = serde_json::to_vec(&a)?;
    let mut data = b"RCPLAY13".to_vec();
    data.extend((metadata.len() as u32).to_le_bytes());
    data.extend(metadata);
    data.extend(&bytes[12 + size..]);
    let game = rc_game::bundle::decode(&data)?;
    assert!(game.state.opening.as_ref().unwrap().intro.is_some());
    fs::write(out.join("geo_01a-first-mission.rcplay"), data)?;
    println!(
        "First mission scene built: {} original PCM samples, intro and transition boundary",
        game.mission.as_ref().unwrap().sounds.len()
    );
    Ok(())
}
