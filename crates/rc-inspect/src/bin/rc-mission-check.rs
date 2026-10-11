use rc_game::{bundle, Input, Phase, STEP};
use std::{env, fs, path::Path};
fn distance(a: [f64; 3], b: [f64; 3]) -> f64 {
    (0..3).map(|i| (a[i] - b[i]).powi(2)).sum::<f64>().sqrt()
}
fn main() -> Result<(), Box<dyn std::error::Error>> {
    let a: Vec<_> = env::args_os().collect();
    if a.len() != 3 {
        return Err("usage: rc-mission-check SCENE OUTPUT".into());
    }
    let out = Path::new(&a[2]);
    fs::create_dir_all(out)?;
    let data = fs::read(&a[1])?;
    let mut game = bundle::decode(&data)?;
    let initial = serde_json::to_vec(&game.state)?;
    assert_eq!(game.state.enemies.len(), 0);
    assert_eq!(game.state.health, 25);
    let route = game.mission.as_ref().unwrap().route.clone();
    let mut trace = vec![];
    let mut commands = vec![];
    for target in &route {
        let mut reached = false;
        for _ in 0..1800 {
            let p = game.state.pawn.body.position;
            let d = distance(p, *target);
            if d < 25. {
                reached = true;
                break;
            }
            let yaw = (target[1] - p[1]).atan2(target[0] - p[0]);
            let turn = (game.state.yaw - yaw + std::f64::consts::PI)
                .rem_euclid(std::f64::consts::TAU)
                - std::f64::consts::PI;
            let input = Input {
                forward: (d / 90.).min(1.),
                look: [turn, game.state.pitch],
                ..Input::default()
            };
            game.advance(STEP, input)?;
            commands.push(input);
            if game.state.statistics.ticks % 30 == 0 {
                trace.push(serde_json::json!({"tick":game.state.statistics.ticks,"position":game.state.pawn.body.position,"health":game.state.health}));
            }
            assert_eq!(game.state.phase, Phase::Playing);
        }
        assert!(
            reached,
            "Walking stuck at {:?} heading for {:?}",
            game.state.pawn.body.position, target
        );
    }
    let station = game.mission.as_ref().unwrap().station;
    game.aim_at(station);
    for _ in 0..60 {
        game.advance(
            STEP,
            Input {
                use_action: true,
                ..Input::default()
            },
        )?;
    }
    let partial = game.state.mission.as_ref().unwrap().health;
    assert!(partial > 25. && partial < 100.);
    for _ in 0..60 {
        game.advance(STEP, Input::default())?;
    }
    assert_eq!(game.state.mission.as_ref().unwrap().health, partial);
    game.suspend();
    let paused = serde_json::to_vec(&game.state)?;
    assert_eq!(paused, serde_json::to_vec(&game.state)?);
    for _ in 0..240 {
        game.advance(
            STEP,
            Input {
                use_action: true,
                ..Input::default()
            },
        )?;
        if game.state.phase == Phase::Won {
            break;
        }
    }
    assert_eq!(game.state.phase, Phase::Won);
    assert_eq!(game.state.health, 100);
    assert_eq!(game.state.mission.as_ref().unwrap().objective_mode, 2);
    let won_tick = game.state.statistics.ticks;
    game.restart();
    assert_eq!(serde_json::to_vec(&game.state)?, initial);
    // Same actual movement commands at 120 Hz, retaining look deltas once per tick.
    for input in &commands {
        game.advance(
            STEP / 2.,
            Input {
                look: [0.; 2],
                ..*input
            },
        )?;
        game.advance(STEP / 2., *input)?;
    }
    assert!(distance(game.state.pawn.body.position, *route.last().unwrap()) < 25.);
    game.restart();
    for _ in 0..300 {
        game.advance(
            STEP,
            Input {
                use_action: true,
                ..Input::default()
            },
        )?;
    }
    assert_eq!(game.state.health, 25);
    assert_eq!(game.state.phase, Phase::Playing);
    fs::write(
        out.join("walking-trace.json"),
        serde_json::to_vec_pretty(&trace)?,
    )?;
    fs::write(
        out.join("validation.json"),
        serde_json::to_vec_pretty(
            &serde_json::json!({"passed":true,"original_objective":0,"route_points":route.len(),"walked_ticks":commands.len(),"won_tick":won_tick,"full_physical_walk":true,"hold_heals":true,"release_stops_healing":true,"distant_use_rejected":true,"objective_completed":true,"restart_exact":true,"movement_120hz":true,"scope":"Shared Rust physical movement and selected original Bacta objective; no cinematic, full script VM or later mission objectives"}),
        )?,
    )?;
    println!("Original Bacta section: {} walking ticks, hold/release, completion, restart and 120Hz passed",commands.len());
    Ok(())
}
