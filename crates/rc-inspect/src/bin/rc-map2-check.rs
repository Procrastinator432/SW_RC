//! Real first-map input replay, transactional travel, then map2 controls/reset.
use rc_game::{Game, Input, Phase, STEP};
use std::{env, fs, path::Path};
fn frame(g: &Game, out: &Path, name: &str) -> Result<(), Box<dyn std::error::Error>> {
    let mut ppm = b"P6\n640 400\n255\n".to_vec();
    for p in g.render(640, 400)? {
        ppm.extend([(p >> 16) as u8, (p >> 8) as u8, p as u8]);
    }
    fs::write(out.join(format!("{name}.ppm")), ppm)?;
    Ok(())
}
fn main() -> Result<(), Box<dyn std::error::Error>> {
    let a: Vec<_> = env::args().collect();
    if a.len() != 5 {
        return Err("usage: rc-map2-check MAP1 MAP2 INPUT OUT".into());
    }
    let one = fs::read(&a[1])?;
    let two = fs::read(&a[2])?;
    let input = fs::read(&a[3])?;
    let out = Path::new(&a[4]);
    fs::create_dir_all(out)?;
    let mut g = rc_game::bundle::decode(&one)?;
    let before = serde_json::to_vec(&g.state)?;
    assert!(g.travel_to(&two).is_err());
    assert_eq!(before, serde_json::to_vec(&g.state)?);
    assert_eq!(&input[..8], b"RCINPUT1");
    for row in input[12..].chunks_exact(20) {
        let v = |i| f32::from_le_bytes(row[i..i + 4].try_into().unwrap()) as f64;
        let b = u32::from_le_bytes(row[16..20].try_into().unwrap());
        g.advance(
            STEP,
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
            },
        )?;
    }
    assert_eq!(g.state.phase, Phase::Won);
    let prior = serde_json::to_vec(&g.state)?;
    assert!(g.travel_to(b"invalid").is_err());
    assert!(g.travel_to(&one).is_err());
    assert_eq!(prior, serde_json::to_vec(&g.state)?);
    let handoff = g.state.spire.as_ref().unwrap().handoff.as_ref().unwrap();
    let mut bad = handoff.clone();
    bad.player["ammo"] = serde_json::json!(999);
    let mut standalone = rc_game::bundle::decode(&two)?;
    let clean = serde_json::to_vec(&standalone.state)?;
    assert!(standalone.restore_handoff(&bad).is_err());
    assert_eq!(clean, serde_json::to_vec(&standalone.state)?);
    let mut next = g.travel_to(&two)?;
    assert_eq!(next.state.phase, Phase::Playing);
    assert_eq!(next.state.statistics.ticks, 0);
    assert_eq!(
        (next.state.health, next.state.ammo, next.state.reserve),
        (g.state.health, g.state.ammo, g.state.reserve)
    );
    assert!(
        next.state.opening.is_none() && next.state.spire.is_none() && next.state.mission.is_none()
    );
    let c = next.state.map.as_ref().unwrap().companion.as_ref().unwrap();
    assert!(c.joined);
    assert_eq!(c.health, g.state.squad.as_ref().unwrap().health);
    assert!(next.state.map.as_ref().unwrap().restored);
    assert!(next.restore_handoff(handoff).is_err());
    let entry = serde_json::to_vec(&next.state)?;
    let graphics = next.render_frame(320, 200)?.gpu_packet(&next.scene);
    frame(&next, out, "entry")?;
    let origin = next.state.pawn.body.position;
    let mut crouched = false;
    for i in 0..360 {
        next.advance(
            STEP,
            Input {
                forward: if i < 120 { 0.5 } else { 0. },
                strafe: if (120..180).contains(&i) { 0.5 } else { 0. },
                fire: (180..192).contains(&i),
                reload: i == 210,
                jump: i == 90,
                crouch: (240..270).contains(&i),
                ..Input::default()
            },
        )?;
        assert_eq!(next.state.phase, Phase::Playing);
        crouched |= next.state.crouched;
    }
    let moved = ((0..3)
        .map(|i| (next.state.pawn.body.position[i] - origin[i]).powi(2))
        .sum::<f64>())
    .sqrt();
    assert!(
        moved > 30.
            && crouched
            && next.state.statistics.shots > 0
            && next.state.statistics.reloads > 0
    );
    assert!(next
        .state
        .map
        .as_ref()
        .unwrap()
        .events
        .iter()
        .any(|e| e.sound == 0));
    frame(&next, out, "exploration")?;
    let explored = serde_json::to_value(&next.state)?;
    next.restart();
    assert_eq!(entry, serde_json::to_vec(&next.state)?);
    assert_eq!(
        graphics,
        next.render_frame(320, 200)?.gpu_packet(&next.scene)
    );
    next.advance(
        STEP,
        Input {
            look: [std::f64::consts::PI, 0.],
            ..Input::default()
        },
    )?;
    frame(&next, out, "companion")?;
    fs::write(
        out.join("validation.json"),
        serde_json::to_vec_pretty(
            &serde_json::json!({"passed":true,"first_map_inputs":g.state.statistics.ticks,"normal_first_map_inputs_only":true,"early_and_invalid_travel_atomic":true,"invalid_inventory_atomic":true,"entry":serde_json::from_slice::<serde_json::Value>(&entry)?,"explored":explored,"distance_moved":moved,"movement_jump_crouch_fire_reload":true,"restart_exact_state_and_frame":true,"scope":"Original map2 environment/entry and bounded weapon/companion host; map2 mission scripts/encounters remain pending"}),
        )?,
    )?;
    println!("Map1→Map2 passed: {} first-map inputs; preserved HP/ammo/squad; moved {moved:.2}; controls and exact reset",g.state.statistics.ticks);
    Ok(())
}
