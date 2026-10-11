use rc_game::{bundle, jump, replay, Input, Phase, STEP};
use std::{env, fs, path::Path};
fn main() -> Result<(), Box<dyn std::error::Error>> {
    let args: Vec<_> = env::args_os().collect();
    if args.len() != 3 {
        return Err("usage: rc-play-jump-check SCENE OUTPUT".into());
    }
    let b = fs::read(&args[1])?;
    let out = Path::new(&args[2]);
    fs::create_dir_all(out)?;
    let mut g = bundle::decode(&b)?;
    let initial = replay::state_fingerprint(&g)?;
    // Natural post-melee draws, then full start/flight/land/wait execution.
    // Neutral recipient flags give the selected enemy time to complete its arc.
    for v in g
        .combat
        .as_mut()
        .unwrap()
        .game_rules
        .as_mut()
        .unwrap()
        .neutral_volumes
        .values_mut()
    {
        *v = true;
    }
    let mut stages = std::collections::BTreeSet::new();
    let mut first_start = None;
    let mut first_land = None;
    for _ in 0..1800 {
        g.advance(STEP, Input::default())?;
        let s = g.state.combat.as_ref().unwrap().jump.as_ref().unwrap();
        if first_start.is_none() && s.started > 0 {
            first_start = Some(g.state.statistics.ticks);
        }
        if first_land.is_none() && s.landed > 0 {
            first_land = Some(g.state.statistics.ticks);
        }
        for m in s.motions.iter().flatten() {
            stages.insert(format!("{:?}", m.stage));
        }
        if s.completed > 0 {
            break;
        }
    }
    let natural = serde_json::to_value(g.state.combat.as_ref().unwrap().jump.as_ref().unwrap())?;
    println!(
        "Natural jump: started={}, completed={}, returns={}, stages={stages:?}",
        natural["started"], natural["completed"], natural["returns"]
    );
    assert!(natural["completed"].as_u64().unwrap() > 0);
    assert_eq!(stages.len(), 4);
    assert!(natural["max_height"].as_f64().unwrap() > 50.);
    assert!(natural["returns"].as_u64().unwrap() > 0);
    g.restart();
    assert_eq!(initial, replay::state_fingerprint(&g)?);
    // Reset and cadence also include RNG, pending post-melee flag and reservation.
    let mut cadence = vec![];
    for dt in [STEP / 2., STEP * 2., STEP * 6.] {
        let mut g = bundle::decode(&b)?;
        for v in g
            .combat
            .as_mut()
            .unwrap()
            .game_rules
            .as_mut()
            .unwrap()
            .neutral_volumes
            .values_mut()
        {
            *v = true;
        }
        for _ in 0..(12. / dt).round() as usize {
            g.advance(dt, Input::default())?;
        }
        cadence.push(serde_json::to_vec(&g.state)?);
    }
    assert!(cadence.windows(2).all(|w| w[0] == w[1]));
    // Explicit host request isolates interruptions without altering production RNG.
    let mut interruptions = vec![];
    for kill in [false, true] {
        let mut g = bundle::decode(&b)?;
        g.state
            .combat
            .as_mut()
            .unwrap()
            .jump
            .as_mut()
            .unwrap()
            .requests[0] = true;
        for _ in 0..180 {
            g.advance(STEP, Input::default())?;
            if g.state
                .combat
                .as_ref()
                .unwrap()
                .jump
                .as_ref()
                .unwrap()
                .motions[0]
                .as_ref()
                .is_some_and(|m| m.stage == jump::Stage::Flight)
            {
                break;
            }
        }
        assert!(g
            .state
            .combat
            .as_ref()
            .unwrap()
            .jump
            .as_ref()
            .unwrap()
            .motions[0]
            .is_some());
        if kill {
            g.state.enemies[0].health = 0;
        } else {
            g.state.enemies[0].flash = 0.2;
        }
        g.advance(STEP, Input::default())?;
        let s = g.state.combat.as_ref().unwrap().jump.as_ref().unwrap();
        assert!(s.motions[0].is_none() && !s.requests[0] && s.interrupted == 1);
        assert!(rc_game::navigation::ground(&g.world, g.state.enemies[0].position, 4.)?.is_some());
        interruptions.push(serde_json::json!({"death":kill,"state":s}));
    }
    let mut blocked = bundle::decode(&b)?;
    blocked
        .state
        .combat
        .as_mut()
        .unwrap()
        .jump
        .as_mut()
        .unwrap()
        .requests[0] = true;
    for _ in 0..180 {
        blocked.advance(STEP, Input::default())?;
        if blocked
            .state
            .combat
            .as_ref()
            .unwrap()
            .jump
            .as_ref()
            .unwrap()
            .motions[0]
            .as_ref()
            .is_some_and(|m| m.stage == jump::Stage::Flight)
        {
            break;
        }
    }
    // Prepared unexpected flight displacement meets the real floor far from target.
    let m = blocked
        .state
        .combat
        .as_mut()
        .unwrap()
        .jump
        .as_mut()
        .unwrap()
        .motions[0]
        .as_mut()
        .unwrap();
    m.velocity = [0., 0., -10000.];
    blocked.advance(STEP, Input::default())?;
    assert_eq!(
        blocked
            .state
            .combat
            .as_ref()
            .unwrap()
            .jump
            .as_ref()
            .unwrap()
            .blocked,
        1
    );
    assert!(blocked
        .state
        .combat
        .as_ref()
        .unwrap()
        .jump
        .as_ref()
        .unwrap()
        .motions[0]
        .is_none());
    assert!(
        rc_game::navigation::ground(&blocked.world, blocked.state.enemies[0].position, 4.)?
            .is_some()
    );
    let mut refused = bundle::decode(&b)?;
    refused
        .combat
        .as_mut()
        .unwrap()
        .jump
        .as_mut()
        .unwrap()
        .nodes
        .clear();
    refused
        .state
        .combat
        .as_mut()
        .unwrap()
        .jump
        .as_mut()
        .unwrap()
        .requests[0] = true;
    refused.advance(STEP, Input::default())?;
    assert_eq!(
        refused
            .state
            .combat
            .as_ref()
            .unwrap()
            .jump
            .as_ref()
            .unwrap()
            .no_destination,
        1
    );
    let mut rollback = bundle::decode(&b)?;
    rollback
        .state
        .combat
        .as_mut()
        .unwrap()
        .jump
        .as_mut()
        .unwrap()
        .requests[0] = true;
    rollback
        .world
        .actors
        .push(rc_package::world_collision::BodyActor {
            name: "Incomplete test collider".into(),
            shape: rc_package::world_collision::BodyShape::Unsupported("test".into()),
        });
    let before = replay::state_fingerprint(&rollback)?;
    assert!(rollback.advance(STEP, Input::default()).is_err());
    assert_eq!(before, replay::state_fingerprint(&rollback)?);
    let mut reserved = bundle::decode(&b)?;
    reserved
        .state
        .combat
        .as_mut()
        .unwrap()
        .jump
        .as_mut()
        .unwrap()
        .requests
        .fill(true);
    reserved.advance(STEP, Input::default())?;
    let ids = reserved
        .state
        .combat
        .as_ref()
        .unwrap()
        .jump
        .as_ref()
        .unwrap()
        .motions
        .iter()
        .flatten()
        .map(|m| m.node)
        .collect::<Vec<_>>();
    assert_eq!(ids.len(), 3);
    assert_eq!(
        ids.iter().collect::<std::collections::BTreeSet<_>>().len(),
        3
    );
    let mut g = bundle::decode(&b)?;
    for _ in 0..3600 {
        g.advance(STEP, Input::default())?;
        if g.state.phase == Phase::Lost {
            break;
        }
    }
    assert_eq!(g.state.phase, Phase::Lost);
    assert!(g
        .state
        .combat
        .as_ref()
        .unwrap()
        .jump
        .as_ref()
        .unwrap()
        .motions
        .iter()
        .all(Option::is_none));
    assert!(!g
        .state
        .combat
        .as_ref()
        .unwrap()
        .jump
        .as_ref()
        .unwrap()
        .requests
        .iter()
        .any(|v| *v));
    let defeat_tick = g.state.statistics.ticks;
    g.restart();
    for i in 0..g.state.enemies.len() {
        for _ in 0..600 {
            if g.state.enemies[i].health == 0 {
                break;
            }
            g.aim_at(g.state.enemies[i].position);
            g.advance(
                STEP,
                Input {
                    fire: true,
                    ..Input::default()
                },
            )?;
        }
        assert_eq!(g.state.enemies[i].health, 0);
    }
    assert_eq!((g.state.statistics.hits, g.state.statistics.kills), (9, 3));
    fs::write(
        out.join("jump-validation.json"),
        serde_json::to_vec_pretty(
            &serde_json::json!({"passed":true,"natural_post_melee_jump":natural,"first_start":first_start,"first_land":first_land,"stages":stages,"restart_exact":true,"cadence_exact":true,"interruptions":interruptions,"defeat_tick":defeat_tick,"three_kills_9_hits":true,"runtime_collision_failure_settles":true,"no_destination_graceful":true,"incomplete_collision_rolls_back":true,"reservations_exclusive":true}),
        )?,
    )?;
    let mut trace_game = bundle::decode(&b)?;
    let mut trace = vec![serde_json::json!({"tick":0,"words":([0u32;12])})];
    let mut last = [0; 12];
    for _ in 0..3600 {
        trace_game.advance(STEP, Input::default())?;
        let words = trace_game
            .state
            .combat
            .as_ref()
            .unwrap()
            .jump
            .as_ref()
            .unwrap()
            .observation();
        if words != last {
            trace.push(serde_json::json!({"tick":trace_game.state.statistics.ticks,"words":words}));
            last = words;
        }
        if trace_game.state.phase == Phase::Lost {
            break;
        }
    }
    fs::write(
        out.join("jump-observer.json"),
        serde_json::to_vec_pretty(&trace)?,
    )?;
    println!("Jump, reset, cadence, hit/death interruption, defeat and three kills passed");
    Ok(())
}
