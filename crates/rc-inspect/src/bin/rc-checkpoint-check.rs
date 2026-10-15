use rc_game::{Game, Input, Phase, STEP};
use std::fs;
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
fn image(g: &Game) -> Result<Vec<u8>, String> {
    Ok(g.render_frame(160, 100)?.gpu_packet(&g.scene))
}
fn main() -> Result<(), Box<dyn std::error::Error>> {
    let out = rc_inspect::build_inputs::output()?;
    fs::create_dir_all(&out)?;
    let stages = [
        (
            "A",
            rc_inspect::build_inputs::input("map_a_scene")?,
            rc_inspect::build_inputs::input("map_a_replay")?,
            vec![1, 600, 1930, 4000, 10000, 15000, 18730],
        ),
        (
            "B",
            rc_inspect::build_inputs::input("map_b_scene")?,
            rc_inspect::build_inputs::input("map_b_replay")?,
            vec![1, 2000, 6000, 10000, 14000, 16414],
        ),
        (
            "C",
            rc_inspect::build_inputs::input("map_c_scene")?,
            rc_inspect::build_inputs::input("map_c_replay")?,
            vec![
                1, 900, 2000, 3400, 4700, 6000, 7300, 8000, 9000, 10000, 11000, 11800,
            ],
        ),
    ];
    let mut game: Option<Game> = None;
    let mut checks = vec![];
    for (label, path, recording, samples) in stages {
        let bytes = fs::read(path)?;
        let identity = format!("{}:{}", bytes.len(), rc_game::replay::fingerprint(&bytes));
        let mut g = if let Some(previous) = game.take() {
            previous.travel_to(&bytes)?
        } else {
            rc_game::bundle::decode(&bytes)?
        };
        let entry = serde_json::to_value(&g.state)?;
        let entry_frame = image(&g)?;
        let rows = fs::read(recording)?;
        let count = (rows.len() - 12) / 20;
        let mut i = 0;
        while i < count {
            replay(
                &mut g,
                &[
                    b"RCINPUT1".as_slice(),
                    &1u32.to_le_bytes(),
                    &rows[12 + i * 20..12 + (i + 1) * 20],
                ]
                .concat(),
            )?;
            i += 1;
            if !samples.contains(&i) {
                continue;
            }
            let save = g.checkpoint(&identity)?;
            let state = serde_json::to_value(&g.state)?;
            let frame = image(&g)?;
            let mut restored = rc_game::bundle::decode(&bytes)?;
            restored
                .restore_checkpoint(&save, &identity)
                .map_err(|e| format!("{label} tick{i}: {e}"))?;
            assert_eq!(
                state,
                serde_json::to_value(&restored.state)?,
                "{label} tick{i} roundtrip"
            );
            assert_eq!(frame, image(&restored)?);
            let marker = restored.checkpoint_marker();
            assert_eq!(marker, g.checkpoint_marker());
            restored.restart();
            assert_eq!(entry, serde_json::to_value(&restored.state)?);
            assert_eq!(entry_frame, image(&restored)?);
            restored.restore_checkpoint(&save, &identity)?;
            for bad in [
                save[..save.len() - 1].to_vec(),
                {
                    let mut b = save.clone();
                    b[20] ^= 1;
                    b
                },
                b"garbage".to_vec(),
            ] {
                assert!(restored.restore_checkpoint(&bad, &identity).is_err());
                assert_eq!(state, serde_json::to_value(&restored.state)?);
                assert_eq!(frame, image(&restored)?);
            }
            assert!(restored.restore_checkpoint(&save, "wrong scene").is_err());
            assert_eq!(state, serde_json::to_value(&restored.state)?);
            fs::write(out.join(format!("{label}-{i}.rcsave")), &save)?;
            let until = (i + 90).min(count);
            let follow = [
                b"RCINPUT1".as_slice(),
                &((until - i) as u32).to_le_bytes(),
                &rows[12 + i * 20..12 + until * 20],
            ]
            .concat();
            replay(&mut restored, &follow)?;
            replay(&mut g, &follow)?;
            assert_eq!(
                serde_json::to_value(&g.state)?,
                serde_json::to_value(&restored.state)?,
                "{label} continuation at{i}"
            );
            assert_eq!(image(&g)?, image(&restored)?);
            println!(
                "{label} tick{i}: {} bytes, exact state/frame, 90-step continuation and restart",
                save.len()
            );
            checks.push(serde_json::json!({"map":label,"tick":i,"bytes":save.len(),"exact_state_frame":true,"continuation":until-i,"atomic_corruption":true,"restart_entry_exact":true}));
            i = until;
        }
        assert_eq!(g.state.phase, Phase::Won);
        game = Some(g);
    }
    let mut f = game
        .unwrap()
        .travel_to(&fs::read(rc_inspect::build_inputs::input(
            "factory_scene",
        )?)?)?;
    for _ in 0..30 {
        f.advance(STEP, Input::default())?;
    }
    let marker = f.checkpoint_marker();
    f.advance(
        STEP / 2.,
        Input {
            jump: true,
            fire: true,
            look: [0.03, 0.01],
            ..Input::default()
        },
    )?;
    let b = fs::read(rc_inspect::build_inputs::input("factory_scene")?)?;
    let identity = format!("{}:{}", b.len(), rc_game::replay::fingerprint(&b));
    let save = f.checkpoint(&identity)?;
    let mut restored = rc_game::bundle::decode(&b)?;
    restored.restore_checkpoint(&save, &identity)?;
    assert_eq!(marker, restored.checkpoint_marker());
    f.advance(STEP / 2., Input::default())?;
    restored.advance(STEP / 2., Input::default())?;
    assert_eq!(
        serde_json::to_value(&f.state)?,
        serde_json::to_value(&restored.state)?
    );
    assert_eq!(f.state.statistics.shots, 1);
    assert_eq!(f.state.statistics.jumps, 1);
    fs::write(out.join("factory-substep.rcsave"), save)?;
    fs::write(
        out.join("validation.json"),
        serde_json::to_vec_pretty(
            &serde_json::json!({"passed":true,"checkpoints":checks,"actual_a_b_c_completion":true,"factory_transition":true,"pending_input_and_fractional_time_exact":true,"scene_mismatch_atomic":true}),
        )?,
    )?;
    Ok(())
}
