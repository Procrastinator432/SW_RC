//! Original-geometry C breach lifecycle fixtures; assigned recovery setup is explicit.
use rc_game::{Game, Input, Phase, STEP};
use std::{env, fs, path::Path};
fn breach(g: &Game) -> &rc_game::map3_breach::State {
    rc_game::breach_orders::state(&g.state).unwrap()
}
fn breach_mut(g: &mut Game) -> &mut rc_game::map3_breach::State {
    g.state
        .map
        .as_mut()
        .unwrap()
        .opening
        .as_mut()
        .unwrap()
        .planning
        .as_mut()
        .unwrap()
        .guard
        .as_mut()
        .unwrap()
        .breach
        .as_mut()
        .unwrap()
}
fn member(g: &mut Game, i: usize) -> &mut rc_game::map_runtime::Companion {
    let m = g.state.map.as_mut().unwrap();
    match i {
        0 => m.companion.as_mut().unwrap(),
        1 => m.fixer.as_mut().unwrap(),
        _ => &mut m.opening.as_mut().unwrap().sev,
    }
}
fn advance(g: &mut Game, use_action: bool) -> Result<(), String> {
    g.advance(
        STEP,
        Input {
            use_action,
            ..Default::default()
        },
    )?;
    assert_eq!(
        g.state.phase,
        Phase::Playing,
        "tick {} health {} position {:?}",
        g.state.statistics.ticks,
        g.state.health,
        g.state.pawn.body.position
    );
    Ok(())
}
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
fn events(g: &Game) -> &std::collections::BTreeMap<String, u64> {
    &g.state
        .map
        .as_ref()
        .unwrap()
        .opening
        .as_ref()
        .unwrap()
        .event_ticks
}
fn pose(g: &Game) -> Vec<u8> {
    serde_json::to_vec(&breach(g).animations).unwrap()
}
fn isolate(g: &mut Game) {
    g.state.health = 100;
    g.state.enemy_traces.clear();
    for e in &mut g.state.enemies {
        e.health = 0;
    }
    if let Some(c) = &mut g.state.combat {
        c.projectiles.clear();
    }
    for i in 0..3 {
        let c = member(g, i);
        c.health = 100;
        c.down = false;
    }
}
fn run() -> Result<(), Box<dyn std::error::Error>> {
    let a: Vec<_> = env::args_os().collect();
    let scene = fs::read(&a[1])?;
    let rows = fs::read(&a[2])?;
    let entry = fs::read(&a[3])?;
    let out = Path::new(&a[4]);
    fs::create_dir_all(out)?;
    let identity = format!("{}:{}", scene.len(), rc_game::replay::fingerprint(&scene));
    let mut g = rc_game::bundle::decode(&scene)?;
    g.restore_checkpoint(&entry, &identity)?;
    assert_eq!(g.checkpoint(&identity)?, entry);
    let start = g.state.statistics.ticks as usize;
    let mut snapshots: [Option<Vec<u8>>; 3] = [None, None, None];
    for row in rows[12 + start * 20..].chunks_exact(20) {
        g.advance(STEP, input(row))?;
        let b = breach(&g);
        let slot = if b.stage == 1 {
            Some(0)
        } else if b.stage == 2 {
            Some(if events(&g).contains_key("MDBDestroySwitch") {
                2
            } else {
                1
            })
        } else {
            None
        };
        if let Some(i) = slot {
            if snapshots[i].is_none() {
                snapshots[i] = Some(g.checkpoint(&identity)?);
                println!("Captured C fixture {i} at {}", g.state.statistics.ticks);
            }
        }
    }
    assert_eq!(g.state.phase, Phase::Won);
    assert_eq!(breach(&g).stage, 3);
    let route_ticks = g.state.statistics.ticks;
    println!("Unchanged C route complete at {route_ticks}; checking formation cancellation");
    drop(g);
    let mut g = rc_game::bundle::decode(&scene)?;
    g.restore_checkpoint(snapshots[0].as_ref().ok_or("No formation")?, &identity)?;
    isolate(&mut g);
    advance(&mut g, false)?;
    advance(&mut g, true)?;
    assert_eq!(breach(&g).stage, 0);
    for _ in 0..30 {
        advance(&mut g, true)?;
        assert_eq!(breach(&g).stage, 0);
    }
    advance(&mut g, false)?;
    advance(&mut g, true)?;
    assert!(breach(&g).stage > 0);
    drop(g);
    let mut cases = vec![];
    for (i, slot) in [(0, 1), (1, 2), (2, 1)] {
        println!("Checking recovery member {i}, snapshot {slot}");
        let mut g = rc_game::bundle::decode(&scene)?;
        g.restore_checkpoint(
            snapshots[slot].as_ref().ok_or("No animation checkpoint")?,
            &identity,
        )?;
        isolate(&mut g);
        advance(&mut g, false)?;
        // Position the test player at a supported, visible point near this member.
        let at = member(&mut g, i).position;
        let mut nearby = None;
        for d in [[90., 0.], [-90., 0.], [0., 90.], [0., -90.]] {
            if let Some(p) = rc_game::mission_motion::ground(
                &g.world,
                [at[0] + d[0], at[1] + d[1], at[2] + 120.],
                rc_game::squad::EXTENT,
                300.,
            )? {
                let eye = [p[0], p[1], p[2] + 60.];
                let q = g.world.sweep_motion(eye, at, [0.; 3])?;
                if q.complete
                    && q.contacts.is_empty()
                    && g.world.body_placement(p, rc_game::squad::EXTENT)?.state
                        != rc_package::movement::PlacementState::Penetrating
                {
                    nearby = Some(p);
                    break;
                }
            }
        }
        g.state.pawn.body.position = nearby.ok_or("No visible supported revival fixture")?;
        g.state.pawn.body.velocity = [0.; 3];
        let c = member(&mut g, i);
        c.health = 0;
        c.down = true;
        advance(&mut g, false)?;
        assert!(breach(&g).control.as_ref().unwrap().paused);
        let frozen = pose(&g);
        let original_animation_tick = breach(&g).animation_tick;
        let switch_tick = events(&g).get("MDBDestroySwitch").copied();
        let balcony = serde_json::to_vec(&breach(&g).balcony)?;
        for _ in 0..30 {
            advance(&mut g, false)?;
            assert_eq!(pose(&g), frozen);
            assert_eq!(serde_json::to_vec(&breach(&g).balcony)?, balcony);
        }
        if switch_tick.is_some() {
            assert_eq!(breach(&g).door_fraction, 1.);
        }
        // Save part-way through a real held-use revival, including its partial timer.
        for _ in 0..60 {
            advance(&mut g, true)?;
            assert_eq!(pose(&g), frozen);
        }
        assert_eq!(
            breach(&g)
                .control
                .as_ref()
                .unwrap()
                .revival
                .as_ref()
                .unwrap()
                .member,
            i
        );
        g.suspend();
        let saved = g.checkpoint(&identity)?;
        let mode = format!("member-{i}");
        fs::write(out.join(format!("{mode}-checkpoint.bin")), &saved)?;
        let mut cold = rc_game::bundle::decode(&scene)?;
        cold.restore_checkpoint(&saved, &identity)?;
        assert_eq!(cold.checkpoint(&identity)?, saved);
        assert_eq!(g.render(640, 400)?, cold.render(640, 400)?);
        let mut flags = vec![];
        for _ in 0..150 {
            advance(&mut g, true)?;
            advance(&mut cold, true)?;
            flags.push(16u32);
            assert_eq!(pose(&g), frozen);
            assert_eq!(g.checkpoint(&identity)?, cold.checkpoint(&identity)?);
        }
        assert!(!member(&mut g, i).down);
        assert_eq!(member(&mut g, i).health, 100);
        assert!(breach(&g).control.as_ref().unwrap().paused);
        // Move only the player back to the original marker-side command position.
        // This assignment is separate from the persisted revival continuation.
        fs::write(
            out.join(format!("{mode}-inputs.json")),
            serde_json::to_vec(&flags)?,
        )?;
        fs::write(
            out.join(format!("{mode}-final.bin")),
            g.checkpoint(&identity)?,
        )?;
        let original: serde_json::Value =
            serde_json::from_slice(&snapshots[slot].as_ref().unwrap()[20..])?;
        let marker_position: rc_game::Point =
            serde_json::from_value(original["state"]["pawn"]["body"]["position"].clone())?;
        g.state.pawn.body.position = marker_position;
        g.state.pawn.body.velocity = [0.; 3];
        advance(&mut g, false)?;
        advance(&mut g, true)?;
        assert!(
            !breach(&g).control.as_ref().unwrap().paused,
            "Marker not usable for resume"
        );
        for _ in 0..1800 {
            if breach(&g).stage == 3 {
                break;
            }
            // Completion assertions isolate the later room fight; the saved
            // native continuation above has no per-tick state assignments.
            isolate(&mut g);
            advance(&mut g, false)?;
        }
        assert_eq!(breach(&g).stage, 3);
        assert_eq!(breach(&g).animation_tick, original_animation_tick);
        if let Some(t) = switch_tick {
            assert_eq!(events(&g).get("MDBDestroySwitch"), Some(&t));
        }
        assert_eq!(
            breach(&g)
                .animations
                .iter()
                .flat_map(|a| &a.notifies)
                .filter(|(_, id)| *id == 309)
                .count(),
            1
        );
        if i == 0 {
            let before = g.checkpoint(&identity)?;
            breach_mut(&mut g).control.as_mut().unwrap().paused = true;
            let bad = g.checkpoint(&identity)?;
            fs::write(out.join("invalid-stage-control.bin"), &bad)?;
            g.restore_checkpoint(&before, &identity)?;
            assert!(g.restore_checkpoint(&bad, &identity).is_err());
            assert_eq!(g.checkpoint(&identity)?, before);
        }
        cases.push(serde_json::json!({"member":i,"after_switch":slot==2,"cold_restore_exact":true,"revival_steps":210,"persisted_continuation_steps":flags.len(),"animation_frozen_during_recovery":true,"resumed_without_restart":true,"switch_not_repeated":true,"final_stage":3}));
    }
    fs::write(
        out.join("validation.json"),
        serde_json::to_vec_pretty(
            &serde_json::json!({"passed":true,"original_c_input_route_complete":true,"replayed_c_inputs":rows[12..].len()/20-start,"final_tick":route_ticks,"formation_cancel_release_reissue":true,"invalid_stage_control_rejected_atomically":true,"scope":"Original scene and unchanged input-only route; separate recovery fixtures assign enemy health, squad incapacitation and nearby supported player positions. Resume position assignment is after saved revival continuation.","cases":cases}),
        )?,
    )?;
    println!("C input route, cancel/reissue, all three revivals, frozen animation and atomic restore passed");
    Ok(())
}
fn main() -> Result<(), Box<dyn std::error::Error>> {
    std::thread::Builder::new()
        .name("breach-fixtures".into())
        .stack_size(16 * 1024 * 1024)
        .spawn(|| run().map_err(|e| e.to_string()))?
        .join()
        .map_err(|_| "C fixture worker panicked")??;
    Ok(())
}
