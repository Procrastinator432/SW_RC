use rc_game::{
    map3_opening::{Action, Actor, Assets, Notify, Program},
    visual::Mesh,
};
use rc_inspect::{assets::Assets as Resolver, visual_assets};
use rc_package::{properties, read_package};
use std::{fs, path::Path};
fn copy<T: serde::Serialize, U: serde::de::DeserializeOwned>(
    v: &T,
) -> Result<U, serde_json::Error> {
    serde_json::from_value(serde_json::to_value(v)?)
}
fn skin(
    root: &Path,
    mesh: &mut Mesh,
    template: &str,
    resolver: &mut Resolver,
) -> Result<(), Box<dyn std::error::Error>> {
    let data = fs::read(root.join("Properties/properties.u"))?;
    let p = read_package(&data)?;
    let e = p
        .exports
        .iter()
        .enumerate()
        .find(|(i, e)| {
            e.class == 0
                && p.object_path(*i as i32 + 1)
                    .is_ok_and(|s| s.eq_ignore_ascii_case(template))
        })
        .ok_or("Missing skin template")?
        .1;
    let props = rc_package::classes::read(&p, &data, e)?.properties;
    let payload = p.payload(&data, e)?;
    let a = props
        .values
        .iter()
        .find(|v| v.name == "Skins")
        .ok_or("Missing template skins")?;
    let skins =
        properties::object_array(&p, &payload[a.payload_offset..a.payload_offset + a.bytes])?;
    if skins.len() != mesh.materials.len() {
        return Err("Skin count mismatch".into());
    }
    let catalog = rc_inspect::defaults::load(root)?;
    for (material, (_, source)) in mesh.materials.iter_mut().zip(skins) {
        let mut d = resolver.diffuse_at_time(&source, 256, 0.)?;
        let (class, props) = resolver.material_properties(&source)?;
        if class == "Engine.Shader"
            && matches!(
                catalog
                    .instance(&class, &props, "Opacity", 0, &source)?
                    .value,
                properties::Value::Object { index: 0, .. }
            )
        {
            for p in &mut d.texture.pixels {
                *p |= 0xff000000;
            }
        }
        material.source = source;
        material.chain = d.chain;
        material.width = d.texture.width;
        material.height = d.texture.height;
        material.pixels = d.texture.pixels;
        material.uv_scale = d.scale;
        material.uv_transform = d.uv_transform;
    }
    Ok(())
}
fn main() -> Result<(), Box<dyn std::error::Error>> {
    let root = Path::new("D:/SteamLibrary/steamapps/common/Star Wars Republic Commando/GameData");
    let out = Path::new("analysis/mission20-map3-opening/delivery");
    fs::create_dir_all(out)?;
    let raw = fs::read("analysis/mission19-map3/delivery/geo_01c.rcplay")?;
    let n = u32::from_le_bytes(raw[8..12].try_into()?) as usize;
    let mut map: rc_game::map_runtime::Assets = serde_json::from_slice(&raw[12..12 + n])?;
    let rows: Vec<serde_json::Value> = serde_json::from_slice(&fs::read(
        "analysis/mission19-map3/original-map-inventory.json",
    )?)?;
    let row = |id| rows.iter().find(|x| x["original"] == id).unwrap();
    let props = |id| {
        row(id)["properties"]["values"]
            .as_array()
            .unwrap()
            .iter()
            .map(|p| {
                (
                    p["name"].as_str().unwrap().to_string(),
                    p["value"]["value"].clone(),
                )
            })
            .collect::<serde_json::Map<_, _>>()
    };
    let vector = |id| -> Result<[f64; 3], serde_json::Error> {
        serde_json::from_value(props(id)["Location"].clone())
    };
    let rotation = |id| -> Result<[i32; 3], serde_json::Error> {
        serde_json::from_value(
            props(id)
                .get("Rotation")
                .cloned()
                .unwrap_or(serde_json::json!([0, 0, 0])),
        )
    };
    let mut resolver = Resolver::new(&[
        &root.join("Textures"),
        &root.join("Animations"),
        &root.join("System"),
    ])?;
    let player: Mesh = serde_json::from_slice(&fs::read(
        "analysis/mission20-map3-opening/player-mesh.json",
    )?)?;
    let mut sev = visual_assets::load_from_animation(
        root,
        "Clone.ukx",
        "CloneCommando",
        &["SevIntro"],
        &mut resolver,
        Some("Clone.cGEO01CloneSet"),
    )?;
    sev.clips.extend(copy::<_, Vec<rc_game::visual::Clip>>(
        &map.squad_mesh.as_ref().unwrap().clips,
    )?);
    skin(root, &mut sev, "Commando07DeltaDefaults", &mut resolver)?;
    let mut scorch: Mesh = copy(map.squad_mesh.as_ref().unwrap())?;
    scorch
        .clips
        .extend(copy::<_, Vec<rc_game::visual::Clip>>(&player.clips)?);
    let mut fixer: Mesh = copy(map.fixer_mesh.as_ref().unwrap())?;
    fixer
        .clips
        .extend(copy::<_, Vec<rc_game::visual::Clip>>(&player.clips)?);
    let mut warrior0 = visual_assets::load_from_animation(
        root,
        "Geonosian.ukx",
        "GeonosianWarrior",
        &["SevIntroNoPike"],
        &mut resolver,
        Some("Geonosian.cGEO01WarriorSet"),
    )?;
    let mut warrior1 = visual_assets::load_from_animation(
        root,
        "Geonosian.ukx",
        "GeonosianWarrior",
        &["SevIntroWithPike"],
        &mut resolver,
        Some("Geonosian.cGEO01WarriorSet"),
    )?;
    let sunfac = visual_assets::load(
        root,
        "Geonosian.ukx",
        "GeonosianWarrior",
        &["ActionBreathe"],
        &mut resolver,
    )?;
    warrior0
        .clips
        .extend(copy::<_, Vec<rc_game::visual::Clip>>(&sunfac.clips)?);
    warrior1
        .clips
        .extend(copy::<_, Vec<rc_game::visual::Clip>>(&sunfac.clips)?);
    let rope = visual_assets::load_from_animation(
        root,
        "miscanimprops.ukx",
        "Rope",
        &["ActionBreathe", "CloneEntrance1760"],
        &mut resolver,
        Some("miscanimprops.RopeSet"),
    )?;
    let mut actors = vec![];
    for (id, role, mesh, extent, armed, remove) in [
        (95, "scorch", scorch, Some([40., 40., 84.]), true, false),
        (97, "fixer", fixer, Some([40., 40., 84.]), true, false),
        (96, "sev", sev, Some([40., 40., 84.]), true, false),
        (290, "warrior0", warrior0, None, false, true),
        (291, "warrior1", warrior1, None, false, true),
        (328, "sunfac", sunfac, None, false, false),
        (100, "rope62", copy(&rope)?, None, false, false),
        (102, "rope40", rope, None, false, false),
    ] {
        actors.push(Actor {
            original: id,
            role: role.into(),
            mesh: Some(mesh),
            position: vector(id)?,
            rotation: rotation(id)?,
            height: if role.starts_with("rope") {
                0.
            } else {
                row(id)["resolved"]["CollisionHeight"]["value"]["value"]
                    .as_f64()
                    .unwrap()
            },
            idle: "ActionBreathe".into(),
            extent,
            armed,
            remove_after_intro: remove,
            sounds: vec![],
        });
    }
    let raw_b = fs::read("analysis/mission18-map2-route/delivery/geo_01b.rcplay")?;
    let size = u32::from_le_bytes(raw_b[8..12].try_into()?) as usize;
    let b: serde_json::Value = serde_json::from_slice(&raw_b[12..12 + size])?;
    let squad_sound = map.sounds.len();
    map.sounds
        .push(serde_json::from_value(b["sounds"][30].clone())?);
    let mut receipts = vec![];
    let mut sound_map = std::collections::BTreeMap::<String, usize>::new();
    for (i, s) in map.sounds.iter().enumerate() {
        sound_map.insert(s.source.clone(), i);
    }
    let mut sound = |source: &str| -> Result<usize, Box<dyn std::error::Error>> {
        if let Some(id) = sound_map.get(source) {
            return Ok(*id);
        }
        let (s, _) = rc_inspect::mission_assets::sound_variant(root, source, &mut receipts)?;
        let id = map.sounds.len();
        map.sounds.push(s);
        sound_map.insert(source.into(), id);
        Ok(id)
    };
    let inventory: Vec<serde_json::Value> = serde_json::from_slice(&fs::read(
        "analysis/mission20-map3-opening/animation-inventory.json",
    )?)?;
    let mut player_sounds = vec![];
    for role in 0..=actors.len() {
        let mesh = if role == 0 {
            &player
        } else {
            actors[role - 1].mesh.as_ref().unwrap()
        };
        let mut notifies = vec![];
        for clip in &mesh.clips {
            if let Some(record) = inventory.iter().find(|v| {
                v["name"] == clip.sequence.name.name
                    && clip.source.contains(v["set"].as_str().unwrap())
            }) {
                for n in record["notifies"].as_array().unwrap() {
                    if n["class"] == "Engine.AnimNotify_Sound" {
                        let property = n["properties"]["values"]
                            .as_array()
                            .unwrap()
                            .iter()
                            .find(|v| v["name"] == "Sound")
                            .unwrap();
                        let source = property["value"]["value"]["path"].as_str().unwrap();
                        notifies.push(Notify {
                            original: n["original"].as_u64().unwrap() as u32,
                            sound: sound(source)?,
                        });
                    } else if n["class"] == "Gameplay.AnimNotify_FireWeapon" {
                        notifies.push(Notify {
                            original: n["original"].as_u64().unwrap() as u32,
                            sound: squad_sound,
                        });
                    }
                }
            }
        }
        if role == 0 {
            player_sounds = notifies
        } else {
            actors[role - 1].sounds = notifies
        }
    }
    let mut scripts = vec![];
    let mut omitted = vec![];
    for (id, actor) in [
        (103, 0),
        (98, 3),
        (99, 1),
        (101, 2),
        (264, 0),
        (292, 4),
        (293, 5),
    ] {
        let mut actions = vec![];
        for action in row(id)["actions"].as_array().unwrap() {
            let n = action[0].as_u64().unwrap() as u32;
            if n == 0 || (id == 98 && n > 785) || (id == 99 && n > 802) || (id == 101 && n > 810) {
                continue;
            }
            let r = row(n);
            let op = r["class"].as_str().unwrap().rsplit('.').next().unwrap();
            let d = props(n);
            let text = |key: &str| {
                d.get(key)
                    .and_then(|v| v.as_str())
                    .unwrap_or("")
                    .to_string()
            };
            let mut a = Action {
                original: n,
                op: String::new(),
                text: String::new(),
                role: 0,
                seconds: 0.,
            };
            match op {
                "ACTION_WaitForEvent" => {
                    a.op = "wait".into();
                    a.text = text("ExternalEvent")
                }
                "ACTION_TriggerEvent" => {
                    a.op = "emit".into();
                    a.text = text("Event")
                }
                "ACTION_WaitForTimer" => {
                    a.op = "timer".into();
                    a.seconds = d["PauseTime"].as_f64().unwrap()
                }
                "ACTION_PlayAnim" | "ACTION_PlayPropAnim" => {
                    a.op = "play".into();
                    a.role = actor;
                    a.text = text("BaseAnim");
                    a.seconds = if op == "ACTION_PlayAnim"
                        && d.get("bWaitUntilAnimEnd")
                            .and_then(|v| v.as_bool())
                            .unwrap_or(true)
                    {
                        1.
                    } else {
                        0.
                    };
                }
                "ACTION_PlaySound" => {
                    a.op = "sound".into();
                    a.role = sound(d["Sound"]["path"].as_str().unwrap())?;
                    a.text = text("Comment");
                    a.seconds = 0.;
                }
                "ACTION_ChangeSquadMember"
                    if d.get("bAddMember")
                        .and_then(|v| v.as_bool())
                        .unwrap_or(true) =>
                {
                    a.op = "join".into()
                }
                "ACTION_Briefing" if d.get("bEndBriefing") == Some(&serde_json::json!(true)) => {
                    a.op = "unlock".into()
                }
                "ACTION_ObjectiveUpdate" => {
                    if let Some(obj) = d.get("ObjectiveNum") {
                        a.op = "objective".into();
                        a.role = obj.as_u64().unwrap() as usize;
                        actions.push(a);
                        a = Action {
                            original: n,
                            op: String::new(),
                            text: String::new(),
                            role: 0,
                            seconds: 0.,
                        };
                    }
                    if let Some(obj) = d.get("ChangeToObjNum") {
                        a.op = "active".into();
                        a.role = obj.as_u64().unwrap() as usize;
                    }
                }
                _ => {
                    omitted.push(serde_json::json!({"original":n,"operation":op,"scope":"Not executed in this selected opening lane"}));
                    continue;
                }
            }
            if !a.op.is_empty() {
                actions.push(a)
            }
        }
        scripts.push(Program {
            original: id,
            actions,
        });
    }
    // The rope receiver is triggered by the matching clone lane, concurrently with its body animation.
    for (index, trigger) in [(6, "CloneRope62"), (7, "CloneRope40")] {
        scripts.push(Program {
            original: actors[index].original,
            actions: vec![
                Action {
                    original: actors[index].original,
                    op: "wait".into(),
                    text: trigger.into(),
                    role: 0,
                    seconds: 0.,
                },
                Action {
                    original: actors[index].original,
                    op: "play".into(),
                    text: "CloneEntrance1760".into(),
                    role: index + 1,
                    seconds: 0.,
                },
            ],
        });
    }

    for p in &mut scripts {
        for a in &mut p.actions {
            if a.op == "sound" {
                a.seconds = wave_seconds(&map.sounds[a.role].wave)?;
            }
        }
    }
    let scale = std::f64::consts::TAU / 65536.;
    let a = Assets {
        planning: None,
        player: Some(player),
        start: vector(92)?,
        rotation: rotation(92)?,
        player_sounds,
        actors,
        scripts,
        limits: [
            -2000. * scale,
            2000. * scale,
            -9239. * scale,
            -5239. * scale,
        ],
        eye_height: match rc_inspect::defaults::load(root)?
            .resolve("CTCharacters.PlayerCommando", "BaseEyeHeight", 0)?
            .value
        {
            properties::Value::Float(v) => v as f64,
            _ => return Err("Missing original eye height".into()),
        },
        provenance: serde_json::json!({"original_programs":[103,98,99,101,264,292,293],"omitted":omitted,"scope":"Native entry clips and notify timing; selected script lanes, bounded collision/follow host; planning-room combat, flyby, particles and Karma remain pending"}),
    };
    map.opening = Some(Box::new(a));
    let metadata = serde_json::to_vec(&map)?;
    let mut bytes = b"RCPLAY14".to_vec();
    bytes.extend((metadata.len() as u32).to_le_bytes());
    bytes.extend(&metadata);
    bytes.extend(&raw[12 + n..]);
    let game = rc_game::bundle::decode(&bytes)?;
    fs::write(out.join("geo_01c.rcplay"), &bytes)?;
    fs::write(
        out.join("build-receipt.json"),
        serde_json::to_vec_pretty(
            &serde_json::json!({"bytes":bytes.len(),"metadata_bytes":metadata.len(),"sounds":map.sounds.len(),"provenance":game.map.as_ref().unwrap().opening.as_ref().unwrap().assets.provenance,"sound_receipts":receipts}),
        )?,
    )?;
    println!(
        "Third-map opening: {} bytes, {} metadata bytes, {} sounds",
        bytes.len(),
        metadata.len(),
        map.sounds.len()
    );
    Ok(())
}
fn wave_seconds(bytes: &[u8]) -> Result<f64, String> {
    let mut offset = 12;
    let mut rate = 0;
    let mut data = 0;
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
