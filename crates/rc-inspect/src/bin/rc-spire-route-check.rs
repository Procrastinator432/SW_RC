//! Offline path feasibility after the accepted normal demolition inputs. Released gates
//! are diagnostic assumptions here; the separate live replay must trigger them normally.
use rc_game::{navigation, Input, Point, STEP};
use rc_package::{
    properties::{self, Value},
    read_package,
};
use std::{env, fs, path::Path};
fn distance(a: Point, b: Point) -> f64 {
    (0..3).map(|i| (a[i] - b[i]).powi(2)).sum::<f64>().sqrt()
}
fn walk(g: &rc_game::Game, from: Point, to: Point) -> Result<bool, String> {
    let mut pawn = rc_package::controller::ControlledBody {
        body: rc_package::physics::BodyState {
            position: from,
            velocity: [0.; 3],
            grounded: true,
        },
        jump_held: false,
    };
    let mut best = distance(from, to);
    let mut stalled = 0;
    for _ in 0..600 {
        let p = pawn.body.position;
        if distance(p, to) < 30. && pawn.body.grounded {
            return Ok(true);
        }
        let yaw = ((to[1] - p[1]).atan2(to[0] - p[0]) * 65536. / std::f64::consts::TAU) as i32;
        pawn = g
            .volumes
            .pawn_diagnostic_tick(
                &g.world,
                &pawn,
                rc_package::controller::ViewInput {
                    forward: 1.,
                    strafe: 0.,
                    yaw,
                    walking: false,
                    jump: false,
                },
                STEP,
                &g.profile,
                rc_package::pawn_motion::PawnRuntimeOptions {
                    maximum_desired_speed: g.profile.controller.speed,
                    weapon_modifier: 1.,
                    crouch_ratio: 1.,
                    wounded_ratio: 1.,
                    crouched: false,
                    wounded: false,
                    wants_to_crouch: false,
                    current_jump_z: g.profile.controller.jump_speed,
                },
            )?
            .state;
        let d = distance(pawn.body.position, to);
        if d < best - 1. {
            best = d;
            stalled = 0;
        } else {
            stalled += 1;
        }
        if stalled > 70 {
            return Ok(false);
        }
    }
    Ok(false)
}
fn main() -> Result<(), Box<dyn std::error::Error>> {
    let args: Vec<_> = env::args_os().collect();
    if args.len() != 5 {
        return Err(
            "usage: rc-spire-route-check GAME_DATA SPIRE_SCENE DEMOLITION_INPUT OUTPUT".into(),
        );
    }
    let root = Path::new(&args[1]);
    let out = Path::new(&args[4]);
    fs::create_dir_all(out)?;
    let mut g = rc_game::bundle::decode(&fs::read(&args[2])?)?;
    let inputs = fs::read(&args[3])?;
    for row in inputs[12..].chunks_exact(20) {
        let f = |i| f32::from_le_bytes(row[i..i + 4].try_into().unwrap()) as f64;
        let b = u32::from_le_bytes(row[16..20].try_into().unwrap());
        g.advance(
            STEP,
            Input {
                forward: f(0),
                strafe: f(4),
                look: [f(8), f(12)],
                jump: b & 1 != 0,
                fire: b & 2 != 0,
                reload: b & 4 != 0,
                walking: b & 8 != 0,
                use_action: b & 16 != 0,
                throw_grenade: b & 32 != 0,
                crouch: b & 64 != 0,
            },
        )?;
    }
    assert_eq!(g.state.phase, rc_game::Phase::Playing);
    assert!(
        g.state
            .demolition
            .as_ref()
            .unwrap()
            .flow
            .objective_completed
    );
    let start = g.state.pawn.body.position;
    for id in [860, 861, 862, 34, 35] {
        g.disable_dynamic(id)?;
    }
    fs::write(
        out.join("door-collision.json"),
        serde_json::to_vec_pretty(&g.world.sweep_motion(
            [3200., -15565., -2005.],
            [3200., -16152., -2005.],
            g.profile.physics.extent,
        )?)?,
    )?;
    let bytes = fs::read(root.join("Maps/geo_01a.ctm"))?;
    let p = read_package(&bytes)?;
    let cat = rc_inspect::defaults::load(root)?;
    let mut nodes = vec![navigation::Node {
        original: -1,
        name: "AcceptedBunkerPassage".into(),
        position: start,
        extra_cost: 0.,
    }];
    let extent = g.profile.physics.extent;
    let mut targets = vec![];
    for (i, e) in p.exports.iter().enumerate() {
        let id = i + 1;
        if p.object_path(e.class)? != "Engine.PathNode" && ![39, 24, 9, 780].contains(&id) {
            continue;
        }
        let props = properties::read(&p, &bytes, e)?;
        let Value::Vector(pos) = cat
            .instance(&p.object_path(e.class)?, &props, "Location", 0, "geo_01a")?
            .value
        else {
            continue;
        };
        let mut pos = pos.map(f64::from);
        if id == 24 {
            pos[1] += 250.;
        }
        if !(2000. ..=6500.).contains(&pos[0]) || !(-17500. ..=-9500.).contains(&pos[1]) {
            continue;
        }
        let mut choices = vec![];
        for lift in [0., 32., 64., 120., 360., 640.] {
            let from = [pos[0], pos[1], pos[2] + lift];
            if let Ok(Some(hit)) = g.world.floor_contact(from, extent, 1280., 0.7) {
                let at = [from[0], from[1], from[2] - 1280. * hit.hit.fraction + 0.5];
                if g.world.body_placement(at, extent)?.state
                    != rc_package::movement::PlacementState::Penetrating
                {
                    choices.push(at);
                }
            }
        }
        let Some(at) = choices
            .into_iter()
            .min_by(|a, b| distance(*a, pos).total_cmp(&distance(*b, pos)))
        else {
            continue;
        };
        if [39, 24, 9].contains(&id) {
            targets.push((id, nodes.len()));
        }
        nodes.push(navigation::Node {
            original: id as i32,
            name: p.object_path(id as i32)?,
            position: at,
            extra_cost: 0.,
        });
    }
    println!("Checking {} spire anchors", nodes.len());
    let mut edges = vec![];
    let mut rejected = vec![];
    for i in 0..nodes.len() {
        for j in i + 1..nodes.len() {
            let d = distance(nodes[i].position, nodes[j].position);
            if d > 900. {
                continue;
            }
            let hit = g
                .world
                .sweep_motion(nodes[i].position, nodes[j].position, extent)?;
            let mut clear = hit.contacts.is_empty();
            if clear {
                for step in 0..=(d / 48.).ceil() as usize {
                    let mix = (step as f64 * 48. / d).min(1.);
                    let at = std::array::from_fn(|axis| {
                        nodes[i].position[axis]
                            + (nodes[j].position[axis] - nodes[i].position[axis]) * mix
                    });
                    if g.world
                        .floor_contact(at, extent, g.profile.physics.step_height + 4., 0.7)?
                        .is_none()
                    {
                        clear = false;
                        break;
                    }
                }
            }
            for (s, e) in [(i, j), (j, i)] {
                let pass = clear
                    || d < 700. && walk(&g, nodes[s].position, nodes[e].position).unwrap_or(false);
                if pass {
                    edges.push(navigation::Edge {
                        original: 0,
                        start: s,
                        end: e,
                        distance: d,
                    });
                } else {
                    rejected.push((nodes[s].original, nodes[e].original));
                }
            }
        }
    }
    let graph = navigation::Assets {
        provenance: serde_json::json!({"scope":"Offline original anchors, physical standing links, released final gates assumed"}),
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
    fs::write(
        out.join("rejected-links.json"),
        serde_json::to_vec(&rejected)?,
    )?;
    let mut previous = 0;
    let mut route = vec![];
    let mut links = vec![];
    for id in [39, 24, 9] {
        let target = targets
            .iter()
            .find(|(v, _)| *v == id)
            .ok_or(format!("Missing supported trigger{id}"))?
            .1;
        let path = graph
            .shortest(&[(previous, 0.)], &[(target, 0.)])
            .ok_or(format!(
                "No walking route {}→{id}",
                graph.nodes[previous].original
            ))?;
        for pair in path.windows(2) {
            if !walk(
                &g,
                graph.nodes[pair[0]].position,
                graph.nodes[pair[1]].position,
            )? {
                return Err(format!(
                    "Final route blocked {}→{}",
                    graph.nodes[pair[0]].original, graph.nodes[pair[1]].original
                )
                .into());
            }
            links.push((graph.nodes[pair[0]].original, graph.nodes[pair[1]].original));
        }
        route.extend(path.into_iter().skip(1).map(|i| graph.nodes[i].position));
        previous = target;
    }
    fs::write(out.join("route.json"), serde_json::to_vec_pretty(&route)?)?;
    fs::write(
        out.join("route-validation.json"),
        serde_json::to_vec_pretty(
            &serde_json::json!({"passed":true,"anchors":graph.nodes.len(),"links":links,"route":route,"scope":"Offline gate-open feasibility, not live objective6 acceptance"}),
        )?,
    )?;
    println!(
        "Standing original spire path found: {} waypoints",
        route.len()
    );
    Ok(())
}
