//! Behavioral integration probes for the selected local combat bundle.
use rc_game::{bundle, replay, Game, Input, Phase, STEP};
use std::{env, fs, path::Path};
fn run(g: &mut Game, ticks: usize, input: Input) -> Result<(), String> {
    for _ in 0..ticks {
        g.advance(STEP, input)?;
    }
    Ok(())
}
fn main() -> Result<(), Box<dyn std::error::Error>> {
    let args: Vec<_> = env::args_os().collect();
    if args.len() != 3 {
        return Err("usage: rc-play-combat-check SCENE OUTPUT_DIRECTORY".into());
    }
    let bytes = fs::read(&args[1])?;
    let out = Path::new(&args[2]);
    fs::create_dir_all(out)?;
    let mut g = bundle::decode(&bytes)?;
    let initial = replay::state_fingerprint(&g)?;
    assert_eq!((g.state.ammo, g.state.reserve), (60, 240));
    assert!(g.state.enemies.iter().all(|e| e.health == 100));
    g.aim_at(g.state.enemies[0].position);
    g.advance(
        STEP,
        Input {
            fire: true,
            ..Input::default()
        },
    )?;
    assert_eq!(g.state.statistics.hits, 0);
    assert_eq!(g.state.combat.as_ref().unwrap().projectiles.len(), 1);
    assert_eq!(g.state.ammo, 59);
    run(&mut g, 30, Input::default())?;
    assert_eq!(g.state.statistics.hits, 1);
    assert_eq!(g.state.enemies[0].health, 80);
    g.restart();
    assert_eq!(initial, replay::state_fingerprint(&g)?);
    // Compare the same fixed simulation under three host frame cadences.
    let mut fingerprints = vec![];
    for frame in [STEP / 2., STEP * 2., STEP * 6.] {
        let mut probe = bundle::decode(&bytes)?;
        probe.aim_at(probe.state.enemies[0].position);
        for _ in 0..(2. / frame).round() as usize {
            probe.advance(
                frame,
                Input {
                    fire: true,
                    ..Input::default()
                },
            )?;
        }
        fs::write(
            out.join(format!("cadence-{}.json", (1. / frame).round() as u32)),
            serde_json::to_vec_pretty(&probe.state)?,
        )?;
        fingerprints.push(serde_json::to_vec(&probe.state)?);
    }
    assert!(fingerprints.windows(2).all(|p| p[0] == p[1]));
    for i in 0..g.state.enemies.len() {
        for _ in 0..240 {
            if g.state.enemies[i].health == 0 {
                break;
            }
            g.aim_at(g.state.enemies[i].position);
            g.advance(
                STEP,
                Input {
                    fire: true,
                    ..Input::default()
                },
            )?;
        }
        assert_eq!(g.state.enemies[i].health, 0);
    }
    assert_eq!(g.state.statistics.kills, 3);
    assert_eq!(g.state.statistics.hits, 15);
    run(&mut g, 100, Input::default())?; // finish bodyfall notifies
    let total = g.state.ammo + g.state.reserve;
    g.advance(
        STEP,
        Input {
            reload: true,
            ..Input::default()
        },
    )?;
    assert!(g.state.reload_remaining > 0.);
    run(&mut g, 100, Input::default())?;
    assert_eq!(g.state.ammo, 60);
    assert_eq!(g.state.ammo + g.state.reserve, total);
    let events = &g.state.combat.as_ref().unwrap().events;
    let assets = g.combat.as_ref().unwrap();
    assert_eq!(
        events
            .iter()
            .filter(|e| e.sound == assets.fire_sound)
            .count(),
        g.state.statistics.shots as usize
    );
    let reload_sound = assets
        .notifies
        .iter()
        .find(|n| n.weapon && n.clip == "Reload")
        .unwrap()
        .sound;
    assert_eq!(events.iter().filter(|e| e.sound == reload_sound).count(), 1);
    let death_events: Vec<_> = events.iter().filter(|e| e.owner >= 0).collect();
    assert_eq!(death_events.len(), 6);
    for owner in 0..3 {
        assert_eq!(death_events.iter().filter(|e| e.owner == owner).count(), 2);
    }
    let combat = g.state.clone();
    let eye_z = g.eye()[2];
    g.aim_at([g.exit[0], g.exit[1], eye_z]);
    run(
        &mut g,
        240,
        Input {
            forward: 1.,
            ..Input::default()
        },
    )?;
    assert_eq!(g.state.phase, Phase::Won);
    let won = g.state.clone();
    g.restart();
    run(&mut g, 1800, Input::default())?;
    assert_eq!(g.state.phase, Phase::Lost);
    let lost = g.state.clone();
    g.restart();
    assert_eq!(initial, replay::state_fingerprint(&g)?);
    g.advance(
        STEP,
        Input {
            fire: true,
            ..Input::default()
        },
    )?;
    let before = replay::state_fingerprint(&g)?;
    assert!(g.advance(f64::NAN, Input::default()).is_err());
    assert_eq!(before, replay::state_fingerprint(&g)?);
    let pixels = g.render(640, 400)?;
    let mut ppm = b"P6\n640 400\n255\n".to_vec();
    for p in pixels {
        ppm.extend_from_slice(&[(p >> 16) as u8, (p >> 8) as u8, p as u8]);
    }
    fs::write(out.join("projectile.ppm"), ppm)?;
    let packet = g.render_frame(640, 400)?.gpu_packet(&g.scene);
    assert_eq!(before, replay::state_fingerprint(&g)?);
    assert_eq!(packet, g.render_frame(640, 400)?.gpu_packet(&g.scene));
    g.suspend();
    assert!(g.state.combat.as_ref().unwrap().events.is_empty());
    assert!(bundle::decode(&bytes[..bytes.len() - 1]).is_err());
    fs::write(
        out.join("combat-validation.json"),
        serde_json::to_vec_pretty(
            &serde_json::json!({"delayed_hit":true,"cadence_120_30_10_exact":true,"all_three_enemies_15_hits":true,"reload_conserves_ammo":true,"sound_events_once":true,"enemy_bodyfalls_per_instance":true,"reset_exact":true,"invalid_input_transaction":true,"render_read_only":true,"truncated_bundle_rejected":true,"combat":combat,"won":won,"lost":lost}),
        )?,
    )?;
    println!("Combat scene passed: delayed hits, cadence, 3 kills/15 hits, reload, audio notifies, victory/defeat/reset");
    Ok(())
}
