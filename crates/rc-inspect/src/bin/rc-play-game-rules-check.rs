//! Exercise actual notify/projectile paths for all three installed difficulty levels.
use rc_game::{bundle, replay, Input, Phase, STEP};
use std::{env, fs, path::Path};
fn main() -> Result<(), Box<dyn std::error::Error>> {
    let args: Vec<_> = env::args_os().collect();
    if args.len() != 3 {
        return Err("usage: rc-play-game-rules-check SCENE_DIRECTORY OUTPUT_DIRECTORY".into());
    }
    let scenes = Path::new(&args[1]);
    let out = Path::new(&args[2]);
    fs::create_dir_all(out)?;
    let mut reports = vec![];
    for (index, name) in ["easy", "normal", "hard"].into_iter().enumerate() {
        let bytes = fs::read(scenes.join(format!("geo_01a-{name}.rcplay")))?;
        let mut g = bundle::decode(&bytes)?;
        let initial = replay::state_fingerprint(&g)?;
        let a = g.combat.as_ref().unwrap().game_rules.as_ref().unwrap();
        assert_eq!(a.selected, index);
        let factor = a.modifiers[index];
        let mut first = None;
        for _ in 0..3600 {
            g.advance(STEP, Input::default())?;
            let p = g
                .state
                .combat
                .as_ref()
                .unwrap()
                .melee
                .as_ref()
                .unwrap()
                .player;
            if first.is_none() && (p.health != 100. || p.shields != 100.) {
                let effective = 125. * factor;
                assert_eq!(p.shields, (100. - effective).max(0.));
                assert_eq!(p.health, 100. - (effective - 100.).max(0.));
                first = Some(
                    serde_json::json!({"tick":g.state.statistics.ticks,"damage":effective,"player":p}),
                );
            }
            if g.state.phase == Phase::Lost {
                break;
            }
        }
        assert!(first.is_some());
        assert_eq!(g.state.phase, Phase::Lost);
        let defeat_tick = g.state.statistics.ticks;
        g.restart();
        assert_eq!(initial, replay::state_fingerprint(&g)?);
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
        assert_eq!(g.state.statistics.hits, 9);
        assert_eq!(g.state.statistics.kills, 3);
        let mut cadence = vec![];
        for elapsed in [STEP / 2., STEP * 2., STEP * 6.] {
            let mut g = bundle::decode(&bytes)?;
            for _ in 0..(12. / elapsed).round() as usize {
                g.advance(elapsed, Input::default())?;
            }
            cadence.push(serde_json::to_vec(&g.state)?);
        }
        assert!(cadence.windows(2).all(|w| w[0] == w[1]));
        // Synthetic neutral flags exercise both real runtime recipients, keeping
        // the shipped map flags unchanged. Neither health, shields nor recharge
        // deadline may change; outgoing projectiles must not kill anything.
        let mut g = bundle::decode(&bytes)?;
        for neutral in g
            .combat
            .as_mut()
            .unwrap()
            .game_rules
            .as_mut()
            .unwrap()
            .neutral_volumes
            .values_mut()
        {
            *neutral = true;
        }
        let original = g
            .state
            .combat
            .as_ref()
            .unwrap()
            .melee
            .as_ref()
            .unwrap()
            .player;
        for _ in 0..600 {
            g.advance(STEP, Input::default())?;
        }
        assert_eq!(
            g.state
                .combat
                .as_ref()
                .unwrap()
                .melee
                .as_ref()
                .unwrap()
                .player,
            original
        );
        for _ in 0..200 {
            g.aim_at(g.state.enemies[0].position);
            g.advance(
                STEP,
                Input {
                    fire: true,
                    ..Input::default()
                },
            )?;
        }
        assert!(g.state.statistics.hits > 0);
        assert_eq!(g.state.statistics.kills, 0);
        assert!(g.state.enemies.iter().all(|e| e.health == 100));
        assert!(g.state.enemies.iter().all(|e| e.flash == 0.));
        // Missing metadata cannot partially commit a tick, including animation
        // and Lunge state changed before its hit notification.
        let mut g = bundle::decode(&bytes)?;
        g.combat
            .as_mut()
            .unwrap()
            .game_rules
            .as_mut()
            .unwrap()
            .neutral_volumes
            .clear();
        let mut error_seen = false;
        for _ in 0..600 {
            let before = replay::state_fingerprint(&g)?;
            if let Err(error) = g.advance(STEP, Input::default()) {
                assert!(error.contains("Missing damage volume flag"));
                assert_eq!(before, replay::state_fingerprint(&g)?);
                error_seen = true;
                break;
            }
        }
        assert!(error_seen);
        reports.push(serde_json::json!({"level":name,"factor":factor,"first_hit":first,"defeat_tick":defeat_tick,"three_kills_9_hits":true,"restart_exact":true,"cadence_exact":true,"neutral_player_and_enemy":true,"damage_error_rolls_back":true}));
    }
    fs::write(
        out.join("game-rules-validation.json"),
        serde_json::to_vec_pretty(&serde_json::json!({"passed":true,"levels":reports}))?,
    )?;
    println!("Three difficulty levels: exact notify damage, defeat, 9-hit victory, reset, cadence, neutral protection and rollback passed");
    Ok(())
}
