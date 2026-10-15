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
fn main() -> Result<(), Box<dyn std::error::Error>> {
    let out = Path::new("analysis/mission25-map3-balcony/delivery/playthrough");
    fs::create_dir_all(out)?;
    let bytes = fs::read("analysis/mission25-map3-balcony/delivery/geo_01c.rcplay")?;
    let handoff = serde_json::from_slice(&fs::read(
        "analysis/mission19-map3/delivery/playthrough/map2-handoff.json",
    )?)?;
    let mut g = rc_game::bundle::decode(&bytes)?;
    g.restore_handoff(&handoff)?;
    assert!(g
        .world
        .actors
        .iter()
        .any(|a| a.name == "Prop3"
            && matches!(a.shape, rc_package::world_collision::BodyShape::Clear)));
    let initial = serde_json::to_value(&g.state)?;
    let initial_frame = g.render_frame(320, 200)?.gpu_packet(&g.scene);
    let prefix = fs::read("analysis/mission24-map3-breach/delivery/playthrough/input-replay.bin")?;
    let mut rows = prefix[12..12 + 6480 * 20].to_vec();
    play_inputs(&mut g, &rows)?;
    println!(
        "Prefix HP{} {:?}",
        g.state.health, g.state.pawn.body.position
    );
    let mut planner = rc_game::bundle::decode(&bytes)?;
    for a in &mut planner.world.actors {
        if [
            "WindowShatter_Test1",
            "MoverDB_GEO29",
            "MoverDBSlave_GEO29",
            "MoverDB_GEO18",
            "MoverDBSlave_GEO18",
            "Prop3",
        ]
        .contains(&a.name.as_str())
        {
            a.shape = rc_package::world_collision::BodyShape::Clear;
        }
    }
    let mut route = vec![];
    let mut next = 0;
    let mut assigned_prefix = None;
    let mut shield_collision_before_visible = false;
    for tick in 0..6000 {
        let b = g
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
            .breach
            .as_ref()
            .unwrap()
            .balcony
            .as_ref()
            .unwrap();
        if let Some(at) = b.shield_tick {
            assert_eq!(b.shield_visible, g.state.statistics.ticks >= at + 120);
            assert!(g.world.actors.iter().any(|a| a.name == "Prop3"
                && matches!(a.shape, rc_package::world_collision::BodyShape::Hulls(_))));
            shield_collision_before_visible |= !b.shield_visible;
        }
        if b.positioned_tick.is_some() && b.stage == 6 {
            println!("Position reached {tick}");
            break;
        }
        if b.assigned_tick.is_some() && assigned_prefix.is_none() {
            assigned_prefix = Some(rows.len());
        }
        let assigned = b.assigned_tick.is_some();
        let p = g
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
            .unwrap();
        let a = &p.balcony.as_ref().unwrap().assets;
        let goal = a.standby;
        let crate_position = a.crate_position;
        let pos = g.state.pawn.body.position;
        let distance = |a: [f64; 3], b: [f64; 3]| (a[0] - b[0]).hypot(a[1] - b[1]);
        if !assigned && distance(pos, goal) > 35. && (route.is_empty() || tick % 180 == 0) {
            route = rc_game::squad::plan_with_tolerance(
                &p.assets.navigation,
                &planner.world,
                pos,
                goal,
                rc_game::squad::EXTENT,
                4.,
            )?;
            next = 0;
            if route.is_empty() {
                println!("No route {pos:?} to {goal:?}");
                break;
            }
        }
        while next < route.len() && distance(pos, route[next]) < 18. {
            next += 1;
        }
        let target = route.get(next).copied().unwrap_or(goal);
        let enemy = g
            .state
            .enemies
            .iter()
            .enumerate()
            .filter(|(_, e)| e.health > 0 && distance(pos, e.position) < 1850.)
            .filter(|(_, e)| {
                g.world
                    .sweep_motion(g.eye(), e.position, [0.; 3])
                    .is_ok_and(|q| q.complete && q.contacts.is_empty())
            })
            .min_by(|a, b| distance(a.1.position, pos).total_cmp(&distance(b.1.position, pos)))
            .map(|(i, _)| i);
        let looking = enemy.map_or(
            if distance(pos, goal) < 35. || assigned {
                crate_position
            } else {
                target
            },
            |i| g.state.enemies[i].position,
        );
        let look = aim(&g, looking);
        let yaw = g.state.yaw - look[0];
        let dx = target[0] - pos[0];
        let dy = target[1] - pos[1];
        let d = dx.hypot(dy).max(1.);
        let speed = if !assigned && distance(pos, goal) > 35. && enemy.is_none() {
            0.65
        } else {
            0.
        };
        let reload = g.state.ammo < 2 && g.state.reserve > 0;
        step(
            &mut g,
            &mut rows,
            Input {
                look,
                forward: speed * (dx * yaw.cos() + dy * yaw.sin()) / d,
                strafe: speed * (dx * yaw.sin() - dy * yaw.cos()) / d,
                fire: enemy.is_some(),
                reload,
                use_action: enemy.is_some() || distance(pos, goal) < 35.,
                ..Input::default()
            },
        )?;
        if tick % 600 == 0 {
            let o = g.state.map.as_ref().unwrap().opening.as_ref().unwrap();
            let b = o
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
                .unwrap();
            println!(
                "{tick} HP{} {:?} route{next}/{} sun{} waypoint{} sev{:?} assigned{:?}",
                g.state.health,
                g.state.pawn.body.position,
                route.len(),
                b.stage,
                b.waypoint,
                o.sev.position,
                b.assigned_tick
            );
        }
        if g.state.phase != rc_game::Phase::Playing {
            println!("Lost");
            break;
        }
    }
    fs::write(
        out.join("debug-state.json"),
        serde_json::to_vec_pretty(&g.state)?,
    )?;
    let o = g.state.map.as_ref().unwrap().opening.as_ref().unwrap();
    let b = o
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
        .unwrap();
    assert!(b.positioned_tick.is_some(), "Sev not in position");
    assert_eq!(b.bindings.len(), 8);
    assert!(shield_collision_before_visible);
    assert_eq!(b.animation.notifies.len(), 8);
    assert!(b.shield_visible && b.stage == 6);
    assert!(o.events.contains("EnableBalconySnipe") && o.events.contains("InSnipePosition"));
    assert_eq!(o.objectives, [false, true, true, true, true, false, false]);
    let assets = &g
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
        .assets;
    assert!(
        g.world
            .sweep_motion(
                assets.sun_route[0],
                *assets.sun_route.last().unwrap(),
                [0.; 3]
            )?
            .contacts
            .iter()
            .any(|c| c.object == "Prop3"),
        "Active shield blocks traces"
    );
    let final_state = serde_json::to_value(&g.state)?;
    let mut image = b"P6\n640 400\n255\n".to_vec();
    for pixel in g.render(640, 400)? {
        image.extend([(pixel >> 16) as u8, (pixel >> 8) as u8, pixel as u8]);
    }
    fs::write(out.join("sev-positioned.ppm"), image)?;
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
        let mut paced = rc_game::bundle::decode(&bytes)?;
        paced.restore_handoff(&handoff)?;
        play_inputs(
            &mut paced,
            &rows[..assigned_prefix.ok_or("No assign checkpoint")?],
        )?;
        for _ in 0..fps * 15 {
            paced.advance(1. / fps as f64, Input::default())?;
        }
        let state = serde_json::to_value(&paced.state)?;
        if let Some(expected) = &paced_reference {
            assert_eq!(expected, &state, "Sniper pacing {fps}");
        } else {
            paced_reference = Some(state);
        }
    }
    let mut replay = b"RCINPUT1".to_vec();
    replay.extend(((rows.len() / 20) as u32).to_le_bytes());
    replay.extend(&rows);
    fs::write(out.join("input-replay.bin"), replay)?;
    fs::write(
        out.join("validation.json"),
        serde_json::to_vec_pretty(
            &serde_json::json!({"passed":true,"inputs":rows.len()/20,"ordinary_input_replay_exact":true,"exact_restart_state_and_frame":true,"frame_rates_exact":[20,60,144],"eight_aa_room_factories":true,"sev_physically_positioned":true,"final":final_state}),
        )?,
    )?;
    println!("Balcony proof passed: {} inputs", rows.len() / 20);
    Ok(())
}
