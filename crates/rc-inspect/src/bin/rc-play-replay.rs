//! Record/replay real frame inputs and collect opt-in, inclusive CPU timings.
use rc_game::{
    replay::{Action, Replay},
    Game, Input, Phase, STEP,
};
use rc_package::performance;
use std::{env, fs, path::Path, time::Instant};

fn record(bundle: &[u8]) -> Result<Replay, String> {
    let mut game = rc_game::bundle::decode(bundle)?;
    let mut replay = Replay::new(bundle);
    fn frames(
        r: &mut Replay,
        g: &mut Game,
        input: Input,
        count: usize,
        stage: &str,
    ) -> Result<(), String> {
        for _ in 0..count {
            r.record(
                g,
                Action::Frame {
                    elapsed: STEP,
                    input,
                },
                false,
                stage,
            )?;
        }
        Ok(())
    }
    fn look(r: &mut Replay, g: &mut Game, point: [f64; 3], stage: &str) -> Result<(), String> {
        let eye = g.eye();
        let dx = point[0] - eye[0];
        let dy = point[1] - eye[1];
        let yaw = dy.atan2(dx);
        let pitch = (point[2] - eye[2]).atan2(dx.hypot(dy));
        let delta = (g.state.yaw - yaw + std::f64::consts::PI).rem_euclid(std::f64::consts::TAU)
            - std::f64::consts::PI;
        r.record(
            g,
            Action::Frame {
                elapsed: 0.,
                input: Input {
                    look: [delta, g.state.pitch - pitch],
                    ..Input::default()
                },
            },
            false,
            stage,
        )
    }
    replay.record(
        &mut game,
        Action::Frame {
            elapsed: 0.,
            input: Input::default(),
        },
        true,
        "spawn",
    )?;
    frames(
        &mut replay,
        &mut game,
        Input {
            jump: true,
            ..Input::default()
        },
        1,
        "jump",
    )?;
    frames(&mut replay, &mut game, Input::default(), 120, "landing")?;
    if !game.state.pawn.body.grounded || game.state.statistics.jumps != 1 {
        return Err("Jump/landing failed".into());
    }
    frames(
        &mut replay,
        &mut game,
        Input {
            strafe: 0.2,
            ..Input::default()
        },
        12,
        "strafe",
    )?;
    replay.record(
        &mut game,
        Action::Frame {
            elapsed: STEP / 4.,
            input: Input {
                jump: true,
                ..Input::default()
            },
        },
        false,
        "queued_jump",
    )?;
    replay.record(&mut game, Action::Suspend, true, "pause")?;
    frames(&mut replay, &mut game, Input::default(), 1, "resume")?;
    if game.state.statistics.jumps != 1 {
        return Err("Paused input leaked".into());
    }
    replay.record(&mut game, Action::Restart, true, "restart")?;
    for index in 0..game.state.enemies.len() {
        let target = game.state.enemies[index].position;
        look(&mut replay, &mut game, target, "combat")?;
        frames(
            &mut replay,
            &mut game,
            Input {
                fire: true,
                ..Input::default()
            },
            11,
            "combat",
        )?;
        frames(&mut replay, &mut game, Input::default(), 10, "combat")?;
    }
    if game.state.statistics.kills != 3 {
        return Err("Combat failed".into());
    }
    frames(
        &mut replay,
        &mut game,
        Input {
            reload: true,
            ..Input::default()
        },
        1,
        "reload",
    )?;
    frames(&mut replay, &mut game, Input::default(), 75, "reload")?;
    if game.state.statistics.reloads != 1 || game.state.ammo != 20 {
        return Err("Reload failed".into());
    }
    let target = [game.exit[0], game.exit[1], game.eye()[2]];
    look(&mut replay, &mut game, target, "exit")?;
    for _ in 0..240 {
        frames(
            &mut replay,
            &mut game,
            Input {
                forward: 1.,
                ..Input::default()
            },
            1,
            "exit",
        )?;
        if game.state.phase == Phase::Won {
            break;
        }
    }
    if game.state.phase != Phase::Won {
        return Err("Victory failed".into());
    }
    replay.record(
        &mut game,
        Action::Frame {
            elapsed: STEP,
            input: Input::default(),
        },
        true,
        "victory",
    )?;
    replay.record(&mut game, Action::Restart, false, "restart")?;
    for _ in 0..1800 {
        frames(&mut replay, &mut game, Input::default(), 1, "enemy_damage")?;
        if game.state.phase == Phase::Lost {
            break;
        }
    }
    if game.state.phase != Phase::Lost {
        return Err("Defeat failed".into());
    }
    replay.record(
        &mut game,
        Action::Frame {
            elapsed: STEP,
            input: Input::default(),
        },
        true,
        "defeat",
    )?;
    replay.record(&mut game, Action::Restart, true, "restart")?;
    replay.record(
        &mut game,
        Action::Frame {
            elapsed: 0.25,
            input: Input::default(),
        },
        false,
        "time_limit",
    )?;
    if game.state.statistics.discarded_seconds <= 0. {
        return Err("Time limit failed".into());
    }
    replay.record(&mut game, Action::Restart, true, "final_reset")?;
    Ok(replay)
}
fn run(
    bundle: &[u8],
    replay: &Replay,
    profile: bool,
    sample_render: bool,
) -> Result<serde_json::Value, String> {
    replay.validate_bundle(bundle)?;
    let load = Instant::now();
    let mut game = rc_game::bundle::decode(bundle)?;
    let load_ns = load.elapsed().as_nanos() as u64;
    let mut samples = vec![];
    let mut totals = performance::Counters::default();
    let mut render_frames = 0;
    for (index, entry) in replay.entries.iter().enumerate() {
        if profile {
            performance::begin_detailed()?;
        }
        let start = Instant::now();
        let result = Replay::verify_entry(&mut game, entry, false);
        let simulation_wall_ns = start.elapsed().as_nanos() as u64;
        let draw = entry.pixels_640x400.is_some() || (sample_render && index % 15 == 0);
        let draw_start = Instant::now();
        let render_result = if draw {
            rc_game::replay::pixels(&game).map(Some)
        } else {
            Ok(None)
        };
        let render_wall_ns = if draw {
            draw_start.elapsed().as_nanos() as u64
        } else {
            0
        };
        let counters = if profile {
            performance::finish()?
        } else {
            performance::Counters::default()
        };
        result.map_err(|e| format!("Entry {index}: {e}"))?;
        let rendered = render_result?;
        if let Some(expected) = &entry.pixels_640x400 {
            if rendered.as_ref() != Some(expected) {
                return Err(format!("Entry {index}: pixels differ"));
            }
        }
        render_frames += u32::from(draw);
        for c in 0..performance::CATEGORIES {
            totals.ns[c] = totals.ns[c].saturating_add(counters.ns[c]);
            totals.calls[c] = totals.calls[c].saturating_add(counters.calls[c]);
        }
        samples.push(serde_json::json!({"entry":index,"stage":entry.stage,"ticks":entry.expected_ticks,
            "simulation_and_state_check_ns":simulation_wall_ns,"render_and_pixel_check_ns":render_wall_ns,
            "rendered":draw,"ns":counters.ns,"calls":counters.calls}));
    }
    Ok(
        serde_json::json!({"profile_enabled":profile,"load_ns":load_ns,"rendered_frames":render_frames,
        "totals_ns":totals.ns,"totals_calls":totals.calls,"samples":samples,
        "final_state":game.state,"exact_replay":true}),
    )
}
fn main() -> Result<(), Box<dyn std::error::Error>> {
    let args: Vec<_> = env::args_os().collect();
    if args.len() != 5 {
        return Err(
            "Usage: rc-play-replay record|verify <bundle> <replay.json> <report.json>".into(),
        );
    }
    let bundle = fs::read(&args[2])?;
    let trace_path = Path::new(&args[3]);
    let replay = if args[1] == "record" {
        let replay = record(&bundle)?;
        fs::write(trace_path, serde_json::to_vec_pretty(&replay)?)?;
        replay
    } else if args[1] == "verify" || args[1] == "check" {
        let bytes = fs::read(trace_path)?;
        if bytes.len() > 64 * 1024 * 1024 {
            return Err("Replay exceeds 64 MiB".into());
        }
        serde_json::from_slice::<Replay>(&bytes)?
    } else {
        return Err("Unknown replay mode".into());
    };
    if args[1] == "check" {
        let result = run(&bundle, &replay, false, false)?;
        fs::write(&args[4], serde_json::to_vec_pretty(&result)?)?;
        println!("{} commands checked exactly", replay.entries.len());
        return Ok(());
    }
    // Warm up assets/rendering before paired measured runs. No frame output influences simulation.
    run(&bundle, &replay, false, false)?;
    let mut pairs = vec![];
    for pair in 0..3 {
        let order = if pair % 2 == 0 {
            [false, true]
        } else {
            [true, false]
        };
        for enabled in order {
            pairs.push(run(&bundle, &replay, enabled, true)?);
        }
    }
    let report = serde_json::json!({"schema":1,"scope":"Offline input replay; sampled CPU frames, not interactive FPS or GPU presentation latency. Inclusive categories overlap.",
        "os":env::consts::OS,"arch":env::consts::ARCH,"debug_assertions":cfg!(debug_assertions),
        "resolution":[640,400],"entries":replay.entries.len(),"categories":performance::NAMES,"runs":pairs});
    fs::write(&args[4], serde_json::to_vec_pretty(&report)?)?;
    println!(
        "{} input commands replayed exactly in six paired runs; profile: {}",
        replay.entries.len(),
        Path::new(&args[4]).display()
    );
    Ok(())
}
