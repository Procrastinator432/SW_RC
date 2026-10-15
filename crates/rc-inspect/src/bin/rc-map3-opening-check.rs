use rc_game::{Game, Input, STEP};
use std::{fs, path::Path};
fn frame(g: &Game, out: &Path, n: &str) -> Result<(), String> {
    let mut bytes = b"P6\n640 400\n255\n".to_vec();
    for p in g.render(640, 400)? {
        bytes.extend([(p >> 16) as u8, (p >> 8) as u8, p as u8]);
    }
    fs::write(out.join(format!("{n}.ppm")), bytes).map_err(|e| e.to_string())
}
fn main() -> Result<(), Box<dyn std::error::Error>> {
    let out = Path::new("analysis/mission20-map3-opening/delivery/playthrough");
    fs::create_dir_all(out)?;
    let bytes = fs::read("analysis/mission20-map3-opening/delivery/geo_01c.rcplay")?;
    let mut g = rc_game::bundle::decode(&bytes)?;
    let handoff = serde_json::from_slice(&fs::read(
        "analysis/mission19-map3/delivery/playthrough/map2-handoff.json",
    )?)?;
    g.restore_handoff(&handoff)?;
    let initial = serde_json::to_vec(&g.state)?;
    frame(&g, out, "entry")?;
    let entry_frame = g.render_frame(320, 200)?.gpu_packet(&g.scene);
    let mut hold_reference = None;
    let mut rows = b"RCINPUT1".to_vec();
    rows.extend(1800u32.to_le_bytes());
    for i in 0..1800 {
        let input = if i >= 1500 {
            Input {
                forward: if i < 1560 { 0.5 } else { 0. },
                strafe: if (1560..1620).contains(&i) { 0.5 } else { 0. },
                fire: (1680..1692).contains(&i),
                reload: i == 1710,
                jump: i == 1650,
                crouch: (1740..1770).contains(&i),
                ..Input::default()
            }
        } else {
            Input::default()
        };
        for f in [input.forward, input.strafe, input.look[0], input.look[1]] {
            rows.extend((f as f32).to_le_bytes());
        }
        let flags = u32::from(input.jump)
            | (u32::from(input.fire) * 2)
            | (u32::from(input.reload) * 4)
            | (u32::from(input.crouch) * 64);
        rows.extend(flags.to_le_bytes());
        g.advance(STEP, input)?;
        if i == 1499 {
            hold_reference = Some(serde_json::to_vec(&g.state)?);
        }
        if i % 60 == 0 {
            let s = g.state.map.as_ref().unwrap().opening.as_ref().unwrap();
            println!(
                "{} locked={} pos={:?} blocked={:?}",
                i,
                s.locked,
                g.state.pawn.body.position,
                s.animations.iter().map(|s| s.blocked).collect::<Vec<_>>()
            );
        }
        if [90, 240, 420, 660, 1499, 1799].contains(&i) {
            frame(&g, out, &format!("frame-{i}"))?;
            fs::write(
                out.join(format!("state-{i}.json")),
                serde_json::to_vec_pretty(&g.state)?,
            )?;
        }
    }
    fs::write(out.join("input-replay.bin"), rows)?;
    let state = g.state.map.as_ref().unwrap().opening.as_ref().unwrap();
    assert!(!state.locked && state.joined && state.objectives[1]);
    assert!(state.events.contains("TargetSunFac") && state.events.contains("CheckpointLevelStart"));
    assert!(state.animations.iter().all(|a| a.finished));
    assert_eq!(g.state.health, 10);
    assert!(g.state.statistics.shots > 0);
    assert!(g.state.pawn.body.position[2] < 0.);
    assert!(state.sev_body.grounded);
    assert_eq!(state.animations[0].notifies.len(), 5);
    assert_eq!(state.animations[3].notifies.len(), 9);
    assert_eq!(state.animations[4].notifies.len(), 2);
    assert_eq!(state.animations[5].notifies.len(), 3);
    for c in g
        .state
        .map
        .as_ref()
        .unwrap()
        .companion
        .iter()
        .chain(g.state.map.as_ref().unwrap().fixer.iter())
        .chain(std::iter::once(&state.sev))
    {
        assert_ne!(
            g.world
                .body_placement(c.position, rc_game::squad::EXTENT)?
                .state,
            rc_package::movement::PlacementState::Penetrating
        );
    }
    let final_state = serde_json::to_value(&g.state)?;
    g.restart();
    assert_eq!(initial, serde_json::to_vec(&g.state)?);
    assert_eq!(entry_frame, g.render_frame(320, 200)?.gpu_packet(&g.scene));
    frame(&g, out, "restart")?;
    for fps in [20, 60, 144] {
        let mut paced = rc_game::bundle::decode(&bytes)?;
        paced.restore_handoff(&handoff)?;
        for _ in 0..fps * 25 {
            paced.advance(1. / fps as f64, Input::default())?;
        }
        assert_eq!(
            hold_reference.as_ref().unwrap(),
            &serde_json::to_vec(&paced.state)?,
            "Frame pacing {fps}"
        );
    }
    let mut adversarial = rc_game::bundle::decode(&bytes)?;
    adversarial.restore_handoff(&handoff)?;
    while adversarial
        .state
        .map
        .as_ref()
        .unwrap()
        .opening
        .as_ref()
        .unwrap()
        .locked
    {
        adversarial.advance(
            STEP,
            Input {
                forward: 1.,
                strafe: 1.,
                look: [0.5, 0.5],
                fire: true,
                reload: true,
                jump: true,
                crouch: true,
                throw_grenade: true,
                ..Input::default()
            },
        )?;
    }
    assert_eq!(
        adversarial.state.pawn.body.position,
        adversarial
            .state
            .map
            .as_ref()
            .unwrap()
            .opening
            .as_ref()
            .unwrap()
            .animations[0]
            .position
    );
    assert_eq!(
        (
            adversarial.state.ammo,
            adversarial.state.reserve,
            adversarial.state.statistics.shots,
            adversarial.state.statistics.jumps
        ),
        (20, 141, 0, 0)
    );
    adversarial.advance(STEP, Input::default())?;
    assert_eq!(adversarial.state.statistics.shots, 0);
    fs::write(
        out.join("locked-input-state.json"),
        serde_json::to_vec_pretty(&adversarial.state)?,
    )?;
    fs::write(
        out.join("validation.json"),
        serde_json::to_vec_pretty(
            &serde_json::json!({"passed":true,"inputs":1800,"entry":"Actual completed GEO_01B schema2 handoff fixture","no_live_pose_resource_event_overrides":true,"exact_restart":true,"exact_restart_frame":true,"frame_rates_exact":[20,60,144],"locked_inputs_do_not_spend_or_move":true,"original_notify_counts":[5,9,2,3],"sev_grounded":true,"final":final_state}),
        )?,
    )?;
    println!("Opening complete, Sev joined, original objective1 and checkpoint event, normal controls and restart passed");
    Ok(())
}
