//! Paired B gameplay and explicit scene rebind of existing visual fixtures.
use rc_game::{Input, STEP};
use std::{env, fs, path::Path};
fn main() -> Result<(), Box<dyn std::error::Error>> {
    let worker = std::thread::Builder::new()
        .stack_size(16 * 1024 * 1024)
        .spawn(run)?;
    worker
        .join()
        .map_err(|_| "Voice checker panicked")?
        .map_err(Into::into)
}
fn run() -> Result<(), String> {
    let a: Vec<_> = env::args_os().collect();
    if a.len() != 6 {
        return Err("usage: rc-map2-voice-check OLD NEW REPLAY FIXTURES OUTPUT".into());
    }
    let run = || -> Result<(), Box<dyn std::error::Error>> {
        let old = fs::read(&a[1])?;
        let new = fs::read(&a[2])?;
        let out = Path::new(&a[5]);
        fs::create_dir_all(out)?;
        let oid = format!("{}:{}", old.len(), rc_game::replay::fingerprint(&old));
        let nid = format!("{}:{}", new.len(), rc_game::replay::fingerprint(&new));
        let mut snapshots = 0;
        for entry in fs::read_dir(&a[4])? {
            let path = entry?.path();
            if path.extension().is_none_or(|e| e != "bin") {
                continue;
            }
            let source = fs::read(&path)?;
            let mut before = rc_game::bundle::decode(&old)?;
            before.restore_checkpoint(&source, &oid)?;
            let cp = before.checkpoint(&nid)?;
            let mut after = rc_game::bundle::decode(&new)?;
            after.restore_checkpoint(&cp, &nid)?;
            assert_eq!(before.render(640, 400)?, after.render(640, 400)?);
            assert_eq!(cp, after.checkpoint(&nid)?);
            fs::write(out.join(path.file_name().unwrap()), cp)?;
            snapshots += 1;
        }
        let mut before = rc_game::bundle::decode(&old)?;
        let mut after = rc_game::bundle::decode(&new)?;
        let rows = fs::read(&a[3])?;
        assert_eq!(&rows[..8], b"RCINPUT1");
        let mut ticks = 0;
        let mut cold = 0;
        for row in rows[12..].chunks_exact(20) {
            let f = |i| f32::from_le_bytes(row[i..i + 4].try_into().unwrap()) as f64;
            let b = u32::from_le_bytes(row[16..20].try_into().unwrap());
            let input = Input {
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
            };
            before.advance(STEP, input)?;
            after.advance(STEP, input)?;
            assert_eq!(before.checkpoint("pair")?, after.checkpoint("pair")?);
            ticks += 1;
            if ticks % 3000 == 0 {
                let cp = after.checkpoint(&nid)?;
                let mut restored = rc_game::bundle::decode(&new)?;
                restored.restore_checkpoint(&cp, &nid)?;
                assert_eq!(after.render(320, 200)?, restored.render(320, 200)?);
                after = restored;
                cold += 1;
            }
        }
        assert!(after
            .state
            .map
            .as_ref()
            .unwrap()
            .gameplay
            .as_ref()
            .unwrap()
            .transition
            .is_some());
        fs::write(
            out.join("validation.json"),
            serde_json::to_vec_pretty(
                &serde_json::json!({"passed":true,"gameplay_steps_exact":ticks,"visual_snapshots_exact":snapshots,"cold_restores":cold,"scope":"Metadata-only B scene update: old scene-bound fixtures explicitly rebound for tests only; input-only B run has identical full states and frames"}),
            )?,
        )?;
        println!("{ticks} identical B states, {snapshots} unchanged visual fixtures, {cold} cold restores");
        Ok(())
    };
    run().map_err(|e| e.to_string())
}
