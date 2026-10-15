use rc_game::{Game, Input, STEP};
use std::{fs, path::Path};
fn run(g: &mut Game, rows: &mut Vec<u8>, input: Input) -> Result<(), String> {
    let v = [input.forward, input.strafe, input.look[0], input.look[1]].map(|v| v as f32);
    for f in v {
        rows.extend(f.to_le_bytes());
    }
    let flags = u32::from(input.fire) * 2;
    rows.extend(flags.to_le_bytes());
    g.advance(
        STEP,
        Input {
            forward: v[0] as f64,
            strafe: v[1] as f64,
            look: [v[2] as f64, v[3] as f64],
            fire: input.fire,
            ..Input::default()
        },
    )?;
    Ok(())
}
fn frame(g: &Game, out: &Path, name: &str) -> Result<(), String> {
    let mut b = b"P6\n640 400\n255\n".to_vec();
    for p in g.render(640, 400)? {
        b.extend([(p >> 16) as u8, (p >> 8) as u8, p as u8]);
    }
    fs::write(out.join(format!("{name}.ppm")), b).map_err(|e| e.to_string())
}
fn main() -> Result<(), Box<dyn std::error::Error>> {
    let out = Path::new("analysis/mission22-map3-escape/delivery/playthrough");
    fs::create_dir_all(out)?;
    let bytes = fs::read("analysis/mission22-map3-escape/delivery/geo_01c.rcplay")?;
    let handoff = serde_json::from_slice(&fs::read(
        "analysis/mission19-map3/delivery/playthrough/map2-handoff.json",
    )?)?;
    let mut g = rc_game::bundle::decode(&bytes)?;
    g.restore_handoff(&handoff)?;
    let initial = serde_json::to_value(&g.state)?;
    let initial_frame = g.render_frame(320, 200)?.gpu_packet(&g.scene);
    let prefix =
        fs::read("analysis/mission21-map3-planning/delivery/playthrough/input-replay.bin")?;
    let mut rows = vec![];
    let mut max_open = 0f64;
    for (i, chunk) in prefix[12..].chunks_exact(20).enumerate() {
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
        let e = g
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
            .escape
            .as_ref()
            .unwrap();
        max_open = max_open.max(e.door_fraction);
        if [1590, 1680, 1770, 1860, 2040, 3356].contains(&i) {
            println!(
                "{i} stage={} waypoint={} position={:?} door={} blocked={}",
                e.stage, e.waypoint, e.position, e.door_fraction, e.blocked
            );
            frame(&g, out, &format!("escape-{i}"))?;
        }
    }
    let before = serde_json::to_value(&g.state)?;
    let door_probe = g.world.sweep_motion(
        [-19852.51953125, 9200., -566.],
        [-19852.51953125, 8700., -566.],
        [40., 40., 84.],
    )?;
    assert!(door_probe
        .contacts
        .iter()
        .any(|c| c.object == "MoverDB_GEO29" || c.object == "MoverDBSlave_GEO29"));
    let mut paced_reference = None;
    for fps in [20, 60, 144] {
        let mut paced = rc_game::bundle::decode(&bytes)?;
        paced.restore_handoff(&handoff)?;
        for chunk in prefix[12..12 + 1556 * 20].chunks_exact(20) {
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
        let state = serde_json::to_value(&paced.state)?;
        if let Some(reference) = &paced_reference {
            assert_eq!(reference, &state, "Escape pacing {fps}");
        } else {
            paced_reference = Some(state);
        }
    }

    let e = g
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
        .escape
        .as_ref()
        .unwrap();
    assert_eq!(e.stage, 3);
    assert_eq!(max_open, 1.);
    assert_eq!(e.door_fraction, 0.);
    assert!(e.locked);
    assert!(e.trigger_tick.is_none());
    assert_eq!(e.teleport_tick.unwrap() - e.arrived_tick.unwrap(), 180);
    assert_eq!(
        e.position,
        [-15673.9462890625, 7760.6943359375, -759.8717041015625]
    );
    for _ in 0..600 {
        let eye = g.eye();
        let goal = if g.state.pawn.body.position[0] < -19360. {
            [-19300., 10650., -587.]
        } else {
            [-19300., 9700., -587.]
        };
        let d = std::array::from_fn::<_, 3, _>(|i| goal[i] - eye[i]);
        let yaw = d[1].atan2(d[0]);
        let look = [
            (g.state.yaw - yaw + std::f64::consts::PI).rem_euclid(std::f64::consts::TAU)
                - std::f64::consts::PI,
            g.state.pitch,
        ];
        run(
            &mut g,
            &mut rows,
            Input {
                forward: 0.8,
                look,
                ..Input::default()
            },
        )?;
        let e = g
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
            .escape
            .as_ref()
            .unwrap();
        if e.trigger_tick.is_some() {
            break;
        }
    }
    let o = g.state.map.as_ref().unwrap().opening.as_ref().unwrap();
    let e = o.planning.as_ref().unwrap().escape.as_ref().unwrap();
    println!(
        "Approach player={:?} trigger={:?}",
        g.state.pawn.body.position, e.trigger_tick
    );
    assert!(o.events.contains("StartSBDIntro"));
    assert_eq!(e.door_fraction, 0.);
    assert!(g.state.health > 0);
    assert!(!o.objectives[0] && !o.objectives[3]);
    frame(&g, out, "proximity-trigger")?;
    let final_state = serde_json::to_value(&g.state)?;
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
            &serde_json::json!({"passed":true,"inputs":rows.len()/20,"ordinary_input_replay_exact":true,"exact_restart_state_and_frame":true,"maximum_door_open":max_open,"original_three_second_wait_ticks":180,"escape_frame_rates_exact":[20,60,144],"closed_door_blocks_player":true,"before_approach":before,"final":final_state,"scope":"Actual B carry, original C opening and planning combat, physical Sun Fac escape, guard-door close/lock, original teleport and proximity-trigger event. SBD execution pending"}),
        )?,
    )?;
    println!("Sun Fac escape check passed; {} inputs", rows.len() / 20);
    Ok(())
}
