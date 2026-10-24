//! Explicit original-geometry B work fixtures, plus the unchanged input-only B route.
use rc_game::{Game, Input, Phase, STEP};
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
fn work(g: &Game) -> &rc_game::map_gameplay::State {
    g.state.map.as_ref().unwrap().gameplay.as_ref().unwrap()
}
fn work_mut(g: &mut Game) -> &mut rc_game::map_gameplay::State {
    g.state.map.as_mut().unwrap().gameplay.as_mut().unwrap()
}
fn advance(g: &mut Game, use_action: bool) -> Result<(), String> {
    g.advance(
        STEP,
        Input {
            use_action,
            ..Default::default()
        },
    )?;
    if g.state.phase != Phase::Playing {
        return Err("Isolated work fixture left Playing".into());
    }
    Ok(())
}
fn isolate(g: &mut Game) {
    g.state.health = 100;
    g.state.damage_flash = 0.;
    g.state.enemy_traces.clear();
    if let Some(c) = g.state.map.as_mut().unwrap().companion.as_mut() {
        c.health = 100;
        c.down = false;
    }
    work_mut(g).fixer.health = 100;
    work_mut(g).fixer.down = false;
    work_mut(g).revive_progress = 0.;
    for e in &mut g.state.enemies {
        e.health = 0;
    }
    if let Some(c) = &mut g.state.combat {
        c.projectiles.clear();
    }
    let capacities: Vec<_> = g
        .map
        .as_ref()
        .unwrap()
        .gameplay
        .as_ref()
        .unwrap()
        .assets
        .factories
        .iter()
        .map(|f| f.capacity)
        .collect();
    let s = work_mut(g);
    s.factories = capacities;
    s.enemy_deaths.fill(true);
}
fn run() -> Result<(), Box<dyn std::error::Error>> {
    let args: Vec<_> = env::args_os().collect();
    let scene = fs::read(&args[1])?;
    let rows = fs::read(&args[2])?;
    let entry = fs::read(&args[3])?;
    let out = Path::new(&args[4]);
    fs::create_dir_all(out)?;
    let identity = format!("{}:{}", scene.len(), rc_game::replay::fingerprint(&scene));
    let mut normal = rc_game::bundle::decode(&scene)?;
    normal.restore_checkpoint(&entry, &identity)?;
    assert_eq!(normal.checkpoint(&identity)?, entry);
    let start = normal.state.statistics.ticks as usize;
    assert_eq!(start, 240, "Expected the genuine campaign entry checkpoint");
    let mut partials: [Option<Vec<u8>>; 2] = [None, None];
    for row in rows[12 + start * 20..].chunks_exact(20) {
        normal.advance(STEP, input(row))?;
        for (i, partial) in partials.iter_mut().enumerate() {
            if partial.is_none()
                && work(&normal).progress[i] >= 0.999
                && !work(&normal).armed[i]
                && !work(&normal).used[i]
            {
                *partial = Some(normal.checkpoint(&identity)?);
            }
        }
    }
    assert_eq!(normal.state.phase, Phase::Won);
    assert!(work(&normal).objectives.iter().all(|x| *x));
    assert!(normal.campaign_handoff()?.is_some());
    drop(normal);
    let mut cases = vec![];
    for (index, mode) in [(0, "demolition"), (1, "slice")] {
        let mut g = rc_game::bundle::decode(&scene)?;
        g.restore_checkpoint(
            partials[index]
                .as_ref()
                .ok_or("Missing input-driven partial work")?,
            &identity,
        )?;
        let prefix_tick = g.state.statistics.ticks;
        isolate(&mut g);
        let original = work(&g).progress[index];
        advance(&mut g, false)?;
        assert_eq!(work(&g).progress[index], original);
        assert_eq!(work(&g).orders.as_ref().unwrap().working, None);
        if index == 0 {
            let player = g.state.pawn.body.position;
            let mut nearby = None;
            for d in [[90., 0.], [-90., 0.], [0., 90.], [0., -90.]] {
                if let Some(p) = rc_game::mission_motion::ground(
                    &g.world,
                    [player[0] + d[0], player[1] + d[1], player[2] + 120.],
                    rc_game::squad::EXTENT,
                    300.,
                )? {
                    if g.world.body_placement(p, rc_game::squad::EXTENT)?.state
                        != rc_package::movement::PlacementState::Penetrating
                    {
                        nearby = Some(p);
                        break;
                    }
                }
            }
            let c = g.state.map.as_mut().unwrap().companion.as_mut().unwrap();
            c.position = nearby.ok_or("No supported nearby revival fixture")?;
            c.down = true;
            c.health = 0;
            advance(&mut g, false)?;
        }
        g.suspend();
        let saved = g.checkpoint(&identity)?;
        fs::write(out.join(format!("{mode}-checkpoint.bin")), &saved)?;
        let mut cold = rc_game::bundle::decode(&scene)?;
        cold.restore_checkpoint(&saved, &identity)?;
        assert_eq!(cold.checkpoint(&identity)?, saved);
        assert_eq!(g.render(640, 400)?, cold.render(640, 400)?);
        if index == 1 {
            let mut guard = rc_game::bundle::decode(&scene)?;
            guard.restore_checkpoint(&saved, &identity)?;
            work_mut(&mut guard).fixer.down = true;
            work_mut(&mut guard).fixer.health = 0;
            for _ in 0..60 {
                advance(&mut guard, true)?;
                assert_eq!(work(&guard).progress[index], original);
            }
            let mut invalid = rc_game::bundle::decode(&scene)?;
            invalid.restore_checkpoint(&saved, &identity)?;
            work_mut(&mut invalid).orders.as_mut().unwrap().reserved[0] = Some(index);
            let bad = invalid.checkpoint(&identity)?;
            fs::write(out.join("invalid-reservation.bin"), &bad)?;
            assert!(g.restore_checkpoint(&bad, &identity).is_err());
            assert_eq!(g.checkpoint(&identity)?, saved);
        }
        let mut flags = vec![];
        for _ in 0..60 {
            advance(&mut g, false)?;
            advance(&mut cold, false)?;
            flags.push(0u32);
            assert_eq!(work(&g).progress[index], original);
            assert_eq!(g.checkpoint(&identity)?, cold.checkpoint(&identity)?);
        }
        let use_steps = 360;
        for tick in 0..use_steps {
            advance(&mut g, true)?;
            advance(&mut cold, true)?;
            flags.push(16);
            if index == 0 && tick < 180 {
                assert_eq!(
                    work(&g).progress[index],
                    original,
                    "Revival and work advanced together"
                );
            }
            assert_eq!(g.checkpoint(&identity)?, cold.checkpoint(&identity)?);
            if tick % 60 == 0 {
                assert_eq!(g.render(640, 400)?, cold.render(640, 400)?);
            }
        }
        assert!(
            (work(&g).progress[index]
                - g.map
                    .as_ref()
                    .unwrap()
                    .gameplay
                    .as_ref()
                    .unwrap()
                    .assets
                    .interactions[index]
                    .seconds)
                .abs()
                < 1e-8
        );
        if index == 0 {
            assert!(work(&g).armed[index]);
            assert!(!work(&g).used[index]);
            assert!(
                !g.state
                    .map
                    .as_ref()
                    .unwrap()
                    .companion
                    .as_ref()
                    .unwrap()
                    .down
            );
        } else {
            assert!(work(&g).used[index]);
        }
        let job = &g
            .map
            .as_ref()
            .unwrap()
            .gameplay
            .as_ref()
            .unwrap()
            .assets
            .interactions[index];
        assert!(work(&g).events.contains(&job.underway));
        assert_eq!(work(&g).events.contains(&job.event), index == 1);
        fs::write(
            out.join(format!("{mode}-inputs.json")),
            serde_json::to_vec(&flags)?,
        )?;
        fs::write(
            out.join(format!("{mode}-final.bin")),
            g.checkpoint(&identity)?,
        )?;
        cases.push(serde_json::json!({"mode":mode,"interaction":index,"prefix_tick":prefix_tick,"initial_progress":original,"continuation_steps":flags.len(),"cold_restore_exact":true,"release_retains_progress":true,"ready_guard":true,"scorch_revive_blocks_work_same_tick":index==0,"final_progress":work(&g).progress[index],"armed":work(&g).armed[index],"used":work(&g).used[index]}));
    }
    fs::write(
        out.join("validation.json"),
        serde_json::to_vec_pretty(
            &serde_json::json!({"passed":true,"original_b_input_route_complete":true,"input_source_entry":"Genuine 0.80 campaign checkpoint at tick 240; no prefix state assignment","replayed_b_inputs":rows[12..].len()/20-start,"scope":"Isolated original-geometry fixtures after input-only prefixes: health, factory capacities, enemy death flags, nearby supported Scorch position and incapacitation are assigned test setup; Fixer availability guard is not a Fixer revival implementation","invalid_reservation_rejected_atomically":true,"cases":cases}),
        )?,
    )?;
    println!("B input route, demolition/slice pauses, revival exclusion and cold work continuation passed");
    Ok(())
}
fn main() -> Result<(), Box<dyn std::error::Error>> {
    std::thread::Builder::new()
        .name("map2-order-fixtures".into())
        .stack_size(16 * 1024 * 1024)
        .spawn(|| run().map_err(|e| e.to_string()))?
        .join()
        .map_err(|_| "B work fixture worker panicked")??;
    Ok(())
}
