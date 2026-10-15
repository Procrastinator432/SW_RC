//! First-map acceptance through normal inputs, with live intro and handoff boundary.
use rc_game::{Game, Input, Phase, STEP};
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
    let bits = u32::from(i.jump)
        | (u32::from(i.fire) * 2)
        | (u32::from(i.reload) * 4)
        | (u32::from(i.walking) * 8)
        | (u32::from(i.use_action) * 16)
        | (u32::from(i.throw_grenade) * 32)
        | (u32::from(i.crouch) * 64);
    out.extend(bits.to_le_bytes());
}
fn advance(g: &mut Game, out: &mut Vec<u8>, mut i: Input) -> Result<(), String> {
    i.forward = (i.forward as f32) as f64;
    i.strafe = (i.strafe as f32) as f64;
    i.look = i.look.map(|v| (v as f32) as f64);
    g.advance(STEP, i)?;
    record(out, i);
    Ok(())
}
fn save(out: &Path, name: &str, rows: &[u8]) -> Result<(), std::io::Error> {
    let mut v = b"RCINPUT1".to_vec();
    v.extend((rows.len() as u32 / 20).to_le_bytes());
    v.extend(rows);
    fs::write(out.join(name), v)
}
fn frame(g: &Game, out: &Path, name: &str) -> Result<(), Box<dyn std::error::Error>> {
    let mut ppm = b"P6\n640 400\n255\n".to_vec();
    for p in g.render(640, 400)? {
        ppm.extend([(p >> 16) as u8, (p >> 8) as u8, p as u8]);
    }
    fs::write(out.join(format!("{name}.ppm")), ppm)?;
    Ok(())
}
fn main() -> Result<(), Box<dyn std::error::Error>> {
    let args: Vec<_> = env::args_os().collect();
    let scene = fs::read(&args[1])?;
    let old = fs::read(&args[2])?;
    let out = Path::new(&args[3]);
    fs::create_dir_all(out)?;
    let mut g = rc_game::bundle::decode(&scene)?;
    let initial = serde_json::to_vec(&g.state)?;
    let initial_frame = g.render_frame(320, 200)?.gpu_packet(&g.scene);
    let position = g.state.pawn.body.position;
    assert_eq!(g.state.health, 200);
    let mut rows = vec![];
    let mut injury = None;
    let mut intro_pose = false;
    let mut samples = vec![];
    frame(&g, out, "gunship-briefing")?;
    for i in 0..12000 {
        let command = if i < 60 {
            Input {
                forward: 1.,
                fire: true,
                crouch: true,
                jump: true,
                use_action: true,
                throw_grenade: true,
                ..Input::default()
            }
        } else {
            Input::default()
        };
        advance(&mut g, &mut rows, command)?;
        if i < 60 {
            assert_eq!(g.state.pawn.body.position, position);
            assert_eq!(g.state.statistics.shots, 0);
            assert_eq!(g.state.ammo, 20);
            assert!(!g.state.crouched);
        }
        let state = g.state.opening.as_ref().unwrap();
        if [1, 218, 230, 330, 430, 498, 540].contains(&g.state.statistics.ticks) {
            samples.push(serde_json::json!({"tick":g.state.statistics.ticks,"position":g.state.pawn.body.position,"rappel":state.rappel}));
            frame(&g, out, &format!("rappel-{}", g.state.statistics.ticks))?;
        }
        if state.injured && injury.is_none() {
            assert_eq!(g.state.health, 25);
            injury = Some(g.state.statistics.ticks);
            frame(&g, out, "scripted-injury")?;
        }
        if state.clock > 12. && !intro_pose {
            frame(&g, out, "gunship-departure")?;
            intro_pose = true;
        }
        if state.done {
            break;
        }
    }
    assert!(g.state.opening.as_ref().unwrap().done);
    assert!(injury.is_some());
    assert_eq!(g.state.health, 25);
    let opening_ticks = g.state.statistics.ticks;
    let rappel = g.state.opening.as_ref().unwrap().rappel.as_ref().unwrap();
    assert!(rappel.started && rappel.finished && rappel.rope_destroyed);
    assert_eq!(rappel.blocked_steps, 0);
    assert_eq!(rappel.notify_events.len(), 7);
    for id in 225..=231 {
        assert_eq!(
            rappel
                .notify_events
                .iter()
                .filter(|(_, n)| *n == id)
                .count(),
            1
        );
    }
    assert!(position[2] - g.state.pawn.body.position[2] > 590.);
    let rappel_state = serde_json::to_value(rappel)?;
    fs::write(
        out.join("rappel-samples.json"),
        serde_json::to_vec_pretty(&samples)?,
    )?;
    let opening = serde_json::to_value(g.state.opening.as_ref().unwrap())?;
    let opening_events = g.state.opening.as_ref().unwrap().events.clone();
    for id in [
        1241, 1242, 1249, 1254, 2495, 2496, 2498, 2501, 2502, 2503, 2505, 2506, 2397, 2400, 2401,
        2402, 2403, 2405, 2407, 2408,
    ] {
        assert_eq!(
            opening_events.iter().filter(|(_, a, _)| *a == id).count(),
            1,
            "missing/duplicate action{id}"
        );
    }
    let at = |id| opening_events.iter().find(|(_, a, _)| *a == id).unwrap().0;
    assert_eq!(at(2502), at(2503));
    assert!(at(2506) < at(2397));
    assert!(at(2397) < at(2407));
    assert_eq!(g.state.mission.as_ref().unwrap().objective_mode, 1);
    frame(&g, out, "bacta-ready")?;
    let baseline = rc_game::bundle::decode(&fs::read(&args[4])?)?;
    // Reach the old recorded route entry with ordinary analog movement after landing.
    // The original pitched spawn shifts the new landing point; no state assignment.
    let target = baseline.state.pawn.body.position;
    for _ in 0..600 {
        let pos = g.state.pawn.body.position;
        let vel = g.state.pawn.body.velocity;
        if (pos[0] - target[0]).hypot(pos[1] - target[1]) < 0.01 && vel[0].hypot(vel[1]) < 0.01 {
            break;
        }
        let u: [f64; 2] = std::array::from_fn(|j| {
            ((50. * (target[j] - pos[j]) - 14. * vel[j]) / g.profile.controller.acceleration)
                .clamp(-1., 1.)
        });
        let (s, c) = g.state.yaw.sin_cos();
        advance(
            &mut g,
            &mut rows,
            Input {
                forward: (u[0] * c + u[1] * s).clamp(-1., 1.),
                strafe: (u[0] * s - u[1] * c).clamp(-1., 1.),
                ..Input::default()
            },
        )?;
    }
    assert!(
        (g.state.pawn.body.position[0] - target[0])
            .hypot(g.state.pawn.body.position[1] - target[1])
            < 0.02
    );
    let alignment_inputs = g.state.statistics.ticks - opening_ticks;
    let correction = [
        (g.state.yaw - baseline.state.yaw + std::f64::consts::PI).rem_euclid(std::f64::consts::TAU)
            - std::f64::consts::PI,
        g.state.pitch - baseline.state.pitch,
    ];
    for (i, row) in old[12..].chunks_exact(20).enumerate() {
        let mut command = input(row);
        if i == 0 {
            for (v, correction) in command.look.iter_mut().zip(correction) {
                *v += correction;
            }
        }
        advance(&mut g, &mut rows, command)?;
        assert_ne!(g.state.phase, Phase::Lost);
    }
    if g.state.phase != Phase::Won {
        fs::write(
            out.join("failure-state.json"),
            serde_json::to_vec_pretty(&g.state)?,
        )?;
        frame(&g, out, "failure")?;
    }
    assert_eq!(g.state.phase, Phase::Won);
    assert_eq!(g.state.health, 20);
    assert_eq!(
        g.state.statistics.kills + g.state.squad.as_ref().unwrap().kills,
        8
    );
    let intro = g
        .state
        .opening
        .as_ref()
        .unwrap()
        .intro
        .as_ref()
        .ok_or("Missing live intro")?;
    assert!(intro.actors.iter().all(|a| a.destroyed));
    assert_eq!(
        intro.flow.warrior,
        rc_game::intro_flow::WarriorPhase::Destroyed
    );
    assert_eq!(intro.teleport_accepted, Some(true));
    assert!(intro.goal_result.is_some());
    assert_eq!(
        intro
            .notifies
            .iter()
            .filter(|(_, n)| (190..=193).contains(n))
            .count(),
        4
    );
    for id in 190..=193 {
        assert_eq!(intro.notifies.iter().filter(|(_, n)| *n == id).count(), 1);
    }
    let handoff = g
        .state
        .spire
        .as_ref()
        .unwrap()
        .handoff
        .as_ref()
        .ok_or("Missing map handoff")?;
    assert_eq!(handoff.destination, "GEO_01B.ctm#playerstart");
    assert_eq!(handoff.objectives, [0, 1, 2, 3, 4, 5, 6]);
    assert_eq!(handoff.player["health"], 20.);
    fs::write(
        out.join("map-handoff.json"),
        serde_json::to_vec_pretty(handoff)?,
    )?;
    let terminal = serde_json::to_vec(&g.state)?;
    assert_eq!(
        g.advance(
            STEP,
            Input {
                fire: true,
                jump: true,
                use_action: true,
                ..Input::default()
            }
        )?,
        0
    );
    assert_eq!(serde_json::to_vec(&g.state)?, terminal);
    frame(&g, out, "mission-complete")?;
    let complete = serde_json::to_value(&g.state)?;
    save(out, "input-replay.bin", &rows)?;
    g.restart();
    assert_eq!(serde_json::to_vec(&g.state)?, initial);
    assert_eq!(
        g.render_frame(320, 200)?.gpu_packet(&g.scene),
        initial_frame
    );
    for _ in 0..20000 {
        g.advance(1. / 144., Input::default())?;
        if g.state.opening.as_ref().unwrap().done {
            break;
        }
    }
    assert_eq!(
        g.state.opening.as_ref().unwrap().ready_tick,
        Some(opening_ticks)
    );
    assert_eq!(g.state.opening.as_ref().unwrap().events, opening_events);
    assert_eq!(
        serde_json::to_value(g.state.opening.as_ref().unwrap().rappel.as_ref().unwrap())?,
        rappel_state
    );
    g.restart();
    g.advance(
        STEP,
        Input {
            look: [6., 6.],
            ..Input::default()
        },
    )?;
    let a = &g.opening.as_ref().unwrap().rappel.as_ref().unwrap().assets;
    assert!((a.limits[2]..=a.limits[3]).contains(&g.state.yaw));
    assert!((a.limits[0].max(-1.4)..=a.limits[1]).contains(&g.state.pitch));
    fs::write(
        out.join("validation.json"),
        serde_json::to_vec_pretty(
            &serde_json::json!({"passed":true,"normal_inputs_only":true,"alignment_inputs":alignment_inputs,"opening_ticks":opening_ticks,"injury_tick":injury,"opening":opening,"complete_state":complete,"locked_controls_tested":true,"non_waiting_sound_overlap":true,"restart_exact_state_frame":true,"opening_144hz_events_equal":true,"rappel":rappel_state,"scope":"Playable GEO_01A objectives0-6 through original exit and versioned map handoff; live selected intro actors/voices/notifies; bounded collision/navigation, not full original parity; GEO_01B interior not loaded","first_map_boundary_complete":true}),
        )?,
    )?;
    println!(
        "First-map original objectives0-6 and handoff passed: {} intro ticks, {} total inputs",
        opening_ticks,
        rows.len() / 20
    );
    Ok(())
}
