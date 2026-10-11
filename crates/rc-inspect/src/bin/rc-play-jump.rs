use rc_game::{bundle, combat, jump, visual};
use rc_inspect::defaults;
use rc_package::{
    jump_goal, properties::Value, read_package, skeletal_animation::read_mesh_animation,
};
use std::{env, fs, path::Path};
fn split<'a>(b: &'a [u8], magic: &[u8; 8]) -> Result<(&'a [u8], &'a [u8]), String> {
    if !b.starts_with(magic) || b.len() < 12 {
        return Err("Invalid scene layer".into());
    }
    let n = u32::from_le_bytes(b[8..12].try_into().unwrap()) as usize;
    Ok((
        b.get(12..12 + n).ok_or("Truncated scene")?,
        b.get(12 + n..).ok_or("Missing base")?,
    ))
}
fn main() -> Result<(), Box<dyn std::error::Error>> {
    let a: Vec<_> = env::args_os().collect();
    if a.len() != 5 {
        return Err("usage: rc-play-jump GAME_DATA M3I_SCENES DEFENSIVE_RECEIPT OUTPUT".into());
    }
    let game = Path::new(&a[1]);
    let out = Path::new(&a[4]);
    fs::create_dir_all(out)?;
    let catalog = defaults::load(game)?;
    let number = |class: &str, name: &str| -> Result<f32, String> {
        let r = catalog.resolve(class, name, 0)?;
        if r.external_config_or_localization {
            return Err("External jump configuration".into());
        }
        match r.value {
            Value::Float(v) => Ok(v),
            _ => Err(format!("Invalid {class}.{name}")),
        }
    };
    let air_speed = number("CTCharacters.GeonosianWarrior", "AirSpeed")?;
    let radius = number("CTCharacters.GeonosianWarrior", "CollisionRadius")?;
    let probability = number(
        "CTGame.StateGeonosianWarriorAttack",
        "DefensiveJumpProbability",
    )?;
    let player_probability = number(
        "CTGame.StateGeonosianWarriorAttack",
        "DefensiveJumpProbabilityForPlayer",
    )?;
    let report: serde_json::Value = serde_json::from_slice(&fs::read(&a[3])?)?;
    let source = fs::read(game.join("Animations/geonosian.ukx"))?;
    let pkg = read_package(&source)?;
    let e = pkg
        .exports
        .iter()
        .enumerate()
        .find_map(|(i, e)| {
            pkg.object_path(i as i32 + 1)
                .is_ok_and(|n| n == "GeonosianWarriorSet")
                .then_some(e)
        })
        .ok_or("Missing warrior animation")?;
    let bank = read_mesh_animation(&pkg, &source, e)?;
    if bank
        .reference_bones
        .first()
        .is_none_or(|b| b.name.name != "Move")
    {
        return Err("Missing Move reference bone".into());
    }
    let takeoff = bank
        .sequences
        .iter()
        .find(|s| s.name.name == "TakeOff")
        .ok_or("Missing TakeOff")?;
    let track = takeoff.tracks.first().ok_or("Missing TakeOff track")?;
    let translation = jump_goal::sequence_translation(
        *track.positions.first().ok_or("Missing root keys")?,
        *track.positions.last().unwrap(),
        track.position_scale_bits,
    );
    let provenance = serde_json::json!({"animation":"geonosian.GeonosianWarriorSet","names":"Core FName.StaticInit: TakeOff=0x374, Land=0x375; ctgame JumpTo reads Names+dd0/dd4", "speed":catalog.resolve("CTCharacters.GeonosianWarrior","AirSpeed",0)?,"radius":catalog.resolve("CTCharacters.GeonosianWarrior","CollisionRadius",0)?,"probability":catalog.resolve("CTGame.StateGeonosianWarriorAttack","DefensiveJumpProbability",0)?,"player_probability":catalog.resolve("CTGame.StateGeonosianWarriorAttack","DefensiveJumpProbabilityForPlayer",0)?,"root_translation":translation,"post_load":"Move reference bone and word_50=0; first/last signed compressed positions decoded separately at 10513afe..10513b9c", "physics":"JumpTo ordinary start queues TakeOff then ChangePhysics flags0x15: Falling2, idle2, velocity; native speed+530 is AirSpeed (GetMaxSpeed flying branch)","boundaries":"Ground JumpDest only; original linked order, prepared availability/zone/arc feasibility; scaled encounter body/root motion, portable RSQRT seed and ballistic static sweeps, no general GoalQueue/Perch or native physFalling parity"});
    for level in ["easy", "normal", "hard"] {
        let b = fs::read(Path::new(&a[2]).join(format!("geo_01a-{level}.rcplay")))?;
        let (c, base) = split(&b, b"RCPLAY03")?;
        let mut combat: combat::Assets = serde_json::from_slice(c)?;
        let (v, base) = split(base, b"RCPLAY02")?;
        let mut visual: visual::Assets = serde_json::from_slice(v)?;
        if combat.jump.is_some() {
            return Err("Scene already has jump profile".into());
        }
        for name in ["TakeOff", "Land", "FallBreathe"] {
            let sequence = bank
                .sequences
                .iter()
                .find(|s| s.name.name == name)
                .ok_or("Missing jump clip")?;
            visual.enemy.clips.push(visual::Clip {
                source: format!("geonosian.GeonosianWarriorSet:{name}"),
                mapping: visual.enemy.clips[0].mapping.clone(),
                sequence: serde_json::from_value(serde_json::to_value(sequence)?)?,
            });
        }
        let mut nodes = vec![];
        for original in report["original_linked_order"]
            .as_array()
            .ok_or("Missing linked map order")?
        {
            let id = original["original"]
                .as_u64()
                .ok_or("Missing original node id")? as u32;
            if let Some(n) = report["nodes"].as_array().unwrap().iter().find(|n| {
                n["original"].as_u64() == Some(id as u64) && n["class"] == "Engine.JumpDest"
            }) {
                nodes.push(jump::Node {
                    original: id,
                    name: n["name"].as_str().unwrap().into(),
                    position: serde_json::from_value(
                        n["values"]["Location"]["value"]["value"].clone(),
                    )?,
                });
            }
        }
        if nodes.len() != 15 {
            return Err("Missing original JumpDest nodes".into());
        }
        combat.jump = Some(jump::Assets {
            provenance: provenance.clone(),
            nodes,
            air_speed,
            collision_radius: radius,
            probability,
            player_probability,
            root_translation: translation,
        });
        let result = bundle::encode_combat(&bundle::encode_animated(base, &visual)?, &combat)?;
        let g = bundle::decode(&result)?;
        let mut candidates = vec![];
        for (i, _) in g.state.enemies.iter().enumerate() {
            for node in &combat.jump.as_ref().unwrap().nodes {
                if let Some(c) = jump::candidate(&g, i, node)? {
                    candidates.push(serde_json::json!({"owner":i,"node":node.name,"motion":c}));
                }
            }
        }
        fs::write(
            out.join(format!("jump-candidates-{level}.json")),
            serde_json::to_vec_pretty(&candidates)?,
        )?;
        fs::write(out.join(format!("geo_01a-{level}.rcplay")), result)?;
        println!("{level}: AirSpeed={air_speed}, radius={radius}, root={translation:?}, feasible spawn arcs={}",candidates.len());
    }
    fs::write(
        out.join("jump-receipt.json"),
        serde_json::to_vec_pretty(&provenance)?,
    )?;
    Ok(())
}
