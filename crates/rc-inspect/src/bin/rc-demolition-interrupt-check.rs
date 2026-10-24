//! Isolated original-geometry interruption/reassignment checks after a genuine input prefix.
use rc_game::{demolition::Phase as Work, Game, Input, Phase, STEP};
use std::{env, fs, path::Path};
fn input(row: &[u8]) -> Input {
    let f = |i| f32::from_le_bytes(row[i..i + 4].try_into().unwrap()) as f64;
    let b = u32::from_le_bytes(row[16..20].try_into().unwrap());
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
    }
}
fn neutral(g: &mut Game) -> Result<(), String> {
    g.advance(STEP, Input::default())?;
    if g.state.phase != Phase::Playing {
        return Err("Fixture left Playing".into());
    }
    Ok(())
}
fn count(g: &Game, tag: &str) -> usize {
    g.state
        .demolition
        .as_ref()
        .unwrap()
        .effects
        .iter()
        .filter(|(_, e)| matches!(e,rc_game::demolition::Effect::Event{tag:t}if t==tag))
        .count()
}
fn run() -> Result<(), Box<dyn std::error::Error>> {
    let args: Vec<_> = env::args_os().collect();
    let scene = fs::read(&args[1])?;
    let rows = fs::read(&args[2])?;
    let out = Path::new(&args[3]);
    fs::create_dir_all(out)?;
    let identity = format!("{}:{}", scene.len(), rc_game::replay::fingerprint(&scene));
    let mut g = rc_game::bundle::decode(&scene)?;
    for row in rows[12..].chunks_exact(20) {
        g.advance(STEP, input(row))?;
        if g.state.demolition.as_ref().unwrap().flow.phase == Work::Starting {
            break;
        }
    }
    assert_eq!(
        g.state.demolition.as_ref().unwrap().flow.phase,
        Work::Starting
    );
    assert!(!g.state.demolition.as_ref().unwrap().flow.placed);
    let mut normal = rc_game::bundle::decode(&scene)?;
    for row in rows[12..].chunks_exact(20) {
        normal.advance(STEP, input(row))?;
    }
    assert_eq!(
        normal.state.phase,
        Phase::Won,
        "Original input-only A replay no longer completes"
    );
    assert!(normal.campaign_handoff()?.is_some());
    let prefix_tick = g.state.statistics.ticks;
    let scorch = g.state.rendezvous.as_ref().unwrap().pawn.body.position;
    let mut near = None;
    for delta in [
        [120., 0., 0.],
        [-120., 0., 0.],
        [0., 120., 0.],
        [0., -120., 0.],
    ] {
        let at = [scorch[0] + delta[0], scorch[1] + delta[1], scorch[2] + 120.];
        if let Some(at) =
            rc_game::mission_motion::ground(&g.world, at, g.profile.physics.extent, 300.)?
        {
            if g.wall_fraction(
                [at[0], at[1], at[2] + 60.],
                [scorch[0], scorch[1], scorch[2] + 55.],
            )? > 0.99
            {
                near = Some(at);
                break;
            }
        }
    }
    // Explicit fixture setup: nearby supported player position; isolate existing enemies.
    g.state.pawn.body.position = near.ok_or("No supported nearby player fixture")?;
    g.state.pawn.body.velocity = [0.; 3];
    g.state.pawn.body.grounded = true;
    g.state.health = 100;
    for e in &mut g.state.enemies {
        e.health = 0;
    }
    for w in &mut g.state.artillery.as_mut().unwrap().warriors {
        w.health = 0;
    }
    neutral(&mut g)?;
    g.aim_at([scorch[0], scorch[1], scorch[2] + 55.]);
    g.advance(
        STEP,
        Input {
            use_action: true,
            ..Default::default()
        },
    )?;
    assert_eq!(
        g.state.demolition.as_ref().unwrap().flow.phase,
        Work::Available,
        "Nearby Use did not cancel before placement"
    );
    assert_eq!(count(&g, "SpawnDemoDroids"), 1);
    neutral(&mut g)?;
    let charge = g.demolition.as_ref().unwrap().assets.charge;
    g.aim_at(charge);
    g.advance(
        STEP,
        Input {
            use_action: true,
            ..Default::default()
        },
    )?;
    assert!(g.state.demolition.as_ref().unwrap().flow.working());
    assert_eq!(count(&g, "SpawnDemoDroids"), 1);
    for _ in 0..120 {
        neutral(&mut g)?;
        if g.state.demolition.as_ref().unwrap().flow.placed {
            break;
        }
    }
    assert!(g.state.demolition.as_ref().unwrap().flow.placed);
    let before = g.state.demolition.as_ref().unwrap().flow.clone();
    g.state.squad.as_mut().unwrap().health = 0;
    neutral(&mut g)?;
    assert!(g.state.demolition.as_ref().unwrap().flow.paused);
    assert!(g.state.squad.as_ref().unwrap().down);
    let frozen = g.state.demolition.as_ref().unwrap().flow.clone();
    g.suspend();
    let paused = g.checkpoint(&identity)?;
    fs::write(out.join("paused-checkpoint.bin"), &paused)?;
    let mut cold = rc_game::bundle::decode(&scene)?;
    cold.restore_checkpoint(&paused, &identity)?;
    assert_eq!(cold.checkpoint(&identity)?, paused);
    assert_eq!(cold.render(640, 400)?, g.render(640, 400)?);
    let mut invalid = rc_game::bundle::decode(&scene)?;
    invalid.restore_checkpoint(&paused, &identity)?;
    invalid.state.demolition.as_mut().unwrap().flow.return_phase = Some(Work::Armed);
    let rejected = invalid.checkpoint(&identity)?;
    assert!(g.restore_checkpoint(&rejected, &identity).is_err());
    assert_eq!(g.checkpoint(&identity)?, paused);
    for _ in 0..60 {
        neutral(&mut g)?;
        neutral(&mut cold)?;
        assert_eq!(g.state.demolition.as_ref().unwrap().flow, frozen);
        assert_eq!(g.checkpoint(&identity)?, cold.checkpoint(&identity)?);
    }
    // Revive using the existing nearby hold interaction, rather than assigning health back.
    let at = g.state.rendezvous.as_ref().unwrap().pawn.body.position;
    g.aim_at(at);
    for _ in 0..600 {
        g.advance(
            STEP,
            Input {
                use_action: true,
                ..Default::default()
            },
        )?;
        if !g.state.squad.as_ref().unwrap().down {
            break;
        }
    }
    assert!(!g.state.squad.as_ref().unwrap().down);
    assert_eq!(g.state.demolition.as_ref().unwrap().flow, frozen);
    neutral(&mut g)?;
    // Explicit return-route fixture: relocate Scorch to an earlier authored route point.
    let destination = *g.demolition.as_ref().unwrap().assets.route.last().unwrap();
    let depart = *g
        .demolition
        .as_ref()
        .unwrap()
        .assets
        .route
        .iter()
        .rev()
        .find(|p| distance(**p, destination) > 200.)
        .ok_or("No earlier return route point")?;
    let r = g.state.rendezvous.as_mut().unwrap();
    r.pawn.body.position = depart;
    r.pawn.body.velocity = [0.; 3];
    r.pawn.body.grounded = true;
    g.aim_at(charge);
    g.advance(
        STEP,
        Input {
            use_action: true,
            ..Default::default()
        },
    )?;
    assert!(!g.state.demolition.as_ref().unwrap().flow.paused);
    assert_eq!(
        g.state.demolition.as_ref().unwrap().flow.phase,
        Work::Moving
    );
    assert!(g
        .state
        .demolition
        .as_ref()
        .unwrap()
        .flow
        .return_phase
        .is_some());
    g.suspend();
    let returning = g.checkpoint(&identity)?;
    fs::write(out.join("returning-checkpoint.bin"), &returning)?;
    cold = rc_game::bundle::decode(&scene)?;
    cold.restore_checkpoint(&returning, &identity)?;
    assert_eq!(cold.checkpoint(&identity)?, returning);
    let mut walked = 0.;
    let mut steps = 0;
    for _ in 0..1800 {
        let from = g.state.rendezvous.as_ref().unwrap().pawn.body.position;
        neutral(&mut g)?;
        neutral(&mut cold)?;
        let to = g.state.rendezvous.as_ref().unwrap().pawn.body.position;
        assert!(
            (to[0] - from[0]).hypot(to[1] - from[1]) <= 450. * STEP + 0.01,
            "Return moved farther than one walking step"
        );
        walked += distance(from, to);
        steps += 1;
        assert_eq!(g.checkpoint(&identity)?, cold.checkpoint(&identity)?);
        if steps % 60 == 0 {
            assert_eq!(cold.render(640, 400)?, g.render(640, 400)?);
        }
        if g.state.demolition.as_ref().unwrap().flow.phase == Work::Armed {
            break;
        }
    }
    assert_eq!(
        g.state.demolition.as_ref().unwrap().flow.phase,
        Work::Armed,
        "Return never completed setup"
    );
    assert!(walked > 100.);
    assert_eq!(count(&g, "SpawnDemoDroids"), 1);
    assert_eq!(count(&g, "DemoUnderway"), 1);
    assert_eq!(count(&g, "DemoChargeArmed"), 1);
    fs::write(
        out.join("validation.json"),
        serde_json::to_vec_pretty(
            &serde_json::json!({"passed":true,"scope":"Explicit isolated original-geometry fixtures after input prefix; nearby player, enemy health and earlier authored Scorch route position are assigned test setup, not an input-only mission","prefix_inputs":prefix_tick,"original_a_input_only_replay_complete":true,"pre_placement_cancel_and_reissue":true,"spawn_request_once":true,"incapacitation_pauses_work":true,"invalid_return_state_rejected_atomically":true,"paused_cold_restore_exact":true,"paused_steps":60,"real_hold_revive":true,"returning_cold_restore_exact":true,"return_continuation_steps":steps,"walked_distance":walked,"placement_once":true,"armed_once":true,"before_work":before,"final_work":g.state.demolition.as_ref().unwrap().flow}),
        )?,
    )?;
    println!(
        "Demolition interruption, revive and physical return passed: {steps} steps, {walked} units"
    );
    Ok(())
}
fn distance(a: [f64; 3], b: [f64; 3]) -> f64 {
    (0..3).map(|i| (a[i] - b[i]).powi(2)).sum::<f64>().sqrt()
}

fn main() -> Result<(), Box<dyn std::error::Error>> {
    // Several simultaneous complete Game fixtures exceed the Windows main stack.
    std::thread::Builder::new()
        .name("marker-fixtures".into())
        .stack_size(16 * 1024 * 1024)
        .spawn(|| run().map_err(|e| e.to_string()))?
        .join()
        .map_err(|_| "Demolition fixture worker panicked")??;
    Ok(())
}
