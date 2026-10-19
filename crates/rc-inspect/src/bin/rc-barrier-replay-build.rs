//! Rejoin the pinned route after free barrier dynamics using ordinary controls.
use rc_game::{Game, Input, STEP};
use std::{env, fs, path::Path};
fn input(row: &[u8]) -> Input {
    let v = |i| f32::from_le_bytes(row[i..i + 4].try_into().unwrap()) as f64;
    let b = u32::from_le_bytes(row[16..20].try_into().unwrap());
    Input {
        forward: v(0),
        strafe: v(4),
        look: [v(8), v(12)],
        jump: b & 1 != 0,
        fire: b & 2 != 0,
        reload: b & 4 != 0,
        walking: b & 8 != 0,
        use_action: b & 16 != 0,
        throw_grenade: b & 32 != 0,
        crouch: b & 64 != 0,
    }
}
fn step(g: &mut Game, row: &[u8]) -> Result<(), String> {
    if g.advance(STEP, input(row))? != 1 {
        return Err(format!("Barrier route stopped: {:?}", g.state.phase));
    }
    Ok(())
}
fn row(forward: f32, strafe: f32, jump: bool, look: [f32; 2]) -> Vec<u8> {
    let mut row = vec![];
    for v in [forward, strafe, look[0], look[1]] {
        row.extend(v.to_le_bytes());
    }
    row.extend(u32::from(jump).to_le_bytes());
    row
}
fn select_charge(game: &mut Game, output: &mut Vec<u8>) -> Result<(), String> {
    let charge = game.demolition.as_ref().unwrap().assets.charge;
    let range = game.demolition.as_ref().unwrap().assets.flow.player_range - 30.;
    let original_view = [game.state.yaw, game.state.pitch];
    let at = game.state.pawn.body.position;
    let distance = (charge[0] - at[0]).hypot(charge[1] - at[1]);
    let target: [f64; 2] =
        std::array::from_fn(|k| charge[k] + (at[k] - charge[k]) * (range / distance).min(1.));
    for _ in 0..300 {
        let p = game.state.pawn.body.position;
        let v = game.state.pawn.body.velocity;
        let delta = [target[0] - p[0], target[1] - p[1]];
        if delta[0].hypot(delta[1]) < 0.05 && v[0].hypot(v[1]) < 0.05 {
            break;
        }
        let yaw = ((game.state.yaw * 65536. / std::f64::consts::TAU) as i32 >> 2) & 16383;
        let (sin, cos) = (f64::from(yaw) * std::f64::consts::TAU / 16384.).sin_cos();
        let a: [f64; 2] = std::array::from_fn(|k| {
            (20. * delta[k] - 4. * v[k]) / game.profile.controller.acceleration
        });
        let r = row(
            (a[0] * cos + a[1] * sin).clamp(-0.25, 0.25) as f32,
            (a[0] * sin - a[1] * cos).clamp(-0.25, 0.25) as f32,
            false,
            [0.; 2],
        );
        step(game, &r)?;
        output.extend(r);
    }
    let eye = game.eye();
    let d: [f64; 3] = std::array::from_fn(|k| charge[k] - eye[k]);
    let target_yaw = d[1].atan2(d[0]);
    let target_pitch = d[2].atan2(d[0].hypot(d[1]));
    let wrap = |v: f64| {
        (v + std::f64::consts::PI).rem_euclid(std::f64::consts::TAU) - std::f64::consts::PI
    };
    let mut use_row = row(
        0.,
        0.,
        false,
        [
            wrap(game.state.yaw - target_yaw) as f32,
            (game.state.pitch - target_pitch) as f32,
        ],
    );
    use_row[16..20].copy_from_slice(&16u32.to_le_bytes());
    step(game, &use_row)?;
    output.extend(use_row);
    if !game
        .state
        .demolition
        .as_ref()
        .unwrap()
        .flow
        .plunger_selected
    {
        return Err("Ordinary movement/aim/use could not select the armed charge".into());
    }
    let restore = row(
        0.,
        0.,
        false,
        [
            wrap(game.state.yaw - original_view[0]) as f32,
            (game.state.pitch - original_view[1]) as f32,
        ],
    );
    step(game, &restore)?;
    output.extend(restore);
    Ok(())
}
fn main() -> Result<(), Box<dyn std::error::Error>> {
    let args: Vec<_> = env::args().collect();
    if args.len() != 4 {
        return Err("usage: rc-barrier-replay-build SCENE REPLAY OUT".into());
    }
    let scene = fs::read(&args[1])?;
    if !scene.starts_with(b"RCPLAY13") {
        return Err("Expected selected opening".into());
    }
    let n = u32::from_le_bytes(scene[8..12].try_into()?) as usize;
    let mut meta: serde_json::Value = serde_json::from_slice(&scene[12..12 + n])?;
    if meta["rigid"].as_array().is_none_or(|a| a.len() != 4) {
        return Err("Missing four free barriers".into());
    }
    meta.as_object_mut().unwrap().remove("rigid");
    meta.as_object_mut().unwrap().remove("rigid_interactions");
    let meta = serde_json::to_vec(&meta)?;
    let mut legacy = b"RCPLAY13".to_vec();
    legacy.extend((meta.len() as u32).to_le_bytes());
    legacy.extend(meta);
    legacy.extend_from_slice(&scene[12 + n..]);
    let mut old = rc_game::bundle::decode(&legacy)?;
    let mut new = rc_game::bundle::decode(&scene)?;
    let mut replay = fs::read(&args[2])?;
    if replay.len() < 12
        || &replay[..8] != b"RCINPUT1"
        || (replay.len() - 12) % 20 != 0
        || u32::from_le_bytes(replay[8..12].try_into()?) as usize != (replay.len() - 12) / 20
    {
        return Err("Invalid input recording".into());
    }
    let before = rc_game::replay::fingerprint(&replay);
    let mut insertion = vec![];
    let mut receipt = None;
    let mut insert_at = 0;
    for (i, r) in replay[12..].chunks_exact(20).enumerate() {
        let cleared = |g: &Game| {
            g.state
                .mission
                .as_ref()
                .and_then(|m| m.grenades.as_ref())
                .is_some_and(|s| s.walls_released && s.secured_seconds >= 3.)
        };
        if cleared(&old)
            && old.state.pawn.body.velocity[0].hypot(old.state.pawn.body.velocity[1]) < 0.08
            && old.state.pawn.body.grounded
            && !input(r).crouch
        {
            let target = old.state.pawn.body.position;
            let from = new.state.pawn.body.position;
            let route = &new
                .mission
                .as_ref()
                .unwrap()
                .grenades
                .as_ref()
                .unwrap()
                .route;
            let end = old
                .state
                .mission
                .as_ref()
                .unwrap()
                .grenades
                .as_ref()
                .unwrap()
                .waypoint
                .min(route.len() - 1);
            let nearest = (0..=end)
                .min_by(|a, b| {
                    let distance = |i: usize| (route[i][0] - from[0]).hypot(route[i][1] - from[1]);
                    distance(*a).total_cmp(&distance(*b))
                })
                .unwrap();
            let mut waypoints = route[nearest..=end].to_vec();
            waypoints.push(target);
            let mut waypoint = 0;
            let mut previous = from;
            let mut blocked = 0;
            // Let ongoing bodies settle while preserving all live mission logic.
            for _ in 0..180 {
                let r = row(0., 0., false, [0.; 2]);
                step(&mut new, &r)?;
                insertion.extend(r);
            }
            for j in 0..4800 {
                let p = new.state.pawn.body.position;
                let v = new.state.pawn.body.velocity;
                let goal = waypoints[waypoint];
                let delta = [goal[0] - p[0], goal[1] - p[1]];
                let distance = delta[0].hypot(delta[1]);
                if waypoint + 1 < waypoints.len() && distance < 1. && v[0].hypot(v[1]) < 1. {
                    waypoint += 1;
                    continue;
                }
                if distance < 0.08
                    && waypoint + 1 == waypoints.len()
                    && v[0].hypot(v[1]) < 0.08
                    && (p[2] - target[2]).abs() < 1.
                    && new.state.pawn.body.grounded
                {
                    break;
                }
                if (p[0] - previous[0]).hypot(p[1] - previous[1]) < 0.05 && distance > 10. {
                    blocked += 1;
                } else {
                    blocked = 0;
                }
                previous = p;
                let yaw = ((new.state.yaw * 65536. / std::f64::consts::TAU) as i32 >> 2) & 16383;
                let (sin, cos) = (f64::from(yaw) * std::f64::consts::TAU / 16384.).sin_cos();
                let acceleration: [f64; 2] = std::array::from_fn(|k| {
                    (20. * delta[k] - 4. * v[k]) / new.profile.controller.acceleration
                });
                let forward =
                    (acceleration[0] * cos + acceleration[1] * sin).clamp(-0.7, 0.7) as f32;
                let strafe =
                    (acceleration[0] * sin - acceleration[1] * cos).clamp(-0.7, 0.7) as f32;
                let jump = blocked >= 15 && j % 45 == 0;
                let correction = if j == 0 {
                    [
                        (new.state.yaw - old.state.yaw + std::f64::consts::PI)
                            .rem_euclid(std::f64::consts::TAU)
                            - std::f64::consts::PI,
                        new.state.pitch - old.state.pitch,
                    ]
                } else {
                    [0.; 2]
                };
                let controls = row(forward, strafe, jump, correction.map(|v| v as f32));
                step(&mut new, &controls)?;
                insertion.extend(controls);
            }
            let p = new.state.pawn.body.position;
            if (p[0] - target[0]).hypot(p[1] - target[1]) >= 0.08 || (p[2] - target[2]).abs() >= 1.
            {
                return Err(format!("Could not rejoin barrier exit with ordinary controls: {p:?} -> {target:?}; from {from:?}").into());
            }
            insert_at = 12 + i * 20;
            receipt = Some(
                serde_json::json!({"input_row":i,"inserted_rows":insertion.len()/20,"from":from,"target":target,"arrived":p,"waypoints":waypoints,"policy":"Ordinary movement/look/jump rows at a stationary reference exit, following the existing original route anchors; 180 quiet ticks allow live free bodies to settle. No runtime pose, health, inventory or collision assignment."}),
            );
            break;
        }
        step(&mut old, r)?;
        step(&mut new, r)?;
    }
    let mut receipt = receipt.ok_or("Baseline never released barriers")?;
    // The counterfactual legacy scene supplies only the stationary exit target.
    // Its old rest-pose collision is not shipped and is not an acceptance run.
    // Require the actual, freely moving scene to complete below.
    let mut consumed = 0;
    let mut continuation = vec![];
    let mut charge_rows = 0;
    for r in replay[insert_at..].chunks_exact(20) {
        if new.state.phase == rc_game::Phase::Won {
            break;
        }
        step(&mut new, r)?;
        continuation.extend_from_slice(r);
        consumed += 20;
        if input(r).use_action
            && new.state.demolition.as_ref().is_some_and(|s| {
                s.flow.phase == rc_game::demolition::Phase::Armed && !s.flow.plunger_selected
            })
        {
            let before = continuation.len();
            select_charge(&mut new, &mut continuation)?;
            charge_rows += (continuation.len() - before) / 20;
        }
    }
    if new.state.phase != rc_game::Phase::Won {
        fs::create_dir_all(&args[3])?;
        fs::write(
            Path::new(&args[3]).join("incomplete.json"),
            serde_json::to_vec_pretty(&new.state)?,
        )?;
        return Err(format!(
            "Corrected route did not complete: {:?}, {:?}",
            new.state.phase, new.state.pawn.body.position
        )
        .into());
    }
    receipt["unused_rows_after_win"] = ((replay.len() - insert_at - consumed) / 20).into();
    receipt["completed_health"] = new.state.health.into();
    receipt["charge_targeting_rows"] = charge_rows.into();
    replay.truncate(insert_at);
    replay.extend(insertion);
    replay.extend(continuation);
    let count = ((replay.len() - 12) / 20) as u32;
    replay[8..12].copy_from_slice(&count.to_le_bytes());
    let out = Path::new(&args[3]);
    fs::create_dir_all(out)?;
    fs::write(out.join("map_a_replay.bin"), &replay)?;
    fs::write(
        out.join("barrier-rejoin.json"),
        serde_json::to_vec_pretty(
            &serde_json::json!({"input_fingerprint":before,"output_fingerprint":rc_game::replay::fingerprint(&replay),"correction":receipt}),
        )?,
    )?;
    println!("Barrier route rejoined: {receipt}");
    Ok(())
}
