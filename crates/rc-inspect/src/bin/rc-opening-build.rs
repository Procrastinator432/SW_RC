//! Bind selected original landing scripts and gunship poses to the accepted scene.
use rc_game::opening::{Action, Assets, Op, Program};
use rc_package::{
    properties::{self, Value},
    read_package,
};
use std::{env, fs, path::Path};
fn main() -> Result<(), Box<dyn std::error::Error>> {
    let args: Vec<_> = env::args_os().collect();
    if args.len() != 4 {
        return Err("usage: rc-opening-build GAME_DATA SQUAD_SCENE OUT".into());
    }
    let root = Path::new(&args[1]);
    let out = Path::new(&args[3]);
    fs::create_dir_all(out)?;
    let base = fs::read(&args[2])?;
    if !base.starts_with(b"RCPLAY12") {
        return Err("Expected accepted squad scene".into());
    }
    let game = rc_game::bundle::decode(&base)?;
    let bank = &game.mission.as_ref().unwrap().sounds;
    let bytes = fs::read(root.join("Maps/geo_01a.ctm"))?;
    let p = read_package(&bytes)?;
    let cat = rc_inspect::defaults::load(root)?;
    let number = |v: Value| -> Result<f64, String> {
        match v {
            Value::Float(v) => Ok(v as f64),
            Value::Int(v) => Ok(v as f64),
            _ => Err("Expected original number".into()),
        }
    };
    let mut sounds = vec![];
    let mut audio = vec![];
    let mut receipts = vec![];
    let mut programs = vec![];
    for (id, trigger) in [
        (12, "levelstart"),
        (523, "Rappel"),
        (497, "StartBactaTraining"),
    ] {
        let e = &p.exports[id - 1];
        let props = properties::read(&p, &bytes, e)?;
        let a = props
            .values
            .iter()
            .find(|v| v.name == "Actions")
            .ok_or("Missing original script actions")?;
        let payload = p.payload(&bytes, e)?;
        let ids =
            properties::object_array(&p, &payload[a.payload_offset..a.payload_offset + a.bytes])?;
        let mut actions = vec![];
        for (index, name) in ids {
            let e = &p.exports[index as usize - 1];
            let class = p.object_path(e.class)?;
            let props = properties::read(&p, &bytes, e)?;
            let get = |name| {
                if let Some(v) = props.values.iter().find(|v| v.name == name) {
                    Ok(v.value.clone())
                } else {
                    cat.instance(&class, &props, name, 0, "geo_01a")
                        .map(|v| v.value)
                }
            };
            let text = |name| -> Result<String, String> {
                match get(name)? {
                    Value::Name(v) | Value::String(v) => Ok(v),
                    Value::Object { path, .. } => Ok(path),
                    _ => Err(format!("Invalid {name}")),
                }
            };
            let mut reason = None;
            let op = match class.rsplit('.').next().unwrap() {
                "ACTION_WaitForEvent" => {
                    reason=Some("selected program trigger; initial levelstart and BactaDisabled bound at attachment");
                    None
                }
                "ACTION_PlaySound" => {
                    let path = text("Sound")?;
                    let (sound, seconds) =
                        rc_inspect::mission_assets::sound_variant(root, &path, &mut audio)?;
                    let role = if let Some(i) = bank.iter().position(|s| s.source == path) {
                        i
                    } else if let Some(i) = sounds
                        .iter()
                        .position(|s: &rc_game::combat::Sound| s.source == path)
                    {
                        bank.len() + i
                    } else {
                        let i = bank.len() + sounds.len();
                        sounds.push(sound);
                        i
                    };
                    let wait = matches!(get("bWaitForFinish")?, Value::Bool(true));
                    Some(Op::Sound {
                        role,
                        caption_seconds: seconds,
                        seconds: if wait { seconds } else { 0. },
                        text: text("Comment").unwrap_or_default(),
                    })
                }
                "ACTION_WaitForTimer" => Some(Op::Wait {
                    seconds: number(get("PauseTime")?)?,
                }),
                "ACTION_WaitForRandomTimer" => {
                    let field = props
                        .values
                        .iter()
                        .find(|v| v.name == "PauseTime")
                        .ok_or("Missing random timer")?;
                    let payload = p.payload(&bytes, e)?;
                    let interval = properties::tagged_struct(
                        &p,
                        &payload[field.payload_offset..field.payload_offset + field.bytes],
                    )?;
                    let values: Vec<_> = interval
                        .values
                        .iter()
                        .filter_map(|v| {
                            if let Value::Float(f) = v.value {
                                Some(f as f64)
                            } else {
                                None
                            }
                        })
                        .collect();
                    if values.len() != 2 {
                        return Err("Unsupported random range".into());
                    }
                    audio.push(serde_json::json!({"original":index,"interval":values,"adapter":"deterministic midpoint, not native RNG parity"}));
                    Some(Op::Wait {
                        seconds: (values[0] + values[1]) * 0.5,
                    })
                }
                "ACTION_TriggerEvent" => Some(Op::Event {
                    tag: text("Event")?,
                }),
                "ACTION_HurtActor" if id == 497 => Some(Op::Injury {
                    damage: number(get("amount")?)? as f32,
                }),
                "ACTION_BlinkHUDHealth" => Some(Op::Blink {
                    seconds: number(get("BlinkTime")?)?,
                }),
                "ACTION_ShowPrompt" => Some(Op::Prompt {
                    text: text("PromptText")?,
                    seconds: number(get("PromptTime")?)?,
                }),
                "ACTION_Briefing" if matches!(get("bEndBriefing")?, Value::Bool(true)) => {
                    Some(Op::Event {
                        tag: "EndBriefing".into(),
                    })
                }
                _ => {
                    reason=Some("not executed in selected briefing host: rope/player animation camera, decorative actors, collision/visor shader or inventory boundary remains open");
                    None
                }
            };
            receipts.push(serde_json::json!({"script":id,"original":index,"name":name,"class":class,"properties":props,"operation":op,"omitted_reason":reason}));
            if let Some(op) = op {
                actions.push(Action {
                    original: index as u32,
                    op,
                });
            }
        }
        programs.push(Program {
            original: id as u32,
            trigger: trigger.into(),
            actions,
        });
    }
    let mut resolver = rc_inspect::assets::Assets::new(&[
        &root.join("Textures"),
        &root.join("Animations"),
        &root.join("System"),
    ])?;
    let gunship = rc_inspect::visual_assets::load_from_animation(
        root,
        "vehicles.ukx",
        "Gunship",
        &[
            "WarriorIntroGunshipBreathe",
            "WarriorIntroGunship",
            "WarriorIntroGunshipLoop",
            "GunshipAmbushSpiderdroid",
            "GunshipAmbushSpiderdroidLoop",
        ],
        &mut resolver,
        Some("Vehicles.cGEO01GunshipSet"),
    )?;
    let copy: rc_game::visual::Mesh = serde_json::from_value(serde_json::to_value(&gunship)?)?;
    let prepared = copy.prepare_reference()?;
    let clip = prepared.clip("WarriorIntroGunshipBreathe")?;
    let geometry = prepared.geometry_with_root(&prepared.channel(clip, 0.)?, false)?;
    let actor = &game.artillery.as_ref().unwrap().assets.actors[0];
    let mut bounds = [[f64::INFINITY; 3], [f64::NEG_INFINITY; 3]];
    for t in &geometry.triangles {
        for v in t.points {
            let at = rc_package::mesh::transform_point(
                v,
                actor.location,
                actor.rotation,
                [1.; 3],
                [0.; 3],
            );
            for i in 0..3 {
                bounds[0][i] = bounds[0][i].min(at[i] as f64);
                bounds[1][i] = bounds[1][i].max(at[i] as f64);
            }
        }
    }
    let camera_target = std::array::from_fn(|i| (bounds[0][i] + bounds[1][i]) * 0.5);
    let mut camera_origin = None;
    for n in 0..16 {
        let yaw = n as f64 * std::f64::consts::TAU / 16.;
        let eye = [
            camera_target[0] + yaw.cos() * 2000.,
            camera_target[1] + yaw.sin() * 2000.,
            camera_target[2] + 900.,
        ];
        if game.wall_fraction(eye, camera_target)? == 1. {
            camera_origin = Some(eye);
            break;
        }
    }
    let camera_origin = camera_origin.ok_or("No clear exterior gunship framing")?;
    fs::write(
        out.join("gunship-bounds.json"),
        serde_json::to_vec_pretty(
            &serde_json::json!({"bounds":bounds,"camera_target":camera_target,"camera_origin":camera_origin,"policy":"collision-checked exterior gunship framing during briefing lock; explicit port camera, no original rope camera parity"}),
        )?,
    )?;
    let initial = cat.resolve("CTCharacters.PlayerCommando", "MaxHealth", 0)?;
    let explosion_props = properties::read(&p, &bytes, &p.exports[210])?;
    let Value::Vector(explosion) = cat
        .instance(
            &p.object_path(p.exports[210].class)?,
            &explosion_props,
            "Location",
            0,
            "geo_01a",
        )?
        .value
    else {
        return Err("Missing explosion factory location".into());
    };
    let assets = Assets {
        rigid_interactions: None,
        ambient_profiles: vec![],
        ambient_policy: None,
        rigid: vec![],
        world_effects: vec![],
        bacta: None,
        sound_policies: vec![],
        enemy_fire_source: None,
        sound_bindings: vec![],
        stop_at_transition: false,
        intro: None,
        rappel: None,
        camera_origin,
        camera_target,
        gunship: Some(gunship),
        programs,
        sounds,
        initial_health: number(initial.value.clone())? as f32,
        explosion: explosion.map(f64::from),
        provenance: serde_json::json!({"scripts":receipts,"audio":audio,"initial_health":initial,"gunship":"Actor580 / Script23 cGEO01GunshipSet; Breathe -> GunshipIdle Intro -> Loop; attack controller takes over later", "boundaries":"selected three script lanes, deterministic random timer midpoint, existing sprite explosion adapter; prepared post-rappel player placement, no rope camera/player root movement/intro Warrior/decorative troop/full particle/visor-shader parity", "mission_health":"original temporary MaxHealth200 before scripted175 damage ->25; existing post-injury mission MaxHealth100 retained"}),
    };
    let meta = serde_json::to_vec(&assets)?;
    let mut result = b"RCPLAY13".to_vec();
    result.extend((meta.len() as u32).to_le_bytes());
    result.extend(meta);
    result.extend(base);
    let decoded = rc_game::bundle::decode(&result)?;
    fs::write(
        out.join("opening-receipt.json"),
        serde_json::to_vec_pretty(&assets.provenance)?,
    )?;
    fs::write(out.join("geo_01a-opening.rcplay"), &result)?;
    fs::write(
        out.join("build-validation.json"),
        serde_json::to_vec_pretty(
            &serde_json::json!({"passed":true,"bytes":result.len(),"sounds":decoded.mission.as_ref().unwrap().sounds.len(),"initial_health":decoded.state.health}),
        )?,
    )?;
    println!(
        "Opening scene: {} bytes, {} sounds",
        result.len(),
        decoded.mission.as_ref().unwrap().sounds.len()
    );
    Ok(())
}
