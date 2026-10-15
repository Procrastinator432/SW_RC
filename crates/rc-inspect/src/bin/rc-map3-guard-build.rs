use rc_game::{
    map3_guard::{Actor, Assets, Spawn},
    map3_opening::{Action, Notify, Program},
    map3_planning::Factory,
};
use std::{collections::BTreeMap, fs, path::Path};
fn main() -> Result<(), Box<dyn std::error::Error>> {
    let root = Path::new("D:/SteamLibrary/steamapps/common/Star Wars Republic Commando/GameData");
    let out = Path::new("analysis/mission23-map3-guard/delivery");
    fs::create_dir_all(out)?;
    let raw = fs::read("analysis/mission22-map3-escape/delivery/geo_01c.rcplay")?;
    let n = u32::from_le_bytes(raw[8..12].try_into()?) as usize;
    let mut map: rc_game::map_runtime::Assets = serde_json::from_slice(&raw[12..12 + n])?;
    let rows: Vec<serde_json::Value> = serde_json::from_slice(&fs::read(
        "analysis/mission19-map3/original-map-inventory.json",
    )?)?;
    let row = |id| rows.iter().find(|r| r["original"] == id).unwrap();
    let props = |id| {
        row(id)["properties"]["values"]
            .as_array()
            .unwrap()
            .iter()
            .map(|p| {
                (
                    p["name"].as_str().unwrap().to_owned(),
                    p["value"]["value"].clone(),
                )
            })
            .collect::<BTreeMap<_, _>>()
    };
    let point = |id| -> Result<[f64; 3], serde_json::Error> {
        serde_json::from_value(props(id)["Location"].clone())
    };
    let catalog = rc_inspect::defaults::load(root)?;
    let mut resolver = rc_inspect::assets::Assets::new(&[
        &root.join("Textures"),
        &root.join("Animations"),
        &root.join("System"),
    ])?;
    let mut sbd = rc_inspect::visual_assets::load(
        root,
        "Droid.ukx",
        "SuperBattleDroid",
        &["ActionBreathe", "WalkForward"],
        &mut resolver,
    )?;
    let intro = rc_inspect::visual_assets::load_from_animation(
        root,
        "Droid.ukx",
        "SuperBattleDroid",
        &["IntroSBD"],
        &mut resolver,
        Some("Droid.cGEO01SBDSet"),
    )?;
    sbd.clips.extend(intro.clips);
    let intro = rc_inspect::visual_assets::load_from_animation(
        root,
        "Droid.ukx",
        "BattleDroid",
        &["IntroSBDRightPush", "IntroSBDLeftSmash"],
        &mut resolver,
        Some("Droid.cGEO01BattleDroidSet"),
    )?;
    map.opening
        .as_mut()
        .unwrap()
        .planning
        .as_mut()
        .unwrap()
        .droid
        .as_mut()
        .unwrap()
        .clips
        .extend(intro.clips);
    let g = rc_game::bundle::decode(&raw)?;
    let mut factories = vec![];
    let mut placements = vec![];
    for id in [196, 197, 228, 229, 230, 231, 232, 233] {
        let p = props(id);
        let class = p["Prototype"]["path"].as_str().unwrap();
        let scalar = |key| -> Result<f64, Box<dyn std::error::Error>> {
            match catalog.resolve(class, key, 0)?.value {
                rc_package::properties::Value::Float(v) => Ok(v as f64),
                _ => Err("Missing original factory scalar".into()),
            }
        };
        let extent = [
            scalar("CollisionRadius")?,
            scalar("CollisionRadius")?,
            scalar("CollisionHeight")?,
        ];
        let authored = point(id)?;
        let mut supported = None;
        'search: for radius in [0., 40., 80., 120., 160.] {
            for angle in 0..8 {
                let yaw = angle as f64 * std::f64::consts::TAU / 8.;
                for rise in [0., 40., 80., 160., 250.] {
                    if let Some(at) = rc_game::mission_motion::ground(
                        &g.world,
                        [
                            authored[0] + radius * yaw.cos(),
                            authored[1] + radius * yaw.sin(),
                            authored[2] + rise,
                        ],
                        extent,
                        800.,
                    )? {
                        supported = Some(at);
                        break 'search;
                    }
                }
            }
        }
        let position = supported.ok_or(format!("No supported original factory {id}"))?;
        placements.push(serde_json::json!({"original":id,"authored":authored,"supported":position,"extent":extent}));
        factories.push(Spawn {
            factory: Factory {
                original: id,
                position,
                health: scalar("Health")? as u32,
                extent,
                warrior: class.contains("Warrior"),
            },
            sbd: class.contains("SuperBattleDroid"),
        });
    }
    let fire_sound = map.sounds.len();
    map.sounds
        .push(rc_inspect::mission_assets::sound(root, "GEN_Sound.Weapons.wep_GEN_sbdGun_01")?.0);
    let mut actors = vec![];
    let original_assets: serde_json::Value = serde_json::from_slice(&fs::read(
        "analysis/mission23-map3-guard/original-assets.json",
    )?)?;
    for (id, clip) in [
        (321, "IntroSBDRightPush"),
        (322, "IntroSBD"),
        (323, "IntroSBDLeftSmash"),
    ] {
        let notifies = original_assets["clips"]
            .as_array()
            .unwrap()
            .iter()
            .find(|s| s["sequence"]["name"]["name"] == clip)
            .unwrap()["notifies"]
            .as_array()
            .unwrap();
        let sounds = notifies
            .iter()
            .filter(|n| n["class"] == "Gameplay.AnimNotify_FireWeapon")
            .map(|n| Notify {
                original: n["original"].as_u64().unwrap() as u32,
                sound: fire_sound,
            })
            .collect();
        actors.push(Actor {
            original: id,
            sbd: id == 322,
            position: point(id)?,
            rotation: serde_json::from_value(props(id)["Rotation"].clone())?,
            clip: clip.into(),
            height: row(id)["resolved"]["CollisionHeight"]["value"]["value"]
                .as_f64()
                .unwrap(),
            sounds,
        });
    }
    let next = rows
        .iter()
        .find(|r| r["resolved"]["Tag"]["value"]["value"] == "BreachDoorSunFac")
        .ok_or("Missing next original door tracker")?["original"]
        .as_u64()
        .unwrap() as u32;
    map.opening
        .as_mut()
        .unwrap()
        .planning
        .as_mut()
        .unwrap()
        .guard = Some(Box::new(Assets {
        breach: None,
        fire_sound,
        mesh: Some(sbd),
        actors,
        factories,
        next_target: point(next)?,
        provenance: serde_json::json!({"intro_actors":[321,322,323],"original_programs":[147,324,325,326,358],"original_factories":[196,197,228,229,230,231,232,233],"scope":"Native animation channels/notifies/root matrices and selected original actions; bounded spawn/combat/focus host, intro invulnerable until animation end, decompose droid bodies on AnimEnd; no native AI/Karma/armor equivalence","fire_sound":"Properties.BattleDroidBlasterDefaults.FireSound; shared bounded SBD cue; full native SBD weapon/audio binding pending"}),
    }));
    let mut sound_receipts = vec![];
    let mut scripts = vec![];
    for (script, ids) in [
        (147, (904..919).collect::<Vec<_>>()),
        (358, (1497..1504).collect()),
    ] {
        let mut actions = vec![];
        for id in ids {
            let p = props(id);
            if p.get("bDisabled").is_some_and(|v| v == true) {
                continue;
            }
            let mut a = Action {
                original: id,
                op: String::new(),
                text: String::new(),
                role: 0,
                seconds: 0.,
            };
            let class = row(id)["class"]
                .as_str()
                .unwrap()
                .rsplit('.')
                .next()
                .unwrap();
            match class {
                "ACTION_WaitForEvent" => {
                    a.op = "wait".into();
                    a.text = p["ExternalEvent"].as_str().unwrap().into();
                }
                "ACTION_WaitForTimer" => {
                    a.op = "timer".into();
                    a.seconds = p["PauseTime"].as_f64().unwrap();
                }
                "ACTION_TriggerEvent" => {
                    a.op = "emit".into();
                    a.text = p["Event"].as_str().unwrap().into();
                }
                "ACTION_GotoState" => {
                    assert_eq!(p["State"], "Unlocked");
                    a.op = "emit".into();
                    a.text = "GuardRoomDoorUnlocked".into();
                }
                "ACTION_ObjectiveUpdate" => {
                    a.op = "objective".into();
                    a.role = p["ObjectiveNum"].as_u64().unwrap() as usize;
                }
                "ACTION_PlaySound" => {
                    let (sound, _) = rc_inspect::mission_assets::sound_variant(
                        root,
                        p["Sound"]["path"].as_str().unwrap(),
                        &mut sound_receipts,
                    )?;
                    a.op = "sound".into();
                    a.role = map.sounds.len();
                    a.text = p["Comment"].as_str().unwrap().into();
                    a.seconds = wave_seconds(&sound.wave)?;
                    map.sounds.push(sound);
                }
                "ACTION_ShowActorObjectiveTracker" => {
                    a.op = "emit".into();
                    a.text = "TrackBreachDoorSunFac".into();
                }
                _ => return Err(format!("Unsupported selected original action {class}").into()),
            }
            actions.push(a);
        }
        scripts.push(Program {
            original: script,
            actions,
        });
    }
    map.opening.as_mut().unwrap().scripts.extend(scripts);
    let metadata = serde_json::to_vec(&map)?;
    let metadata_bytes = metadata.len();
    let mut bytes = b"RCPLAY14".to_vec();
    bytes.extend((metadata.len() as u32).to_le_bytes());
    bytes.extend(metadata);
    bytes.extend(&raw[12 + n..]);
    let decoded = rc_game::bundle::decode(&bytes)?;
    fs::write(out.join("geo_01c.rcplay"), &bytes)?;
    fs::write(
        out.join("build-receipt.json"),
        serde_json::to_vec_pretty(
            &serde_json::json!({"bytes":bytes.len(),"metadata_bytes":metadata_bytes,"factory_placements":placements,"sounds":map.sounds.len(),"sound_receipts":sound_receipts,"native_intro_sequences":original_assets["clips"],"next_tracker":next,"entry":decoded.state}),
        )?,
    )?;
    println!(
        "Guard room: {} bytes; {} metadata bytes; {} sounds",
        bytes.len(),
        metadata_bytes,
        map.sounds.len()
    );
    Ok(())
}
fn wave_seconds(bytes: &[u8]) -> Result<f64, String> {
    let (mut offset, mut rate, mut data) = (12, 0, 0);
    while offset + 8 <= bytes.len() {
        let n = u32::from_le_bytes(bytes[offset + 4..offset + 8].try_into().unwrap()) as usize;
        let p = offset + 8;
        if p + n > bytes.len() {
            return Err("Truncated wave".into());
        }
        match &bytes[offset..offset + 4] {
            b"fmt " if n >= 12 => {
                rate = u32::from_le_bytes(bytes[p + 8..p + 12].try_into().unwrap())
            }
            b"data" => data = n,
            _ => {}
        }
        offset = p + n + (n & 1);
    }
    if rate == 0 || data == 0 {
        return Err("Missing wave duration".into());
    }
    Ok(data as f64 / rate as f64)
}
