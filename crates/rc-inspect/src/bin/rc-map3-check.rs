//! Actual A/B controller replay, atomic third-map travel, two-member carry and restart.
use rc_game::{Game, Input, Phase, STEP};
use std::{fs, path::Path};
fn replay(g: &mut Game, bytes: &[u8]) -> Result<u32, String> {
    if &bytes[..8] != b"RCINPUT1" {
        return Err("Bad replay".into());
    }
    let n = u32::from_le_bytes(bytes[8..12].try_into().unwrap());
    assert_eq!(bytes.len(), 12 + n as usize * 20);
    for r in bytes[12..].chunks_exact(20) {
        let v = |i| f32::from_le_bytes(r[i..i + 4].try_into().unwrap()) as f64;
        let b = u32::from_le_bytes(r[16..20].try_into().unwrap());
        assert_eq!(
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
                    crouch: b & 64 != 0
                }
            )?,
            1
        );
    }
    Ok(n)
}
fn frame(g: &Game, p: &Path) -> Result<(), String> {
    let mut b = b"P6\n640 400\n255\n".to_vec();
    for p in g.render(640, 400)? {
        b.extend([(p >> 16) as u8, (p >> 8) as u8, p as u8]);
    }
    fs::write(p, b).map_err(|e| e.to_string())
}
fn main() -> Result<(), Box<dyn std::error::Error>> {
    let out = Path::new("analysis/mission19-map3/delivery/playthrough");
    fs::create_dir_all(out)?;
    let one = fs::read("analysis/mission15-complete/delivery/geo_01a-first-mission.rcplay")?;
    let two = fs::read("analysis/mission18-map2-route/delivery/geo_01b.rcplay")?;
    let three = fs::read("analysis/mission19-map3/delivery/geo_01c.rcplay")?;
    let mut g = rc_game::bundle::decode(&one)?;
    assert!(g.travel_to(&three).is_err());
    let n1 = replay(
        &mut g,
        &fs::read("analysis/mission15-complete/delivery/playthrough/input-replay.bin")?,
    )?;
    assert_eq!(g.state.phase, Phase::Won);
    assert!(g.travel_to(&three).is_err());
    g = g.travel_to(&two)?;
    assert!(g.campaign_handoff()?.is_none());
    let before = serde_json::to_vec(&g.state)?;
    assert!(g.travel_to(&three).is_err());
    assert_eq!(before, serde_json::to_vec(&g.state)?);
    let n2 = replay(
        &mut g,
        &fs::read("analysis/mission18-map2-route/input-replay.bin")?,
    )?;
    assert_eq!(g.state.phase, Phase::Won);
    let h = g.campaign_handoff()?.ok_or("No map2 handoff")?;
    assert_eq!(
        serde_json::to_vec(&h)?,
        serde_json::to_vec(&g.campaign_handoff()?.unwrap())?
    );
    assert_eq!((h.schema, h.trigger), (2, 567));
    fs::write(
        out.join("map2-handoff.json"),
        serde_json::to_vec_pretty(&h)?,
    )?;
    let final_b = serde_json::to_vec(&g.state)?;
    assert!(g.travel_to(&two).is_err());
    assert!(g.travel_to(b"invalid").is_err());
    assert_eq!(final_b, serde_json::to_vec(&g.state)?);
    let mut c = g.travel_to(&three)?;
    let entry = serde_json::to_vec(&c.state)?;
    assert_eq!(
        (c.state.health, c.state.ammo, c.state.reserve),
        (10, 20, 141)
    );
    let m = c.state.map.as_ref().unwrap();
    assert_eq!(m.grenades, g.state.map.as_ref().unwrap().grenades);
    assert_eq!(
        m.companion.as_ref().unwrap().health,
        h.squad["health"].as_u64().unwrap() as u32
    );
    assert_eq!(
        m.fixer.as_ref().unwrap().health,
        h.map2.as_ref().unwrap().fixer["health"].as_u64().unwrap() as u32
    );
    let p = c.state.pawn.body.position;
    assert_eq!(p, [-20039.7890625, 12575.505859375, 1144.]);
    for member in m.companion.iter().chain(m.fixer.iter()) {
        assert!(distance(p, member.position) < 500.);
        assert_ne!(
            c.world
                .body_placement(member.position, rc_game::squad::EXTENT)?
                .state,
            rc_package::movement::PlacementState::Penetrating
        );
    }
    assert!(
        distance(
            m.companion.as_ref().unwrap().position,
            m.fixer.as_ref().unwrap().position
        ) >= 100.
    );
    let packet = c.render_frame(320, 200)?.gpu_packet(&c.scene);
    frame(&c, &out.join("entry.ppm"))?;
    fs::write(out.join("entry-state.json"), &entry)?;
    let mut saved = rc_game::bundle::decode(&three)?;
    let clean = serde_json::to_vec(&saved.state)?;
    for case in 0..9 {
        let mut bad = h.clone();
        match case {
            0 => bad.destination = "GEO_01B.ctm#playerstart".into(),
            1 => bad.source = "GEO_01A.ctm".into(),
            2 => bad.schema = 1,
            3 => bad.player["ammo"] = 999.into(),
            4 => bad.map2.as_mut().unwrap().fixer["health"] = 9999.into(),
            5 => bad.map2.as_mut().unwrap().objectives[7] = false,
            6 => bad.map2 = None,
            7 => bad.map2.as_mut().unwrap().fixer["down"] = serde_json::Value::Null,
            _ => bad.trigger = 9,
        }
        assert!(saved.restore_handoff(&bad).is_err(), "bad case {case}");
        assert_eq!(clean, serde_json::to_vec(&saved.state)?);
    }
    saved.restore_handoff(&h)?;
    assert_eq!(entry, serde_json::to_vec(&saved.state)?);
    assert_eq!(
        packet,
        saved.render_frame(320, 200)?.gpu_packet(&saved.scene)
    );
    assert!(saved.restore_handoff(&h).is_err());
    let mut inputs = b"RCINPUT1".to_vec();
    inputs.extend(360u32.to_le_bytes());
    for i in 0..360 {
        let flags = (if (180..192).contains(&i) { 2 } else { 0 })
            | (if i == 210 { 4 } else { 0 })
            | (if i == 30 { 1 } else { 0 })
            | (if (240..270).contains(&i) { 64 } else { 0 });
        let v = [
            if (60..90).contains(&i) { 0.5_f32 } else { 0. },
            if (100..130).contains(&i) { 0.5_f32 } else { 0. },
            0.,
            0.,
        ];
        for f in v {
            inputs.extend(f.to_le_bytes());
        }
        inputs.extend((flags as u32).to_le_bytes());
    }
    replay(&mut c, &inputs)?;
    fs::write(out.join("input-replay.bin"), inputs)?;
    assert_eq!(c.state.phase, Phase::Playing);
    assert!(
        c.state.statistics.shots > 0
            && c.state.statistics.reloads > 0
            && c.state.statistics.jumps > 0
    );
    let moved = distance(p, c.state.pawn.body.position);
    assert!(moved > 100.);
    let explored = serde_json::to_value(&c.state)?;
    frame(&c, &out.join("controls.ppm"))?;
    assert!(c.campaign_handoff()?.is_none());
    assert!(c.restore_handoff(&h).is_err());
    c.restart();
    assert_eq!(entry, serde_json::to_vec(&c.state)?);
    assert_eq!(packet, c.render_frame(320, 200)?.gpu_packet(&c.scene));
    fs::write(
        out.join("validation.json"),
        serde_json::to_vec_pretty(
            &serde_json::json!({"passed":true,"real_map1_inputs":n1,"real_map2_inputs":n2,"map3_inputs":360,"early_wrong_corrupt_travel_atomic":true,"invalid_saved_receipts_atomic":9,"two_members_carried":true,"original_playerstart":true,"safe_separate_companion_placements":true,"saved_restore_exact_state_frame":true,"restart_exact_state_frame":true,"movement_distance":moved,"entry":serde_json::from_slice::<serde_json::Value>(&entry)?,"explored":explored,"scope":"Third map import and travel; original Sun Fac mission gameplay is not yet integrated"}),
        )?,
    )?;
    println!("Actual A({n1})→B({n2})→C(360) passed; both companions, inventory, atomic rejection, saved resume and exact restart");
    Ok(())
}

fn distance(a: [f64; 3], b: [f64; 3]) -> f64 {
    (0..3).map(|i| (a[i] - b[i]).powi(2)).sum::<f64>().sqrt()
}
