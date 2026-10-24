//! Input-only A->B route for the audited marker package; checkpoint fixture prefixes.
use rc_game::{Game, Input, STEP};
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
fn distance(a: rc_game::Point, b: rc_game::Point) -> f64 {
    a.into_iter()
        .zip(b)
        .map(|(a, b)| (a - b) * (a - b))
        .sum::<f64>()
        .sqrt()
}
fn work(g: &Game) -> &rc_game::map_gameplay::State {
    g.state.map.as_ref().unwrap().gameplay.as_ref().unwrap()
}

fn work_mut(g: &mut Game) -> &mut rc_game::map_gameplay::State {
    g.state.map.as_mut().unwrap().gameplay.as_mut().unwrap()
}
fn members(g: &Game) -> [rc_game::Point; 2] {
    [
        g.state
            .map
            .as_ref()
            .unwrap()
            .companion
            .as_ref()
            .unwrap()
            .position,
        work(g).fixer.position,
    ]
}
fn tick_fixture(g: &mut Game, use_action: bool) -> Result<(), String> {
    let before = members(g);
    g.advance(
        STEP,
        Input {
            use_action,
            ..Default::default()
        },
    )?;
    assert_eq!(g.state.phase, rc_game::Phase::Playing);
    for (from, to) in before.into_iter().zip(members(g)) {
        assert!(
            (from[0] - to[0]).hypot(from[1] - to[1]) <= 450. * STEP + 0.01,
            "Unbounded squad step"
        );
    }
    Ok(())
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
fn fixtures(scene: &[u8], out: &Path) -> Result<(), Box<dyn std::error::Error>> {
    let identity = format!("{}:{}", scene.len(), rc_game::replay::fingerprint(scene));
    let mut cases = vec![];
    for i in 0..5 {
        let mut g = rc_game::bundle::decode(scene)?;
        g.restore_checkpoint(
            &fs::read(out.join(format!("task-{i}-prefix.bin")))?,
            &identity,
        )?;
        isolate(&mut g);
        let owner = usize::from(
            !g.map
                .as_ref()
                .unwrap()
                .gameplay
                .as_ref()
                .unwrap()
                .assets
                .interactions[i]
                .demolition,
        );
        tick_fixture(&mut g, false)?;
        tick_fixture(&mut g, true)?;
        assert!(
            work(&g).execution.as_ref().unwrap().tasks[owner]
                .as_ref()
                .unwrap()
                .paused
        );
        let progress = work(&g).progress[i];
        for _ in 0..30 {
            tick_fixture(&mut g, false)?;
            assert_eq!(work(&g).progress[i], progress);
        }
        let paused = g.checkpoint(&identity)?;
        fs::write(out.join(format!("task-{i}-paused.bin")), &paused)?;
        let mut guard = rc_game::bundle::decode(scene)?;
        guard.restore_checkpoint(&paused, &identity)?;
        assert_eq!(g.render(640, 400)?, guard.render(640, 400)?);
        drop(guard);
        tick_fixture(&mut g, true)?;
        assert_eq!(work(&g).execution.as_ref().unwrap().target(owner), Some(i));
        g.suspend();
        let saved = g.checkpoint(&identity)?;
        fs::write(out.join(format!("task-{i}-checkpoint.bin")), &saved)?;
        let mut cold = rc_game::bundle::decode(scene)?;
        cold.restore_checkpoint(&saved, &identity)?;
        assert_eq!(cold.checkpoint(&identity)?, saved);
        assert_eq!(g.render(640, 400)?, cold.render(640, 400)?);
        let initial = members(&g)[owner];
        let site = g
            .map
            .as_ref()
            .unwrap()
            .gameplay
            .as_ref()
            .unwrap()
            .work_sites
            .as_ref()
            .unwrap()[i]
            .position;
        let mut flags = vec![];
        let mut moved = 0.;
        let mut ticks = 0;
        for _ in 0..2400 {
            let at = members(&g)[owner];
            let before = work(&g).progress[i];
            tick_fixture(&mut g, false)?;
            tick_fixture(&mut cold, false)?;
            flags.push(0u32);
            ticks += 1;
            assert_eq!(g.checkpoint(&identity)?, cold.checkpoint(&identity)?);
            let after = members(&g)[owner];
            moved += (at[0] - after[0]).hypot(at[1] - after[1]);
            if distance(after, site) > 16. {
                assert_eq!(
                    work(&g).progress[i],
                    before,
                    "Remote work before reaching original marker"
                );
            }
            if ticks % 240 == 0 {
                assert_eq!(g.render(640, 400)?, cold.render(640, 400)?);
            }
            if work(&g).armed[i] || work(&g).used[i] {
                break;
            }
        }
        assert!(
            work(&g).armed[i] || work(&g).used[i],
            "Marker {i} did not finish"
        );
        assert_eq!(work(&g).execution.as_ref().unwrap().tasks[owner], None);
        for _ in 0..30 {
            tick_fixture(&mut g, false)?;
            tick_fixture(&mut cold, false)?;
            flags.push(0u32);
        }
        assert_eq!(g.checkpoint(&identity)?, cold.checkpoint(&identity)?);
        fs::write(
            out.join(format!("task-{i}-inputs.json")),
            serde_json::to_vec(&flags)?,
        )?;
        fs::write(
            out.join(format!("task-{i}-final.bin")),
            g.checkpoint(&identity)?,
        )?;
        let mut unavailable = rc_game::bundle::decode(scene)?;
        unavailable.restore_checkpoint(&saved, &identity)?;
        if owner == 0 {
            let c = unavailable
                .state
                .map
                .as_mut()
                .unwrap()
                .companion
                .as_mut()
                .unwrap();
            c.health = 0;
            c.down = true;
        } else {
            let c = &mut work_mut(&mut unavailable).fixer;
            c.health = 0;
            c.down = true;
        }
        let before = work(&unavailable).progress[i];
        for _ in 0..30 {
            tick_fixture(&mut unavailable, false)?;
            assert_eq!(work(&unavailable).progress[i], before);
        }
        assert!(
            work(&unavailable).execution.as_ref().unwrap().tasks[owner]
                .as_ref()
                .unwrap()
                .paused
        );
        if i == 0 {
            let before = g.checkpoint(&identity)?;
            work_mut(&mut unavailable).execution.as_mut().unwrap().tasks[1] =
                Some(rc_game::marker_execution::Task {
                    interaction: i,
                    paused: false,
                });
            let bad = unavailable.checkpoint(&identity)?;
            fs::write(out.join("invalid-task.bin"), &bad)?;
            assert!(g.restore_checkpoint(&bad, &identity).is_err());
            assert_eq!(g.checkpoint(&identity)?, before);
        }
        cases.push(serde_json::json!({"mode":format!("task-{i}"),"interaction":i,"owner":owner,"steps":flags.len(),"initial_distance":distance(initial,site),"walked_distance":moved,"pause_release_retains_progress":true,"no_work_before_arrival":true,"completed_without_held_use":true,"unavailable_member_pauses":true,"cold_restore_exact":true}));
        println!(
            "Marker fixture {i}: {} released-input steps, {moved:.1} units walked",
            flags.len()
        );
    }
    fs::write(
        out.join("validation.json"),
        serde_json::to_vec_pretty(
            &serde_json::json!({"passed":true,"cases":cases,"invalid_task_rejected_atomically":true,"scope":"Original command prefixes from the input-only route. Separate fixtures assign health, enemy death flags, factory capacities and projectile clearing to isolate movement/work. Actor positions are not assigned. Availability guard explicitly assigns owner incapacitation."}),
        )?,
    )?;
    Ok(())
}
fn run() -> Result<(), Box<dyn std::error::Error>> {
    let a: Vec<_> = env::args_os().collect();
    let scene_a = fs::read(&a[1])?;
    let entry_a = fs::read(&a[2])?;
    let inputs_a = fs::read(&a[3])?;
    let scene_b = fs::read(&a[4])?;
    let inputs_b = fs::read(&a[5])?;
    let out = Path::new(&a[6]);
    fs::create_dir_all(out)?;
    if a.get(7).and_then(|s| s.to_str()) == Some("--fixtures-only") {
        return fixtures(&scene_b, out);
    }
    let mut first = rc_game::bundle::decode(&scene_a)?;
    first.restore_checkpoint(
        &entry_a,
        &format!(
            "{}:{}",
            scene_a.len(),
            rc_game::replay::fingerprint(&scene_a)
        ),
    )?;
    let start = first.state.statistics.ticks as usize;
    for row in inputs_a[12 + start * 20..].chunks_exact(20) {
        first.advance(STEP, input(row))?;
    }
    let handoff = first.campaign_handoff()?.ok_or("A incomplete")?;
    drop(first);
    let mut g = rc_game::bundle::decode(&scene_b)?;
    g.restore_handoff(&handoff)?;
    let identity = format!(
        "{}:{}",
        scene_b.len(),
        rc_game::replay::fingerprint(&scene_b)
    );
    let sites = &g
        .map
        .as_ref()
        .unwrap()
        .gameplay
        .as_ref()
        .unwrap()
        .work_sites;
    println!(
        "Resolved original work sites: {}",
        serde_json::to_string(sites)?
    );
    let mut snapshots: [Option<Vec<u8>>; 5] = std::array::from_fn(|_| None);
    let mut last = [false; 5];
    let mut replay = vec![];
    let mut waits = vec![];
    for row in inputs_b[12..].chunks_exact(20) {
        if let Some(i) = if !input(row).use_action {
            work(&g)
                .execution
                .as_ref()
                .and_then(|e| (0..2).find_map(|p| e.target(p)))
        } else {
            None
        } {
            let begin = g.state.statistics.ticks;
            for _ in 0..2400 {
                if work(&g).armed[i] || work(&g).used[i] {
                    break;
                }
                g.advance(STEP, Input::default())?;
                replay.extend_from_slice(&[0u8; 20]);
                assert_eq!(
                    g.state.phase,
                    rc_game::Phase::Playing,
                    "Lost while waiting for marker {i}"
                );
            }
            assert!(
                work(&g).armed[i] || work(&g).used[i],
                "Marker {i} failed to finish at {:?}",
                members(&g)
            );
            println!(
                "Autonomous marker {i} finished after {} released-input steps",
                g.state.statistics.ticks - begin
            );
            waits.push(g.state.statistics.ticks - begin);
        }
        g.advance(STEP, input(row))?;
        replay.extend_from_slice(row);
        let w = work(&g);
        for (i, snapshot) in snapshots.iter_mut().enumerate() {
            let owner = usize::from(
                !g.map
                    .as_ref()
                    .unwrap()
                    .gameplay
                    .as_ref()
                    .unwrap()
                    .assets
                    .interactions[i]
                    .demolition,
            );
            if snapshot.is_none()
                && w.execution
                    .as_ref()
                    .is_some_and(|e| e.target(owner) == Some(i))
            {
                *snapshot = Some(g.checkpoint(&identity)?);
                fs::write(
                    out.join(format!("task-{i}-prefix.bin")),
                    snapshot.as_ref().unwrap(),
                )?;
                println!(
                    "Assigned task {i} at tick {} progress {}",
                    g.state.statistics.ticks, w.progress[i]
                );
            }
            if (w.armed[i] || w.used[i]) && !last[i] {
                last[i] = true;
                println!("Completed task {i} at tick {}", g.state.statistics.ticks);
            }
        }
        if g.state.statistics.ticks == 240 {
            fs::write(out.join("entry-checkpoint.bin"), g.checkpoint(&identity)?)?;
        }
    }
    let mut file = b"RCINPUT1".to_vec();
    file.extend(((replay.len() / 20) as u32).to_le_bytes());
    file.extend(&replay);
    fs::write(out.join("map_b_replay.bin"), file)?;
    fs::write(out.join("route-final.bin"), g.checkpoint(&identity)?)?;
    println!(
        "B final {:?}, progress {:?}, objectives {:?}",
        g.state.phase,
        work(&g).progress,
        work(&g).objectives
    );
    assert_eq!(g.state.phase, rc_game::Phase::Won);
    for (i, snapshot) in snapshots.iter().enumerate() {
        fs::write(
            out.join(format!("task-{i}-prefix.bin")),
            snapshot.as_ref().ok_or("Missing assignment")?,
        )?;
    }
    fs::write(
        out.join("route-validation.json"),
        serde_json::to_vec_pretty(
            &serde_json::json!({"passed":true,"unchanged_a_inputs":true,"original_b_inputs":inputs_b[12..].len()/20,"b_inputs":replay.len()/20,"added_release_waits":waits,"scope":"Genuine 0.82 A checkpoint with unchanged A scene; remaining A inputs then real handoff to upgraded B; original B inputs plus released-input waits when releasing Use before task completion; no actor/state assignment"}),
        )?,
    )?;
    fixtures(&scene_b, out)?;
    Ok(())
}
fn main() -> Result<(), Box<dyn std::error::Error>> {
    std::thread::Builder::new()
        .name("marker-execution-fixtures".into())
        .stack_size(16 * 1024 * 1024)
        .spawn(|| run().map_err(|e| e.to_string()))?
        .join()
        .map_err(|_| "B marker worker panicked")??;
    Ok(())
}
