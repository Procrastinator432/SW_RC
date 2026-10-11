//! Offline physical feasibility of the original path after the crash-site gate.
//! This does not attach objective2 to the playable mission.
use rc_game::{bundle, navigation, Point};
use rc_package::{
    properties::{self, Value},
    read_package,
};
use std::{env, fs, path::Path};
fn dist(a: Point, b: Point) -> f64 {
    (0..3).map(|i| (a[i] - b[i]).powi(2)).sum::<f64>().sqrt()
}
fn walk(game: &rc_game::Game, from: Point, to: Point) -> Result<bool, String> {
    let mut pawn = rc_package::controller::ControlledBody {
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
        let p = pawn.body.position;
        if dist(p, to) < 35. && pawn.body.grounded {
            return Ok(true);
        }
        let yaw = ((to[1] - p[1]).atan2(to[0] - p[0]) * 65536. / std::f64::consts::TAU) as i32;
        pawn = game
            .volumes
            .pawn_diagnostic_tick(
                &game.world,
                &pawn,
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
            )?
            .state;
        let d = dist(pawn.body.position, to);
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
    let args: Vec<_> = env::args_os().collect();
    if args.len() != 4 && args.len() != 5 {
        return Err(
            "usage: rc-grenade-route-check GAME_DATA CRASH_SCENE OUTPUT_DIR [--without-karma-wall|--released-barriers]"
                .into(),
        );
    }
    let root = Path::new(&args[1]);
    let out = Path::new(&args[3]);
    fs::create_dir_all(out)?;
    let mut game = bundle::decode(&fs::read(&args[2])?)?;
    let start = *game
        .mission
        .as_ref()
        .ok_or("No mission")?
        .crash
        .as_ref()
        .ok_or("No crash route")?
        .route
        .last()
        .ok_or("No last crash anchor")?;
    for id in [1122, 915, 617] {
        game.disable_dynamic(id)?;
    }
    if game.mission.as_ref().is_some_and(|m| m.grenades.is_some()) {
        for id in [909, 910, 912, 913, 964] {
            game.disable_dynamic(id)?;
        }
    }
    let released_barriers = args.get(4).is_some_and(|s| s == "--released-barriers");
    let diagnostic_without_wall =
        released_barriers || args.get(4).is_some_and(|s| s == "--without-karma-wall");
    if args.len() == 5 && !diagnostic_without_wall {
        return Err("Unknown route diagnostic flag".into());
    }
    if released_barriers {
        for actor in &mut game.world.actors {
            if actor.name == "Prop3" {
                actor.shape = rc_package::world_collision::BodyShape::Clear;
            }
        }
    }
    if diagnostic_without_wall {
        for actor in &mut game.world.actors {
            if ["KarmaProp7", "KarmaProp4", "KarmaProp0", "KarmaProp6"]
                .contains(&actor.name.as_str())
            {
                actor.shape = rc_package::world_collision::BodyShape::Clear;
            }
        }
    }
    let map = fs::read(root.join("Maps/geo_01a.ctm"))?;
    let pkg = read_package(&map)?;
    let catalog = rc_inspect::defaults::load(root)?;
    let value = |id: u32, name: &str| -> Result<Value, String> {
        let e = &pkg.exports[id as usize - 1];
        Ok(catalog
            .instance(
                &pkg.object_path(e.class)?,
                &properties::read(&pkg, &map, e)?,
                name,
                0,
                "geo_01a.ctm",
            )?
            .value)
    };
    let mut probes = vec![];
    for id in [293, 764, 238, 445, 746, 610] {
        let pos = match value(id, "Location")? {
            Value::Vector(v) => v.map(f64::from),
            _ => return Err("No anchor location".into()),
        };
        for lift in [120., 360., 640., 1200.] {
            let start = [pos[0], pos[1], pos[2] + lift];
            let hit = game
                .world
                .floor_contact(start, game.profile.physics.extent, 2000., 0.7);
            let placement = if let Ok(Some(h)) = &hit {
                Some(game.world.body_placement(
                    [start[0], start[1], start[2] - 2000. * h.hit.fraction + 0.5],
                    game.profile.physics.extent,
                )?)
            } else {
                None
            };
            probes.push(
                serde_json::json!({"original":id,"lift":lift,"floor":hit,"placement":placement}),
            );
        }
    }
    fs::write(
        out.join("anchor-probes.json"),
        serde_json::to_vec_pretty(&probes)?,
    )?;
    let ground = |position: Point, extent: Point| -> Result<Point, String> {
        let mut choices = vec![];
        for lift in [0., 32., 64., 120., 360., 640.] {
            let start = [position[0], position[1], position[2] + lift];
            let floor = match game.world.floor_contact(start, extent, 1280., 0.7) {
                Ok(v) => v,
                Err(e) if e == "floor probe starts penetrating geometry" => continue,
                Err(e) => return Err(e),
            };
            if let Some(hit) = floor {
                let point = [
                    start[0],
                    start[1],
                    start[2] - 1280. * hit.hit.fraction + 0.5,
                ];
                if game.world.body_placement(point, extent)?.state
                    != rc_package::movement::PlacementState::Penetrating
                {
                    choices.push(point);
                }
            }
        }
        choices
            .into_iter()
            .min_by(|a, b| dist(*a, position).total_cmp(&dist(*b, position)))
            .ok_or("No nonpenetrating supported original anchor floor".into())
    };
    let mut nodes = vec![navigation::Node {
        original: 294,
        name: "CrashExitStand".into(),
        position: start,
        extra_cost: 0.,
    }];
    let mut targets = vec![];
    for (i, e) in pkg.exports.iter().enumerate() {
        let id = i as u32 + 1;
        if pkg.object_path(e.class)? != "Engine.PathNode" && ![293, 238, 746, 610].contains(&id) {
            continue;
        }
        let pos = match value(id, "Location")? {
            Value::Vector(v) => v.map(f64::from),
            _ => continue,
        };
        if !(-7000. ..=8000.).contains(&pos[0]) || !(-16000. ..=-3500.).contains(&pos[1]) {
            continue;
        }
        let Ok(pos) = ground(pos, game.profile.physics.extent) else {
            continue;
        };
        if game
            .world
            .body_placement(pos, game.profile.physics.extent)?
            .state
            == rc_package::movement::PlacementState::Penetrating
        {
            continue;
        }
        if [293, 764, 238, 445, 746, 610].contains(&id) {
            targets.push((id, nodes.len()));
        }
        nodes.push(navigation::Node {
            original: id as i32,
            name: pkg.object_path(id as i32)?,
            position: pos,
            extra_cost: 0.,
        });
    }
    let mut edges = vec![];
    let mut rejected = vec![];
    println!("Checking {} original route anchors", nodes.len());
    for i in 0..nodes.len() {
        for j in i + 1..nodes.len() {
            let d = dist(nodes[i].position, nodes[j].position);
            if d > 1000. {
                continue;
            }
            let hit = game.world.sweep_motion(
                nodes[i].position,
                nodes[j].position,
                game.profile.physics.extent,
            )?;
            if !hit.complete {
                return Err("Incomplete route collision".into());
            }
            if !hit.contacts.is_empty() && d > 700. {
                continue;
            }
            let mut supported = hit.contacts.is_empty();
            if supported {
                let steps = (d / 48.).ceil().max(1.) as usize;
                for step in 0..=steps {
                    let p = std::array::from_fn(|axis| {
                        nodes[i].position[axis]
                            + (nodes[j].position[axis] - nodes[i].position[axis]) * step as f64
                                / steps as f64
                    });
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
                        supported = false;
                        break;
                    }
                }
            }
            for (s, e) in [(i, j), (j, i)] {
                let verified = if supported {
                    true
                } else if d < 700. {
                    match walk(&game, nodes[s].position, nodes[e].position) {
                        Ok(v) => v,
                        Err(error) => {
                            rejected.push(serde_json::json!({"from":nodes[s].original,"to":nodes[e].original,"error":error}));
                            false
                        }
                    }
                } else {
                    false
                };
                if verified {
                    edges.push(navigation::Edge {
                        original: 0,
                        start: s,
                        end: e,
                        distance: d,
                    });
                }
            }
        }
    }
    fs::write(
        out.join("rejected-route-links.json"),
        serde_json::to_vec_pretty(&rejected)?,
    )?;
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

    let mut route = vec![];
    let mut last = 0;
    let mut segments = vec![];
    for id in [293, 764, 238, 445, 746, 610] {
        let end = targets
            .iter()
            .find(|(i, _)| *i == id)
            .ok_or(format!("Unsupported grenade anchor {id}"))?
            .1;
        let Some(path) = graph.shortest(&[(last, 0.)], &[(end, 0.)]) else {
            let mut seen = std::collections::BTreeSet::from([last]);
            let mut pending = vec![last];
            while let Some(n) = pending.pop() {
                for edge in &graph.edges {
                    if edge.start == n && seen.insert(edge.end) {
                        pending.push(edge.end);
                    }
                }
            }
            let mut pairs = vec![];
            for &s in &seen {
                for e in 0..graph.nodes.len() {
                    if !seen.contains(&e) {
                        pairs.push((dist(graph.nodes[s].position, graph.nodes[e].position), s, e));
                    }
                }
            }
            pairs.sort_by(|a, b| a.0.total_cmp(&b.0));
            let mut boundaries = vec![];
            for (distance, s, e) in pairs.into_iter().take(10) {
                boundaries.push(serde_json::json!({"distance":distance,"from":graph.nodes[s].original,"to":graph.nodes[e].original,
          "sweep":game.world.sweep_motion(graph.nodes[s].position,graph.nodes[e].position,game.profile.physics.extent)?}));
            }
            fs::write(
                out.join("blocked-route-links.json"),
                serde_json::to_vec_pretty(&boundaries)?,
            )?;
            fs::write(
                out.join("route-validation.json"),
                serde_json::to_vec_pretty(
                    &serde_json::json!({"passed":false,"reason":format!("No physical route to {id}"),"reachable_anchors":seen.len(),"anchors":graph.nodes.len(),"diagnostic_without_karma_wall":diagnostic_without_wall,"scope":"Offline feasibility diagnostic; not a playable route."}),
                )?,
            )?;
            return Err(format!("No physical route to {id}").into());
        };
        for edge in path.windows(2) {
            if !walk(
                &game,
                graph.nodes[edge[0]].position,
                graph.nodes[edge[1]].position,
            )? {
                return Err(format!("Selected route failed pawn movement: {:?}", edge).into());
            }
        }
        let points = path
            .iter()
            .skip(1)
            .map(|&i| graph.nodes[i].position)
            .collect::<Vec<_>>();
        segments.push(serde_json::json!({"target":id,"points":points}));
        route.extend(points);
        last = end;
    }
    fs::write(
        out.join("pickup-route.json"),
        serde_json::to_vec_pretty(
            &serde_json::json!({"start":start,"route":route,"segments":segments}),
        )?,
    )?;
    fs::write(
        out.join("route-validation.json"),
        serde_json::to_vec_pretty(
            &serde_json::json!({"passed":true,"anchors":graph.nodes.len(),"links":graph.edges.len(),"guide_points":route.len(),"original_targets":[293,764,238,445,746,610],"all_selected_links_walked":true,"diagnostic_without_karma_wall":diagnostic_without_wall,"scope":"Offline original-anchor path with pawn movement on the post-crash collision world. Three existing barriers disabled. When requested, four Karma wall colliders removed as an explicit diagnostic only. No mission progression, pickup contact, Karma wall animation, Android touch, or new playable scene acceptance."}),
        )?,
    )?;
    println!(
        "Grenade route: {} original anchors, {} physical links, {} guide points",
        graph.nodes.len(),
        graph.edges.len(),
        route.len()
    );
    Ok(())
}
