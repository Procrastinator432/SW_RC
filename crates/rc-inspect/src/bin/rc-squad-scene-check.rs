//! End-to-end normal input acceptance for the live demolition scene.
use rc_game::{Input, Phase, STEP};
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
fn engage(g: &mut rc_game::Game, out: &mut Vec<u8>) -> Result<bool, String> {
    let eye = g.eye();
    let at = g
        .state
        .artillery
        .as_ref()
        .unwrap()
        .warriors
        .iter()
        .filter(|w| w.health > 0)
        .map(|w| w.pawn.body.position)
        .find(|p| {
            rc_distance(*p, eye) < 2200. && g.wall_fraction(eye, *p).is_ok_and(|f| f >= 0.999)
        });
    if let Some(at) = at {
        let look = aim(g, at);
        tick(
            g,
            out,
            Input {
                look,
                fire: true,
                ..Input::default()
            },
        )?;
        Ok(true)
    } else {
        Ok(false)
    }
}
fn rc_distance(a: [f64; 3], b: [f64; 3]) -> f64 {
    (0..3).map(|i| (a[i] - b[i]).powi(2)).sum::<f64>().sqrt()
}
fn walk(g: &mut rc_game::Game, out: &mut Vec<u8>, target: [f64; 3]) -> Result<(), String> {
    for _ in 0..1800 {
        if g.state.phase == Phase::Won {
            return Ok(());
        }
        if engage(g, out)? {
            continue;
        }
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
#[allow(dead_code)]
fn danger(scene: &Path, input: &[u8], out: &Path) -> Result<(), Box<dyn std::error::Error>> {
    let mut g = rc_game::bundle::decode(&fs::read(scene)?)?;
    let mut replay = vec![];
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

    let area = g.spire.as_ref().unwrap().assets.route[0];
    walk(&mut g, &mut replay, area)?;
    let result = walk(&mut g, &mut replay, [4600., -10650., -2005.]);
    if let Err(e) = result {
        if g.state.phase != Phase::Lost {
            return Err(e.into());
        }
    }
    for _ in 0..1000 {
        if g.state.phase == Phase::Lost {
            break;
        }
        let i = Input::default();
        g.advance(STEP, i)?;
        record(&mut replay, i);
    }
    assert_eq!(
        g.state.phase,
        Phase::Lost,
        "Standing in the impact zone did not cause defeat"
    );
    let damage = &g.state.artillery.as_ref().unwrap().damage;
    assert!(damage.iter().any(|d| d.victim == -1 && d.amount > 0));
    frame(&g, out, "bombardment-defeat")?;
    fs::write(
        out.join("danger-validation.json"),
        serde_json::to_vec_pretty(
            &serde_json::json!({"passed":true,"phase":g.state.phase,"health":g.state.health,"ticks":g.state.statistics.ticks,"damage":damage,"normal_inputs_only":true}),
        )?,
    )?;
    let mut bytes = b"RCINPUT1".to_vec();
    bytes.extend(((replay.len() / 20) as u32).to_le_bytes());
    bytes.extend(replay);
    fs::write(out.join("danger-input-replay.bin"), bytes)?;
    Ok(())
}
fn main() -> Result<(), Box<dyn std::error::Error>> {
    let args: Vec<_> = env::args_os().collect();
    if args.len() != 4 {
        return Err("usage: rc-squad-scene-check SCENE DEMOLITION_INPUT OUTPUT".into());
    }
    let out = Path::new(&args[3]);
    fs::create_dir_all(out)?;
    let mut g = rc_game::bundle::decode(&fs::read(&args[1])?)?;
    let initial = serde_json::to_vec(&g.state)?;
    let initial_frame = g.render_frame(320, 200)?.gpu_packet(&g.scene);
    let world = rc_render::gpu::world(&g.scene)?;
    let door_from = [3200., -15565., -2005.];
    let door_to = [3200., -16152., -2005.];
    let blocked = |g: &rc_game::Game| -> Result<bool, String> {
        Ok(g.world
            .sweep_motion(door_from, door_to, g.profile.physics.extent)?
            .contacts
            .iter()
            .any(|c| c.object == "Dynamic.34" || c.object == "Dynamic.35"))
    };
    assert!(blocked(&g)?);
    let barrier_blocked = |g: &rc_game::Game| -> Result<bool, String> {
        Ok(g.world
            .sweep_motion(
                [4820., -10500., -2005.],
                [4262., -11306., -2005.],
                g.profile.physics.extent,
            )?
            .contacts
            .iter()
            .any(|c| ["Dynamic.860", "Dynamic.861", "Dynamic.862"].contains(&c.object.as_str())))
    };
    assert!(barrier_blocked(&g)?);
    let input = fs::read(&args[2])?;
    assert!(input.starts_with(b"RCINPUT1") && (input.len() - 12) % 20 == 0);
    assert_eq!(
        u32::from_le_bytes(input[8..12].try_into()?) as usize,
        (input.len() - 12) / 20
    );
    let mut replay = vec![];
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
    assert!(
        g.state
            .demolition
            .as_ref()
            .unwrap()
            .flow
            .objective_completed
    );
    assert!(!g.state.spire.as_ref().unwrap().flow.barrier_open);
    let route = g.spire.as_ref().unwrap().assets.route.clone();
    let exit = g.spire.as_ref().unwrap().assets.flow.exit.clone();
    let safe = g.state.pawn.body.position;
    let viewpoint = g.spire.as_ref().unwrap().assets.route[2];
    let mut watched = false;
    let mut barrier_tick = None;
    for target in route {
        walk(&mut g, &mut replay, target)?;
        if g.state.spire.as_ref().unwrap().flow.entered && !watched {
            watched = true;
            walk(&mut g, &mut replay, safe)?;
            let look = aim(&g, [2000., -12000., -1400.]);
            tick(
                &mut g,
                &mut replay,
                Input {
                    look,
                    ..Input::default()
                },
            )?;
            for i in 0..1500 {
                if !engage(&mut g, &mut replay)? {
                    tick(&mut g, &mut replay, Input::default())?;
                }
                if i == 600 {
                    walk(&mut g, &mut replay, target)?;
                    walk(&mut g, &mut replay, viewpoint)?;
                    let look = aim(&g, [1000., -12500., -700.]);
                    tick(
                        &mut g,
                        &mut replay,
                        Input {
                            look,
                            ..Input::default()
                        },
                    )?;
                }
                if [240, 480, 720, 960, 1200].contains(&i) {
                    frame(&g, out, &format!("artillery-{i}"))?;
                }
            }
            walk(&mut g, &mut replay, target)?;
        }
        if g.state.spire.as_ref().unwrap().flow.entered
            && !g.state.spire.as_ref().unwrap().flow.barrier_open
        {
            let look = aim(&g, [4600., -10900., -2050.]);
            tick(
                &mut g,
                &mut replay,
                Input {
                    look,
                    ..Input::default()
                },
            )?;
            frame(&g, out, "artillery-barricade-closed")?;
            for _ in 0..2400 {
                tick(&mut g, &mut replay, Input::default())?;
                if g.state.spire.as_ref().unwrap().flow.barrier_open {
                    barrier_tick = Some(g.state.statistics.ticks);
                    break;
                }
            }
            assert!(g.state.spire.as_ref().unwrap().flow.barrier_open);
            frame(&g, out, "artillery-passage-open")?;
        }
        if g.state.phase == Phase::Won {
            break;
        }
    }
    assert_eq!(g.state.phase, Phase::Won);
    let at = g.state.pawn.body.position;
    let extent = g.profile.physics.extent;
    assert!(
        (at[0] - exit.position[0]).hypot(at[1] - exit.position[1])
            < exit.radius + extent[0].max(extent[1])
            && (at[2] - exit.position[2]).abs() < exit.height + extent[2]
    );
    assert!(!blocked(&g)?);
    assert!(!barrier_blocked(&g)?);
    let state = g.state.spire.as_ref().unwrap();
    assert!(state.flow.completed && state.flow.door_open && state.flow.barrier_open);
    assert!(
        state.flow.conversation.next == g.spire.as_ref().unwrap().assets.flow.conversation.len()
            && state.flow.conversation.remaining < 1e-9
    );
    let events = &state.effects;
    for id in [34, 35, 860, 861, 862] {
        assert_eq!(
            events
                .iter()
                .filter(|(_, e)| matches!(e,rc_game::spire::Effect::Release(v) if *v==id))
                .count(),
            1
        );
    }
    assert_eq!(events.iter().filter(|(_,e)|matches!(e,rc_game::spire::Effect::Exit(url) if url=="GEO_01B.ctm#playerstart")).count(),1);
    let a = g.state.artillery.as_ref().unwrap();
    fs::write(
        out.join("artillery-state.json"),
        serde_json::to_vec_pretty(a)?,
    )?;
    assert!(a.destroyed_tick.is_some());
    for label in ["SpawnWarriorAttack", "SpiderDroidDestroyed"] {
        assert_eq!(a.effects.iter().filter(|(_, e)| e == label).count(), 1);
    }
    assert_eq!(
        a.effects
            .iter()
            .filter(|(_, e)| e == "CTEffects.SpiderDroid_BeamAutoDestroy")
            .count(),
        2
    );
    assert_eq!(
        a.effects
            .iter()
            .filter(|(_, e)| e == "AttackWarriorDead")
            .count(),
        3
    );
    assert_eq!(a.impacts.len(), 4);
    assert_eq!(a.warrior_factory.spawned, 3);
    assert!(a.warriors.iter().all(|w| w.health == 0));
    assert_eq!(
        g.state.statistics.kills + g.state.squad.as_ref().unwrap().kills,
        8
    );
    frame(&g, out, "spire-exit-complete")?;
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
    assert!(blocked(&g)?);
    assert!(barrier_blocked(&g)?);
    assert_eq!(rc_render::gpu::world(&g.scene)?, world);

    fs::write(
        out.join("validation.json"),
        serde_json::to_vec_pretty(
            &serde_json::json!({"passed":true,"squad_integrated":true,"input_driven_objectives":[0,1,2,3,4,5,6],"complete_state":complete,"restart_exact":true,"restart_frame_and_closed_collision":true,"static_gpu_packet_unchanged":true,"closed_barricade_blocks":true,"released_barricade_clear_sweep":true,"closed_door_blocks":true,"released_door_clear_sweep":true,"barrier_tick":barrier_tick,"original_exit_trigger_entered":true,"next_url":"GEO_01B.ctm#playerstart","scope":"Original artillery clips/notifies, four swept projectiles and three factory Warriors; explicit effect/AI adapters, next map loading open"}),
        )?,
    )?;
    println!(
        "Contiguous objectives0-6 passed at tick{}, original exit reached, restart exact",
        complete["statistics"]["ticks"]
    );
    Ok(())
}
