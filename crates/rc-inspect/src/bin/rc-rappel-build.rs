//! Add original live rappel bindings to the frozen opening scene.
use rc_game::{
    opening::{Action, Assets, Op},
    rappel,
};
use rc_package::{
    properties::{self, Value},
    read_package,
};
use std::{env, fs, path::Path};
fn main() -> Result<(), Box<dyn std::error::Error>> {
    let args: Vec<_> = env::args_os().collect();
    if args.len() != 5 {
        return Err("usage: rc-rappel-build GAME_DATA OPENING_SCENE ASSET_DIR OUT".into());
    }
    let root = Path::new(&args[1]);
    let dir = Path::new(&args[3]);
    let out = Path::new(&args[4]);
    fs::create_dir_all(out)?;
    let data = fs::read(&args[2])?;
    if !data.starts_with(b"RCPLAY13") {
        return Err("Expected RCPLAY13".into());
    }
    let n = u32::from_le_bytes(data[8..12].try_into()?) as usize;
    let mut a: Assets = serde_json::from_slice(&data[12..12 + n])?;
    if a.rappel.is_some() {
        return Err("Already has rappel".into());
    }
    let g = rc_game::bundle::decode(&data)?;
    let base = g.mission.as_ref().unwrap().sounds.len();
    let map = fs::read(root.join("Maps/geo_01a.ctm"))?;
    let pkg = read_package(&map)?;
    let cat = rc_inspect::defaults::load(root)?;
    let value = |id: usize, name: &str| -> Result<Value, String> {
        let e = &pkg.exports[id - 1];
        let p = properties::read(&pkg, &map, e)?;
        if let Some(v) = p.values.iter().find(|p| p.name == name) {
            Ok(v.value.clone())
        } else {
            Ok(cat
                .instance(&pkg.object_path(e.class)?, &p, name, 0, "geo_01a")?
                .value)
        }
    };
    let vector = |id, name| -> Result<[f32; 3], String> {
        if let Value::Vector(v) = value(id, name)? {
            Ok(v)
        } else {
            Err("Missing vector".into())
        }
    };
    let rotation = |id, name| -> Result<[i32; 3], String> {
        if let Value::Rotator(v) = value(id, name)? {
            Ok(v)
        } else {
            Err("Missing rotation".into())
        }
    };
    let number = |id, name| -> Result<f64, String> {
        match value(id, name)? {
            Value::Float(v) => Ok(v as f64),
            Value::Int(v) => Ok(v as f64),
            _ => Err("Missing number".into()),
        }
    };
    let player: rc_game::visual::Mesh =
        serde_json::from_slice(&fs::read(dir.join("player-mesh.json"))?)?;
    let rope = serde_json::from_slice(&fs::read(dir.join("rope-mesh.json"))?)?;
    let clone = fs::read(root.join("Animations/Clone.ukx"))?;
    let cp = read_package(&clone)?;
    let mut receipts = vec![];
    let mut extra = vec![];
    let mut notifies = vec![];
    for notify in &player.clips[0].sequence.notifies {
        let e = &cp.exports[notify.object_index as usize - 1];
        let props = properties::read(&cp, &clone, e)?;
        let class = cp.object_path(e.class)?;
        let role = if class == "Engine.AnimNotify_Sound" {
            let path = props
                .values
                .iter()
                .find_map(|p| {
                    if p.name == "Sound" {
                        if let Value::Object { path, .. } = &p.value {
                            Some(path)
                        } else {
                            None
                        }
                    } else {
                        None
                    }
                })
                .ok_or("Missing Foley sound")?;
            let (sound, _) = rc_inspect::mission_assets::sound_variant(root, path, &mut receipts)?;
            let role = if let Some(i) = extra
                .iter()
                .position(|s: &rc_game::combat::Sound| s.source == sound.source)
            {
                base + i
            } else {
                let role = base + extra.len();
                extra.push(sound);
                role
            };
            Some(role)
        } else if class == "Engine.AnimNotify_Footstep" {
            None
        } else {
            return Err("Unknown rappel notify".into());
        };
        receipts.push(serde_json::json!({"original":notify.object_index,"class":class,"properties":props,"role":role,"boundary":if role.is_none(){"material footstep dispatch not bound; event retained"}else{"original PCM"}}));
        notifies.push(rappel::Notify {
            original: notify.object_index as u32,
            time_bits: notify.time_bits,
            role,
        });
    }
    a.sounds.extend(extra);
    let scale = std::f64::consts::TAU / 65536.;
    let limits = [
        number(1238, "MinPitch")?,
        number(1238, "MaxPitch")?,
        number(1238, "MinYaw")?,
        number(1238, "MaxYaw")?,
    ]
    .map(|v| v * scale);
    let Value::Float(eye) = cat
        .resolve("CTCharacters.PlayerCommando", "BaseEyeHeight", 0)?
        .value
    else {
        return Err("Missing eye height".into());
    };
    a.rappel = Some(rappel::Assets {
        player: Some(player),
        rope: Some(rope),
        start: vector(10, "Location")?.map(f64::from),
        rotation: rotation(10, "Rotation")?,
        rope_location: vector(11, "Location")?,
        rope_rotation: rotation(11, "Rotation")?,
        eye_height: eye as f64,
        limits,
        notifies,
    });
    let program = a.programs.iter_mut().find(|p| p.original == 12).unwrap();
    let i = program
        .actions
        .iter()
        .position(|a| a.original == 1245)
        .ok_or("Missing rappel timing")?;
    program.actions.insert(
        i,
        Action {
            original: 1244,
            op: Op::Event {
                tag: "PlayerRappel".into(),
            },
        },
    );
    let i = program
        .actions
        .iter()
        .position(|a| a.original == 1249)
        .ok_or("Missing rope destruction timing")?;
    program.actions.insert(
        i,
        Action {
            original: 1248,
            op: Op::Event {
                tag: "DestroyCloneRope07".into(),
            },
        },
    );
    a.provenance["rappel"] = serde_json::json!({"notifies":receipts,"actions":[1244,1248],"camera":"original Pawn.Location + BaseEyeHeight; no head camera","collision":"port static-body sweeps, pre-animation hold and final root pose once; not native moveSmooth/PlayerStand parity","rope":"original Rope7 ActionBreathe until original destruction; CloneEntrance600 playback receiver not proven"});
    let meta = serde_json::to_vec(&a)?;
    let mut result = b"RCPLAY13".to_vec();
    result.extend((meta.len() as u32).to_le_bytes());
    result.extend(meta);
    result.extend(&data[12 + n..]);
    let game = rc_game::bundle::decode(&result)?;
    fs::write(out.join("geo_01a-rappel.rcplay"), &result)?;
    fs::write(
        out.join("rappel-receipt.json"),
        serde_json::to_vec_pretty(&a.provenance["rappel"])?,
    )?;
    println!(
        "Live rappel: {} bytes, {} sounds, start {:?}",
        result.len(),
        game.mission.as_ref().unwrap().sounds.len(),
        game.state.pawn.body.position
    );
    Ok(())
}
