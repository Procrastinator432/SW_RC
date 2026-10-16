//! Mission-input AI acceptance, plus explicitly isolated original-geometry probes.
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
fn advance(g: &mut Game, out: &mut Vec<u8>, i: Input) -> Result<(), String> {
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
    let owned_voice_admission = !g.opening.as_ref().unwrap().assets.sound_policies.is_empty();
    if owned_voice_admission {
        assert!(opening_events
            .iter()
            .any(|(_, id, label)| *id == 2503 && label.ends_with("BlockedByOwnedVoice")));
        let rejected_role = g
            .opening
            .as_ref()
            .unwrap()
            .assets
            .programs
            .iter()
            .flat_map(|p| &p.actions)
            .find_map(|a| {
                if a.original == 2503 {
                    if let rc_game::opening::Op::Sound { role, .. } = a.op {
                        return Some(role);
                    }
                }
                None
            })
            .unwrap();
        assert!(!g
            .state
            .mission
            .as_ref()
            .unwrap()
            .events
            .iter()
            .any(|e| e.sound == rejected_role));
    }
    assert!(at(2506) < at(2397));
    assert!(at(2397) < at(2407));
    assert_eq!(g.state.mission.as_ref().unwrap().objective_mode, 1);
    frame(&g, out, "bacta-ready")?;
    for row in old[12..].chunks_exact(20) {
        advance(&mut g, &mut rows, input(row))?;
        assert_ne!(g.state.phase, Phase::Lost);
    }
    assert_eq!(g.state.phase, Phase::Won);
    assert_eq!(g.state.health, 20);
    assert_eq!(
        g.state.statistics.kills + g.state.squad.as_ref().unwrap().kills,
        8
    );
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
    fs::write(
        out.join("validation.json"),
        serde_json::to_vec_pretty(
            &serde_json::json!({"passed":true,"normal_inputs_only":true,"opening_ticks":opening_ticks,"injury_tick":injury,"opening":opening,"complete_state":complete,"locked_controls_tested":true,"non_waiting_actions_share_tick":true,"owned_voice_admission":owned_voice_admission,"restart_exact_state_frame":true,"opening_144hz_events_equal":true,"scope":"Original selected briefing/injury scripts plus Gunship poses; prepared post-rappel player placement, no full rope camera/intro Warrior/decorative troop parity"}),
        )?,
    )?;
    println!(
        "Opening + original objectives0-6 passed: {} intro ticks, {} total inputs",
        opening_ticks,
        rows.len() / 20
    );
    Ok(())
}
