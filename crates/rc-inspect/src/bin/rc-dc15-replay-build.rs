//! Adapt campaign inputs using ordinary look/fire rows for the energy weapon.
//! Bounded movement corrections follow legacy route targets; full acceptance follows.
use rc_game::{Game, Input, Phase, STEP};
use std::{env, fs, path::Path};
fn input(r: &[u8]) -> Input {
    let f = |i| f32::from_le_bytes(r[i..i + 4].try_into().unwrap()) as f64;
    let b = u32::from_le_bytes(r[16..20].try_into().unwrap());
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
fn step(g: &mut Game, i: Input) -> Result<(), String> {
    if g.advance(STEP, i)? != 1 {
        return Err(format!(
            "Energy replay stopped: {:?} tick {}",
            g.state.phase, g.state.statistics.ticks
        ));
    }
    Ok(())
}
fn vulnerable(g: &Game) -> bool {
    g.state
        .mission
        .as_ref()
        .and_then(|m| m.crash.as_ref())
        .is_some_and(|c| c.flow.combat == rc_game::crash::CombatStage::Vulnerable)
}
fn aimed_row(
    g: &Game,
    skip: &std::collections::BTreeSet<usize>,
) -> Result<(Vec<u8>, Option<usize>), String> {
    let eye = g.eye();
    let mut targets = vec![];
    for (i, e) in g
        .state
        .enemies
        .iter()
        .enumerate()
        .filter(|(i, e)| e.health > 0 && !skip.contains(i))
    {
        let p = [e.position[0], e.position[1], e.position[2] + 35.];
        if g.wall_fraction(eye, p)? >= 1. - 1e-6 {
            targets.push((e.health, i, p));
        }
    }
    let mut row = vec![0; 20];
    let mut target = None;
    if let Some((_, i, p)) = targets.into_iter().min_by_key(|(hp, _, _)| *hp) {
        target = Some(i);
        let d: [f64; 3] = std::array::from_fn(|i| p[i] - eye[i]);
        let yaw = d[1].atan2(d[0]);
        let pitch = d[2].atan2(d[0].hypot(d[1])).clamp(-1.4, 1.4);
        let look = [
            (g.state.yaw - yaw + std::f64::consts::PI).rem_euclid(std::f64::consts::TAU)
                - std::f64::consts::PI,
            g.state.pitch - pitch,
        ];
        for (k, v) in look.into_iter().enumerate() {
            row[8 + k * 4..12 + k * 4].copy_from_slice(&(v as f32).to_le_bytes());
        }
        row[16..20].copy_from_slice(&2_u32.to_le_bytes());
    }
    Ok((row, target))
}
fn legacy_camera_scene(raw: &[u8]) -> Result<Vec<u8>, Box<dyn std::error::Error>> {
    let mut at = 0;
    let mut out = vec![];
    loop {
        let n = u32::from_le_bytes(
            raw.get(at + 8..at + 12)
                .ok_or("Short reference layer")?
                .try_into()?,
        ) as usize;
        let end = at + 12 + n;
        if raw.get(at..at + 8) == Some(b"RCPLAY01") {
            let mut base: serde_json::Value =
                serde_json::from_slice(raw.get(at + 12..end).ok_or("Short reference metadata")?)?;
            base.as_object_mut()
                .ok_or("Invalid reference metadata")?
                .remove("dc15");
            let data = serde_json::to_vec(&base)?;
            out.extend_from_slice(b"RCPLAY01");
            out.extend((data.len() as u32).to_le_bytes());
            out.extend(data);
            out.extend_from_slice(&raw[end..]);
            return Ok(out);
        }
        out.extend_from_slice(raw.get(at..end).ok_or("Short reference layer")?);
        at = end;
    }
}
fn main() -> Result<(), Box<dyn std::error::Error>> {
    let args: Vec<_> = env::args().collect();
    if args.len() < 4 {
        return Err(
            "usage: rc-dc15-replay-build SCENE REPLAY OUT [PREVIOUS_SCENE PREVIOUS_REPLAY]..."
                .into(),
        );
    }
    let prefix_end = args.len() - (args.len() - 4) % 2;
    let reference_entry: Option<rc_game::transition::Handoff> = if prefix_end < args.len() {
        Some(serde_json::from_slice(&fs::read(&args[prefix_end])?)?)
    } else {
        None
    };
    let scene = fs::read(&args[1])?;
    let raw = fs::read(&args[2])?;
    if raw.len() < 12 || &raw[..8] != b"RCINPUT1" || (raw.len() - 12) % 20 != 0 {
        return Err("Invalid recording".into());
    }
    let out = Path::new(&args[3]);
    fs::create_dir_all(out)?;
    let identity = format!("{}:{}", scene.len(), rc_game::replay::fingerprint(&scene));
    let mut previous: Option<Game> = None;
    for pair in args[4..prefix_end].chunks_exact(2) {
        let bytes = fs::read(&pair[0])?;
        let mut old = if let Some(p) = previous {
            p.travel_to(&bytes)?
        } else {
            rc_game::bundle::decode(&bytes)?
        };
        let replay = fs::read(&pair[1])?;
        if replay.len() < 12 || &replay[..8] != b"RCINPUT1" || (replay.len() - 12) % 20 != 0 {
            return Err("Invalid campaign prefix recording".into());
        }
        for row in replay[12..].chunks_exact(20) {
            step(&mut old, input(row))?;
        }
        if old.state.phase != Phase::Won {
            return Err("Campaign prefix did not complete".into());
        }
        previous = Some(old);
    }
    let entry = previous
        .as_ref()
        .map(|p| p.campaign_handoff())
        .transpose()?
        .flatten();
    let mut g = rc_game::bundle::decode(&scene)?;
    if let Some(h) = &entry {
        g.restore_handoff(h)?;
    }
    if g.dc15.is_none() {
        return Err("Need original energy profile".into());
    }
    let mut reference = if g.map.is_some() {
        let mut old = rc_game::bundle::decode(&legacy_camera_scene(&scene)?)?;
        if let Some(h) = &reference_entry {
            old.restore_handoff(h)?;
        } else if let Some(h) = &entry {
            let mut legacy = h.clone();
            let player = legacy
                .player
                .as_object_mut()
                .ok_or("Invalid entry player")?;
            player.remove("dc15");
            player.remove("dc15_fire_remaining");
            player.insert("ammo".into(), rc_game::MAGAZINE.into());
            player.insert("reserve".into(), 200.into());
            old.restore_handoff(&legacy)?;
        }
        Some(old)
    } else {
        None
    };
    let mut output = vec![];
    let mut view = [g.state.yaw, g.state.pitch];
    let mut camera_dirty = false;
    let mut adapted = false;
    let mut inserted = 0;
    let mut consumed = 0;
    let empty = std::collections::BTreeSet::new();
    let mut attempted = std::collections::BTreeSet::new();
    for r in raw[12..].chunks_exact(20) {
        let mut row = r.to_vec();
        let raw_input = input(r);
        let reference_body = reference.as_ref().map(|old| old.state.pawn.body);
        if let Some(old) = &mut reference {
            step(old, raw_input)?;
            if old.state.phase == Phase::Lost {
                fs::write(
                    out.join("reference-lost.json"),
                    serde_json::to_vec_pretty(&old.state)?,
                )?;
                fs::write(
                    out.join("end-state.json"),
                    serde_json::to_vec_pretty(&g.state)?,
                )?;
                return Err(format!(
                    "Legacy route reference lost at source row {}, HP {}",
                    consumed + 1,
                    old.state.health
                )
                .into());
            }
        }
        if !camera_dirty {
            view = [g.state.yaw, g.state.pitch];
        }
        consumed += 1;
        if consumed % 2000 == 0 {
            eprintln!("Energy preparation: source row {consumed}, live tick {}, phase {:?}, position {:?}, reference {:?}", g.state.statistics.ticks, g.state.phase, g.state.pawn.body.position, reference.as_ref().map(|r| r.state.pawn.body.position));
        }
        if !adapted && vulnerable(&g) && raw_input.forward == 0. && raw_input.strafe == 0. {
            adapted = true;
            camera_dirty = true;
            for _ in 0..1800 {
                if !vulnerable(&g) {
                    break;
                }
                let (shot, _) = aimed_row(&g, &empty)?;
                step(&mut g, input(&shot))?;
                output.extend(shot);
                inserted += 1;
                if g.state.phase != Phase::Playing {
                    break;
                }
            }
        }
        if g.state.phase != Phase::Playing {
            break;
        }
        if g.map.is_some() && raw_input.fire && raw_input.forward == 0. && raw_input.strafe == 0. {
            let mut ineffective = 0;
            let mut seen = std::collections::BTreeSet::new();
            for _ in 0..900 {
                let (shot, target) = aimed_row(&g, &attempted)?;
                if let Some(i) = target {
                    seen.insert(i);
                }
                if !input(&shot).fire {
                    break;
                }
                camera_dirty = true;
                let shots = g.state.statistics.shots;
                let health: u64 = g.state.enemies.iter().map(|e| u64::from(e.health)).sum();
                step(&mut g, input(&shot))?;
                output.extend(shot);
                inserted += 1;
                if g.state.statistics.shots > shots {
                    let after: u64 = g.state.enemies.iter().map(|e| u64::from(e.health)).sum();
                    ineffective = if after >= health { ineffective + 1 } else { 0 };
                }
                if g.state.phase != Phase::Playing || ineffective >= 3 {
                    break;
                }
            }
            attempted.extend(seen);
        }
        if g.state.phase != Phase::Playing {
            break;
        }
        if let Some(old) = &reference {
            view = [old.state.yaw, old.state.pitch];
        } else {
            view[0] = (view[0] - raw_input.look[0]).rem_euclid(std::f64::consts::TAU);
            view[1] = (view[1] - raw_input.look[1]).clamp(-1.4, 1.4);
        }
        let desired = view;
        if let Some(body) = reference_body {
            let q = ((desired[0] * 65536. / std::f64::consts::TAU) as i32 >> 2) & 16383;
            let (sin, cos) = (f64::from(q) * std::f64::consts::TAU / 16384.).sin_cos();
            let correction: [f64; 2] = std::array::from_fn(|k| {
                (20. * (body.position[k] - g.state.pawn.body.position[k])
                    + 4. * (body.velocity[k] - g.state.pawn.body.velocity[k]))
                    / g.profile.controller.acceleration
            });
            let forward =
                (raw_input.forward + correction[0] * cos + correction[1] * sin).clamp(-1., 1.);
            let strafe =
                (raw_input.strafe + correction[0] * sin - correction[1] * cos).clamp(-1., 1.);
            row[0..4].copy_from_slice(&(forward as f32).to_le_bytes());
            row[4..8].copy_from_slice(&(strafe as f32).to_le_bytes());
        }
        let restoring = camera_dirty;
        camera_dirty = false;
        let delta = [
            (g.state.yaw - desired[0] + std::f64::consts::PI).rem_euclid(std::f64::consts::TAU)
                - std::f64::consts::PI,
            g.state.pitch - desired[1],
        ];
        if restoring || camera_dirty || reference.is_some() {
            for (k, v) in delta.into_iter().enumerate() {
                row[8 + k * 4..12 + k * 4].copy_from_slice(&(v as f32).to_le_bytes());
            }
        }
        let i = input(&row);
        step(&mut g, i)?;
        output.extend(row);
        if g.state.phase != Phase::Playing {
            break;
        }
    }
    fs::write(
        out.join("end-state.json"),
        serde_json::to_vec_pretty(&g.state)?,
    )?;
    if let Some(old) = &reference {
        fs::write(
            out.join("reference-end-state.json"),
            serde_json::to_vec_pretty(&old.state)?,
        )?;
    }
    if g.state.phase == Phase::Lost {
        return Err(format!(
            "Energy inputs did not complete: {:?}, tick {}, HP {}",
            g.state.phase, g.state.statistics.ticks, g.state.health
        )
        .into());
    }
    let mut fresh = rc_game::bundle::decode(&scene)?;
    if let Some(h) = &entry {
        fresh.restore_handoff(h)?;
    }
    for r in output.chunks_exact(20) {
        step(&mut fresh, input(r))?;
    }
    if fresh.checkpoint(&identity)? != g.checkpoint(&identity)? {
        return Err("Independent energy recording differs".into());
    }
    let count = output.len() / 20;
    let mut bytes = b"RCINPUT1".to_vec();
    bytes.extend((count as u32).to_le_bytes());
    bytes.extend(output);
    fs::write(out.join("replay.bin"), bytes)?;
    fs::write(
        out.join("preparation.json"),
        serde_json::to_vec_pretty(
            &serde_json::json!({"prepared":true,"mission_completed":g.state.phase==Phase::Won,"ticks":count,"health":g.state.health,"inserted_combat_rows":inserted,"discarded_rows_after_win":(raw.len()-12)/20-consumed,"independent_replay_exact":true,"policy":"Ordinary look/fire inputs finish visible enemies at stationary recording rows. For B/C, a counterfactual legacy-weapon scene supplies route and view targets; bounded movement inputs correct position/velocity drift. No runtime position, health, actor, energy or objective state assignment; recording independently replayed from entry. Barrier route adaptation and full mission acceptance follow in the central pipeline."}),
        )?,
    )?;
    println!("Energy recording prepared: {count} ticks");
    Ok(())
}
