//! Normal input replay from landing through objective4; no gameplay state injection.
use rc_game::{Input, Phase, STEP};
use std::{env, fs, path::Path};
fn input(row: &[u8]) -> Input {
    let f = |i| f32::from_le_bytes(row[i..i + 4].try_into().unwrap()) as f64;
    let b = u32::from_le_bytes(row[16..20].try_into().unwrap());
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
    }
}
fn record(out: &mut Vec<u8>, i: Input) {
    for f in [i.forward, i.strafe, i.look[0], i.look[1]] {
        out.extend((f as f32).to_le_bytes());
    }
    let flags = u32::from(i.jump)
        | (u32::from(i.fire) * 2)
        | (u32::from(i.reload) * 4)
        | (u32::from(i.walking) * 8)
        | (u32::from(i.use_action) * 16)
        | (u32::from(i.throw_grenade) * 32)
        | (u32::from(i.crouch) * 64);
    out.extend(flags.to_le_bytes());
}
fn frame(g: &rc_game::Game, out: &Path, name: &str) -> Result<(), Box<dyn std::error::Error>> {
    let pixels = g.render(640, 400)?;
    let mut ppm = b"P6\n640 400\n255\n".to_vec();
    for p in pixels {
        ppm.extend([(p >> 16) as u8, (p >> 8) as u8, p as u8]);
    }
    fs::write(out.join(format!("{name}.ppm")), ppm)?;
    Ok(())
}
fn main() -> Result<(), Box<dyn std::error::Error>> {
    let args: Vec<_> = env::args_os().collect();
    if args.len() != 4 {
        return Err("usage: rc-rendezvous-scene-check SCENE PREVIOUS_INPUT OUTPUT".into());
    }
    let out = Path::new(&args[3]);
    fs::create_dir_all(out)?;
    let mut g = rc_game::bundle::decode(&fs::read(&args[1])?)?;
    let initial = serde_json::to_vec(&g.state)?;
    let world = rc_render::gpu::world(&g.scene)?;
    let prefix = fs::read(&args[2])?;
    if !prefix.starts_with(b"RCINPUT1") || (prefix.len() - 12) % 20 != 0 {
        return Err("Invalid previous input stream".into());
    }
    let mut replay = vec![];
    for row in prefix[12..].chunks_exact(20) {
        let i = input(row);
        g.advance(STEP, i)?;
        record(&mut replay, i);
    }
    assert_eq!(g.state.phase, Phase::Playing);
    assert!(g.state.thermal.as_ref().unwrap().gun.completed);
    assert!(g.state.rendezvous.as_ref().unwrap().radio_started);
    let gun_completed_tick = g.state.bunker.as_ref().unwrap().complete_tick.unwrap();
    assert!(!g.state.rendezvous.as_ref().unwrap().flow.squad_joined);
    frame(&g, out, "gun-complete-continuing")?;
    let route = g.rendezvous.as_ref().unwrap().assets.player_route.clone();
    let mut trace = vec![];
    let mut crouch_release_checked = false;
    for (index, target) in route.iter().enumerate() {
        let mut reached = false;
        let mut best = f64::INFINITY;
        let mut stalled = 0;
        for _ in 0..1800 {
            let p = g.state.pawn.body.position;
            let d = (0..3)
                .map(|i| (p[i] - target[i]).powi(2))
                .sum::<f64>()
                .sqrt();
            if d < 25. {
                reached = true;
                break;
            }
            let yaw = (target[1] - p[1]).atan2(target[0] - p[0]);
            let turn = (g.state.yaw - yaw + std::f64::consts::PI).rem_euclid(std::f64::consts::TAU)
                - std::f64::consts::PI;
            let i = Input {
                forward: ((d / 90.).min(1.) as f32) as f64,
                look: [(turn as f32) as f64, (g.state.pitch as f32) as f64],
                crouch: true,
                ..Input::default()
            };
            g.advance(STEP, i)?;
            record(&mut replay, i);
            if g.state.phase == Phase::Lost {
                return Err("Player lost on Scorch route".into());
            }
            if d < best - 1. {
                best = d;
                stalled = 0;
            } else {
                stalled += 1;
            }
            if stalled > 180 {
                fs::write(
                    out.join("failure.json"),
                    serde_json::to_vec_pretty(
                        &serde_json::json!({"index":index,"target":target,"state":g.state}),
                    )?,
                )?;
                return Err(format!(
                    "Scorch route stalls at {index}, {:?}",
                    g.state.pawn.body.position
                )
                .into());
            }
        }
        if !reached {
            return Err(format!("Scorch guide point {index} not reached").into());
        }
        if index == 2 {
            let foot = g.state.pawn.body.position[2] - 56.;
            let i = Input::default();
            g.advance(STEP, i)?;
            record(&mut replay, i);
            assert!(
                g.state.crouched,
                "Standing must remain blocked under original rubble"
            );
            assert!((g.state.pawn.body.position[2] - 56. - foot).abs() < 1.);
            crouch_release_checked = true;
            frame(&g, out, "crouch-passage")?;
        }
        if index == 7 {
            frame(&g, out, "scorch-intro-route")?;
        }
        trace.push(serde_json::json!({"waypoint":index,"position":g.state.pawn.body.position,"crouched":g.state.crouched,"companion":g.state.rendezvous}));
    }
    for _ in 0..1800 {
        if g.state.phase != Phase::Playing {
            break;
        }
        let i = Input::default();
        g.advance(STEP, i)?;
        record(&mut replay, i);
    }
    if g.state.phase != Phase::Won {
        fs::write(
            out.join("failure.json"),
            serde_json::to_vec_pretty(&g.state)?,
        )?;
        return Err("Scorch encounter never completes".into());
    }
    let s = g.state.rendezvous.as_ref().unwrap();
    assert_eq!(
        g.state.bunker.as_ref().unwrap().complete_tick,
        Some(gun_completed_tick)
    );
    assert!(
        s.flow.squad_joined
            && s.flow.objective_completed
            && s.flow.charge_enabled
            && s.intro_seen
            && s.behind_seen
    );
    assert_eq!(s.motion, 4, "Scorch must reach greeting spot");
    assert!(crouch_release_checked);
    let sound_counts = g.state.mission.as_ref().unwrap().events.iter().fold(
        std::collections::BTreeMap::<usize, usize>::new(),
        |mut m, e| {
            *m.entry(e.sound).or_default() += 1;
            m
        },
    );
    assert!(s.effects.iter().any(
        |(_, e)| matches!(e,rc_game::rendezvous::Effect::ShowMarker{tag}if tag=="MarkerDemolition")
    ));
    let complete = g.state.clone();
    let scorch = s.pawn.body.position;
    g.aim_at(scorch);
    frame(&g, out, "scorch-recruited")?;
    assert_eq!(world, rc_render::gpu::world(&g.scene)?);
    g.restart();
    assert_eq!(initial, serde_json::to_vec(&g.state)?);
    let mut bytes = b"RCINPUT1".to_vec();
    bytes.extend(((replay.len() / 20) as u32).to_le_bytes());
    bytes.extend(replay);
    fs::write(out.join("input-replay.bin"), bytes)?;
    fs::write(
        out.join("validation.json"),
        serde_json::to_vec_pretty(
            &serde_json::json!({"passed":true,"input_driven_objectives":[0,1,2,3,4],"complete_state":complete,"trace":trace,"restart_exact":true,"static_gpu_packet_unchanged":true,"blocked_uncrouch_checked":true,"sound_counts":sound_counts,"scope":"Rust normal-input acceptance. Companion movement is a bounded adapter; full original factory combat and native GoalQueue parity are not claimed."}),
        )?,
    )?;
    println!(
        "Contiguous objectives0–4 passed: tick{}, Scorch joined, restart exact",
        complete.statistics.ticks
    );
    Ok(())
}
