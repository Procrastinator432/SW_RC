//! Exercise the extracted event host; no physical walk, AI or Android acceptance claim.
use rc_game::crash::{Assets, CombatStage, Effect, Stage, State};
use std::{env, fs, path::Path};

fn permutations(ids: &mut [u32], at: usize, result: &mut Vec<Vec<u32>>) {
    if at == ids.len() {
        result.push(ids.to_vec());
        return;
    }
    for i in at..ids.len() {
        ids.swap(at, i);
        permutations(ids, at + 1, result);
        ids.swap(at, i);
    }
}
fn main() -> Result<(), Box<dyn std::error::Error>> {
    let args: Vec<_> = env::args_os().collect();
    if args.len() != 3 {
        return Err("usage: rc-crash-check ASSETS OUTPUT_DIR".into());
    }
    let a: Assets = serde_json::from_slice(&fs::read(&args[1])?)?;
    a.validate()?;
    let out = Path::new(&args[2]);
    fs::create_dir_all(out)?;
    let far = [1e6, 1e6, 0.];
    let extent = [22., 22., 60.];
    let total = 1. + a.advisor_duration + a.retreat_delay;
    let mut delays = vec![];
    let mut reference = None;
    for hz in [30, 60, 120, 240] {
        let mut s = State::new(&a)?;
        let initial = s.clone();
        // Independent scripts: an artificial contact can spawn before Bacta.
        // The physical barrier, not an invented event prerequisite, prevents this in play.
        assert_eq!(
            s.advance(&a, 0., a.spawn.position, extent)?
                .iter()
                .filter(|e| matches!(e, Effect::Spawn(_)))
                .count(),
            5
        );
        assert!(!s
            .advance(&a, 0., a.vulnerability.position, extent)?
            .is_empty());
        assert!(s.spawned && s.vulnerable);
        assert_eq!(s.stage, Stage::Bacta);
        s = initial.clone();
        let mut effects = s.bacta_used(&a);
        assert_eq!(
            effects,
            [
                Effect::Event("BactaUsed".into()),
                Effect::CollisionDisabled(a.bacta_barrier),
                Effect::ObjectiveCompleted(0)
            ]
        );
        assert!(s.bacta_used(&a).is_empty());
        let steps = (total * hz as f64).ceil() as u32;
        for tick in 0..steps {
            effects.extend(s.advance(&a, 1. / hz as f64, far, extent)?);
            if tick + 1 < steps {
                assert!(!effects
                    .iter()
                    .any(|e| matches!(e, Effect::HurtBarrier { .. })));
            }
        }
        assert_eq!(s.stage, Stage::Approach);
        assert_eq!(
            effects
                .iter()
                .filter(|e| matches!(e, Effect::HurtBarrier { .. }))
                .count(),
            1
        );
        assert!(effects.contains(&Effect::HurtBarrier {
            original: a.path_barrier,
            amount: 50000
        }));
        assert_eq!(
            effects
                .iter()
                .filter(|e| matches!(e, Effect::PlaySound(_)))
                .count(),
            2
        );
        if let Some(expected) = &reference {
            assert_eq!(&effects, expected);
        } else {
            reference = Some(effects.clone());
        }
        delays.push(serde_json::json!({"hz":hz,"steps":steps,"delay":total,"effects":effects}));
    }
    let mut s = State::new(&a)?;
    s.bacta_used(&a);
    for _ in 0..(total.ceil() as usize) {
        s.advance(&a, 1., far, extent)?;
    }
    // The earlier vulnerability event must not be saved for the waiting script.
    assert!(s
        .advance(&a, 0., a.vulnerability.position, extent)?
        .is_empty());
    let effects = s.advance(&a, 0., a.spawn.position, extent)?;
    assert_eq!(
        effects
            .iter()
            .filter(|e| matches!(e, Effect::Spawn(_)))
            .count(),
        5
    );
    assert!(s.spawned && !s.vulnerable);
    assert!(s.advance(&a, 1., a.spawn.position, extent)?.is_empty());
    for f in &a.factories {
        assert!(s.pawn_died(&a, f.original)?.is_empty());
    }
    assert_eq!(s.counter_remaining, 5);
    let before = s.clone();
    assert!(s.pawn_died(&a, u32::MAX).is_err());
    assert_eq!(s, before);
    let effects = s.advance(&a, 0., a.vulnerability.position, extent)?;
    assert!(effects.contains(&Effect::Invulnerable {
        tag: "GunshipDroids".into(),
        enabled: false
    }));
    assert!(s.vulnerable);
    let ready = s.clone();
    let mut orders = vec![];
    let mut vertical = State::new(&a)?;
    vertical.bacta_used(&a);
    for _ in 0..total.ceil() as usize {
        vertical.advance(&a, 1., far, extent)?;
    }
    let mut above = a.spawn.position;
    above[2] += a.spawn.height + extent[2] + 1.;
    assert!(vertical.advance(&a, 0., above, extent)?.is_empty());
    assert!(!vertical.spawned);
    // Repeated contact across the Bacta delay cannot spawn the factories twice.
    let mut early = State::new(&a)?;
    early.bacta_used(&a);
    let initial_spawn = early.advance(&a, 0., a.spawn.position, extent)?;
    assert_eq!(
        initial_spawn
            .iter()
            .filter(|e| matches!(e, Effect::Spawn(_)))
            .count(),
        5
    );
    for _ in 0..total.ceil() as usize {
        early.advance(&a, 1., a.spawn.position, extent)?;
    }
    assert!(early.spawned);
    early.advance(&a, 0., far, extent)?;
    assert!(early.advance(&a, 0., a.spawn.position, extent)?.is_empty());
    assert!(early.spawned);
    permutations(
        &mut a.factories.iter().map(|f| f.original).collect::<Vec<_>>(),
        0,
        &mut orders,
    );
    for (order_index, order) in orders.iter().enumerate() {
        let mut s = ready.clone();
        let mut events = vec![];
        for (i, id) in order.iter().enumerate() {
            events.extend(s.pawn_died(&a, *id)?);
            assert_eq!(s.counter_remaining, 4 - i as u32);
            let current = s.clone();
            assert!(s.pawn_died(&a, *id)?.is_empty());
            assert_eq!(s, current);
            if i < 4 {
                assert_ne!(s.combat, CombatStage::Secured);
                assert!(!events.contains(&Effect::ObjectiveCompleted(1)));
            }
        }
        assert_eq!(s.combat, CombatStage::Secured);
        assert_eq!(
            events
                .iter()
                .filter(|e| **e == Effect::Event("CrashSiteSecured".into()))
                .count(),
            1
        );
        assert!(events.contains(&Effect::CollisionDisabled(a.exit_barrier)));
        assert!(events.contains(&Effect::ObjectiveCompleted(1)));
        if order_index == 0 {
            fs::write(
                out.join("completion-trace.json"),
                serde_json::to_vec_pretty(
                    &serde_json::json!({"order":order,"effects":events,"state":s}),
                )?,
            )?;
        }
        assert!(s.advance(&a, 1., a.spawn.position, extent)?.is_empty());
        assert_eq!(State::new(&a)?.counter_remaining, 5);
    }
    let before = ready.clone();
    for seconds in [-1., f64::NAN, f64::INFINITY, 1.01] {
        let mut s = before.clone();
        assert!(s.advance(&a, seconds, far, extent).is_err());
        assert_eq!(s, before);
    }
    for position in [[f64::NAN, 0., 0.], [1e9, 0., 0.]] {
        let mut s = before.clone();
        assert!(s.advance(&a, 0., position, extent).is_err());
        assert_eq!(s, before);
    }
    for which in 0..8 {
        let mut bad = a.clone();
        match which {
            0 => {
                bad.counter = 4;
            }
            1 => {
                bad.factories.pop();
            }
            2 => {
                bad.factories[0].capacity = 2;
            }
            3 => {
                bad.factories[0].original = bad.factories[1].original;
            }
            4 => {
                bad.advisor_duration = f64::NAN;
            }
            5 => {
                bad.spawn.height = 0.;
            }
            6 => {
                bad.path_barrier_damage = 1;
            }
            _ => {
                bad.factories[0].prototype = "Properties.GeonosianWarrior".into();
            }
        }
        assert!(State::new(&bad).is_err());
    }
    fs::write(
        out.join("timer-traces.json"),
        serde_json::to_vec_pretty(&delays)?,
    )?;
    let receipt = serde_json::json!({"passed":true,"death_orders":orders.len(),"timer_rates":[30,60,120,240],
        "five_factories":true,"original_counter":5,"duplicate_deaths_ignored":true,"invulnerability_gate":true,
        "events_not_queued":true,"parallel_script_tracks":true,"invalid_input_unchanged":true,"invalid_assets_rejected":8,
        "scope":"Extracted original event host only; effects not yet connected to physical scene/AI/graphics or Android."});
    fs::write(
        out.join("validation.json"),
        serde_json::to_vec_pretty(&receipt)?,
    )?;
    println!("Crash-site flow: {} death orders, four timer rates, original gates, invalid assets and inputs passed",orders.len());
    Ok(())
}
