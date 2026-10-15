//! End-to-end normal input acceptance for the live demolition scene.
use rc_game::{demolition::Phase as Stage, Input, Phase, STEP};
use std::{env, fs, path::Path};
fn record(out: &mut Vec<u8>, i: Input) {
    for f in [i.forward, i.strafe, i.look[0], i.look[1]] {
        out.extend((f as f32).to_le_bytes());
    }
    let b = u32::from(i.jump)
        | (u32::from(i.fire) * 2)
        | (u32::from(i.reload) * 4)
        | (u32::from(i.walking) * 8)
        | (u32::from(i.use_action) * 16)
        | (u32::from(i.throw_grenade) * 32)
        | (u32::from(i.crouch) * 64);
    out.extend(b.to_le_bytes());
}
fn tick(g: &mut rc_game::Game, out: &mut Vec<u8>, i: Input) -> Result<(), String> {
    g.advance(STEP, i)?;
    record(out, i);
    if g.state.phase == Phase::Lost {
        return Err(format!("Player died at tick{}", g.state.statistics.ticks));
    }
    Ok(())
}
fn aim(g: &rc_game::Game, target: [f64; 3]) -> [f64; 2] {
    let eye = g.eye();
    let d: [f64; 3] = std::array::from_fn(|i| target[i] - eye[i]);
    let yaw = d[1].atan2(d[0]);
    let pitch = d[2].atan2(d[0].hypot(d[1]));
    let turn = (g.state.yaw - yaw + std::f64::consts::PI).rem_euclid(std::f64::consts::TAU)
        - std::f64::consts::PI;
    [
        (turn as f32) as f64,
        ((g.state.pitch - pitch) as f32) as f64,
    ]
}
fn walk(g: &mut rc_game::Game, out: &mut Vec<u8>, target: [f64; 3]) -> Result<(), String> {
    for _ in 0..1800 {
        let p = g.state.pawn.body.position;
        let d = (0..3)
            .map(|i| (p[i] - target[i]).powi(2))
            .sum::<f64>()
            .sqrt();
        if d < 25. {
            return Ok(());
        }
        let yaw = (target[1] - p[1]).atan2(target[0] - p[0]);
        let turn = (g.state.yaw - yaw + std::f64::consts::PI).rem_euclid(std::f64::consts::TAU)
            - std::f64::consts::PI;
        tick(
            g,
            out,
            Input {
                forward: ((d / 90.).min(1.) as f32) as f64,
                look: [(turn as f32) as f64, (g.state.pitch as f32) as f64],
                ..Input::default()
            },
        )?;
    }
    Err(format!(
        "Blocked player route to{target:?}; at{:?}",
        g.state.pawn.body.position
    ))
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
        return Err("usage: rc-demolition-scene-check SCENE SCORCH_INPUT OUTPUT".into());
    }
    let out = Path::new(&args[3]);
    fs::create_dir_all(out)?;
    let mut g = rc_game::bundle::decode(&fs::read(&args[1])?)?;
    let initial = serde_json::to_vec(&g.state)?;
    let initial_frame = g.render_frame(320, 200)?.gpu_packet(&g.scene);
    let world = rc_render::gpu::world(&g.scene)?;
    let mut replay = vec![];
    let input = fs::read(&args[2])?;
    assert!(input.starts_with(b"RCINPUT1") && (input.len() - 12) % 20 == 0);
    for row in input[12..].chunks_exact(20) {
        let f = |i| f32::from_le_bytes(row[i..i + 4].try_into().unwrap()) as f64;
        let b = u32::from_le_bytes(row[16..20].try_into().unwrap());
        tick(
            &mut g,
            &mut replay,
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
            },
        )?;
    }
    assert_eq!(g.state.phase, Phase::Playing);
    assert_eq!(
        g.state.demolition.as_ref().unwrap().flow.phase,
        Stage::Available
    );
    let route = g.demolition.as_ref().unwrap().assets.route.clone();
    let charge = g.demolition.as_ref().unwrap().assets.charge;
    let yaw = g.demolition.as_ref().unwrap().assets.yaw;
    let inside = [
        charge[0] - yaw.cos() * 170.,
        charge[1] - yaw.sin() * 170.,
        route.last().unwrap()[2],
    ];
    let closed = g
        .world
        .sweep_motion(*route.last().unwrap(), inside, [40., 40., 84.])?;
    assert!(
        closed.contacts.iter().any(|c| c.object == "Dynamic.851"),
        "Original door was not blocking passage"
    );
    for p in &route[..route.len() - 1] {
        walk(&mut g, &mut replay, *p)?;
    }
    tick(
        &mut g,
        &mut replay,
        Input {
            look: [std::f64::consts::PI as f32 as f64, 0.],
            use_action: true,
            ..Input::default()
        },
    )?;
    assert_eq!(
        g.state.demolition.as_ref().unwrap().flow.phase,
        Stage::Available,
        "Looking away issued command"
    );
    tick(&mut g, &mut replay, Input::default())?;
    let look = aim(&g, charge);
    tick(
        &mut g,
        &mut replay,
        Input {
            look,
            use_action: true,
            ..Input::default()
        },
    )?;
    assert_eq!(
        g.state.demolition.as_ref().unwrap().flow.phase,
        Stage::Moving,
        "Marker command failed: {:?}",
        g.state.pawn.body.position
    );
    for _ in 0..2400 {
        tick(&mut g, &mut replay, Input::default())?;
        if g.state.demolition.as_ref().unwrap().flow.phase == Stage::Setting {
            walk(&mut g, &mut replay, route[0])?;
            let mut at = g.state.rendezvous.as_ref().unwrap().pawn.body.position;
            at[2] += 35.;
            let look = aim(&g, at);
            tick(
                &mut g,
                &mut replay,
                Input {
                    look,
                    ..Input::default()
                },
            )?;
            frame(&g, out, "scorch-setting")?;
            break;
        }
    }
    for _ in 0..1800 {
        tick(&mut g, &mut replay, Input::default())?;
        if g.state.demolition.as_ref().unwrap().flow.phase == Stage::Armed {
            break;
        }
    }
    assert_eq!(
        g.state.demolition.as_ref().unwrap().flow.phase,
        Stage::Armed
    );
    for _ in 0..1200 {
        tick(&mut g, &mut replay, Input::default())?;
    }
    assert_eq!(
        g.state.demolition.as_ref().unwrap().flow.phase,
        Stage::Armed,
        "Charge exploded without fire"
    );
    assert!(
        !g.state
            .demolition
            .as_ref()
            .unwrap()
            .flow
            .objective_completed
    );
    for point in &route[1..route.len() - 1] {
        walk(&mut g, &mut replay, *point)?;
    }
    let look = aim(&g, charge);
    tick(
        &mut g,
        &mut replay,
        Input {
            look,
            use_action: true,
            ..Input::default()
        },
    )?;
    assert!(
        g.state.demolition.as_ref().unwrap().flow.plunger_selected,
        "Zünder selection failed: {:?} eye{:?}",
        charge,
        g.eye()
    );
    frame(&g, out, "plunger-selected")?;
    for retreat in route[..route.len() - 1].iter().rev() {
        walk(&mut g, &mut replay, *retreat)?;
    }
    assert!(
        (0..3)
            .map(|i| (g.state.pawn.body.position[i] - charge[i]).powi(2))
            .sum::<f64>()
            .sqrt()
            > 350.
    );
    let old_shots = g.state.statistics.shots;
    let fire_tick = g.state.statistics.ticks + 1;
    let look = aim(&g, charge);
    tick(
        &mut g,
        &mut replay,
        Input {
            look,
            fire: true,
            ..Input::default()
        },
    )?;
    for _ in 0..180 {
        tick(&mut g, &mut replay, Input::default())?;
        if g.state
            .demolition
            .as_ref()
            .unwrap()
            .explosion_tick
            .is_some()
        {
            frame(&g, out, "bunker-explosion")?;
            break;
        }
    }
    assert!(g
        .state
        .demolition
        .as_ref()
        .unwrap()
        .explosion_tick
        .is_some());
    let fire_delay = g.state.demolition.as_ref().unwrap().explosion_tick.unwrap() - fire_tick;
    assert!(
        (49..=51).contains(&fire_delay),
        "Explosion did not follow original FireWeapon notify"
    );
    assert_eq!(
        g.state.statistics.shots, old_shots,
        "Detonator fired blaster"
    );
    assert!(g.state.demolition.as_ref().unwrap().navigation_open);
    let opened = g
        .world
        .sweep_motion(*route.last().unwrap(), inside, [40., 40., 84.])?;
    assert!(
        !opened.contacts.iter().any(|c| c.object == "Dynamic.851"),
        "Released door blocks passage: {opened:?}"
    );
    for point in &route[1..] {
        walk(&mut g, &mut replay, *point)?;
    }
    walk(&mut g, &mut replay, inside)?;
    frame(&g, out, "bunker-passage")?;
    assert_eq!(
        g.dynamic
            .as_ref()
            .unwrap()
            .objects
            .last()
            .unwrap()
            .released_hulls
            .as_ref()
            .unwrap()
            .hulls
            .len(),
        23
    );
    for _ in 0..600 {
        if g.state.phase != Phase::Playing {
            break;
        }
        tick(&mut g, &mut replay, Input::default())?;
    }
    assert_eq!(g.state.phase, Phase::Won);
    assert!(
        g.state
            .demolition
            .as_ref()
            .unwrap()
            .flow
            .objective_completed
    );
    frame(&g, out, "demolition-complete")?;
    assert_eq!(rc_render::gpu::world(&g.scene)?, world);
    let complete = serde_json::to_value(&g.state)?;
    let mut bytes = b"RCINPUT1".to_vec();
    bytes.extend(((replay.len() / 20) as u32).to_le_bytes());
    bytes.extend(replay);
    fs::write(out.join("input-replay.bin"), bytes)?;
    g.restart();
    assert_eq!(serde_json::to_vec(&g.state)?, initial);
    assert_eq!(
        g.render_frame(320, 200)?.gpu_packet(&g.scene),
        initial_frame
    );
    assert!(g
        .world
        .sweep_motion(*route.last().unwrap(), inside, [40., 40., 84.])?
        .contacts
        .iter()
        .any(|c| c.object == "Dynamic.851"));
    assert_eq!(rc_render::gpu::world(&g.scene)?, world);
    fs::write(
        out.join("validation.json"),
        serde_json::to_vec_pretty(
            &serde_json::json!({"passed":true,"input_driven_objectives":[0,1,2,3,4,5],"complete_state":complete,"restart_exact":true,"restart_frame_and_closed_collision":true,"static_gpu_packet_unchanged":true,"closed_bunker_blocks":true,"released_passage_walked":true,"incorrect_aim_rejected":true,"armed_without_fire_seconds":20,"fire_notify_delay_ticks":fire_delay,"scope":"Selected demolition objective; factory/AI limits documented separately"}),
        )?,
    )?;
    println!(
        "Contiguous objectives0-5 passed, tick{}, player survived detonation, restart exact",
        complete["statistics"]["ticks"]
    );
    Ok(())
}
