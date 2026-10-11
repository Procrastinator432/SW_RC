//! End-to-end scripted exercise using the same input/tick/render paths as both playable hosts.
use rc_game::{Game, Input, Phase, STEP};
use std::{env, fs, path::Path, time::Instant};
fn frame(g: &Game, dir: &Path, name: &str) -> Result<f64, Box<dyn std::error::Error>> {
    let start = Instant::now();
    let pixels = g.render(640, 400)?;
    let seconds = start.elapsed().as_secs_f64();
    let mut ppm = b"P6\n640 400\n255\n".to_vec();
    for p in pixels {
        ppm.extend_from_slice(&[(p >> 16) as u8, (p >> 8) as u8, p as u8]);
    }
    fs::write(dir.join(format!("{name}.ppm")), ppm)?;
    Ok(seconds)
}
fn main() -> Result<(), Box<dyn std::error::Error>> {
    let args: Vec<_> = env::args_os().collect();
    if args.len() != 3 {
        return Err("Usage: rc-play-check <scene.rcplay> <output-directory>".into());
    }
    let mut g = rc_game::bundle::decode(&fs::read(&args[1])?)?;
    let out = Path::new(&args[2]);
    fs::create_dir_all(out)?;
    let initial = serde_json::to_vec(&g.state)?;
    let render_seconds = frame(&g, out, "spawn")?;
    g.advance(
        STEP,
        Input {
            jump: true,
            ..Input::default()
        },
    )?;
    let base_z = g.state.pawn.body.position[2];
    let mut peak_z = base_z;
    for _ in 0..120 {
        g.advance(STEP, Input::default())?;
        peak_z = peak_z.max(g.state.pawn.body.position[2]);
    }
    assert_eq!(g.state.statistics.jumps, 1);
    assert!(peak_z > base_z + 50.);
    assert!(g.state.pawn.body.grounded);
    frame(&g, out, "landed")?;
    g.restart();
    for i in 0..g.state.enemies.len() {
        g.aim_at(g.state.enemies[i].position);
        for _ in 0..11 {
            g.advance(
                STEP,
                Input {
                    fire: true,
                    ..Input::default()
                },
            )?;
        }
        for _ in 0..10 {
            g.advance(STEP, Input::default())?;
        }
    }
    assert!(g.state.enemies.iter().all(|e| e.health == 0));
    let combat_statistics = g.state.statistics.clone();
    frame(&g, out, "cleared")?;
    let eye_z = g.eye()[2];
    g.aim_at([g.exit[0], g.exit[1], eye_z]);
    for _ in 0..240 {
        g.advance(
            STEP,
            Input {
                forward: 1.,
                ..Input::default()
            },
        )?;
    }
    assert_eq!(g.state.phase, Phase::Won);
    frame(&g, out, "victory")?;
    let won = g.state.clone();
    g.restart();
    assert_eq!(serde_json::to_vec(&g.state)?, initial);
    for _ in 0..1800 {
        g.advance(STEP, Input::default())?;
    }
    assert_eq!(g.state.phase, Phase::Lost);
    frame(&g, out, "defeat")?;
    let lost = g.state.clone();
    g.restart();
    assert_eq!(serde_json::to_vec(&g.state)?, initial);
    let report = serde_json::json!({"scope":"Playable Rust vertical slice on original geo_01a. Provisional encounter/combat/robot geometry; no original AI/campaign parity.",
        "jump_peak_above_first_step":peak_z-base_z, "combat":combat_statistics, "won":won, "lost":lost,
        "restart_restores_initial":true, "render_seconds_640x400":render_seconds,
        "triangles":g.scene.triangles.len(), "textures":g.scene.textures.len(), "colliders":g.world.actors.len()+1 });
    fs::write(
        out.join("validation.json"),
        serde_json::to_vec_pretty(&report)?,
    )?;
    println!(
        "Playable scene passed: jump/landing, {} kills, victory, defeat, restart; render {:.1} ms",
        combat_statistics.kills,
        render_seconds * 1000.
    );
    Ok(())
}
