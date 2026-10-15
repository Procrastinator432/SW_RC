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
    let out = Path::new("analysis/mission27-map3-elevator/delivery/playthrough");
    fs::create_dir_all(out)?;
    let bytes = fs::read("analysis/mission27-map3-elevator/delivery/geo_01c.rcplay")?;
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
    let prefix = fs::read("analysis/mission26-map3-finale/delivery/playthrough/input-replay.bin")?;
    let mut rows = prefix[12..].to_vec();
    play_inputs(&mut g, &rows)?;
    assert!(g.campaign_handoff()?.is_none());
    let mut planner = fresh()?;
    for a in &mut planner.world.actors {
        if [
            "WindowShatter_Test1",
            "Prop3",
            "MoverDB_GEO29",
            "MoverDBSlave_GEO29",
            "MoverDB_GEO18",
            "MoverDBSlave_GEO18",
            "MoverDB_GEO6",
            "MoverDBSlave_GEO6",
        ]
        .contains(&a.name.as_str())
        {
            a.shape = rc_package::world_collision::BodyShape::Clear;
        }
    }
    let nav = &planner
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
        .assets
        .navigation;
    let goal = [-19139.60546875, 4760.44189453125, -570.];
    let goal = rc_game::mission_motion::ground(
        &planner.world,
        [goal[0], goal[1], goal[2] + 160.],
        rc_game::squad::EXTENT,
        800.,
    )?
    .ok_or("Unsupported elevator goal")?;
    let mut route = vec![];
    let mut next = 0;
    let mut door_prefix = None;
    println!("Return from {:?} to {:?}", g.state.pawn.body.position, goal);
    for tick in 0..10000 {
        if g.state.phase != rc_game::Phase::Playing {
            break;
        }
        if door_prefix.is_none() && state(&g).elevator.as_ref().unwrap().door_fraction > 0. {
            door_prefix = Some(rows.len());
        }
        let pos = g.state.pawn.body.position;
        let dist = |a: [f64; 3], b: [f64; 3]| (a[0] - b[0]).hypot(a[1] - b[1]);
        if route.is_empty() || tick % 180 == 0 {
            route = rc_game::squad::plan_with_tolerance(
                nav,
                &planner.world,
                pos,
                goal,
                rc_game::squad::EXTENT,
                4.,
            )?;
            next = 0;
            if route.is_empty() {
                return Err(format!("No return route from {:?} to {:?}", pos, goal).into());
            }
        }
        while next < route.len() && dist(pos, route[next]) < 18. {
            next += 1;
        }
        let target = route.get(next).copied().unwrap_or(goal);
        let mut input = combat_input(&g, target)?;
        let dx = target[0] - pos[0];
        let dy = target[1] - pos[1];
        let d = dx.hypot(dy);
        let yaw = g.state.yaw - input.look[0];
        let speed = (d / 8.).min(1.);
        input.forward = speed * (dx * yaw.cos() + dy * yaw.sin()) / d.max(0.001);
        input.strafe = speed * (dx * yaw.sin() - dy * yaw.cos()) / d.max(0.001);
        if input.fire {
            input.forward = 0.;
            input.strafe = 0.;
        }
        step(&mut g, &mut rows, input)?;
        if tick % 300 == 0 {
            println!(
                "{tick}: {:?} HP{} route{next}/{} door{}",
                g.state.pawn.body.position,
                g.state.health,
                route.len(),
                state(&g).elevator.as_ref().unwrap().door_fraction
            );
        }
    }
    assert_eq!(
        g.state.phase,
        rc_game::Phase::Won,
        "{:?}",
        g.state.pawn.body.position
    );
    let receipt = g.campaign_handoff()?.ok_or("Missing exit receipt")?;
    assert_eq!(receipt.destination, "GEO_03A.ctm#playerstart");
    assert_eq!(receipt.trigger, 39);
    assert!(g
        .state
        .map
        .as_ref()
        .unwrap()
        .opening
        .as_ref()
        .unwrap()
        .objectives
        .iter()
        .all(|v| *v));
    assert!(state(&g).elevator.as_ref().unwrap().approach_tick.is_some());
    let final_state = serde_json::to_value(&g.state)?;
    let final_frame = g.render_frame(320, 200)?.gpu_packet(&g.scene);
    frame(&g, out, "factory-elevator-complete")?;
    // Completion is immutable, wrong destination is transactional.
    assert_eq!(
        g.advance(
            0.25,
            Input {
                fire: true,
                forward: 1.,
                ..Input::default()
            }
        )?,
        0
    );
    assert_eq!(final_state, serde_json::to_value(&g.state)?);
    assert!(g.travel_to(&bytes).is_err());
    assert_eq!(final_state, serde_json::to_value(&g.state)?);
    g.restart();
    assert_eq!(initial, serde_json::to_value(&g.state)?);
    assert_eq!(
        initial_frame,
        g.render_frame(320, 200)?.gpu_packet(&g.scene)
    );
    play_inputs(&mut g, &rows)?;
    assert_eq!(final_state, serde_json::to_value(&g.state)?);
    assert_eq!(final_frame, g.render_frame(320, 200)?.gpu_packet(&g.scene));
    assert_eq!(
        serde_json::to_value(&receipt)?,
        serde_json::to_value(g.campaign_handoff()?.unwrap())?
    );
    let door_prefix = door_prefix.ok_or("Door never moved")?;
    let mut timing = None;
    for fps in [20, 60, 144] {
        let mut test = fresh()?;
        play_inputs(&mut test, &rows[..door_prefix])?;
        for _ in 0..fps * 2 {
            test.advance(1. / fps as f64, Input::default())?;
        }
        let snapshot = serde_json::to_value(&test.state)?;
        if let Some(expected) = &timing {
            assert_eq!(expected, &snapshot, "Door/squad timing {fps}");
        } else {
            timing = Some(snapshot);
        }
    }
    let mut recording = b"RCINPUT1".to_vec();
    recording.extend(((rows.len() / 20) as u32).to_le_bytes());
    recording.extend(&rows);
    fs::write(out.join("input-replay.bin"), recording)?;
    fs::write(
        out.join("map3-handoff.json"),
        serde_json::to_vec_pretty(&receipt)?,
    )?;
    fs::write(
        out.join("validation.json"),
        serde_json::to_vec_pretty(
            &serde_json::json!({"passed":true,"inputs":rows.len()/20,"ordinary_input_replay_exact":true,"exact_restart_state_and_frame":true,"immutable_completed_session":true,"wrong_destination_atomic":true,"door_frame_rates_exact":[20,60,144],"final":final_state,"handoff":receipt}),
        )?,
    )?;
    println!(
        "C elevator complete: {} inputs HP{} {}/{}",
        rows.len() / 20,
        g.state.health,
        g.state.ammo,
        g.state.reserve
    );
    Ok(())
}
