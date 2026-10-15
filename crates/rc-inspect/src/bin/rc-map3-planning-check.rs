use rc_game::{Game, Input, STEP};
use std::{fs, path::Path};
fn frame(g: &Game, out: &Path, name: &str) -> Result<(), String> {
    let mut b = b"P6\n640 400\n255\n".to_vec();
    for p in g.render(640, 400)? {
        b.extend([(p >> 16) as u8, (p >> 8) as u8, p as u8]);
    }
    fs::write(out.join(format!("{name}.ppm")), b).map_err(|e| e.to_string())
}
fn record(g: &mut Game, rows: &mut Vec<u8>, mut input: Input) -> Result<(), String> {
    for f in [input.forward, input.strafe, input.look[0], input.look[1]] {
        rows.extend((f as f32).to_le_bytes());
    }
    input.forward = input.forward as f32 as f64;
    input.strafe = input.strafe as f32 as f64;
    input.look = input.look.map(|v| v as f32 as f64);
    rows.extend(
        (u32::from(input.jump)
            | (u32::from(input.fire) * 2)
            | (u32::from(input.reload) * 4)
            | (u32::from(input.crouch) * 64))
            .to_le_bytes(),
    );
    g.advance(STEP, input)?;
    Ok(())
}
fn aim(g: &Game, at: [f64; 3]) -> [f64; 2] {
    let eye = g.eye();
    let d = std::array::from_fn::<_, 3, _>(|i| at[i] - eye[i]);
    let yaw = d[1].atan2(d[0]);
    let pitch = d[2].atan2(d[0].hypot(d[1]));
    [
        (g.state.yaw - yaw + std::f64::consts::PI).rem_euclid(std::f64::consts::TAU)
            - std::f64::consts::PI,
        g.state.pitch - pitch,
    ]
}
fn main() -> Result<(), Box<dyn std::error::Error>> {
    let out = Path::new("analysis/mission21-map3-planning/delivery/playthrough");
    fs::create_dir_all(out)?;
    let bytes = fs::read("analysis/mission21-map3-planning/delivery/geo_01c.rcplay")?;
    let handoff = serde_json::from_slice(&fs::read(
        "analysis/mission19-map3/delivery/playthrough/map2-handoff.json",
    )?)?;
    let mut g = rc_game::bundle::decode(&bytes)?;
    g.restore_handoff(&handoff)?;
    let initial = serde_json::to_value(&g.state)?;
    let initial_frame = g.render_frame(320, 200)?.gpu_packet(&g.scene);
    let mut rows = vec![];
    for _ in 0..1500 {
        record(&mut g, &mut rows, Input::default())?;
    }
    assert_eq!(g.state.enemies.len(), 5);
    assert_eq!(g.state.health, 10);
    let pre = serde_json::to_value(&g.state)?;
    for fps in [20, 60, 144] {
        let mut paced = rc_game::bundle::decode(&bytes)?;
        paced.restore_handoff(&handoff)?;
        for _ in 0..fps * 25 {
            paced.advance(1. / fps as f64, Input::default())?;
        }
        assert_eq!(
            pre,
            serde_json::to_value(&paced.state)?,
            "Opening pacing {fps}"
        );
    }
    let mut blocked = rc_game::bundle::decode(&bytes)?;
    blocked.restore_handoff(&handoff)?;
    for _ in 0..1500 {
        blocked.advance(STEP, Input::default())?;
    }
    let mut blocker = None;
    for x in (-20440..=-19280).step_by(40) {
        for z in (-660..=-140).step_by(40) {
            let to = [x as f64, 11475.5, z as f64];
            let eye = blocked.eye();
            let p = blocked
                .map
                .as_ref()
                .unwrap()
                .opening
                .as_ref()
                .unwrap()
                .planning
                .as_ref()
                .unwrap();
            if let Some(glass_hit) = p.collider.1.sweep(eye, to, [0.; 3])? {
                let q = blocked.world.sweep_motion(eye, to, [0.; 3])?;
                if q.contacts
                    .first()
                    .is_some_and(|h| h.hit.fraction + 0.00001 < glass_hit.fraction)
                {
                    blocker = Some(to);
                    break;
                }
            }
        }
        if blocker.is_some() {
            break;
        }
    }
    let blocked_target = blocker.ok_or("No real world occlusion probe found")?;
    let look = aim(&blocked, blocked_target);
    let mut negative_rows = vec![];
    record(
        &mut blocked,
        &mut negative_rows,
        Input {
            look,
            fire: true,
            ..Input::default()
        },
    )?;
    assert_eq!(blocked.state.ammo, 19);
    assert_eq!(
        blocked
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
            .glass_health,
        150
    );
    assert_eq!(
        blocked
            .state
            .enemies
            .iter()
            .map(|e| e.health)
            .collect::<Vec<_>>(),
        vec![75, 75, 100, 100, 100]
    );
    fs::write(
        out.join("occlusion.json"),
        serde_json::to_vec_pretty(
            &serde_json::json!({"passed":true,"ordinary_shot":true,"real_world_blocker_target":blocked_target,"impact":blocked.state.impact,"glass_health":150,"enemy_health":[75,75,100,100,100]}),
        )?,
    )?;
    let glass = [-19859.6796875, 11475.5244140625, -400.];
    let look = aim(&g, glass);
    record(
        &mut g,
        &mut rows,
        Input {
            look,
            ..Input::default()
        },
    )?;
    frame(&g, out, "glass-intact")?;
    for i in 0..5 {
        record(
            &mut g,
            &mut rows,
            Input {
                fire: true,
                ..Input::default()
            },
        )?;
        for _ in 0..10 {
            record(&mut g, &mut rows, Input::default())?;
        }
        let s = g
            .state
            .map
            .as_ref()
            .unwrap()
            .opening
            .as_ref()
            .unwrap()
            .planning
            .as_ref()
            .unwrap();
        println!(
            "Shot {} glass={} impact={:?}",
            i + 1,
            s.glass_health,
            g.state.impact
        );
        assert_eq!(s.glass_health, 150 - 30 * (i + 1));
    }
    frame(&g, out, "glass-destroyed")?;
    assert_eq!(
        g.state.enemies.iter().map(|e| e.health).collect::<Vec<_>>(),
        vec![75, 75, 100, 100, 100]
    );
    for t in 0..2400 {
        if g.state.enemies.iter().all(|e| e.health == 0) {
            break;
        }
        let eye = g.eye();
        let target = g
            .state
            .enemies
            .iter()
            .enumerate()
            .filter(|(i, e)| e.health > 0 && (*i < 2 || distance(eye, e.position) < 700.))
            .map(|(_, e)| e)
            .filter(|e| {
                g.world
                    .sweep_motion(eye, e.position, [0.; 3])
                    .is_ok_and(|q| q.contacts.is_empty())
            })
            .min_by(|a, b| distance(eye, a.position).total_cmp(&distance(eye, b.position)))
            .map(|e| e.position);
        let walk = [-19860., 10200., -587.];
        let look = aim(&g, target.unwrap_or(walk));
        let yaw = g.state.yaw - look[0];
        let dx = walk[0] - g.state.pawn.body.position[0];
        let dy = walk[1] - g.state.pawn.body.position[1];
        let d = dx.hypot(dy);
        let (forward, strafe) = if target.is_some() || d < 100. {
            (0., 0.)
        } else {
            (
                (dx * yaw.cos() + dy * yaw.sin()) / d,
                (dx * yaw.sin() - dy * yaw.cos()) / d,
            )
        };
        record(
            &mut g,
            &mut rows,
            Input {
                look,
                forward,
                strafe,
                fire: target.is_some(),
                reload: false,
                ..Input::default()
            },
        )?;
        if t % 300 == 0 {
            println!(
                "Combat {t} pos={:?} health={} enemies={:?}",
                g.state.pawn.body.position,
                g.state.health,
                g.state.enemies.iter().map(|e| e.health).collect::<Vec<_>>()
            );
        }
        if g.state.phase != rc_game::Phase::Playing {
            break;
        }
    }
    frame(&g, out, "planning-combat")?;
    for _ in 0..1200 {
        record(&mut g, &mut rows, Input::default())?;
    }
    println!(
        "Combat result: health={} kills={} enemies={:?}",
        g.state.health,
        g.state.statistics.kills,
        g.state.enemies.iter().map(|e| e.health).collect::<Vec<_>>()
    );
    assert!(g.state.enemies.iter().all(|e| e.health == 0));
    assert!(
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
            .assists
            > 0
    );
    let o = g.state.map.as_ref().unwrap().opening.as_ref().unwrap();
    assert!(o.objectives[1] && o.objectives[2] && !o.objectives[0]);
    assert!(o.events.contains("WindowDestroyed"));
    assert!(g.state.statistics.shots >= 5);
    let final_state = serde_json::to_value(&g.state)?;
    let mut failure = rc_game::bundle::decode(&bytes)?;
    failure.restore_handoff(&handoff)?;
    for chunk in rows[..1556 * 20].chunks_exact(20) {
        let v = |n| f32::from_le_bytes(chunk[n..n + 4].try_into().unwrap()) as f64;
        let flags = u32::from_le_bytes(chunk[16..20].try_into().unwrap());
        failure.advance(
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
    let mut failure_rows = vec![];
    for _ in 0..1800 {
        let look = aim(&failure, [-19860., 10200., -587.]);
        record(
            &mut failure,
            &mut failure_rows,
            Input {
                look,
                forward: 0.8,
                ..Input::default()
            },
        )?;
        if failure.state.phase == rc_game::Phase::Lost {
            break;
        }
    }
    assert_eq!(failure.state.phase, rc_game::Phase::Lost);
    assert!(failure.state.statistics.enemy_shots > 0);
    fs::write(
        out.join("failure-path.json"),
        serde_json::to_vec_pretty(
            &serde_json::json!({"passed":true,"ordinary_inputs_only":true,"prefix_inputs":1556,"additional_inputs":failure_rows.len()/20,"state":failure.state}),
        )?,
    )?;
    failure.restart();
    assert_eq!(initial, serde_json::to_value(&failure.state)?);

    g.restart();
    assert_eq!(initial, serde_json::to_value(&g.state)?);
    assert_eq!(
        initial_frame,
        g.render_frame(320, 200)?.gpu_packet(&g.scene)
    );
    let mut input = b"RCINPUT1".to_vec();
    input.extend(((rows.len() / 20) as u32).to_le_bytes());
    input.extend(&rows);
    fs::write(out.join("input-replay.bin"), input)?;
    for chunk in rows.chunks_exact(20) {
        let f = |n| f32::from_le_bytes(chunk[n..n + 4].try_into().unwrap()) as f64;
        let flags = u32::from_le_bytes(chunk[16..20].try_into().unwrap());
        g.advance(
            STEP,
            Input {
                forward: f(0),
                strafe: f(4),
                look: [f(8), f(12)],
                fire: flags & 2 != 0,
                ..Input::default()
            },
        )?;
    }
    assert_eq!(final_state, serde_json::to_value(&g.state)?);
    fs::write(
        out.join("validation.json"),
        serde_json::to_vec_pretty(
            &serde_json::json!({"passed":true,"inputs":rows.len()/20,"initial":initial,"before_glass":pre,"final":final_state,"restart_state_and_frame":true,"opening_fps":[20,60,144],"real_wall_occlusion":true,"no_enemy_damage_through_glass":true,"ordinary_input_replay_exact":true,"scope":"Actual B carry, original C opening, five planning-room factories, five shots destroy original glass and complete objective2; escape movement and SBD pending"}),
        )?,
    )?;
    println!("Planning-room check passed; {} inputs", rows.len() / 20);
    Ok(())
}

fn distance(a: [f64; 3], b: [f64; 3]) -> f64 {
    (0..3).map(|i| (a[i] - b[i]).powi(2)).sum::<f64>().sqrt()
}
