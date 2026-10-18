//! Ordinary input/player-camera evidence through Bacta, demolition and A→B.
use rc_game::{Game, Input, STEP};
use std::{collections::BTreeMap, env, fs, path::Path};
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
fn capture(
    g: &Game,
    out: &Path,
    tick: usize,
    label: &str,
) -> Result<(), Box<dyn std::error::Error>> {
    let frame = g.render_frame(640, 400)?;
    let mut ppm = b"P6\n640 400\n255\n".to_vec();
    for pixel in frame.render_cpu(&g.scene)? {
        ppm.extend([(pixel >> 16) as u8, (pixel >> 8) as u8, pixel as u8]);
    }
    fs::write(out.join(format!("{tick:05}-{label}.ppm")), ppm)?;
    fs::write(
        out.join(format!("{tick:05}-{label}.json")),
        serde_json::to_vec_pretty(
            &serde_json::json!({"state":g.state,"camera_basis":rc_render::gpu::basis(&g.scene,frame.view),"effect_triangles":frame.effects.len(),"scope":"Ordinary recorded player inputs and runtime camera; no pose/health/aim fixture"}),
        )?,
    )?;
    fs::write(
        out.join(format!("{tick:05}-{label}.frame")),
        frame.gpu_packet(&g.scene),
    )?;
    Ok(())
}
fn main() -> Result<(), Box<dyn std::error::Error>> {
    let args: Vec<_> = env::args().collect();
    if args.len() != 5 {
        return Err("usage: rc-presentation-check MAP_A REPLAY_A MAP_B OUT".into());
    }
    let scene = fs::read(&args[1])?;
    let replay = fs::read(&args[2])?;
    let map_b = fs::read(&args[3])?;
    let out = Path::new(&args[4]);
    fs::create_dir_all(out)?;
    if replay.len() < 12 || &replay[..8] != b"RCINPUT1" || (replay.len() - 12) % 20 != 0 {
        return Err("Invalid input recording".into());
    }
    let identity = format!("{}:{}", scene.len(), rc_game::replay::fingerprint(&scene));
    let mut g = rc_game::bundle::decode(&scene)?;
    let rows: Vec<_> = replay[12..].chunks_exact(20).collect();
    let mut pending = BTreeMap::new();
    pending.insert(0, "entry".to_string());
    let mut views = vec![];
    let mut seen = std::collections::BTreeSet::new();
    let mut checks = vec![];
    let mut injured = false;
    let mut phase = String::new();
    let mut door_started = false;
    let mut door_open = false;
    let mut door_sound = false;
    capture(&g, out, 0, "entry")?;
    for (i, row) in rows.iter().enumerate() {
        if g.advance(STEP, input(row))? != 1 {
            return Err(format!("Map A stopped at input {i}: {:?}", g.state.phase).into());
        }
        let tick = i + 1;
        let opening = g.state.opening.as_ref().ok_or("Missing opening")?;
        if opening.injured && !injured {
            injured = true;
            let fx = opening
                .world_effects
                .as_ref()
                .ok_or("Missing injury effect state")?;
            let at = fx
                .locations
                .iter()
                .find(|(id, _, _)| *id == 823)
                .ok_or("Original injury teleport was omitted")?
                .1;
            if at != g.state.pawn.body.position.map(|v| v as f32) {
                return Err("Injury effect did not capture player position".into());
            }
            pending.insert(tick, "injury".into());
            pending.insert(tick + 20, "injury-effect".into());
        }
        if let Some(b) = &opening.bacta {
            let next = format!("{:?}", b.phase);
            if next != phase {
                phase = next;
                pending.insert(tick, format!("bacta-{phase}"));
                if phase == "Active" {
                    pending.insert(tick + 90, "bacta-active-mid".into());
                }
                if phase == "Holstering" {
                    pending.insert(tick + 60, "bacta-body-tail".into());
                }
            }
        }
        if let Some(fx) = &opening.world_effects {
            for id in &fx.started {
                if seen.insert(*id) {
                    pending.insert(tick, format!("effect-{id}"));
                    pending.insert(tick + 6, format!("effect-{id}-start"));
                    pending.insert(tick + 45, format!("effect-{id}-mid"));
                }
            }
        }
        if let Some(s) = &g.state.spire {
            if let Some(p) = &g.spire {
                let count = s.effects.iter().filter(|(_, effect)| matches!(effect, rc_game::spire::Effect::Sound(role) if *role == p.assets.flow.opening)).count();
                if count > 1 {
                    return Err("Door opening sound repeated".into());
                }
                if count == 1 && !door_sound {
                    if p.assets
                        .door_motion
                        .first()
                        .is_some_and(|m| s.flow.door_elapsed < m.delay)
                    {
                        return Err("Door sound played before original DelayTime".into());
                    }
                    door_sound = true;
                }
            }
            if s.flow.door_started && !door_started {
                door_started = true;
                pending.insert(tick, "door-start".into());
                if let Some(m) = g.spire.as_ref().and_then(|p| p.assets.door_motion.first()) {
                    pending.insert(
                        tick + ((m.delay + m.seconds * 0.5) / STEP) as usize,
                        "door-moving".into(),
                    );
                }
            }
            if s.flow.door_open && !door_open {
                door_open = true;
                pending.insert(tick, "door-open".into());
            }
        }
        if let Some(label) = pending.remove(&tick) {
            capture(&g, out, tick, &label)?;
            views.push(serde_json::json!({"tick":tick,"label":label}));
            if [
                "effect-823-mid",
                "bacta-active-mid",
                "bacta-body-tail",
                "door-moving",
                "door-start",
            ]
            .contains(&label.as_str())
            {
                let saved = g.checkpoint(&identity)?;
                let mut restored = rc_game::bundle::decode(&scene)?;
                restored.restore_checkpoint(&saved, &identity)?;
                assert_eq!(saved, restored.checkpoint(&identity)?);
                assert_eq!(
                    g.render_frame(160, 100)?.gpu_packet(&g.scene),
                    restored.render_frame(160, 100)?.gpu_packet(&restored.scene)
                );
                let mut original = rc_game::bundle::decode(&scene)?;
                original.restore_checkpoint(&saved, &identity)?;
                for row in rows.iter().skip(tick).take(30) {
                    original.advance(STEP, input(row))?;
                    restored.advance(STEP, input(row))?;
                }
                assert_eq!(
                    original.checkpoint(&identity)?,
                    restored.checkpoint(&identity)?
                );
                checks.push(label);
            }
        }
    }
    fs::write(
        out.join("map-a-final.json"),
        serde_json::to_vec_pretty(&g.state)?,
    )?;
    if g.state.phase != rc_game::Phase::Won
        || !door_open
        || !door_sound
        || !seen.contains(&823)
        || !seen.contains(&617)
        || !seen.contains(&1135)
    {
        return Err(format!(
            "Incomplete presentation acceptance: phase={:?}, door={door_open}, effects={seen:?}",
            g.state.phase
        )
        .into());
    }
    let mut b = g.travel_to(&map_b)?;
    capture(&b, out, 0, "map-b-entry")?;
    for _ in 0..60 {
        b.advance(STEP, Input::default())?;
    }
    capture(&b, out, 60, "map-b-live")?;
    fs::write(
        out.join("validation.json"),
        serde_json::to_vec_pretty(
            &serde_json::json!({"passed":true,"scene_identity":identity,"replay_fingerprint":rc_game::replay::fingerprint(&replay),"map_a_ticks":rows.len(),"effects":seen,"views":views,"checkpoint_continuations":checks,"door_started":door_started,"door_open":door_open,"door_sound_after_original_delay":door_sound,"map_b_phase":b.state.phase,"boundary":"Native CPU/player-camera acceptance; Android/GLES/device hearing and global original-engine parity remain separate"}),
        )?,
    )?;
    println!(
        "Presentation accepted through A→B; {} ordinary-player views",
        views.len() + 3
    );
    Ok(())
}
