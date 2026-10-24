//! Original work equipment fixtures and retained-progress sound delivery.
use rc_game::{Game, Input, STEP};
use std::{env, fs, path::Path};
fn work(g: &Game) -> &rc_game::map_gameplay::State {
    g.state.map.as_ref().unwrap().gameplay.as_ref().unwrap()
}
fn normalized(g: &Game) -> serde_json::Value {
    let mut v = serde_json::to_value(&g.state).unwrap();
    v["map"]["events"] = serde_json::json!([]);
    v["map"]["audio_sequence"] = serde_json::json!(0);
    if let Some(w) = v
        .get_mut("visuals")
        .and_then(|v| v.get_mut("weapon_animation"))
        .and_then(|v| v.as_object_mut())
    {
        w.remove("audio");
    }
    v
}
fn count(g: &Game) -> usize {
    let p = g
        .map
        .as_ref()
        .unwrap()
        .gameplay
        .as_ref()
        .unwrap()
        .work_equipment
        .as_ref()
        .unwrap();
    g.state
        .map
        .as_ref()
        .unwrap()
        .events
        .iter()
        .filter(|e| p.assets.sounds.iter().any(|s| s.sound == e.sound))
        .count()
}
fn placement(g: &Game, job: usize) -> Option<usize> {
    let s = work(g);
    let a = &g
        .map
        .as_ref()
        .unwrap()
        .gameplay
        .as_ref()
        .unwrap()
        .work_equipment
        .as_ref()
        .unwrap()
        .assets;
    rc_game::work_equipment::charge_skin(
        s.progress[job],
        a.placed_at[job]?,
        s.armed[job],
        s.used[job],
    )
}
fn photo(g: &Game, path: &Path) -> Result<(), Box<dyn std::error::Error>> {
    let mut ppm = b"P6\n640 400\n255\n".to_vec();
    for p in g.render(640, 400)? {
        ppm.extend([(p >> 16) as u8, (p >> 8) as u8, p as u8]);
    }
    fs::write(path, ppm)?;
    Ok(())
}
fn observer(
    g: &mut Game,
    owner: usize,
    radius: f64,
    target: Option<rc_game::Point>,
) -> Result<(), String> {
    let m = g.state.map.as_ref().unwrap();
    let c = if owner == 0 {
        m.companion.as_ref().unwrap()
    } else {
        &m.gameplay.as_ref().unwrap().fixer
    };
    let at = c.position;
    let yaw = c.yaw;
    for angle in [
        std::f64::consts::FRAC_PI_2,
        -std::f64::consts::FRAC_PI_2,
        std::f64::consts::PI,
        0.,
        std::f64::consts::FRAC_PI_4,
        -std::f64::consts::FRAC_PI_4,
        3. * std::f64::consts::FRAC_PI_4,
        -3. * std::f64::consts::FRAC_PI_4,
    ] {
        let a = yaw + angle;
        if let Some(p) = rc_game::mission_motion::ground(
            &g.world,
            [
                at[0] + a.cos() * radius,
                at[1] + a.sin() * radius,
                at[2] + 160.,
            ],
            g.profile.physics.extent,
            400.,
        )? {
            let aim = target.unwrap_or([at[0], at[1], at[2] + 15.]);
            let eye = [p[0], p[1], p[2] + g.profile.physics.extent[2] * 0.65];
            let q = g.world.sweep_motion(eye, aim, [0.; 3])?;
            let allowed = g
                .map
                .as_ref()
                .unwrap()
                .gameplay
                .as_ref()
                .unwrap()
                .assets
                .interactions
                .iter()
                .find(|j| j.position == aim)
                .map(|j| &j.actor);
            if radius < 180.
                && ((eye[0] - aim[0]).powi(2)
                    + (eye[1] - aim[1]).powi(2)
                    + (eye[2] - aim[2]).powi(2))
                .sqrt()
                    >= 240.
            {
                continue;
            }
            if q.complete
                && q.contacts
                    .iter()
                    .all(|c| allowed == Some(&c.object) || c.hit.fraction > 0.96)
                && g.world.body_placement(p, g.profile.physics.extent)?.state
                    != rc_package::movement::PlacementState::Penetrating
            {
                g.state.pawn.body.position = p;
                g.state.pawn.body.velocity = [0.; 3];
                g.state.pawn.body.grounded = true;
                g.aim_at(aim);
                assert_eq!(g.advance(0., Input::default())?, 0);
                return Ok(());
            }
        }
    }
    Err("No supported observer point".into())
}
fn run() -> Result<(), Box<dyn std::error::Error>> {
    let a: Vec<_> = env::args_os().collect();
    let old = fs::read(&a[1])?;
    let new = fs::read(&a[2])?;
    let source = Path::new(&a[3]);
    let out = Path::new(&a[4]);
    fs::create_dir_all(out)?;
    let oid = format!("{}:{}", old.len(), rc_game::replay::fingerprint(&old));
    let nid = format!("{}:{}", new.len(), rc_game::replay::fingerprint(&new));
    let mut cases = vec![];
    for job in 0..5 {
        let raw = fs::read(source.join(format!("job-{job}-phase-0-checkpoint.bin")))?;
        let mut before = rc_game::bundle::decode(&old)?;
        before.restore_checkpoint(&raw, &oid)?;
        let mut after = rc_game::bundle::decode(&new)?;
        after.restore_checkpoint(&before.checkpoint(&nid)?, &nid)?;
        assert_eq!(normalized(&before), normalized(&after));
        assert_eq!(
            before.scene.triangles.len() - after.scene.triangles.len(),
            2052
        );
        let p = after.map.as_ref().unwrap().gameplay.as_ref().unwrap();
        let demo = p.assets.interactions[job].demolition;
        let owner = usize::from(!demo);
        let expected = p.work_equipment.as_ref().unwrap().assets.schedules[job].len();
        let target = p.assets.interactions[job].position;
        let mut progress = work(&after).progress[job];
        let mut saved_pause = false;
        let mut saved_placed = false;
        let mut steps = 0;
        if demo {
            assert_eq!(placement(&after, job), None);
            photo(&after, &out.join(format!("job-{job}-hand.ppm")))?;
            fs::write(
                out.join(format!("job-{job}-hand.bin")),
                after.checkpoint(&nid)?,
            )?;
        }
        for _ in 0..600 {
            if work(&after).armed[job] || work(&after).used[job] {
                break;
            }
            let input = Input::default();
            before.advance(STEP, input)?;
            after.advance(STEP, input)?;
            steps += 1;
            assert_eq!(normalized(&before), normalized(&after));
            assert!(work(&after).progress[job] >= progress);
            progress = work(&after).progress[job];
            if demo && !saved_placed && placement(&after, job) == Some(0) {
                saved_placed = true;
                photo(&after, &out.join(format!("job-{job}-placed.ppm")))?;
                fs::write(
                    out.join(format!("job-{job}-placed.bin")),
                    after.checkpoint(&nid)?,
                )?;
            }
            if !saved_pause && progress > 1.2 {
                saved_pause = true;
                observer(&mut after, owner, 110., Some(target))?;
                before.state.pawn.body = after.state.pawn.body;
                before.state.yaw = after.state.yaw;
                before.state.pitch = after.state.pitch;
                before.advance(
                    STEP,
                    Input {
                        use_action: true,
                        ..Default::default()
                    },
                )?;
                after.advance(
                    STEP,
                    Input {
                        use_action: true,
                        ..Default::default()
                    },
                )?;
                assert!(
                    work(&after).execution.as_ref().unwrap().tasks[owner]
                        .as_ref()
                        .unwrap()
                        .paused,
                    "Pause failed for {job}: eye {:?} target {:?} trace {:?} held {}",
                    after.eye(),
                    target,
                    after.world.sweep_motion(after.eye(), target, [0.; 3])?,
                    work(&after).execution.as_ref().unwrap().held
                );
                let at = progress;
                let cues = count(&after);
                let cp = after.checkpoint(&nid)?;
                fs::write(out.join(format!("job-{job}-paused.bin")), &cp)?;
                photo(&after, &out.join(format!("job-{job}-paused.ppm")))?;
                let mut cold = rc_game::bundle::decode(&new)?;
                cold.restore_checkpoint(&cp, &nid)?;
                assert_eq!(cp, cold.checkpoint(&nid)?);
                assert_eq!(after.render(640, 400)?, cold.render(640, 400)?);
                for _ in 0..30 {
                    before.advance(STEP, Input::default())?;
                    after.advance(STEP, Input::default())?;
                    assert_eq!(normalized(&before), normalized(&after));
                    assert_eq!(work(&after).progress[job], at);
                    assert_eq!(count(&after), cues);
                }
                before.advance(
                    STEP,
                    Input {
                        use_action: true,
                        ..Default::default()
                    },
                )?;
                after.advance(
                    STEP,
                    Input {
                        use_action: true,
                        ..Default::default()
                    },
                )?;
                assert_eq!(normalized(&before), normalized(&after));
            }
        }
        assert!(work(&after).armed[job] || work(&after).used[job]);
        assert_eq!(
            count(&after),
            expected,
            "Missing/repeated work sounds for job {job}"
        );
        if demo {
            assert!(saved_placed);
            assert_eq!(placement(&after, job), Some(1));
            observer(&mut after, owner, 430., Some(target))?;
            photo(&after, &out.join(format!("job-{job}-armed.ppm")))?;
            fs::write(
                out.join(format!("job-{job}-armed.bin")),
                after.checkpoint(&nid)?,
            )?;
            after.advance(
                STEP,
                Input {
                    fire: true,
                    ..Default::default()
                },
            )?;
            assert!(work(&after).used[job]);
            assert_eq!(placement(&after, job), None);
            photo(&after, &out.join(format!("job-{job}-detonated.ppm")))?;
            fs::write(
                out.join(format!("job-{job}-detonated.bin")),
                after.checkpoint(&nid)?,
            )?;
        }
        cases.push(serde_json::json!({"job":job,"steps":steps,"sounds":expected,"paused_no_replay":true,"cold_state_and_frame_exact":true,"unchanged_gameplay_except_audio":true,"charge_hand_placed_armed_removed":demo}));
        println!("Work job {job}: {expected} original cues, pause/restore, unchanged gameplay and charge visibility passed");
    }
    fs::write(
        out.join("validation.json"),
        serde_json::to_vec_pretty(
            &serde_json::json!({"passed":true,"cases":cases,"work_sound_events":46,"static_charge_triangles_replaced":2052,"scope":"Explicit scene rebind of sanitized 0.84 visual observer fixtures. Supported observer camera positions assigned, no assigned squad positions. Gameplay comparison excludes only map audio events/counter and weapon audio records; sounds and charge visibility tested separately."}),
        )?,
    )?;
    Ok(())
}
fn main() -> Result<(), Box<dyn std::error::Error>> {
    std::thread::Builder::new()
        .name("work-equipment-fixtures".into())
        .stack_size(16 * 1024 * 1024)
        .spawn(|| run().map_err(|e| e.to_string()))?
        .join()
        .map_err(|_| "Work equipment worker panicked")??;
    Ok(())
}
