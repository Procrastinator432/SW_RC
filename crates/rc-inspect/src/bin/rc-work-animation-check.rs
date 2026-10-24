//! Compare gameplay and original work poses against the previous scene package.
use rc_game::{Game, Input, STEP};
use std::{env, fs, path::Path};
fn state(g: &Game) -> Vec<u8> {
    serde_json::to_vec(&g.state).unwrap()
}
fn actor(g: &Game, p: usize) -> &rc_game::map_runtime::Companion {
    let m = g.state.map.as_ref().unwrap();
    if p == 0 {
        m.companion.as_ref().unwrap()
    } else {
        &m.gameplay.as_ref().unwrap().fixer
    }
}
fn observer(g: &mut Game, owner: usize) -> Result<(), String> {
    let c = actor(g, owner);
    let at = c.position;
    let yaw = c.yaw;
    for angle in [
        std::f64::consts::FRAC_PI_2,
        -std::f64::consts::FRAC_PI_2,
        std::f64::consts::PI,
        0.,
    ] {
        let a = yaw + angle;
        if let Some(p) = rc_game::mission_motion::ground(
            &g.world,
            [at[0] + a.cos() * 230., at[1] + a.sin() * 230., at[2] + 160.],
            g.profile.physics.extent,
            400.,
        )? {
            let eye = [p[0], p[1], p[2] + g.profile.physics.extent[2] * 0.65];
            let target = [at[0], at[1], at[2] + 15.];
            let q = g.world.sweep_motion(eye, target, [0.; 3])?;
            if q.complete
                && q.contacts.is_empty()
                && g.world.body_placement(p, g.profile.physics.extent)?.state
                    != rc_package::movement::PlacementState::Penetrating
            {
                g.state.pawn.body.position = p;
                g.state.pawn.body.velocity = [0.; 3];
                g.state.pawn.body.grounded = true;
                g.aim_at(target);
                // Match production controller angle canonicalization without
                // advancing simulation time or moving any actor.
                assert_eq!(g.advance(0., Input::default())?, 0);
                return Ok(());
            }
        }
    }
    Err("No supported visual observer position".into())
}
fn image(g: &Game, p: &Path) -> Result<(), Box<dyn std::error::Error>> {
    let mut raw = b"P6\n640 400\n255\n".to_vec();
    for v in g.render(640, 400)? {
        raw.extend([(v >> 16) as u8, (v >> 8) as u8, v as u8]);
    }
    fs::write(p, raw)?;
    Ok(())
}
fn run() -> Result<(), Box<dyn std::error::Error>> {
    let a: Vec<_> = env::args_os().collect();
    let old = fs::read(&a[1])?;
    let new = fs::read(&a[2])?;
    let fixtures = Path::new(&a[3]);
    let out = Path::new(&a[4]);
    fs::create_dir_all(out)?;
    let old_id = format!("{}:{}", old.len(), rc_game::replay::fingerprint(&old));
    let new_id = format!("{}:{}", new.len(), rc_game::replay::fingerprint(&new));
    let mut cases = vec![];
    for job in 0..5 {
        let mut before = rc_game::bundle::decode(&old)?;
        before.restore_checkpoint(
            &fs::read(fixtures.join(format!("task-{job}-checkpoint.bin")))?,
            &old_id,
        )?;
        let mut after = rc_game::bundle::decode(&new)?;
        // Explicit test-only scene rebinding: assets differ only by audited clips.
        after.restore_checkpoint(&before.checkpoint(&new_id)?, &new_id)?;
        assert_eq!(state(&before), state(&after));
        let owner = usize::from(
            !after
                .map
                .as_ref()
                .unwrap()
                .gameplay
                .as_ref()
                .unwrap()
                .assets
                .interactions[job]
                .demolition,
        );
        let flags: Vec<u32> =
            serde_json::from_slice(&fs::read(fixtures.join(format!("task-{job}-inputs.json")))?)?;
        let mut seen = [false; 3];
        let mut snapshots = vec![];
        let mut comparisons = 0;
        for flag in &flags {
            let input = Input {
                use_action: flag & 16 != 0,
                ..Default::default()
            };
            before.advance(STEP, input)?;
            after.advance(STEP, input)?;
            assert_eq!(state(&before), state(&after));
            comparisons += 1;
            let Some(pose) = after.map_work_pose(owner) else {
                continue;
            };
            if seen[pose.phase] || pose.frame < 0.3 || pose.frame > 0.7 {
                continue;
            }
            seen[pose.phase] = true;
            let retained = after.state.clone();
            observer(&mut after, owner)?;
            before.state = after.state.clone();
            let expected = after.render(640, 400)?;
            let idle = before.render(640, 400)?;
            let changed = expected.iter().zip(&idle).filter(|(a, b)| a != b).count();
            assert!(changed > 100, "Invisible work pose");
            let old_frame = before.render_frame(640, 400)?;
            let new_frame = after.render_frame(640, 400)?;
            let hidden = old_frame.triangles.len() - new_frame.triangles.len();
            assert_eq!(
                hidden,
                after.map.as_ref().unwrap().weapon.triangles.len(),
                "Work pose still carries a duplicate gun"
            );
            let mode = format!("job-{job}-phase-{}", pose.phase);
            let cp = after.checkpoint(&new_id)?;
            fs::write(out.join(format!("{mode}-checkpoint.bin")), &cp)?;
            image(&after, &out.join(format!("{mode}.ppm")))?;
            image(&before, &out.join(format!("{mode}-idle.ppm")))?;
            let mut cold = rc_game::bundle::decode(&new)?;
            cold.restore_checkpoint(&cp, &new_id)?;
            assert_eq!(cold.checkpoint(&new_id)?, cp);
            assert_eq!(cold.render(640, 400)?, expected);
            assert_eq!(cold.map_work_pose(owner), Some(pose));
            for _ in 0..10 {
                assert_eq!(after.render(640, 400)?, expected);
                assert_eq!(after.checkpoint(&new_id)?, cp);
            }
            snapshots.push(serde_json::json!({"mode":mode,"phase":pose.phase,"frame":pose.frame,"clip":if owner==0{after.map.as_ref().unwrap().squad.mesh.clips[pose.clip].sequence.name.name.clone()}else{after.map.as_ref().unwrap().gameplay.as_ref().unwrap().meshes[2].0.mesh.clips[pose.clip].sequence.name.name.clone()},"changed_pixels":changed,"hidden_weapon_triangles":hidden,"cold_state_and_frame_exact":true}));
            before.state = retained.clone();
            after.state = retained;
        }
        assert!(
            seen.iter().all(|v| *v),
            "Missing work phase for job {job}: {seen:?}"
        );
        assert_eq!(state(&before), state(&after));
        assert!(after.map_work_pose(owner).is_none());
        assert_eq!(after.render(640, 400)?, before.render(640, 400)?);
        cases.push(serde_json::json!({"job":job,"owner":owner,"gameplay_comparisons":comparisons,"snapshots":snapshots}));
        println!("Job {job}: all three original phases, hidden gun, exact cold poses and {comparisons} unchanged gameplay steps");
    }
    fs::write(
        out.join("validation.json"),
        serde_json::to_vec_pretty(
            &serde_json::json!({"passed":true,"cases":cases,"scope":"Explicit scene rebind of genuine 0.83 command fixtures after verified asset-only upgrade; no gameplay state changes. Separate supported observer camera positions are assigned only for snapshots, then both original states restored before continuing."}),
        )?,
    )?;
    Ok(())
}
fn main() -> Result<(), Box<dyn std::error::Error>> {
    std::thread::Builder::new()
        .name("work-animation-fixtures".into())
        .stack_size(16 * 1024 * 1024)
        .spawn(|| run().map_err(|e| e.to_string()))?
        .join()
        .map_err(|_| "Work pose worker panicked")??;
    Ok(())
}
