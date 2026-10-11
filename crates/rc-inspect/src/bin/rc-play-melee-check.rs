//! Real bundle simulation: projectile variance, original hit notify, fractional shields and reset.
use rc_game::{bundle, replay, Input, Phase, STEP};
use std::{env, fs, path::Path};
fn main() -> Result<(), Box<dyn std::error::Error>> {
    let args: Vec<_> = env::args_os().collect();
    if args.len() != 3 {
        return Err("usage: rc-play-melee-check SCENE OUTPUT_DIRECTORY".into());
    }
    let bytes = fs::read(&args[1])?;
    let out = Path::new(&args[2]);
    fs::create_dir_all(out)?;
    let mut g = bundle::decode(&bytes)?;
    let initial = replay::state_fingerprint(&g)?;
    assert_eq!(
        g.state
            .combat
            .as_ref()
            .unwrap()
            .melee
            .as_ref()
            .unwrap()
            .player
            .shields,
        100.
    );
    g.aim_at(g.state.enemies[0].position);
    g.advance(
        STEP,
        Input {
            fire: true,
            ..Input::default()
        },
    )?;
    assert_eq!(g.state.statistics.hits, 0);
    for _ in 0..30 {
        g.advance(STEP, Input::default())?;
    }
    assert_eq!(g.state.enemies[0].health, 60);
    let projectile_health = g
        .state
        .combat
        .as_ref()
        .unwrap()
        .melee
        .as_ref()
        .unwrap()
        .enemies[0]
        .health;
    assert_eq!(projectile_health, 60.);
    g.restart();
    assert_eq!(initial, replay::state_fingerprint(&g)?);
    let enemy = g.state.enemies[0].position;
    let exit = g.exit;
    g.set_encounter(&[enemy], exit)?;
    let single_initial = replay::state_fingerprint(&g)?;
    let mut first_hit = None;
    let mut second_hit = None;
    let mut start = None;
    let mut last_shields = 100.;
    let receipt: serde_json::Value = serde_json::from_slice(&fs::read(
        Path::new(&args[1])
            .parent()
            .unwrap()
            .join("melee-receipt.json"),
    )?)?;
    let rate = receipt["shield_rate"]
        .as_f64()
        .ok_or("Missing shield rate")? as f32;
    for _ in 0..2400 {
        let before = g
            .state
            .combat
            .as_ref()
            .unwrap()
            .melee
            .as_ref()
            .unwrap()
            .player;
        g.advance(STEP, Input::default())?;
        let m = g.state.combat.as_ref().unwrap().melee.as_ref().unwrap();
        if start.is_none() && m.attacks[0].active.is_some() {
            start = Some(g.state.statistics.ticks);
        }
        if m.player.shields != last_shields {
            if first_hit.is_none() {
                assert_eq!((m.player.health, m.player.shields), (100., 37.5));
                first_hit = Some(g.state.statistics.ticks);
                let frame = g.render_frame(320, 200)?;
                let mut ppm = b"P6\n320 200\n255\n".to_vec();
                ppm.extend(
                    frame
                        .render_cpu(&g.scene)?
                        .iter()
                        .flat_map(|p| [(p >> 16) as u8, (p >> 8) as u8, *p as u8]),
                );
                fs::write(out.join("first-hit.ppm"), ppm)?;
            } else if m.player.shields == 0. {
                let time = g.state.statistics.ticks as f32 * STEP as f32;
                let recharged = if time > before.next_shield_recharge {
                    (before.shields + rate * STEP as f32).min(100.)
                } else {
                    before.shields
                };
                assert_eq!(m.player.health, before.health - (62.5 - recharged));
                second_hit = Some(g.state.statistics.ticks);
                break;
            }
            last_shields = m.player.shields;
        }
    }
    let (start, first, second) = (
        start.ok_or("No melee attack started")?,
        first_hit.ok_or("No original hit notify")?,
        second_hit.ok_or("No second hit")?,
    );
    assert!(first > start + 20 && second > first + 60);
    let second_health = g
        .state
        .combat
        .as_ref()
        .unwrap()
        .melee
        .as_ref()
        .unwrap()
        .player
        .health;
    assert_eq!(g.state.health, second_health.ceil() as u32);
    let pause = replay::state_fingerprint(&g)?;
    g.suspend();
    // Suspend discards queued sounds; combat HP, shield and pose do not advance.
    let shield = g
        .state
        .combat
        .as_ref()
        .unwrap()
        .melee
        .as_ref()
        .unwrap()
        .player;
    assert_eq!((shield.health, shield.shields), (second_health, 0.));
    g.restart();
    assert_eq!(single_initial, replay::state_fingerprint(&g)?);
    let mut loss = None;
    for _ in 0..3600 {
        g.advance(STEP, Input::default())?;
        if g.state.phase == Phase::Lost {
            loss = Some(g.state.statistics.ticks);
            break;
        }
    }
    assert!(loss.is_some());
    // An attack that has started must not become an unavoidable delayed hit.
    // Retreat during windup, then separately kill it before notify.
    for kill in [false, true] {
        let mut p = bundle::decode(&bytes)?;
        let enemy = p.state.enemies[0].position;
        p.set_encounter(&[enemy], p.exit)?;
        for _ in 0..2400 {
            p.advance(STEP, Input::default())?;
            if p.state
                .combat
                .as_ref()
                .unwrap()
                .melee
                .as_ref()
                .unwrap()
                .attacks[0]
                .active
                .is_some()
            {
                break;
            }
        }
        assert!(p
            .state
            .combat
            .as_ref()
            .unwrap()
            .melee
            .as_ref()
            .unwrap()
            .attacks[0]
            .active
            .is_some());
        if kill {
            p.state.enemies[0].health = 0;
        } else {
            p.aim_at(p.state.enemies[0].position);
        }
        for _ in 0..65 {
            p.advance(
                STEP,
                Input {
                    forward: if kill { 0. } else { -1. },
                    ..Input::default()
                },
            )?;
        }
        let m = p.state.combat.as_ref().unwrap().melee.as_ref().unwrap();
        assert_eq!((m.player.health, m.player.shields), (100., 100.));
    }
    // The same complete new encounter under different host cadences.
    let mut cadence = vec![];
    for frame in [STEP / 2., STEP * 2., STEP * 6.] {
        let mut p = bundle::decode(&bytes)?;
        for _ in 0..(12. / frame).round() as usize {
            p.advance(frame, Input::default())?;
        }
        cadence.push(serde_json::to_vec(&p.state)?);
    }
    assert!(cadence.windows(2).all(|p| p[0] == p[1]));
    // Win with actual aim/fire through projectile and damage code.
    g = bundle::decode(&bytes)?;
    for i in 0..g.state.enemies.len() {
        for _ in 0..200 {
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
    let kills = g.state.statistics.kills;
    let mut render = b"P6\n320 200\n255\n".to_vec();
    for p in g.render(320, 200)? {
        render.extend([(p >> 16) as u8, (p >> 8) as u8, p as u8]);
    }
    fs::write(out.join("cleared.ppm"), render)?;
    fs::write(
        out.join("melee-validation.json"),
        serde_json::to_vec_pretty(
            &serde_json::json!({"passed":true,"projectile_body_damage":40,"enemy_health_after_one_hit":projectile_health,"attack_start_tick":start,"first_hit_tick":first,"second_hit_tick":second,"first_hit_shields":37.5,"second_hit_health":second_health,"loss_tick":loss,"reset_exact":true,"windup_dodge_and_dead_attacker_no_hit":true,"cadence_120_30_10_equal":true,"kills":kills,"scope":"Headless real scene simulation; single-enemy fixture uses public encounter API; no Android/UI claim","pause_fingerprint_before":pause}),
        )?,
    )?;
    Ok(())
}
