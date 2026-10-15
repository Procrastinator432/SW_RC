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
    let out = Path::new("analysis/mission24-map3-breach/delivery/playthrough");
    fs::create_dir_all(out)?;
    let bytes = fs::read("analysis/mission24-map3-breach/delivery/geo_01c.rcplay")?;
    let mut g = rc_game::bundle::decode(&bytes)?;
    g.restore_handoff(&serde_json::from_slice(&fs::read(
        "analysis/mission19-map3/delivery/playthrough/map2-handoff.json",
    )?)?)?;
    let initial = serde_json::to_value(&g.state)?;
    let initial_frame = g.render_frame(320, 200)?.gpu_packet(&g.scene);
    let prefix = fs::read("analysis/mission23-map3-guard/delivery/playthrough/input-replay.bin")?;
    let mut rows = vec![];
    for c in prefix[12..].chunks_exact(20) {
        let v = |i| f32::from_le_bytes(c[i..i + 4].try_into().unwrap()) as f64;
        let f = u32::from_le_bytes(c[16..20].try_into().unwrap());
        step(
            &mut g,
            &mut rows,
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
    println!(
        "Prefix: {:?} HP {}",
        g.state.pawn.body.position, g.state.health
    );
    let mut planner = rc_game::bundle::decode(&bytes)?;
    for a in &mut planner.world.actors {
        if [
            "WindowShatter_Test1",
            "MoverDB_GEO29",
            "MoverDBSlave_GEO29",
            "MoverDB_GEO18",
            "MoverDBSlave_GEO18",
        ]
        .contains(&a.name.as_str())
        {
            a.shape = rc_package::world_collision::BodyShape::Clear;
        }
    }
    let mut stage = 0;
    let mut route: Vec<[f64; 3]> = vec![];
    let mut next = 0;
    let mut healing_prefix = None;
    let mut command_prefix = None;
    for tick in 0..12000 {
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
            .unwrap();
        if stage == 0 && g.state.health == 100 {
            stage = 1;
            route.clear();
            println!("Entry healed at {tick}");
        }
        if stage == 1 && b.healed[1] > 0 && g.state.health == 100 {
            stage = 2;
            route.clear();
            println!("Guard station healed at {tick}");
        }
        if stage == 2 && b.stage > 0 {
            stage = 3;
            command_prefix = Some(rows.len());
            println!("Breach command at {tick}");
        }
        if stage == 3 && b.stage == 3 && b.settling.is_none() && o.objectives[4] {
            println!("Complete at {tick}");
            break;
        }
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
        let station = p.assets.stations[0].position;
        let goal = if stage == 0 {
            [station[0] - 130., station[1], p.assets.spots[0][2]]
        } else if stage == 1 {
            [
                p.assets.stations[1].position[0] + 130.,
                p.assets.stations[1].position[1],
                p.assets.spots[0][2],
            ]
        } else {
            [
                p.assets.spots[0][0] - 160.,
                p.assets.spots[0][1],
                p.assets.spots[0][2],
            ]
        };
        let pos = g.state.pawn.body.position;
        let dist = |a: [f64; 3], b: [f64; 3]| (a[0] - b[0]).hypot(a[1] - b[1]);
        if stage == 0 && dist(pos, goal) < 35. && healing_prefix.is_none() {
            healing_prefix = Some(rows.len());
        }
        if stage < 3 && dist(pos, goal) > 30. && (route.is_empty() || tick % 180 == 0) {
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
                println!("No route stage{stage} {:?} -> {:?}", pos, goal);
                for (label, at) in [("from", pos), ("to", goal)] {
                    for n in &p.assets.navigation.nodes {
                        if dist(at, n.position) < 1500.
                            && rc_game::squad::corridor_with_tolerance(
                                &planner.world,
                                at,
                                n.position,
                                rc_game::squad::EXTENT,
                                4.,
                            )?
                            .is_some()
                        {
                            println!("{label} {} {:?}", n.original, n.position);
                        }
                    }
                }
                break;
            }
        }
        while next < route.len() && dist(pos, route[next]) < 18. {
            next += 1;
        }
        let target = if stage == 3 {
            goal
        } else {
            route.get(next).copied().unwrap_or(goal)
        };
        let enemy = if stage == 1 || stage == 2 {
            g.state
                .enemies
                .iter()
                .enumerate()
                .filter(|(_, e)| e.health > 0 && dist(e.position, pos) < 1500.)
                .filter(|(_, e)| {
                    g.world
                        .sweep_motion(g.eye(), e.position, [0.; 3])
                        .is_ok_and(|q| q.complete && q.contacts.is_empty())
                })
                .min_by(|a, b| dist(a.1.position, pos).total_cmp(&dist(b.1.position, pos)))
                .map(|(i, _)| i)
        } else {
            None
        };
        let look = aim(&g, enemy.map_or(target, |i| g.state.enemies[i].position));
        let yaw = g.state.yaw - look[0];
        let dx = target[0] - pos[0];
        let dy = target[1] - pos[1];
        let d = dx.hypot(dy).max(1.);
        let speed = if stage < 3 && dist(pos, goal) > 30. && enemy.is_none() {
            0.65
        } else {
            0.
        };
        let reload = g.state.ammo < 2 && g.state.reserve > 0;
        step(
            &mut g,
            &mut rows,
            Input {
                forward: speed * (dx * yaw.cos() + dy * yaw.sin()) / d,
                strafe: speed * (dx * yaw.sin() - dy * yaw.cos()) / d,
                look,
                fire: enemy.is_some(),
                reload,
                use_action: enemy.is_some() || dist(pos, goal) < 35.,
                ..Input::default()
            },
        )?;
        if tick % 600 == 0 {
            println!(
                "{tick} stage{stage} {:?} HP{} route {next}/{}",
                g.state.pawn.body.position,
                g.state.health,
                route.len()
            );
        }
        if g.state.health == 0 {
            println!("Player died");
            break;
        }
    }
    let look = aim(&g, [-17609.76171875, 7798.638671875, -543.09375]);
    step(
        &mut g,
        &mut rows,
        Input {
            look,
            ..Input::default()
        },
    )?;
    fs::write(
        out.join("debug-state.json"),
        serde_json::to_vec_pretty(&g.state)?,
    )?;
    let mut replay = b"RCINPUT1".to_vec();
    replay.extend(((rows.len() / 20) as u32).to_le_bytes());
    replay.extend(&rows);
    fs::write(out.join("input-replay.bin"), replay)?;
    let o = g.state.map.as_ref().unwrap().opening.as_ref().unwrap();
    assert!(o.objectives[4], "Goal 4 not reached");
    let b = o
        .planning
        .as_ref()
        .unwrap()
        .guard
        .as_ref()
        .unwrap()
        .breach
        .as_ref()
        .unwrap();
    assert_eq!(b.stage, 3);
    assert_eq!(b.door_fraction, 1.);
    assert!(b.healed.iter().all(|h| *h > 0));
    assert!(b.animations.iter().all(|a| a.finished));
    assert_eq!(
        b.animations
            .iter()
            .map(|a| a.notifies.len())
            .collect::<Vec<_>>(),
        vec![5, 3, 0]
    );
    assert!(o.event_ticks["SunFacRoomDoorOpening"] >= b.animation_tick.unwrap());
    let final_state = serde_json::to_value(&g.state)?;
    let mut image = b"P6\n640 400\n255\n".to_vec();
    for pixel in g.render(640, 400)? {
        image.extend([(pixel >> 16) as u8, (pixel >> 8) as u8, pixel as u8]);
    }
    fs::write(out.join("objective4-complete.ppm"), image)?;
    let handoff = serde_json::from_slice(&fs::read(
        "analysis/mission19-map3/delivery/playthrough/map2-handoff.json",
    )?)?;
    let fresh = || -> Result<Game, Box<dyn std::error::Error>> {
        let mut g = rc_game::bundle::decode(&bytes)?;
        g.restore_handoff(&handoff)?;
        Ok(g)
    };
    let mut branch = fresh()?;
    play_inputs(
        &mut branch,
        &rows[..healing_prefix.ok_or("No healing checkpoint")?],
    )?;
    let hp = branch.state.health;
    let ammo = branch.state.ammo;
    for _ in 0..60 {
        branch.advance(STEP, Input::default())?;
    }
    assert_eq!(hp, branch.state.health, "No automatic healing");
    let pos = branch.state.pawn.body.position;
    for _ in 0..60 {
        branch.advance(
            STEP,
            Input {
                forward: 1.,
                fire: true,
                use_action: true,
                ..Input::default()
            },
        )?;
    }
    assert_eq!(ammo, branch.state.ammo, "Healing suppresses fire");
    assert_eq!(
        pos, branch.state.pawn.body.position,
        "Healing locks walking"
    );
    assert!(branch.state.health > hp);
    for _ in 0..600 {
        branch.advance(
            STEP,
            Input {
                use_action: true,
                ..Input::default()
            },
        )?;
    }
    assert_eq!(branch.state.health, 100);
    let mut paced_reference = None;
    for fps in [20, 60, 144] {
        let mut paced = fresh()?;
        play_inputs(
            &mut paced,
            &rows[..command_prefix.ok_or("No command checkpoint")?],
        )?;
        for _ in 0..fps * 20 {
            paced.advance(1. / fps as f64, Input::default())?;
        }
        let state = serde_json::to_value(&paced.state)?;
        if let Some(expected) = &paced_reference {
            assert_eq!(expected, &state, "Breach pacing {fps}");
        } else {
            paced_reference = Some(state);
        }
    }
    let input = fs::read(out.join("input-replay.bin"))?;
    g.restart();
    assert_eq!(initial, serde_json::to_value(&g.state)?);
    assert_eq!(
        initial_frame,
        g.render_frame(320, 200)?.gpu_packet(&g.scene)
    );
    for c in input[12..].chunks_exact(20) {
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
    assert_eq!(final_state, serde_json::to_value(&g.state)?);
    fs::write(
        out.join("validation.json"),
        serde_json::to_vec_pretty(
            &serde_json::json!({"passed":true,"inputs":(input.len()-12)/20,"ordinary_input_replay_exact":true,"exact_restart_state_and_frame":true,"both_stations_used":true,"healing_requires_use":true,"healing_locks_walking_and_fire":true,"healing_clamped_100":true,"frame_rates_exact":[20,60,144],"original_breach_notify_counts":[5,3,0],"final":final_state}),
        )?,
    )?;
    println!("Breach proof passed: {} inputs", (input.len() - 12) / 20);
    Ok(())
}
