//! Real scene root translation, collision refusal, interruption and deterministic combat.
use rc_game::{bundle, lunge, navigation, replay, Input, Phase, STEP};
use std::{env, fs, path::Path};
fn main() -> Result<(), Box<dyn std::error::Error>> {
    let args: Vec<_> = env::args_os().collect();
    if args.len() != 3 {
        return Err("usage: rc-play-lunge-check SCENE OUTPUT_DIRECTORY".into());
    }
    let bytes = fs::read(&args[1])?;
    let out = Path::new(&args[2]);
    fs::create_dir_all(out)?;
    let mut game = bundle::decode(&bytes)?;
    let initial = replay::state_fingerprint(&game)?;
    let origin = game.state.enemies[0].position;
    let mut max_height = origin[2];
    let mut first_hit = None;
    let mut moved = 0.;
    for _ in 0..900 {
        let before = game.state.enemies[0].position;
        game.advance(STEP, Input::default())?;
        let state = game.state.combat.as_ref().unwrap();
        let lunge = state.lunge.as_ref().unwrap();
        let position = game.state.enemies[0].position;
        if game.state.statistics.ticks == 25 {
            let mut ppm = b"P6\n320 200\n255\n".to_vec();
            for p in game.render(320, 200)? {
                ppm.extend([(p >> 16) as u8, (p >> 8) as u8, p as u8]);
            }
            fs::write(out.join("airborne.ppm"), ppm)?;
        }

        max_height = max_height.max(position[2]);
        moved += (position[0] - before[0]).hypot(position[1] - before[1]);
        let sweep = game
            .world
            .sweep_motion(before, position, navigation::EXTENT)?;
        assert!(sweep.complete && sweep.contacts.is_empty());
        if let Some(motion) = &lunge.motions[0] {
            let expected =
                lunge::point(&game.visuals.as_ref().unwrap().enemy, motion, motion.frame)?;
            assert_eq!(position, expected);
        }
        if first_hit.is_none() && state.melee.as_ref().unwrap().player.shields < 100. {
            first_hit = Some(game.state.statistics.ticks);
        }
        if lunge.completed > 0 {
            break;
        }
    }
    let state = game
        .state
        .combat
        .as_ref()
        .unwrap()
        .lunge
        .as_ref()
        .unwrap()
        .clone();
    assert!(state.started > 0 && state.completed > 0 && moved > 50. && max_height > origin[2] + 5.);
    assert!(first_hit.is_some());
    // Damage cannot arrive from a killed or interrupted motion; vertical position settles safely.
    for kill in [false, true] {
        let mut p = bundle::decode(&bytes)?;
        let enemy = p.state.enemies[0].position;
        p.set_encounter(&[enemy], p.exit)?;
        for _ in 0..120 {
            p.advance(STEP, Input::default())?;
            let s = p.state.combat.as_ref().unwrap();
            if s.lunge.as_ref().is_some_and(|l| l.motions[0].is_some())
                && p.state.enemies[0].position[2] > enemy[2] + 5.
            {
                break;
            }
        }
        assert!(p
            .state
            .combat
            .as_ref()
            .unwrap()
            .lunge
            .as_ref()
            .unwrap()
            .motions[0]
            .is_some());
        if kill {
            p.state.enemies[0].health = 0;
        } else {
            p.state.enemies[0].flash = 2.;
        }
        p.advance(STEP, Input::default())?;
        let s = p.state.combat.as_ref().unwrap();
        assert!(s.lunge.as_ref().unwrap().motions[0].is_none());
        assert_eq!(s.lunge.as_ref().unwrap().interrupted, 1);
        assert!(s.melee.as_ref().unwrap().attacks[0].active.is_none());
        assert!(p
            .world
            .floor_contact(p.state.enemies[0].position, navigation::EXTENT, 4., 0.7)?
            .is_some());
        assert_eq!(s.melee.as_ref().unwrap().player.shields, 100.);
    }
    // Find a real wall heading that blocks the original trajectory.
    let mesh = &game.visuals.as_ref().unwrap().enemy;
    let clip = mesh.clip("Lunge")?;
    let mut blocked = None;
    for node in &game
        .combat
        .as_ref()
        .unwrap()
        .navigation
        .as_ref()
        .unwrap()
        .nodes
    {
        for angle in 0..32 {
            let m = lunge::Motion {
                clip,
                origin: node.position,
                yaw: angle as f64 * std::f64::consts::TAU / 32.,
                frame: 0.,
            };
            if !lunge::feasible(&game.world, mesh, &m)? {
                blocked = Some((node.name.clone(), m));
                break;
            }
        }
        if blocked.is_some() {
            break;
        }
    }
    assert!(blocked.is_some());
    let blocked_motion = blocked.as_ref().unwrap().1.clone();
    for (clip_name, collision) in [("Lunge2", false), ("Lunge", true)] {
        let mut p = bundle::decode(&bytes)?;
        let origin = if collision {
            blocked_motion.origin
        } else {
            p.state.enemies[0].position
        };
        p.set_encounter(&[origin], p.exit)?;
        let mesh = &p.visuals.as_ref().unwrap().enemy;
        let index = mesh.clip(clip_name)?;
        let motion = lunge::Motion {
            clip: index,
            origin,
            yaw: if collision {
                blocked_motion.yaw
            } else {
                (p.state.pawn.body.position[1] - origin[1])
                    .atan2(p.state.pawn.body.position[0] - origin[0])
            },
            frame: 0.,
        };
        if !collision {
            assert!(lunge::feasible(&p.world, mesh, &motion)?);
        }
        let combat = p.state.combat.as_mut().unwrap();
        combat.melee.as_mut().unwrap().attacks[0].active = Some(if collision { 2 } else { 3 });
        combat.melee.as_mut().unwrap().attacks[0].yaw = motion.yaw;
        let mut state = lunge::State::new(1, 1);
        state.motions[0] = Some(motion);
        combat.lunge = Some(state);
        for _ in 0..240 {
            p.advance(STEP, Input::default())?;
            let state = p.state.combat.as_ref().unwrap().lunge.as_ref().unwrap();
            if state.completed > 0 || state.blocked > 0 {
                break;
            }
        }
        let state = p.state.combat.as_ref().unwrap().lunge.as_ref().unwrap();
        if collision {
            assert_eq!(state.blocked, 1);
            assert_eq!(state.completed, 0);
        } else {
            assert_eq!(state.completed, 1);
            assert_eq!(state.blocked, 0);
        }
        assert!(p
            .world
            .floor_contact(p.state.enemies[0].position, navigation::EXTENT, 4., 0.7)?
            .is_some());
    }

    game.restart();
    assert_eq!(initial, replay::state_fingerprint(&game)?);
    let mut cadences = vec![];
    for frame in [STEP / 2., STEP * 2., STEP * 6.] {
        let mut p = bundle::decode(&bytes)?;
        for _ in 0..(12. / frame).round() as usize {
            p.advance(frame, Input::default())?;
        }
        cadences.push(serde_json::to_vec(&p.state)?);
    }
    assert!(cadences.windows(2).all(|p| p[0] == p[1]));
    for _ in 0..3600 {
        game.advance(STEP, Input::default())?;
        if game.state.phase == Phase::Lost {
            break;
        }
    }
    assert_eq!(game.state.phase, Phase::Lost);
    let mut p = bundle::decode(&bytes)?;
    for i in 0..p.state.enemies.len() {
        for _ in 0..200 {
            if p.state.enemies[i].health == 0 {
                break;
            }
            p.aim_at(p.state.enemies[i].position);
            p.advance(
                STEP,
                Input {
                    fire: true,
                    ..Input::default()
                },
            )?;
        }
        assert_eq!(p.state.enemies[i].health, 0);
    }
    fs::write(
        out.join("lunge-validation.json"),
        serde_json::to_vec_pretty(
            &serde_json::json!({"passed":true,"lunge2_completed":true,"runtime_collision_cancels_and_settles":true,"original_root_positions_exact_in_host":true,"first_hit_tick":first_hit,"movement":moved,"height":max_height-origin[2],"state":state,"blocked_original_map_trajectory":blocked,"death_and_hit_interrupt_settle":true,"cadence_120_30_10_equal":true,"reset_rng_and_motion_exact":true,"defeat":true,"projectile_three_kills":true}),
        )?,
    )?;
    println!("Lunge root curve / airborne motion / notify hit / wall refusal / interruption / cadence / defeat / 3 kills passed");
    Ok(())
}
