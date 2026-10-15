use rc_game::{
    map3_finale::{Assets, Cue},
    map3_opening::{Action, Program},
    map3_planning::Factory,
    navigation::{Assets as Nav, Edge, Node},
};
use std::{collections::BTreeMap, fs};
fn main() -> Result<(), Box<dyn std::error::Error>> {
    let root = &rc_inspect::build_inputs::game_data()?;
    let out = rc_inspect::build_inputs::output()?;
    fs::create_dir_all(&out)?;
    let raw = fs::read(rc_inspect::build_inputs::input("map_c_balcony_scene")?)?;
    let n = u32::from_le_bytes(raw[8..12].try_into()?) as usize;
    let mut map: rc_game::map_runtime::Assets = serde_json::from_slice(&raw[12..12 + n])?;
    let mut game = rc_game::bundle::decode(&raw)?;
    let rows: Vec<serde_json::Value> = serde_json::from_slice(&fs::read(
        rc_inspect::build_inputs::input("map_c_inventory")?,
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
    for a in &mut game.world.actors {
        if a.name == "Prop3" {
            a.shape = rc_package::world_collision::BodyShape::Clear;
        }
    }
    let ground = |at: [f64; 3], extent: [f64; 3]| -> Result<Option<[f64; 3]>, String> {
        for rise in [0., 40., 80., 160., 250.] {
            if let Some(p) = rc_game::mission_motion::ground(
                &game.world,
                [at[0], at[1], at[2] + rise],
                extent,
                800.,
            )? {
                return Ok(Some(p));
            }
        }
        Ok(None)
    };
    let mut nav = Nav {
        nodes: vec![],
        edges: vec![],
        provenance: serde_json::json!({"scope":"Original walking points with geometric links for Sun Fac; original gates are checked by every live body step"}),
        lunge_distance: 0.,
        jump_distance: 0.,
        offensive_jump_probability: 0.,
    };
    for r in &rows {
        let id = r["original"].as_u64().unwrap() as u32;
        let p = props(id);
        if !p.contains_key("PathList") {
            continue;
        }
        let Ok(at) = point(id) else {
            continue;
        };
        if !(-16000. ..=-10300.).contains(&at[0]) || !(6000. ..=9000.).contains(&at[1]) {
            continue;
        }
        if let Some(position) = ground(at, [50., 50., 115.])? {
            nav.nodes.push(Node {
                original: id as i32,
                name: r["name"].as_str().unwrap().into(),
                position,
                extra_cost: 0.,
            });
        }
    }
    for i in 0..nav.nodes.len() {
        for j in 0..nav.nodes.len() {
            let d = distance(nav.nodes[i].position, nav.nodes[j].position);
            if i != j
                && d < 1000.
                && rc_game::squad::corridor_with_tolerance(
                    &game.world,
                    nav.nodes[i].position,
                    nav.nodes[j].position,
                    [50., 50., 115.],
                    4.,
                )?
                .is_some()
            {
                nav.edges.push(Edge {
                    original: nav.edges.len() as i32 + 1,
                    start: i,
                    end: j,
                    distance: d,
                });
            }
        }
    }
    nav.validate()?;
    let outside = ground(point(33)?, [50., 50., 115.])?.ok_or("No Sun outside floor")?;
    let fighter = ground(point(133)?, [50., 50., 115.])?.ok_or("No fighter floor")?;
    let boarding = ground(point(512)?, [50., 50., 115.])?.ok_or("No boarding floor")?;
    let run = rc_game::squad::plan_with_tolerance(
        &nav,
        &game.world,
        outside,
        fighter,
        [50., 50., 115.],
        4.,
    )?;
    let board = rc_game::squad::plan_with_tolerance(
        &nav,
        &game.world,
        fighter,
        boarding,
        [50., 50., 115.],
        4.,
    )?;
    if run.is_empty() || board.is_empty() {
        return Err(format!(
            "Missing Sun Fac paths: {outside:?}->{fighter:?}->{boarding:?}; {}/{} steps",
            run.len(),
            board.len()
        )
        .into());
    }
    let mut ship: rc_game::visual::Mesh = serde_json::from_slice(&fs::read(
        rc_inspect::build_inputs::input("map_c_ship_mesh")?,
    )?)?;
    for material in &mut ship.materials {
        shrink_material(material, 128);
    }
    let sun: Vec<rc_game::visual::Clip> = serde_json::from_slice(&fs::read(
        rc_inspect::build_inputs::input("map_c_finale_clips")?,
    )?)?;
    map.opening.as_mut().unwrap().actors[5]
        .mesh
        .as_mut()
        .unwrap()
        .clips
        .extend(sun);
    let sniper: Vec<rc_game::visual::Clip> = serde_json::from_slice(&fs::read(
        rc_inspect::build_inputs::input("map_c_sniper_clips")?,
    )?)?;
    map.opening.as_mut().unwrap().actors[2]
        .mesh
        .as_mut()
        .unwrap()
        .clips
        .extend(sniper);
    let mut receipts = vec![];
    let t = rc_package::mesh_query::ActorTransform {
        location: point(382)?.map(|v| v as f32),
        rotation: serde_json::from_value(props(382)["Rotation"].clone())?,
        scale: [1.; 3],
        pivot: [0.; 3],
    };
    let mut wreck = rc_inspect::mission_assets::mesh_scene(
        root,
        "VehicleMeshes.Confederacy.GeoFighterBroken",
        &t,
        &catalog,
        &mut receipts,
    )?;
    for texture in &mut wreck.textures {
        shrink_texture(texture, 128);
    }
    let mut sniper = rc_inspect::mission_assets::mesh_scene(
        root,
        "Weapons.CloneWeapons.DC17mSniper",
        &rc_package::mesh_query::ActorTransform {
            location: [0.; 3],
            rotation: [0; 3],
            scale: [1.; 3],
            pivot: [0.; 3],
        },
        &catalog,
        &mut receipts,
    )?;
    for texture in &mut sniper.textures {
        shrink_texture(texture, 128);
    }
    let climber_at = point(488)?;
    let mut supported = None;
    'search: for radius in [0., 40., 80., 120., 160.] {
        for k in 0..8 {
            let a = k as f64 * std::f64::consts::TAU / 8.;
            if let Some(at) = ground(
                [
                    climber_at[0] + radius * a.cos(),
                    climber_at[1] + radius * a.sin(),
                    climber_at[2],
                ],
                [50., 50., 115.],
            )? {
                supported = Some(at);
                break 'search;
            }
        }
    }
    let climber = Factory {
        original: 141,
        position: supported.ok_or("No warrior spawn floor")?,
        health: match catalog
            .resolve("CTCharacters.GeonosianWarrior", "Health", 0)?
            .value
        {
            rc_package::properties::Value::Float(h) => h as u32,
            _ => return Err("No warrior health".into()),
        },
        extent: [50., 50., 115.],
        warrior: true,
    };
    let mut get_cue = |id: u32| -> Result<Cue, Box<dyn std::error::Error>> {
        let p = props(id);
        let (sound, _) =
            rc_inspect::mission_assets::sound(root, p["Sound"]["path"].as_str().unwrap())?;
        let seconds = wave_seconds(&sound.wave)?;
        let index = if let Some(i) = map
            .sounds
            .iter()
            .position(|s| s.source == sound.source && s.wave == sound.wave)
        {
            i
        } else {
            let i = map.sounds.len();
            map.sounds.push(sound);
            i
        };
        Ok(Cue {
            original: id,
            sound: index,
            seconds,
            text: p
                .get("Comment")
                .and_then(|v| v.as_str())
                .unwrap_or("")
                .into(),
        })
    };
    let reminders = [1627, 1632, 1637, 1642]
        .map(&mut get_cue)
        .into_iter()
        .collect::<Result<Vec<_>, _>>()?;
    let start = get_cue(1650)?;
    let takeoff = get_cue(1653)?;
    let failures = [1323, 1324]
        .map(&mut get_cue)
        .into_iter()
        .collect::<Result<Vec<_>, _>>()?;
    let mut actions = vec![];
    for id in 1286..=1308 {
        let p = props(id);
        let class = row(id)["class"]
            .as_str()
            .unwrap()
            .rsplit('.')
            .next()
            .unwrap();
        let mut a = Action {
            original: id,
            op: String::new(),
            text: String::new(),
            role: 0,
            seconds: 0.,
        };
        match class {
            "ACTION_WaitForEvent" => {
                a.op = "wait".into();
                a.text = p["ExternalEvent"].as_str().unwrap().into();
            }
            "ACTION_TriggerEvent" => {
                a.op = "emit".into();
                a.text = p["Event"].as_str().unwrap().into();
            }
            "ACTION_WaitForTimer" => {
                a.op = "timer".into();
                a.seconds = p["PauseTime"].as_f64().unwrap();
            }
            "ACTION_GotoState" => {
                a.op = "emit".into();
                a.text = "DisableBalconySnipe".into();
            }
            "ACTION_PlaySound" => {
                let c = get_cue(id)?;
                a.op = "sound".into();
                a.text = c.text;
                a.seconds = c.seconds;
                a.role = c.sound;
            }
            "ACTION_ObjectiveUpdate" => {
                a.op = "objective".into();
                a.role = p["ObjectiveNum"].as_u64().unwrap() as usize;
            }
            "ACTION_ShowActorObjectiveTracker" => {
                a.op = "emit".into();
                a.text = "TrackElevatorDoor".into();
            }
            "ACTION_SetCanCancelMarker" => continue,
            _ => return Err(format!("Unknown finale action {class}").into()),
        };
        actions.push(a);
    }
    map.opening.as_mut().unwrap().scripts.push(Program {
        original: 287,
        actions,
    });
    let (sound, _) =
        rc_inspect::mission_assets::sound(root, "GEN_Sound.Weapons.wep_GEN_sniperFire_01")?;
    let sniper_sound = map.sounds.len();
    map.sounds.push(sound);
    map.opening
        .as_mut()
        .unwrap()
        .planning
        .as_mut()
        .unwrap()
        .guard
        .as_mut()
        .unwrap()
        .breach
        .as_mut()
        .unwrap()
        .balcony
        .as_mut()
        .unwrap()
        .finale = Some(Box::new(Assets {
        elevator: None,
        ship: Some(ship),
        ship_anchor: point(320)?,
        ship_rotation: serde_json::from_value(props(320)["Rotation"].clone())?,
        wreck_snapshot: wreck.snapshot()?,
        sniper_snapshot: sniper.snapshot()?,
        run,
        board,
        climber,
        reminders,
        start,
        takeoff,
        failures,
        sniper_sound,
        shot_delay: 0.8,
        elevator_target: point(45)?,
        provenance: serde_json::json!({"original_programs":[98,193,287,288,289,319,415,416],"scope":"Selected original actions, native animation/root/notify clocks and source fighter meshes; bounded grounded walking, one supported warrior rather than full climb/patrol, sniper cue/FireSniper with five-second shooting host and 0.8s port cadence; success chosen by Sev position, not projectile damage. Original KillMe130 reveals wreck; native effects, AI/Squad, weapon ballistics and exact failure incapacitation pending.","goal0":"Eliminate Sun Fac completed by successful SunFacTerminated; original script287 repeats objective1; elevator goal6 remains pending."}),
    }));
    let metadata = serde_json::to_vec(&map)?;
    let size = metadata.len();
    println!(
        "Proposed metadata {size}; {} original sounds",
        map.sounds.len()
    );
    let mut bytes = b"RCPLAY14".to_vec();
    bytes.extend((size as u32).to_le_bytes());
    bytes.extend(metadata);
    bytes.extend(&raw[12 + n..]);
    let _ = rc_game::bundle::decode(&bytes)?;
    fs::write(out.join("geo_01c.rcplay"), &bytes)?;
    let a = map
        .opening
        .as_ref()
        .unwrap()
        .planning
        .as_ref()
        .unwrap()
        .guard
        .as_ref()
        .unwrap()
        .breach
        .as_ref()
        .unwrap()
        .balcony
        .as_ref()
        .unwrap()
        .finale
        .as_ref()
        .unwrap();
    fs::write(
        out.join("build-receipt.json"),
        serde_json::to_vec_pretty(
            &serde_json::json!({"bytes":bytes.len(),"metadata_bytes":size,"run_steps":a.run.len(),"board_steps":a.board.len(),"sun_start":outside,"fighter":fighter,"boarding":boarding,"climber_authored":climber_at,"climber_supported":a.climber.position,"ship_anchor":a.ship_anchor,"ship_rotation":a.ship_rotation,"original_sounds":map.sounds.len(),"materials":receipts,"compact_audio":"Lossless base64 JSON; old byte arrays supported, audio bank unchanged"}),
        )?,
    )?;
    println!("Finale scene {} bytes", bytes.len());
    Ok(())
}
fn distance(a: [f64; 3], b: [f64; 3]) -> f64 {
    (0..3).map(|i| (a[i] - b[i]).powi(2)).sum::<f64>().sqrt()
}
fn shrink_material(m: &mut rc_game::visual::Material, n: usize) {
    let mut t = rc_render::Texture {
        width: m.width,
        height: m.height,
        pixels: m.pixels.clone(),
    };
    shrink_texture(&mut t, n);
    m.width = t.width;
    m.height = t.height;
    m.pixels = t.pixels;
}
fn shrink_texture(t: &mut rc_render::Texture, n: usize) {
    let w = t.width.min(n);
    let h = t.height.min(n);
    let mut p = Vec::with_capacity(w * h);
    for y in 0..h {
        for x in 0..w {
            p.push(t.pixels[(y * t.height / h) * t.width + x * t.width / w]);
        }
    }
    t.width = w;
    t.height = h;
    t.pixels = p;
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
