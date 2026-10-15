//! Replays the complete route through the production Game interface, without steering.
use rc_game::{Input, STEP};
use std::{fs, path::Path};
fn main() -> Result<(), Box<dyn std::error::Error>> {
    let out = Path::new("analysis/mission18-map2-route/delivery/replay");
    fs::create_dir_all(out)?;
    let scene = fs::read("analysis/mission18-map2-route/delivery/geo_01b.rcplay")?;
    let inputs = fs::read("analysis/mission18-map2-route/input-replay.bin")?;
    assert_eq!(&inputs[..8], b"RCINPUT1");
    let count = u32::from_le_bytes(inputs[8..12].try_into()?);
    assert_eq!(inputs.len(), 12 + count as usize * 20);
    let h = serde_json::from_slice(&fs::read(
        "analysis/mission15-complete/delivery/playthrough/map-handoff.json",
    )?)?;
    let mut g = rc_game::bundle::decode(&scene)?;
    g.restore_handoff(&h)?;
    let entry = serde_json::to_vec(&g.state)?;
    let first_frame = g.render_frame(320, 200)?.gpu_packet(&g.scene);
    let mut maximum_horizontal = 0_f64;
    for (i, r) in inputs[12..].chunks_exact(20).enumerate() {
        let v = |n| f32::from_le_bytes(r[n..n + 4].try_into().unwrap()) as f64;
        let b = u32::from_le_bytes(r[16..20].try_into().unwrap());
        let before = g
            .state
            .map
            .as_ref()
            .unwrap()
            .companion
            .as_ref()
            .unwrap()
            .position;
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
            1,
            "frame {i}"
        );
        let after = g
            .state
            .map
            .as_ref()
            .unwrap()
            .companion
            .as_ref()
            .unwrap()
            .position;
        let d = (after[0] - before[0]).hypot(after[1] - before[1]);
        maximum_horizontal = maximum_horizontal.max(d);
        assert!(
            d <= 450. * STEP + 0.01,
            "Companion moved without a bounded physical step"
        );
    }
    let expected: serde_json::Value =
        serde_json::from_slice(&fs::read("analysis/mission18-map2-route/final-state.json")?)?;
    let actual: serde_json::Value = serde_json::from_slice(&serde_json::to_vec(&g.state)?)?;
    fs::write(
        out.join("actual-state.json"),
        serde_json::to_vec_pretty(&actual)?,
    )?;
    assert!(
        actual == expected,
        "Final state differs; inspect actual-state.json"
    );
    assert_eq!(g.state.phase, rc_game::Phase::Won);
    let state = g.state.map.as_ref().unwrap().gameplay.as_ref().unwrap();
    assert!(state.objectives.iter().all(|v| *v));
    assert_eq!(state.event_counts["FinalHallwayDroids"], 4);
    let mut ppm = b"P6\n640 400\n255\n".to_vec();
    for p in g.render(640, 400)? {
        ppm.extend([(p >> 16) as u8, (p >> 8) as u8, p as u8]);
    }
    fs::write(out.join("exit.ppm"), ppm)?;
    let end = serde_json::to_value(&g.state)?;
    g.restart();
    assert_eq!(entry, serde_json::to_vec(&g.state)?);
    assert_eq!(first_frame, g.render_frame(320, 200)?.gpu_packet(&g.scene));
    // Collision lifecycle resets along with serialized state.
    let gate = g.map.as_ref().unwrap().gameplay.as_ref().unwrap();
    for (prepared, _asset) in gate
        .gates
        .iter()
        .zip(&gate.assets.gates)
        .filter(|(_, a)| a.delay > 0.)
    {
        let (index, _) = prepared.collider.as_ref().ok_or("Missing wreck collider")?;
        assert!(matches!(
            g.world.actors[*index].shape,
            rc_package::world_collision::BodyShape::Hulls(_)
        ));
    }
    fs::write(
        out.join("validation.json"),
        serde_json::to_vec_pretty(
            &serde_json::json!({"passed":true,"inputs":count,"exact_final_state":true,"restart_exact_state_frame_and_collision":true,"maximum_scorch_horizontal_step":maximum_horizontal,"end":end,"no_live_pose_health_enemy_or_event_overrides":true}),
        )?,
    )?;
    println!("Full GEO_01B replay passed: {count} inputs, exact end state, restart and wreck collision reset");
    Ok(())
}
