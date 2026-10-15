use rc_game::{
    map3_balcony::Assets,
    map3_opening::{Action, Program},
    map3_planning::Factory,
    navigation::{Assets as Nav, Edge, Node},
};
use std::{collections::BTreeMap, fs};
fn main() -> Result<(), Box<dyn std::error::Error>> {
    let root = &rc_inspect::build_inputs::game_data()?;
    let out = rc_inspect::build_inputs::output()?;
    fs::create_dir_all(&out)?;
    let raw = fs::read(rc_inspect::build_inputs::input("map_c_breach_scene")?)?;
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
    for actor in &mut game.world.actors {
        if [
            "Prop3",
            "WindowShatter_Test1",
            "MoverDB_GEO29",
            "MoverDBSlave_GEO29",
            "MoverDB_GEO18",
            "MoverDBSlave_GEO18",
        ]
        .contains(&actor.name.as_str())
        {
            actor.shape = rc_package::world_collision::BodyShape::Clear;
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
    let mut factories = vec![];
    let mut placements = vec![];
    for id in [151, 152, 153, 474, 475, 476, 481, 486] {
        let p = props(id);
        let class = p["Prototype"]["path"].as_str().unwrap();
        let scalar = |key| -> Result<f64, Box<dyn std::error::Error>> {
            match catalog.resolve(class, key, 0)?.value {
                rc_package::properties::Value::Float(v) => Ok(v as f64),
                _ => Err("No factory scalar".into()),
            }
        };
        let extent = [
            scalar("CollisionRadius")?,
            scalar("CollisionRadius")?,
            scalar("CollisionHeight")?,
        ];
        let authored = point(id)?;
        let mut position = None;
        'search: for radius in [0., 40., 80., 120., 160.] {
            for k in 0..8 {
                let angle = k as f64 * std::f64::consts::TAU / 8.;
                if let Some(at) = ground(
                    [
                        authored[0] + radius * angle.cos(),
                        authored[1] + radius * angle.sin(),
                        authored[2],
                    ],
                    extent,
                )? {
                    position = Some(at);
                    break 'search;
                }
            }
        }
        let position = position.ok_or("No AA-room factory floor")?;
        factories.push(Factory {
            original: id,
            position,
            health: scalar("Health")? as u32,
            extent,
            warrior: false,
        });
        placements
            .push(serde_json::json!({"original":id,"authored":authored,"supported":position}));
    }
    let mut nav = Nav {
        nodes: vec![],
        edges: vec![],
        provenance: serde_json::json!({"scope":"Original navigation positions with grounded geometric links; planning gates/glass clear, every live step checks actual world. No native ReachSpec parity."}),
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
        if !(-21100. ..=-14900.).contains(&at[0]) || !(4300. ..=13300.).contains(&at[1]) {
            continue;
        }
        if let Some(position) = ground(at, rc_game::squad::EXTENT)? {
            nav.nodes.push(Node {
                original: id as i32,
                name: r["name"].as_str().unwrap().into(),
                position,
                extra_cost: 0.,
            });
        }
    }
    for i in 0..nav.nodes.len() {
        let mut near: Vec<_> = (0..nav.nodes.len())
            .filter(|j| *j != i)
            .map(|j| (j, distance(nav.nodes[i].position, nav.nodes[j].position)))
            .filter(|(_, d)| *d < 1000.)
            .collect();
        near.sort_by(|a, b| a.1.total_cmp(&b.1));
        for (j, d) in near.into_iter().take(14) {
            if rc_game::squad::corridor_with_tolerance(
                &game.world,
                nav.nodes[i].position,
                nav.nodes[j].position,
                rc_game::squad::EXTENT,
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
    println!(
        "Shield original collision {}",
        game.world.actors.iter().any(|a| a.name == "Prop3")
    );
    let start = ground(point(32)?, [50., 50., 115.])?.ok_or("No Sun Fac start floor")?;
    let outside = ground(point(33)?, [50., 50., 115.])?.ok_or("No Sun Fac outside floor")?;
    let sun_route = rc_game::squad::plan_with_tolerance(
        &nav,
        &game.world,
        start,
        outside,
        [50., 50., 115.],
        4.,
    )?;
    if sun_route.is_empty() {
        println!(
            "Sun trace {:?}",
            game.world
                .sweep_motion(start, outside, [50., 50., 115.])?
                .contacts
        );
        return Err(format!("No Sun Fac route {start:?}->{outside:?}").into());
    }
    map.opening
        .as_mut()
        .unwrap()
        .planning
        .as_mut()
        .unwrap()
        .escape
        .as_mut()
        .unwrap()
        .destination = start;
    let standby = ground(point(34)?, rc_game::squad::EXTENT)?.ok_or("No standby floor")?;
    let snipe = ground(point(485)?, rc_game::squad::EXTENT)?.ok_or("No sniper floor")?;
    let mut receipts = vec![];
    let t = rc_package::mesh_query::ActorTransform {
        location: point(412)?.map(|v| v as f32),
        rotation: serde_json::from_value(props(412)["Rotation"].clone())?,
        scale: [1.5; 3],
        pivot: [0.; 3],
    };
    let mut shield = rc_inspect::mission_assets::mesh_scene_skinned(
        root,
        "Arena_GM.Arena.GateShield",
        &t,
        &catalog,
        &mut receipts,
        Some("Arena_Textures.Shader.SpotRaysPCX"),
    )?;
    let mut bounds = [[f64::INFINITY; 3], [f64::NEG_INFINITY; 3]];
    for tri in &shield.triangles {
        for p in tri.points {
            for k in 0..3 {
                bounds[0][k] = bounds[0][k].min(p[k] as f64);
                bounds[1][k] = bounds[1][k].max(p[k] as f64);
            }
        }
    }
    let [minimum, maximum] = &mut bounds;
    for (lo, hi) in minimum.iter_mut().zip(maximum) {
        if *hi - *lo < 1. {
            *lo -= 0.5;
            *hi += 0.5;
        }
    }
    for texture in &mut shield.textures {
        shrink(texture, 64);
    }
    let clips: Vec<rc_game::visual::Clip> = serde_json::from_slice(&fs::read(
        rc_inspect::build_inputs::input("map_c_balcony_clips")?,
    )?)?;
    map.opening.as_mut().unwrap().actors[5]
        .mesh
        .as_mut()
        .unwrap()
        .clips
        .extend(clips);
    let b = map
        .opening
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
        .unwrap();
    b.navigation = nav;
    for material in &mut b.bacta_mesh.as_mut().unwrap().materials {
        let mut texture = rc_render::Texture {
            width: material.width,
            height: material.height,
            pixels: material.pixels.clone(),
        };
        shrink(&mut texture, 64);
        material.width = texture.width;
        material.height = texture.height;
        material.pixels = texture.pixels;
    }
    b.balcony = Some(Box::new(Assets {
        finale: None,
        factories,
        sun_route,
        outside_rotation: serde_json::from_value(props(33)["Rotation"].clone())?,
        shield_snapshot: shield.snapshot()?,
        shield_bounds: bounds,
        standby,
        snipe,
        crate_position: point(484)?,
        crate_actor: row(484)["name"].as_str().unwrap().into(),
        provenance: serde_json::json!({"original_sunfac_actions":[1016,1017,1018,1019,1020,1021,1022],"original_shield_script":413,"shield_collision":"Existing original Prop3 hulls; disabled at level start, enabled by ActivateShield", "scope":"Eight original AA-room factories, selected Sun Fac stationary native gestures and supported movement, shield and Sev standby/assignment; native AI/squad/goals, projectile/effects, Sun Fac run/ship and final objectives pending"}),
    }));
    let mut actions = vec![];
    for (id, op, text, seconds) in [
        (895, "wait", "ActivateShield", 0.),
        (896, "dialog", "GEO_Voice.GEO_01.D38G1513", 0.),
        (897, "emit", "WalkToSnipeMarker", 0.),
        (898, "dialog", "GEO_Voice.GEO_01.CA1G1746", 0.),
        (899, "emit", "TrackSniperCrate", 0.),
        (900, "emit", "ShowBalconySnipe", 0.),
        (901, "emit", "PopUpSnipeMarker", 0.),
        (902, "emit", "SnipeReminder", 0.),
    ] {
        let (op, role, seconds, text) = if op == "dialog" {
            let index = map.sounds.len();
            let sound = rc_inspect::mission_assets::sound(root, text)?.0;
            let duration = wave_seconds(&sound.wave)?;
            map.sounds.push(sound);
            (
                "sound",
                index,
                duration,
                if id == 896 {
                    "There he is, outside the shield."
                } else {
                    "Delta Lead, assign Oh-Seven to sniper position."
                },
            )
        } else {
            (op, 0, seconds, text)
        };
        actions.push(Action {
            original: id,
            op: op.into(),
            text: text.into(),
            role,
            seconds,
        });
    }
    map.opening.as_mut().unwrap().scripts.push(Program {
        original: 145,
        actions,
    });
    map.opening.as_mut().unwrap().scripts.push(Program {
        original: 482,
        actions: vec![
            Action {
                original: 1763,
                op: "wait".into(),
                text: "PopUpSnipeMarker".into(),
                role: 0,
                seconds: 0.,
            },
            Action {
                original: 1765,
                op: "timer".into(),
                text: "".into(),
                role: 0,
                seconds: 0.8,
            },
            Action {
                original: 1767,
                op: "timer".into(),
                text: "".into(),
                role: 0,
                seconds: 0.5,
            },
            Action {
                original: 1768,
                op: "emit".into(),
                text: "EnableBalconySnipe".into(),
                role: 0,
                seconds: 0.,
            },
        ],
    });
    // These intro-only copies are hidden after rappel; live squad uses its separate meshes.
    for actor in map.opening.as_mut().unwrap().actors.iter_mut().take(2) {
        actor.mesh.as_mut().unwrap().clips.retain(|c| {
            ["ActionBreathe", "EntranceRope1760"].contains(&c.sequence.name.name.as_str())
        });
    }
    map.opening.as_mut().unwrap().actors[2]
        .mesh
        .as_mut()
        .unwrap()
        .clips
        .retain(|c| {
            [
                "SevIntro",
                "ActionBreathe",
                "RunForward",
                "CrouchBreathe",
                "CrouchForward",
                "Fire",
                "GestureSignalOK",
                "DoorBreachLoudSM3Coverman",
            ]
            .contains(&c.sequence.name.name.as_str())
        });
    let metadata = serde_json::to_vec(&map)?;
    let size = metadata.len();
    let mut bytes = b"RCPLAY14".to_vec();
    bytes.extend((size as u32).to_le_bytes());
    bytes.extend(metadata);
    bytes.extend(&raw[12 + n..]);
    println!("Proposed metadata {size}; shield bounds {bounds:?}");
    let _ = rc_game::bundle::decode(&bytes)?;
    fs::write(out.join("geo_01c.rcplay"), &bytes)?;
    let b = map
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
        .unwrap();
    fs::write(
        out.join("build-receipt.json"),
        serde_json::to_vec_pretty(
            &serde_json::json!({"bytes":bytes.len(),"metadata_bytes":size,"navigation_nodes":b.navigation.nodes.len(),"navigation_edges":b.navigation.edges.len(),"placements":placements,"standby":standby,"snipe":snipe,"sun_start":start,"sun_outside":outside,"shield_bounds":bounds,"materials":receipts,"sounds":map.sounds.len()}),
        )?,
    )?;
    println!(
        "Balcony scene {} bytes; {}/{} nodes/links",
        bytes.len(),
        b.navigation.nodes.len(),
        b.navigation.edges.len()
    );
    Ok(())
}
fn distance(a: [f64; 3], b: [f64; 3]) -> f64 {
    (0..3).map(|i| (a[i] - b[i]).powi(2)).sum::<f64>().sqrt()
}
fn shrink(t: &mut rc_render::Texture, limit: usize) {
    let w = t.width.min(limit);
    let h = t.height.min(limit);
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
