//! Input-driven contiguous mission check; combat behavior remains a port adapter.
use rc_game::{bundle, Input, Phase, Point, STEP};
use std::{env, fs, path::Path};
fn dist(a: Point, b: Point) -> f64 {
    (0..3).map(|i| (a[i] - b[i]).powi(2)).sum::<f64>().sqrt()
}
fn image(g: &rc_game::Game, out: &Path, name: &str) -> Result<(), Box<dyn std::error::Error>> {
    let pixels = g.render(640, 400)?;
    let mut bytes = Vec::with_capacity(pixels.len() * 4);
    for pixel in pixels {
        bytes.extend_from_slice(&pixel.to_le_bytes());
    }
    fs::write(out.join(format!("{name}.argb")), bytes)?;
    Ok(())
}
fn steer(g: &rc_game::Game, target: Point, forward: f64, fire: bool) -> Input {
    let p = g.state.pawn.body.position;
    let wanted = (target[1] - p[1]).atan2(target[0] - p[0]);
    Input {
        forward,
        fire,
        look: [
            (g.state.yaw - wanted + std::f64::consts::PI).rem_euclid(std::f64::consts::TAU)
                - std::f64::consts::PI,
            g.state.pitch,
        ],
        ..Input::default()
    }
}
fn walking(
    g: &mut rc_game::Game,
    route: &[Point],
    trace: &mut Vec<serde_json::Value>,
) -> Result<(), String> {
    for (i, target) in route.iter().enumerate() {
        let mut reached = false;
        let mut stalled = 0;
        let mut best = f64::INFINITY;
        for _ in 0..1800 {
            let d = dist(g.state.pawn.body.position, *target);
            if d < 25. {
                reached = true;
                break;
            }
            let input = steer(g, *target, (d / 90.).min(1.), false);
            g.advance(STEP, input)?;
            if d < best - 1. {
                best = d;
                stalled = 0;
            } else {
                stalled += 1;
            }
            if stalled > 120 {
                break;
            }
            if g.state.phase != Phase::Playing {
                return Err(format!("Phase {:?} on route point{i}", g.state.phase));
            }
        }
        if !reached {
            return Err(format!(
                "Stuck {:?} -> route{i} {target:?}",
                g.state.pawn.body.position
            ));
        }
        trace.push(serde_json::json!({"waypoint":i,"position":g.state.pawn.body.position,"health":g.state.health,"enemies":g.state.enemies.len(),"dynamic":g.state.dynamic}));
    }
    Ok(())
}
fn main() -> Result<(), Box<dyn std::error::Error>> {
    let args: Vec<_> = env::args_os().collect();
    if args.len() != 3 {
        return Err("usage: rc-crash-scene-check SCENE OUTPUT".into());
    }
    let out = Path::new(&args[2]);
    fs::create_dir_all(out)?;
    let bytes = fs::read(&args[1])?;
    let mut g = bundle::decode(&bytes)?;
    let initial = serde_json::to_vec(&g.state)?;
    assert_eq!(g.state.dynamic.as_ref().unwrap(), &[false, false, false]);
    assert!(g.state.enemies.is_empty());
    let closed_frame = g.render_frame(320, 200)?.gpu_packet(&g.scene);
    let closed_triangles = g.render_frame(320, 200)?.triangles.len();
    let static_packet = rc_render::gpu::world(&g.scene)?;
    let route = g.mission.as_ref().unwrap().route.clone();
    let mut trace = vec![];
    walking(&mut g, &route, &mut trace)?;
    g.aim_at([-635.5656, -999.2758, -2286.]);
    image(&g, out, "barricade-closed")?;
    g.aim_at(g.mission.as_ref().unwrap().station);
    for _ in 0..240 {
        g.advance(
            STEP,
            Input {
                use_action: true,
                ..Input::default()
            },
        )?;
        if g.state.mission.as_ref().unwrap().completed {
            break;
        }
    }
    assert_eq!(g.state.health, 100);
    assert_eq!(g.state.phase, Phase::Playing);
    for _ in 0..400 {
        g.advance(STEP, Input::default())?;
    }
    let state = g.state.mission.as_ref().unwrap().crash.as_ref().unwrap();
    assert_eq!(state.flow.stage, rc_game::crash::Stage::Approach);
    assert_eq!(g.state.dynamic.as_ref().unwrap(), &[true, false, true]);
    assert_eq!(rc_render::gpu::world(&g.scene)?, static_packet);
    let open_triangles = g.render_frame(320, 200)?.triangles.len();
    assert!(open_triangles < closed_triangles);
    g.aim_at([-635.5656, -999.2758, -2286.]);
    image(&g, out, "barricade-open")?;
    let route = g
        .mission
        .as_ref()
        .unwrap()
        .crash
        .as_ref()
        .unwrap()
        .route
        .clone();
    walking(&mut g, &route, &mut trace)?;
    assert_eq!(g.state.enemies.len(), 5);
    assert!(
        g.state
            .mission
            .as_ref()
            .unwrap()
            .crash
            .as_ref()
            .unwrap()
            .flow
            .vulnerable
    );
    let target = g.state.enemies[0].position;
    g.aim_at(target);
    image(&g, out, "crash-site")?;
    let mut combat = vec![];
    for _ in 0..3600 {
        if g.state.phase != Phase::Playing {
            break;
        }
        let target = g
            .state
            .enemies
            .iter()
            .filter(|e| e.health > 0)
            .filter(|e| {
                g.wall_fraction(g.eye(), [e.position[0], e.position[1], e.position[2] + 15.])
                    .is_ok_and(|f| f >= 1.)
            })
            .min_by(|a, b| {
                dist(a.position, g.state.pawn.body.position)
                    .total_cmp(&dist(b.position, g.state.pawn.body.position))
            })
            .map(|e| e.position);
        if let Some(mut target) = target {
            target[2] += 15.;
            g.aim_at(target);
        }
        g.advance(
            STEP,
            Input {
                fire: target.is_some(),
                reload: g.state.ammo == 0,
                ..Input::default()
            },
        )?;
        if g.state.statistics.ticks % 30 == 0 {
            combat.push(serde_json::json!({"tick":g.state.statistics.ticks,"health":g.state.health,"enemy_health":g.state.enemies.iter().map(|e|e.health).collect::<Vec<_>>(),"counter":g.state.mission.as_ref().unwrap().crash.as_ref().unwrap().flow.counter_remaining}));
        }
    }
    fs::write(
        out.join("combat-trace.json"),
        serde_json::to_vec_pretty(&combat)?,
    )?;
    if g.state.phase != Phase::Won {
        return Err(format!(
            "Combat did not finish: {:?}, health{}, enemies{:?}",
            g.state.phase,
            g.state.health,
            g.state
                .enemies
                .iter()
                .map(|e| (e.position, e.health))
                .collect::<Vec<_>>()
        )
        .into());
    }
    assert_eq!(g.state.statistics.kills, 5);
    assert_eq!(g.state.dynamic.as_ref().unwrap(), &[true, true, true]);
    assert_eq!(
        g.state
            .mission
            .as_ref()
            .unwrap()
            .crash
            .as_ref()
            .unwrap()
            .flow
            .counter_remaining,
        0
    );
    let won_tick = g.state.statistics.ticks;
    g.restart();
    assert_eq!(serde_json::to_vec(&g.state)?, initial);
    assert_eq!(g.render_frame(320, 200)?.gpu_packet(&g.scene), closed_frame);
    assert_eq!(rc_render::gpu::world(&g.scene)?, static_packet);
    let before = serde_json::to_vec(&g.state)?;
    assert!(g.disable_dynamic(u32::MAX).is_err());
    assert_eq!(serde_json::to_vec(&g.state)?, before);
    fs::write(
        out.join("walking-trace.json"),
        serde_json::to_vec_pretty(&trace)?,
    )?;
    fs::write(
        out.join("validation.json"),
        serde_json::to_vec_pretty(
            &serde_json::json!({"passed":true,"input_driven_both_routes":true,"five_original_factory_instances":true,"kills":5,"won_tick":won_tick,
        "bacta_continues_without_victory":true,"original_counter_completes":true,"restart_state_and_frame_exact":true,"static_gpu_packet_immutable":true,"transient_barrier_removed":true,"unknown_actor_unchanged":true,
        "scope":"Contiguous original objective0/1 with selected script/actor data. Provisional hitscan, enemy approach/shooting and cylinder combat; no original BattleDroid AI/DC15s parity or Android acceptance."}),
        )?,
    )?;
    println!("Contiguous Bacta/crash section: physical routes, five kills, original counter, dynamic collision/frame and restart passed at tick{won_tick}");
    Ok(())
}
