use rc_game::{Game, Input, STEP};
use std::{fs, path::Path};
fn run(g: &mut Game, rows: &mut Vec<u8>, mut input: Input) -> Result<(), String> {
    let v = [input.forward, input.strafe, input.look[0], input.look[1]].map(|v| v as f32);
    for f in v {
        rows.extend(f.to_le_bytes());
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
fn frame(g: &Game, out: &Path, name: &str) -> Result<(), String> {
    let mut b = b"P6\n640 400\n255\n".to_vec();
    for p in g.render(640, 400)? {
        b.extend([(p >> 16) as u8, (p >> 8) as u8, p as u8]);
    }
    fs::write(out.join(format!("{name}.ppm")), b).map_err(|e| e.to_string())
}
fn main() -> Result<(), Box<dyn std::error::Error>> {
    let out = Path::new("analysis/mission23-map3-guard/delivery/playthrough");
    fs::create_dir_all(out)?;
    let bytes = fs::read("analysis/mission23-map3-guard/delivery/geo_01c.rcplay")?;
    let handoff = serde_json::from_slice(&fs::read(
        "analysis/mission19-map3/delivery/playthrough/map2-handoff.json",
    )?)?;
    let mut g = rc_game::bundle::decode(&bytes)?;
    g.restore_handoff(&handoff)?;
    let initial = serde_json::to_value(&g.state)?;
    let initial_frame = g.render_frame(320, 200)?.gpu_packet(&g.scene);
    let prefix = fs::read("analysis/mission22-map3-escape/delivery/playthrough/input-replay.bin")?;
    let mut rows = vec![];
    for chunk in prefix[12..].chunks_exact(20) {
        let v = |n| f32::from_le_bytes(chunk[n..n + 4].try_into().unwrap()) as f64;
        let flags = u32::from_le_bytes(chunk[16..20].try_into().unwrap());
        run(
            &mut g,
            &mut rows,
            Input {
                forward: v(0),
                strafe: v(4),
                look: [v(8), v(12)],
                fire: flags & 2 != 0,
                ..Input::default()
            },
        )?;
    }
    let trigger_state = serde_json::to_value(&g.state)?;
    let mut pace_reference = None;
    for fps in [20, 60, 144] {
        let mut paced = rc_game::bundle::decode(&bytes)?;
        paced.restore_handoff(&handoff)?;
        for chunk in prefix[12..].chunks_exact(20) {
            let v = |n| f32::from_le_bytes(chunk[n..n + 4].try_into().unwrap()) as f64;
            let flags = u32::from_le_bytes(chunk[16..20].try_into().unwrap());
            paced.advance(
                STEP,
                Input {
                    forward: v(0),
                    strafe: v(4),
                    look: [v(8), v(12)],
                    fire: flags & 2 != 0,
                    ..Input::default()
                },
            )?;
        }
        for _ in 0..fps * 20 {
            paced.advance(1. / fps as f64, Input::default())?;
        }
        let value = serde_json::to_value(&paced.state)?;
        let sbd = paced
            .state
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
            .sbd
            .unwrap();
        let look = aim(&paced, paced.state.enemies[sbd].position);
        let ammo = paced.state.ammo;
        paced.advance(
            STEP,
            Input {
                look,
                use_action: true,
                ..Input::default()
            },
        )?;
        let guard = paced
            .state
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
            .unwrap();
        assert_eq!(
            guard.focus, None,
            "Closed door must block squad target selection"
        );
        assert_eq!(guard.focus_commands, 0);
        assert_eq!(paced.state.ammo, ammo);

        if let Some(reference) = &pace_reference {
            assert_eq!(reference, &value, "SBD pacing {fps}");
        } else {
            pace_reference = Some(value);
        }
    }

    let mut stage = 0;
    let mut ready_frame = false;
    for i in 0..2400 {
        let o = g.state.map.as_ref().unwrap().opening.as_ref().unwrap();
        let guard = o.planning.as_ref().unwrap().guard.as_ref().unwrap();
        if o.objectives[3] && o.events.contains("TrackBreachDoorSunFac") {
            break;
        }
        let sbd = guard.sbd;
        let live_sbd = sbd.filter(|i| g.state.enemies[*i].health > 0);
        let pos = g.state.pawn.body.position;
        if stage == 0 && (pos[1] - 9330.).abs() < 40. {
            stage = 1;
        }
        if stage == 1 && (pos[0] + 19852.).abs() < 40. {
            stage = 2;
        }
        let goal = if stage == 0 {
            [-19300., 9330., -587.]
        } else {
            [-19852., 9430., -587.]
        };
        let target = live_sbd
            .map(|i| g.state.enemies[i].position)
            .unwrap_or(if stage == 2 {
                guard.animations[1].position
            } else {
                goal
            });
        let look = aim(&g, target);
        let yaw = g.state.yaw - look[0];
        let dx = goal[0] - pos[0];
        let dy = goal[1] - pos[1];
        let d = dx.hypot(dy).max(1.);
        let moving = stage < 2 && live_sbd.is_none();
        let speed = if moving { 0.65 } else { 0. };
        let input = Input {
            look,
            forward: (speed * (dx * yaw.cos() + dy * yaw.sin()) / d).clamp(-1., 1.),
            strafe: (speed * (dx * yaw.sin() - dy * yaw.cos()) / d).clamp(-1., 1.),
            fire: live_sbd.is_some(),
            reload: g.state.ammo < 2 && g.state.reserve > 0,
            use_action: live_sbd.is_some(),
            ..Input::default()
        };
        run(&mut g, &mut rows, input)?;
        if i % 120 == 0 {
            let o = g.state.map.as_ref().unwrap().opening.as_ref().unwrap();
            let s = o.planning.as_ref().unwrap().guard.as_ref().unwrap();
            println!(
                "{i} pos={:?} hp={} ammo={} intro={:?} anim={:?} sbd={:?} focus={:?} alive={}",
                g.state.pawn.body.position,
                g.state.health,
                g.state.ammo,
                s.intro_tick,
                s.animations.iter().map(|a| a.finished).collect::<Vec<_>>(),
                s.sbd.map(|i| g.state.enemies[i].health),
                s.focus,
                g.state.enemies.iter().filter(|e| e.health > 0).count()
            );
        }
        if i < 179 {
            assert!(!g
                .state
                .map
                .as_ref()
                .unwrap()
                .opening
                .as_ref()
                .unwrap()
                .events
                .contains("SBDIntro"));
        }
        if [210, 330, 450].contains(&i) {
            frame(&g, out, &format!("intro-{i}"))?;
        }
        if live_sbd.is_some() && !ready_frame {
            frame(&g, out, "combat-ready")?;
            ready_frame = true;
        }
        if g.state.phase != rc_game::Phase::Playing {
            return Err(format!("Lost at step {i}").into());
        }
    }
    let o = g.state.map.as_ref().unwrap().opening.as_ref().unwrap();
    let guard = o.planning.as_ref().unwrap().guard.as_ref().unwrap();
    println!(
        "Final objectives={:?} SBD={:?} focus_commands={} notifies={:?} health={} ammo={}/{}",
        o.objectives,
        guard.sbd,
        guard.focus_commands,
        guard
            .animations
            .iter()
            .map(|a| a.notifies.len())
            .collect::<Vec<_>>(),
        g.state.health,
        g.state.ammo,
        g.state.reserve
    );
    assert!(o.objectives[3] && !o.objectives[0]);
    assert!(o.events.contains("SBDDestroyed") && o.events.contains("TrackBreachDoorSunFac"));
    assert!(
        guard.focus_commands > 0 && guard.spawned && guard.animations.iter().all(|a| a.finished)
    );
    assert_eq!(g.state.enemies.len(), 14);
    assert!(g.state.enemies[guard.sbd.unwrap()].health == 0);
    assert!(!o.planning.as_ref().unwrap().escape.as_ref().unwrap().locked);
    assert_eq!(
        guard
            .animations
            .iter()
            .map(|a| a.notifies.len())
            .collect::<Vec<_>>(),
        vec![3, 2, 10]
    );
    assert_eq!(guard.bindings.len(), 9);
    assert_eq!(g.state.enemies[5].health, 400);
    assert!(guard.intro_tick.unwrap() >= o.event_ticks["SBDIntro"]);
    assert!(guard.destroyed_tick.unwrap() >= o.event_ticks["SBDCombatReady"]);
    assert!(g.state.statistics.enemy_shots > 0 && g.state.health > 0);

    let final_state = serde_json::to_value(&g.state)?;
    frame(&g, out, "objective3-complete")?;
    g.restart();
    assert_eq!(initial, serde_json::to_value(&g.state)?);
    assert_eq!(
        initial_frame,
        g.render_frame(320, 200)?.gpu_packet(&g.scene)
    );
    for chunk in rows.chunks_exact(20) {
        let v = |n| f32::from_le_bytes(chunk[n..n + 4].try_into().unwrap()) as f64;
        let flags = u32::from_le_bytes(chunk[16..20].try_into().unwrap());
        g.advance(
            STEP,
            Input {
                forward: v(0),
                strafe: v(4),
                look: [v(8), v(12)],
                fire: flags & 2 != 0,
                reload: flags & 4 != 0,
                use_action: flags & 16 != 0,
                ..Input::default()
            },
        )?;
    }
    assert_eq!(final_state, serde_json::to_value(&g.state)?);
    let mut input = b"RCINPUT1".to_vec();
    input.extend(((rows.len() / 20) as u32).to_le_bytes());
    input.extend(&rows);
    fs::write(out.join("input-replay.bin"), input)?;
    fs::write(
        out.join("validation.json"),
        serde_json::to_vec_pretty(
            &serde_json::json!({"passed":true,"inputs":rows.len()/20,"ordinary_input_replay_exact":true,"exact_restart_state_and_frame":true,"frame_rates_exact":[20,60,144],"native_notify_counts":[3,2,10],"no_early_intro":true,"closed_door_blocks_focus":true,"at_trigger":trigger_state,"final":final_state,"scope":"Actual B carry and preceding C route, original SBD intro, eight guard factories, live intro SBD/focus combat, SBDDestroyed, objective3 and next door tracker; optional guard-room enemies may remain"}),
        )?,
    )?;
    println!("Guard-room check passed; {} inputs", rows.len() / 20);
    Ok(())
}
