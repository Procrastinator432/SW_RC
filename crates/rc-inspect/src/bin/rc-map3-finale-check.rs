use rc_game::{Game, Input, STEP};
use std::{fs, path::Path};
fn step(g: &mut Game, rows: &mut Vec<u8>, mut input: Input) -> Result<(), String> {
    let v = [input.forward, input.strafe, input.look[0], input.look[1]].map(|v| v as f32);
    for x in v {
        rows.extend(x.to_le_bytes());
    }
    let flags =
        u32::from(input.fire) * 2 + u32::from(input.reload) * 4 + u32::from(input.use_action) * 16;
    rows.extend(flags.to_le_bytes());
    input.forward = v[0] as f64;
    input.strafe = v[1] as f64;
    input.look = [v[2] as f64, v[3] as f64];
    g.advance(STEP, input)?;
    Ok(())
}
fn aim(g: &Game, to: [f64; 3]) -> [f64; 2] {
    let eye = g.eye();
    let d = std::array::from_fn::<_, 3, _>(|i| to[i] - eye[i]);
    [
        (g.state.yaw - d[1].atan2(d[0]) + std::f64::consts::PI).rem_euclid(std::f64::consts::TAU)
            - std::f64::consts::PI,
        g.state.pitch - d[2].atan2(d[0].hypot(d[1])),
    ]
}
fn play_inputs(g: &mut Game, rows: &[u8]) -> Result<(), String> {
    for c in rows.chunks_exact(20) {
        let v = |i| f32::from_le_bytes(c[i..i + 4].try_into().unwrap()) as f64;
        let f = u32::from_le_bytes(c[16..20].try_into().unwrap());
        g.advance(
            STEP,
            Input {
                forward: v(0),
                strafe: v(4),
                look: [v(8), v(12)],
                fire: f & 2 != 0,
                reload: f & 4 != 0,
                use_action: f & 16 != 0,
                ..Input::default()
            },
        )?;
    }
    Ok(())
}
fn distance(a: [f64; 3], b: [f64; 3]) -> f64 {
    (0..3).map(|i| (a[i] - b[i]).powi(2)).sum::<f64>().sqrt()
}
fn frame(g: &Game, out: &Path, name: &str) -> Result<(), String> {
    let mut image = b"P6\n640 400\n255\n".to_vec();
    for pixel in g.render(640, 400)? {
        image.extend([(pixel >> 16) as u8, (pixel >> 8) as u8, pixel as u8]);
    }
    fs::write(out.join(format!("{name}.ppm")), image).map_err(|e| e.to_string())
}
fn state(g: &Game) -> &rc_game::map3_finale::State {
    g.state
        .map
        .as_ref()
        .unwrap()
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
        .unwrap()
        .balcony
        .as_ref()
        .unwrap()
        .finale
        .as_ref()
        .unwrap()
}
fn combat_input(g: &Game, target: [f64; 3]) -> Result<Input, String> {
    let pos = g.state.pawn.body.position;
    let enemy = g
        .state
        .enemies
        .iter()
        .filter(|e| e.health > 0 && distance(e.position, pos) < 1850.)
        .filter(|e| {
            g.world
                .sweep_motion(g.eye(), e.position, [0.; 3])
                .is_ok_and(|q| q.complete && q.contacts.is_empty())
        })
        .min_by(|a, b| distance(a.position, pos).total_cmp(&distance(b.position, pos)));
    Ok(Input {
        look: aim(g, enemy.map_or(target, |e| e.position)),
        fire: enemy.is_some(),
        reload: g.state.ammo < 2 && g.state.reserve > 0,
        use_action: enemy.is_some(),
        ..Input::default()
    })
}
fn main() -> Result<(), Box<dyn std::error::Error>> {
    let out = Path::new("analysis/mission26-map3-finale/delivery/playthrough");
    fs::create_dir_all(out)?;
    let bytes = fs::read("analysis/mission26-map3-finale/delivery/geo_01c.rcplay")?;
    let handoff = serde_json::from_slice(&fs::read(
        "analysis/mission19-map3/delivery/playthrough/map2-handoff.json",
    )?)?;
    let fresh = || -> Result<Game, Box<dyn std::error::Error>> {
        let mut g = rc_game::bundle::decode(&bytes)?;
        g.restore_handoff(&handoff)?;
        Ok(g)
    };
    let mut g = fresh()?;
    let initial = serde_json::to_value(&g.state)?;
    let initial_frame = g.render_frame(320, 200)?.gpu_packet(&g.scene);
    let prefix = fs::read("analysis/mission25-map3-balcony/delivery/playthrough/input-replay.bin")?;
    let mut rows = prefix[12..].to_vec();
    play_inputs(&mut g, &rows)?;
    println!("Prefix: HP{} sun{}", g.state.health, state(&g).sun_stage);
    let mut takeoff_prefix = None;
    let mut flight_frame = false;
    let mut death_frame = false;
    for tick in 0..6000 {
        let s = state(&g);
        let target = s.ship_position;
        if s.takeoff_tick.is_some() && takeoff_prefix.is_none() {
            takeoff_prefix = Some(rows.len());
        }
        if s.ship_stage == 1
            && !flight_frame
            && s.ship_animation
                .playback
                .channels
                .first()
                .is_some_and(|c| f32::from_bits(c.words[7]) > 0.5)
        {
            frame(&g, out, "fighter-takeoff")?;
            flight_frame = true;
        }
        if s.ship_stage == 3
            && !death_frame
            && s.ship_animation
                .playback
                .channels
                .first()
                .is_some_and(|c| f32::from_bits(c.words[7]) > 0.5)
        {
            frame(&g, out, "fighter-crash")?;
            death_frame = true;
        }
        if s.terminated_tick.is_some()
            && g.state
                .map
                .as_ref()
                .unwrap()
                .opening
                .as_ref()
                .unwrap()
                .events
                .contains("TrackElevatorDoor")
        {
            println!("Success at {tick}");
            break;
        }
        let input = combat_input(&g, target)?;
        step(&mut g, &mut rows, input)?;
        if tick % 600 == 0 {
            println!(
                "{tick} HP{} sun{} wp{} ship{} shots{}",
                g.state.health,
                state(&g).sun_stage,
                state(&g).waypoint,
                state(&g).ship_stage,
                state(&g).shots
            );
        }
        if g.state.phase != rc_game::Phase::Playing {
            break;
        }
    }
    fs::write(
        out.join("debug-state.json"),
        serde_json::to_vec_pretty(&g.state)?,
    )?;
    let s = state(&g);
    assert!(s.terminated_tick.is_some(), "Sun Fac not terminated");
    assert_eq!(s.success, Some(true));
    assert_eq!(s.ship_animation.notifies.len(), 9);
    assert!(s.shots > 0);
    assert!(!s.ship_visible && s.wreck_visible);
    let o = g.state.map.as_ref().unwrap().opening.as_ref().unwrap();
    assert_eq!(o.objectives, [true, true, true, true, true, true, false]);
    assert!(o.events.contains("DisableBalconySnipe"));
    assert_eq!(g.state.phase, rc_game::Phase::Playing);
    frame(&g, out, "sunfac-terminated")?;
    let final_state = serde_json::to_value(&g.state)?;
    g.restart();
    assert_eq!(initial, serde_json::to_value(&g.state)?);
    assert_eq!(
        initial_frame,
        g.render_frame(320, 200)?.gpu_packet(&g.scene)
    );
    play_inputs(&mut g, &rows)?;
    assert_eq!(final_state, serde_json::to_value(&g.state)?);
    let mut paced_reference = None;
    for fps in [20, 60, 144] {
        let mut paced = fresh()?;
        play_inputs(
            &mut paced,
            &rows[..takeoff_prefix.ok_or("No takeoff prefix")?],
        )?;
        for _ in 0..fps * 35 {
            paced.advance(1. / fps as f64, Input::default())?;
        }
        let value = serde_json::to_value(&paced.state)?;
        if let Some(expected) = &paced_reference {
            assert_eq!(expected, &value, "Flight pacing {fps}");
        } else {
            paced_reference = Some(value);
        }
    }
    let mut failed = fresh()?;
    let mut failed_rows = prefix[12..12 + 7729 * 20].to_vec();
    play_inputs(&mut failed, &failed_rows)?;
    for tick in 0..12000 {
        if failed.state.phase != rc_game::Phase::Playing {
            break;
        }
        let input = combat_input(&failed, state(&failed).ship_position)?;
        step(&mut failed, &mut failed_rows, input)?;
        if tick % 1200 == 0 {
            println!(
                "Failure branch {tick} sun{} ship{} reminder{}",
                state(&failed).sun_stage,
                state(&failed).ship_stage,
                state(&failed).reminder_index
            );
        }
    }
    let f = state(&failed);
    assert_eq!(failed.state.phase, rc_game::Phase::Lost);
    assert!(f.failed_tick.is_some() && f.terminated_tick.is_none());
    assert_eq!(f.success, Some(false));
    assert_eq!(f.shots, 0);
    assert_eq!(f.reminder_index, 4);
    assert!(
        !failed
            .state
            .map
            .as_ref()
            .unwrap()
            .opening
            .as_ref()
            .unwrap()
            .objectives[0]
    );
    let failure = serde_json::to_value(&failed.state)?;
    let mut failure_replay = b"RCINPUT1".to_vec();
    failure_replay.extend(((failed_rows.len() / 20) as u32).to_le_bytes());
    failure_replay.extend(&failed_rows);
    fs::write(out.join("failure-input-replay.bin"), failure_replay)?;
    failed.restart();
    play_inputs(&mut failed, &failed_rows)?;
    assert_eq!(failure, serde_json::to_value(&failed.state)?);
    let mut replay = b"RCINPUT1".to_vec();
    replay.extend(((rows.len() / 20) as u32).to_le_bytes());
    replay.extend(&rows);
    fs::write(out.join("input-replay.bin"), replay)?;
    fs::write(
        out.join("validation.json"),
        serde_json::to_vec_pretty(
            &serde_json::json!({"passed":true,"inputs":rows.len()/20,"ordinary_input_replay_exact":true,"exact_restart_state_and_frame":true,"frame_rates_exact":[20,60,144],"original_ship_notifies":9,"failure_without_sev_exact_replay":true,"failure_inputs":failed_rows.len()/20,"failure":failure,"final":final_state}),
        )?,
    )?;
    println!(
        "Finale proof passed: {} success / {} failure inputs",
        rows.len() / 20,
        failed_rows.len() / 20
    );
    Ok(())
}
