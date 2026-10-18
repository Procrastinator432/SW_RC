//! Rejoin the recorded exit with regular inputs after the original UsingItem move/turn.
//! No position, health, collision or simulation-state fixture is installed.
use rc_game::{Game, Input, STEP};
use std::{env, fs, path::Path};
fn input(row: &[u8]) -> Input {
    let v = |i| f32::from_le_bytes(row[i..i + 4].try_into().unwrap()) as f64;
    let flags = u32::from_le_bytes(row[16..20].try_into().unwrap());
    Input {
        forward: v(0),
        strafe: v(4),
        look: [v(8), v(12)],
        jump: flags & 1 != 0,
        fire: flags & 2 != 0,
        reload: flags & 4 != 0,
        walking: flags & 8 != 0,
        use_action: flags & 16 != 0,
        throw_grenade: flags & 32 != 0,
        crouch: flags & 64 != 0,
    }
}
fn advance(game: &mut Game, row: &[u8]) -> Result<(), String> {
    if game.advance(STEP, input(row))? != 1 {
        return Err("Replay ended before Bacta release".into());
    }
    Ok(())
}
fn main() -> Result<(), Box<dyn std::error::Error>> {
    let args: Vec<_> = env::args().collect();
    if args.len() != 4 {
        return Err("usage: rc-bacta-replay-build SCENE REPLAY OUT".into());
    }
    let scene = fs::read(&args[1])?;
    if !scene.starts_with(b"RCPLAY13") {
        return Err("Expected opening scene".into());
    }
    let n = u32::from_le_bytes(scene[8..12].try_into()?) as usize;
    let mut metadata: serde_json::Value = serde_json::from_slice(&scene[12..12 + n])?;
    if metadata["bacta"]["controller"].is_null() {
        return Err("Missing original UsingItem bindings".into());
    }
    metadata["bacta"]
        .as_object_mut()
        .ok_or("Missing station")?
        .remove("controller");
    // The pinned recording originally approached the legacy station box.
    // Rejoin that recording's exit, including its pre-use collision behavior.
    metadata["bacta"]
        .as_object_mut()
        .unwrap()
        .remove("collision");
    let meta = serde_json::to_vec(&metadata)?;
    let mut baseline = b"RCPLAY13".to_vec();
    baseline.extend((meta.len() as u32).to_le_bytes());
    baseline.extend(meta);
    baseline.extend_from_slice(&scene[12 + n..]);
    let mut old = rc_game::bundle::decode(&baseline)?;
    let mut new = rc_game::bundle::decode(&scene)?;
    let mut replay = fs::read(&args[2])?;
    if replay.len() < 12
        || &replay[..8] != b"RCINPUT1"
        || (replay.len() - 12) % 20 != 0
        || u32::from_le_bytes(replay[8..12].try_into()?) as usize != (replay.len() - 12) / 20
    {
        return Err("Invalid recorded inputs".into());
    }
    let original_fingerprint = rc_game::replay::fingerprint(&replay);
    let mut receipt = None;
    let mut insertion = vec![];
    let mut insert_at = 0;
    for (i, row) in replay[12..].chunks_exact(20).enumerate() {
        if new.state.mission.as_ref().is_some_and(|m| m.completed) && !input(row).use_action {
            if !old.state.mission.as_ref().is_some_and(|m| m.completed) {
                return Err("Baseline Bacta was not completed".into());
            }
            let start_position = new.state.pawn.body.position;
            let start_view = [new.state.yaw, new.state.pitch];
            let target = old.state.pawn.body.position;
            for step in 0..900 {
                let p = new.state.pawn.body.position;
                let v = new.state.pawn.body.velocity;
                let delta = [target[0] - p[0], target[1] - p[1]];
                if step > 0 && delta[0].hypot(delta[1]) < 0.1 && v[0].hypot(v[1]) < 0.1 {
                    break;
                }
                let yaw = ((new.state.yaw * 65536. / std::f64::consts::TAU) as i32 >> 2) & 16383;
                let (sin, cos) = (f64::from(yaw) * std::f64::consts::TAU / 16384.).sin_cos();
                let acceleration: [f64; 2] = std::array::from_fn(|j| {
                    (20. * delta[j] - 4. * v[j]) / new.profile.controller.acceleration
                });
                let forward =
                    (acceleration[0] * cos + acceleration[1] * sin).clamp(-0.25, 0.25) as f32;
                let strafe =
                    (acceleration[0] * sin - acceleration[1] * cos).clamp(-0.25, 0.25) as f32;
                let correction = if step == 0 {
                    [
                        (new.state.yaw - old.state.yaw + std::f64::consts::PI)
                            .rem_euclid(std::f64::consts::TAU)
                            - std::f64::consts::PI,
                        new.state.pitch - old.state.pitch,
                    ]
                } else {
                    [0.; 2]
                };
                let mut controls = vec![];
                for value in [forward, strafe, correction[0] as f32, correction[1] as f32] {
                    controls.extend(value.to_le_bytes());
                }
                controls.extend(0u32.to_le_bytes());
                advance(&mut new, &controls)?;
                insertion.extend(controls);
            }
            let p = new.state.pawn.body.position;
            if (p[0] - target[0]).hypot(p[1] - target[1]) >= 0.1
                || new.state.pawn.body.velocity[0].hypot(new.state.pawn.body.velocity[1]) >= 0.1
            {
                return Err(format!(
                    "Recorded movement could not rejoin station exit: {p:?} vs {target:?}"
                )
                .into());
            }
            insert_at = 12 + i * 20;
            receipt = Some(
                serde_json::json!({"input_row":i,"inserted_rows":insertion.len()/20,"baseline_position":target,"before_position":start_position,"after_position":p,"before_view":start_view,"baseline_view":[old.state.yaw,old.state.pitch],"policy":"Insert regular movement/look rows after StopUse to rejoin the previous station-exit position/view. All original rows remain byte-identical; no runtime pose/health assignment. Full mission acceptance is checked by rc-checkpoint-check."}),
            );
            break;
        }
        advance(&mut old, row)?;
        advance(&mut new, row)?;
    }
    let receipt = receipt.ok_or("No completed-station release in recording")?;
    replay.splice(insert_at..insert_at, insertion);
    let count = ((replay.len() - 12) / 20) as u32;
    replay[8..12].copy_from_slice(&count.to_le_bytes());
    let out = Path::new(&args[3]);
    fs::create_dir_all(out)?;
    fs::write(out.join("map_a_replay.bin"), &replay)?;
    fs::write(
        out.join("exit-rejoin.json"),
        serde_json::to_vec_pretty(
            &serde_json::json!({"input_fingerprint":original_fingerprint,"output_fingerprint":rc_game::replay::fingerprint(&replay),"correction":receipt}),
        )?,
    )?;
    println!("Rejoined station-exit recording: {receipt}");
    Ok(())
}
