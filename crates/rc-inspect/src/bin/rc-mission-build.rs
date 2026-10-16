use rc_game::{bundle, mission, navigation, Point};
use rc_inspect::defaults;
use rc_package::{
    properties::{self, Value},
    read_package,
};
use std::{env, fs, path::Path};

fn dist(a: Point, b: Point) -> f64 {
    (0..3).map(|i| (a[i] - b[i]).powi(2)).sum::<f64>().sqrt()
}
fn advisor(data: &[u8], object: &str) -> Result<rc_game::combat::Sound, String> {
    let pkg = read_package(data)?;
    let e = pkg
        .exports
        .iter()
        .enumerate()
        .find_map(|(i, e)| {
            pkg.object_path(i as i32 + 1)
                .is_ok_and(|p| p == object)
                .then_some(e)
        })
        .ok_or("Missing advisor line")?;
    if pkg.object_path(e.class)? != "Engine.Sound" {
        return Err("Unsupported advisor sound".into());
    }
    let p = pkg.payload(data, e)?;
    let props = properties::read(&pkg, data, e)?;
    let at = p
        .windows(4)
        .position(|v| v == b"RIFF")
        .ok_or("Missing advisor RIFF")?;
    if at < props.native_offset || at > 128 {
        return Err("Unsupported advisor wrapper".into());
    }
    let n = u32::from_le_bytes(
        p.get(at + 4..at + 8)
            .ok_or("Short RIFF")?
            .try_into()
            .unwrap(),
    ) as usize
        + 8;
    let wave = p.get(at..at + n).ok_or("Short advisor wave")?.to_vec();
    rc_game::combat::validate_wave(&wave)?;
    Ok(rc_game::combat::Sound {
        source: format!("GEO_Voice.{object}"),
        wave,
    })
}
fn walking(game: &rc_game::Game, from: Point, to: Point) -> Result<bool, String> {
    let mut body = rc_package::controller::ControlledBody {
        body: rc_package::physics::BodyState {
            position: from,
            velocity: [0.; 3],
            grounded: true,
        },
        jump_held: false,
    };
    let runtime = rc_package::pawn_motion::PawnRuntimeOptions {
        maximum_desired_speed: game.profile.controller.speed,
        weapon_modifier: 1.,
        crouch_ratio: 1.,
        wounded_ratio: 1.,
        crouched: false,
        wounded: false,
        wants_to_crouch: false,
        current_jump_z: game.profile.controller.jump_speed,
    };
    let mut best = dist(from, to);
    let mut stalled = 0;
    for _ in 0..600 {
        let p = body.body.position;
        if dist(p, to) < 35. && body.body.grounded {
            return Ok(true);
        }
        let yaw = ((to[1] - p[1]).atan2(to[0] - p[0]) * 65536. / std::f64::consts::TAU) as i32;
        let frame = game.volumes.pawn_diagnostic_tick(
            &game.world,
            &body,
            rc_package::controller::ViewInput {
                forward: 1.,
                strafe: 0.,
                yaw,
                walking: false,
                jump: false,
            },
            rc_game::STEP,
            &game.profile,
            runtime,
        )?;
        body = frame.state;
        let d = dist(body.body.position, to);
        if d < best - 1. {
            best = d;
            stalled = 0;
        } else {
            stalled += 1;
        }
        if stalled > 45 {
            return Ok(false);
        }
    }
    Ok(false)
}
fn main() -> Result<(), Box<dyn std::error::Error>> {
    let a: Vec<_> = env::args_os().collect();
    if a.len() != 4 {
        return Err("usage: rc-mission-build GAME_DATA VISUAL_BASE OUTPUT".into());
    }
    let game_data = Path::new(&a[1]);
    let out = Path::new(&a[3]);
    fs::create_dir_all(out)?;
    let base = fs::read(&a[2])?;
    let mut game = bundle::decode(&base)?;
    let map = fs::read(game_data.join("Maps/geo_01a.ctm"))?;
    let pkg = read_package(&map)?;
    let catalog = defaults::load(game_data)?;
    let number = |class: &str, name: &str| -> Result<f32, String> {
        match catalog.resolve(class, name, 0)?.value {
            Value::Float(v) => Ok(v),
            Value::Int(v) => Ok(v as f32),
            _ => Err(format!("Invalid {class}.{name}")),
        }
    };
    let rate = number("MarkerProperties.BactaDispenserGEO", "BactaRate")?;
    let max_health = number("CTCharacters.PlayerCommando", "Health")?;
    let mut nodes = vec![navigation::Node {
        original: 10,
        name: "PlayerStart0.floor".into(),
        position: game.state.pawn.body.position,
        extra_cost: 0.,
    }];
    let mut station = [0.; 3];
    let mut goal = None;
    let ground = |location: Point| -> Result<Option<Point>, String> {
        let p = [location[0], location[1], location[2] + 100.];
        let floor = match game
            .world
            .floor_contact(p, game.profile.physics.extent, 500., 0.7)
        {
            Ok(v) => v,
            Err(e) if e.contains("penetrat") => return Ok(None),
            Err(e) => return Err(e),
        };
        Ok(floor.map(|h| [p[0], p[1], p[2] - 500. * h.hit.fraction + 0.5]))
    };
    for (i, e) in pkg.exports.iter().enumerate() {
        let class = pkg.object_path(e.class)?;
        if ![
            "Engine.PathNode",
            "CTGame.SquadMarkerSpot",
            "MarkerProperties.BactaDispenserGEO",
        ]
        .contains(&class.as_str())
            && i + 1 != 737
        {
            continue;
        }
        let p = properties::read(&pkg, &map, e)?;
        let location = p.values.iter().find_map(|p| match (&*p.name, &p.value) {
            ("Location", Value::Vector(v)) => Some(v.map(f64::from)),
            _ => None,
        });
        let Some(location) = location else {
            continue;
        };
        if i + 1 == 736 {
            station = location;
            continue;
        }
        if location[0] < -3000.
            || location[0] > 3000.
            || location[1] < -2200.
            || location[1] > 2600.
        {
            continue;
        }
        let Some(position) = ground(location)? else {
            continue;
        };
        if game
            .world
            .body_placement(position, game.profile.physics.extent)?
            .state
            == rc_package::movement::PlacementState::Penetrating
        {
            continue;
        }
        if i + 1 == 737 {
            goal = Some(nodes.len());
        }
        nodes.push(navigation::Node {
            original: i as i32 + 1,
            name: pkg.object_path(i as i32 + 1)?,
            position,
            extra_cost: 0.,
        });
    }
    let goal = goal.ok_or("Original Bacta stand not usable")?;
    let mut edges = vec![];
    for start in 0..nodes.len() {
        for end in start + 1..nodes.len() {
            let from = nodes[start].position;
            let to = nodes[end].position;
            let distance = dist(from, to);
            if distance > 1200. {
                continue;
            }
            let sweep = game
                .world
                .sweep_motion(from, to, game.profile.physics.extent)?;
            if !sweep.complete {
                return Err("Incomplete route sweep".into());
            }
            if !sweep.contacts.is_empty() {
                if distance < 700. {
                    for (s, e) in [(start, end), (end, start)] {
                        if walking(&game, nodes[s].position, nodes[e].position)? {
                            edges.push(navigation::Edge {
                                original: 0,
                                start: s,
                                end: e,
                                distance,
                            });
                        }
                    }
                }
                continue;
            }
            let count = (distance / 32.).ceil().max(1.) as usize;
            let mut valid = true;
            for i in 0..=count {
                let p =
                    std::array::from_fn(|n| from[n] + (to[n] - from[n]) * i as f64 / count as f64);
                if game
                    .world
                    .floor_contact(
                        p,
                        game.profile.physics.extent,
                        game.profile.physics.step_height + 4.,
                        0.7,
                    )?
                    .is_none()
                {
                    valid = false;
                    break;
                }
            }
            for (start, end) in [(start, end), (end, start)] {
                if valid
                    || (distance < 700.
                        && walking(&game, nodes[start].position, nodes[end].position)?)
                {
                    edges.push(navigation::Edge {
                        original: 0,
                        start,
                        end,
                        distance,
                    });
                }
            }
        }
    }
    println!(
        "Route graph: {} original anchors, {} directed static-body links, BactaRate {} Health {}",
        nodes.len(),
        edges.len(),
        rate,
        max_health
    );
    let graph = navigation::Assets {
        provenance: serde_json::Value::Null,
        nodes,
        edges,
        lunge_distance: 1.,
        jump_distance: 1.,
        offensive_jump_probability: 0.,
    };
    fs::write(
        out.join("route-graph.json"),
        serde_json::to_vec_pretty(&graph)?,
    )?;
    let path = graph
        .shortest(&[(0, 0.)], &[(goal, 0.)])
        .ok_or("No supported walking route from landing to original Bacta stand")?;
    let selected: Vec<_> = path.iter().map(|&i| &graph.nodes[i]).collect();
    let route: Vec<_> = selected.iter().skip(1).map(|n| n.position).collect();
    let voice = fs::read(game_data.join("Sounds/GEO_Voice.uax"))?;
    let sounds = ["GEO_01.CA1G1749", "GEO_01.CA1G1747", "GEO_01.CA1G1791"]
        .into_iter()
        .map(|n| advisor(&voice, n))
        .collect::<Result<Vec<_>, _>>()?;
    let assets = mission::Assets {
        enemy_fire_sound: None,
        weapon_fire_sound: None,
        grenades: None,
        crash: None,
        sounds,
        provenance: serde_json::json!({"map":"geo_01a","objective_export":16,"objective_index":0,"bacta_export":736,"stand_export":737,"start_export":10,"damage_action":2397,"damage":175.,"bacta_event":"BactaUsed","complete_action":2901,"script":739,"route_nodes":selected,"rate":catalog.resolve("MarkerProperties.BactaDispenserGEO","BactaRate",0)?,"scope":"Entry after cinematic and wound; original objective/station/stand and healing formula; floor-settled PlayerStart and static-body route guidance are port adapters. No original cinematic/intro-warrior/general VM or later objectives."}),
        objective: "Use bacta to heal".into(),
        route,
        station,
        use_radius: 120.,
        max_health,
        start_health: (max_health + number("CTCharacters.PlayerCommando", "Shields")? - 175.)
            .clamp(1., max_health),
        bacta_rate: rate,
    };
    let mut resolver = rc_inspect::assets::Assets::new(&[
        &game_data.join("Animations"),
        &game_data.join("Textures"),
        &game_data.join("System"),
    ])?;
    let device = rc_inspect::visual_assets::load(
        game_data,
        "bactadispensers.ukx",
        "BactaDispenserGEO",
        &[],
        &mut resolver,
    )?
    .prepare_reference()?;
    let mut reference = device.channel(0, 0.)?;
    reference.words[14] = 0;
    let mut device_scene = device.scene(&reference)?;
    let actor_props = properties::read(&pkg, &map, &pkg.exports[735])?;
    let resolve = |name| {
        catalog.instance(
            "MarkerProperties.BactaDispenserGEO",
            &actor_props,
            name,
            0,
            "geo_01a",
        )
    };
    let rotation = match resolve("Rotation")?.value {
        Value::Rotator(v) => v,
        _ => return Err("Device rotation".into()),
    };
    let scalar = match resolve("DrawScale")?.value {
        Value::Float(v) => v,
        _ => return Err("Device scale".into()),
    };
    let scale = match resolve("DrawScale3D")?.value {
        Value::Vector(v) => v.map(|v| v * scalar),
        _ => return Err("Device scale3D".into()),
    };
    let pivot = match resolve("PrePivot")?.value {
        Value::Vector(v) => v,
        _ => return Err("Device pivot".into()),
    };
    for t in &mut device_scene.triangles {
        t.points = t
            .points
            .map(|p| rc_package::mesh::transform_point(p, [0.; 3], rotation, scale, pivot));
    }
    let min_z = device_scene
        .triangles
        .iter()
        .flat_map(|t| t.points.iter().map(|p| p[2]))
        .fold(f32::INFINITY, f32::min);
    let height = number("MarkerProperties.BactaDispenserGEO", "CollisionHeight")?;
    let location = station.map(|v| v as f32);
    for t in &mut device_scene.triangles {
        for p in &mut t.points {
            p[0] += location[0];
            p[1] += location[1];
            p[2] += location[2] - height - min_z;
        }
    }
    let texture_base = game.scene.textures.len();
    game.scene.triangles.extend(device_scene.triangles);
    game.scene
        .appearance
        .extend(device_scene.appearance.into_iter().map(|face| {
            face.map(|mut f| {
                f.texture += texture_base;
                f
            })
        }));
    game.scene.textures.extend(device_scene.textures);
    let mut manifest = bundle::Manifest::capture(
        &game,
        serde_json::json!({"source":"Original geo_01a base plus resolved BactaDispenserGEO reference mesh; bottom alignment and cylinder AABB are port adapters"}),
    )?;
    let radius = number("MarkerProperties.BactaDispenserGEO", "CollisionRadius")? as f64;
    manifest.colliders.push(bundle::Collider {
        name: "BactaDispenserLZ.box-adapter".into(),
        hulls: vec![bundle::HullData {
            offset: 0,
            planes: vec![],
            bounds: [
                std::array::from_fn(|i| station[i] - if i == 2 { height as f64 } else { radius }),
                std::array::from_fn(|i| station[i] + if i == 2 { height as f64 } else { radius }),
            ],
        }],
    });
    let n = u32::from_le_bytes(base[8..12].try_into().unwrap()) as usize;
    let visuals: rc_game::visual::Assets = serde_json::from_slice(&base[12..12 + n])?;
    let rebuilt = bundle::encode(&manifest, &game.scene)?;
    let base = bundle::encode_animated(&rebuilt, &visuals)?;
    let metadata = serde_json::to_vec(&assets)?;
    let mut data = b"RCPLAY04".to_vec();
    data.extend_from_slice(&(metadata.len() as u32).to_le_bytes());
    data.extend(metadata);
    data.extend(base);
    let game = bundle::decode(&data)?;
    fs::write(out.join("geo_01a-landing.rcplay"), data)?;
    fs::write(
        out.join("mission-receipt.json"),
        serde_json::to_vec_pretty(&assets)?,
    )?;
    println!(
        "Built original Bacta objective with {} guidance points: {}",
        assets.route.len(),
        game.mission_status()
    );
    Ok(())
}
