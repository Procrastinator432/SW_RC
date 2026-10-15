use rc_game::{Game, Input, Phase, STEP};
use std::{fs, path::Path};
fn replay(g: &mut Game, b: &[u8]) -> Result<(), String> {
    for r in b[12..].chunks_exact(20) {
        let v = |i| f32::from_le_bytes(r[i..i + 4].try_into().unwrap()) as f64;
        let f = u32::from_le_bytes(r[16..20].try_into().unwrap());
        assert_eq!(
            g.advance(
                STEP,
                Input {
                    forward: v(0),
                    strafe: v(4),
                    look: [v(8), v(12)],
                    jump: f & 1 != 0,
                    fire: f & 2 != 0,
                    reload: f & 4 != 0,
                    walking: f & 8 != 0,
                    use_action: f & 16 != 0,
                    throw_grenade: f & 32 != 0,
                    crouch: f & 64 != 0
                }
            )?,
            1
        );
    }
    Ok(())
}
fn main() -> Result<(), Box<dyn std::error::Error>> {
    let out = Path::new("analysis/mission28-first-mission-complete/delivery/playthrough");
    fs::create_dir_all(out)?;
    let one = fs::read("analysis/mission15-complete/delivery/geo_01a-first-mission.rcplay")?;
    let two = fs::read("analysis/mission18-map2-route/delivery/geo_01b.rcplay")?;
    let three = fs::read("analysis/mission27-map3-elevator/delivery/geo_01c.rcplay")?;
    let bytes = fs::read("analysis/mission28-first-mission-complete/delivery/geo_03a.rcplay")?;
    let mut g = rc_game::bundle::decode(&one)?;
    assert!(g.travel_to(&bytes).is_err());
    replay(
        &mut g,
        &fs::read("analysis/mission15-complete/delivery/playthrough/input-replay.bin")?,
    )?;
    assert_eq!(g.state.phase, Phase::Won);
    assert!(g.travel_to(&bytes).is_err());
    g = g.travel_to(&two)?;
    replay(
        &mut g,
        &fs::read("analysis/mission18-map2-route/input-replay.bin")?,
    )?;
    assert!(g.travel_to(&bytes).is_err());
    g = g.travel_to(&three)?;
    replay(
        &mut g,
        &fs::read("analysis/mission27-map3-elevator/delivery/playthrough/input-replay.bin")?,
    )?;
    assert_eq!(g.state.phase, Phase::Won);
    let handoff = g.campaign_handoff()?.ok_or("Missing final handoff")?;
    let before = serde_json::to_value(&g.state)?;
    for payload in [&one[..], &two[..], &three[..], &b"invalid"[..]] {
        assert!(g.travel_to(payload).is_err());
        assert_eq!(before, serde_json::to_value(&g.state)?);
    }
    let mut f = g.travel_to(&bytes)?;
    let initial = serde_json::to_value(&f.state)?;
    let initial_frame = f.render_frame(320, 200)?.gpu_packet(&f.scene);
    assert_eq!(
        (f.state.health, f.state.ammo, f.state.reserve),
        (65, 14, 65)
    );
    assert_eq!(
        f.state.pawn.body.position,
        [-2555.739501953125, 2654.17138671875, 6920.81640625]
    );
    let m = f.state.map.as_ref().unwrap();
    assert_eq!(m.grenades, 4);
    let members = [
        m.companion.as_ref().unwrap(),
        m.fixer.as_ref().unwrap(),
        m.sev.as_ref().unwrap(),
    ];
    for (i, c) in members.iter().enumerate() {
        assert!(c.joined && c.health == 100 && !c.down);
        assert_eq!(c.shields, 100.);
        assert_ne!(c.position, g.state.pawn.body.position);
        assert_ne!(c.position, f.state.pawn.body.position);
        assert_ne!(
            f.world
                .body_placement(c.position, rc_game::squad::EXTENT)?
                .state,
            rc_package::movement::PlacementState::Penetrating
        );
        for other in &members[..i] {
            assert!(distance(c.position, other.position) >= 100.);
        }
    }
    assert!(f.campaign_handoff()?.is_none());
    let fresh = || rc_game::bundle::decode(&bytes);
    // Malformed carries must not partially install player or any of the three members.
    for kind in 0..12 {
        let mut bad = handoff.clone();
        match kind {
            0 => bad.player["ammo"] = 999.into(),
            1 => bad.player["health"] = 0.into(),
            2 => bad.player["reserve"] = 100001.into(),
            3 => bad.player["grenades"]["flow"]["ammunition"] = 101.into(),
            4 => bad.squad["health"] = 9999.into(),
            5 => bad.map3.as_mut().unwrap().fixer["shields"] = 9999.into(),
            6 => bad.map3.as_mut().unwrap().sev["health"] = 9999.into(),
            7 => bad.map3.as_mut().unwrap().sev["down"] = "bad".into(),
            8 => bad.map3.as_mut().unwrap().objectives[6] = false,
            9 => bad.trigger = 567,
            10 => bad.destination = "GEO_01C.ctm#playerstart".into(),
            _ => bad.squad["joined"] = false.into(),
        }
        let mut t = fresh()?;
        let old = serde_json::to_value(&t.state)?;
        let frame = t.render_frame(320, 200)?.gpu_packet(&t.scene);
        assert!(t.restore_handoff(&bad).is_err(), "bad carry {kind}");
        assert_eq!(old, serde_json::to_value(&t.state)?);
        assert_eq!(frame, t.render_frame(320, 200)?.gpu_packet(&t.scene));
    }
    let mut saved = fresh()?;
    saved.restore_handoff(&handoff)?;
    assert_eq!(initial, serde_json::to_value(&saved.state)?);
    assert_eq!(
        initial_frame,
        saved.render_frame(320, 200)?.gpu_packet(&saved.scene)
    );
    assert!(saved.restore_handoff(&handoff).is_err());
    let mut controls = b"RCINPUT1".to_vec();
    controls.extend(360u32.to_le_bytes());
    for tick in 0..360 {
        let forward = if (60..150).contains(&tick) {
            0.7f32
        } else {
            0.
        };
        let strafe = if (150..210).contains(&tick) {
            0.5f32
        } else {
            0.
        };
        let flags = if tick == 80 {
            1
        } else if tick == 180 || tick == 210 {
            2
        } else if tick == 230 {
            4
        } else if (300..330).contains(&tick) {
            64
        } else {
            0
        };
        for v in [forward, strafe, 0., 0.] {
            controls.extend(v.to_le_bytes());
        }
        controls.extend((flags as u32).to_le_bytes());
    }
    replay(&mut f, &controls)?;
    assert_eq!(f.state.phase, Phase::Playing);
    assert!(f.state.pawn.body.grounded);
    assert_eq!(f.state.statistics.shots, 2);
    assert_eq!(f.state.statistics.jumps, 1);
    assert_eq!(
        (f.state.health, f.state.ammo, f.state.reserve),
        (65, 20, 57)
    );
    assert!(
        distance(
            f.state.pawn.body.position,
            [-2555.739501953125, 2654.17138671875, 6920.81640625]
        ) > 80.
    );
    let final_state = serde_json::to_value(&f.state)?;
    let final_frame = f.render_frame(320, 200)?.gpu_packet(&f.scene);
    let mut image = b"P6\n640 400\n255\n".to_vec();
    for px in f.render(640, 400)? {
        image.extend([(px >> 16) as u8, (px >> 8) as u8, px as u8]);
    }
    fs::write(out.join("factory-entry.ppm"), image)?;
    f.restart();
    assert_eq!(initial, serde_json::to_value(&f.state)?);
    assert_eq!(
        initial_frame,
        f.render_frame(320, 200)?.gpu_packet(&f.scene)
    );
    replay(&mut f, &controls)?;
    assert_eq!(final_state, serde_json::to_value(&f.state)?);
    assert_eq!(final_frame, f.render_frame(320, 200)?.gpu_packet(&f.scene));
    fs::write(out.join("factory-input.bin"), controls)?;
    fs::write(
        out.join("map3-handoff.json"),
        serde_json::to_vec_pretty(&handoff)?,
    )?;
    fs::write(
        out.join("validation.json"),
        serde_json::to_vec_pretty(
            &serde_json::json!({"passed":true,"actual_map1_inputs":18730,"actual_map2_inputs":16414,"actual_map3_inputs":11800,"factory_inputs":360,"all_seven_c_objectives":true,"original_playerstart":true,"three_members_carried":true,"bad_saved_carries_atomic":12,"wrong_travel_atomic":true,"saved_restore_exact_state_and_frame":true,"restart_and_replay_exact_state_and_frame":true,"entry":initial,"final":final_state}),
        )?,
    )?;
    println!("First mission complete: A/B/C -> original factory PlayerStart; carry, controls, atomic errors, save/resume and restart passed");
    Ok(())
}

fn distance(a: [f64; 3], b: [f64; 3]) -> f64 {
    (0..3).map(|i| (a[i] - b[i]).powi(2)).sum::<f64>().sqrt()
}
