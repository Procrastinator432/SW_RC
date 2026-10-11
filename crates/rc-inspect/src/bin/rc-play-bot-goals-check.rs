//! Exercise selected bot budgets through the scene's actual navigation host.
use rc_game::{bundle, replay, Input, STEP};
use std::{env, fs, path::Path};
fn main() -> Result<(), Box<dyn std::error::Error>> {
    let args: Vec<_> = env::args_os().collect();
    if args.len() != 3 {
        return Err("usage: rc-play-bot-goals-check SCENE_DIRECTORY OUTPUT_DIRECTORY".into());
    }
    let out = Path::new(&args[2]);
    fs::create_dir_all(out)?;
    let mut rows = vec![];
    for level in ["easy", "normal", "hard"] {
        let bytes = fs::read(Path::new(&args[1]).join(format!("geo_01a-{level}.rcplay")))?;
        let mut g = bundle::decode(&bytes)?;
        assert_eq!(
            g.combat.as_ref().unwrap().bot.as_ref().unwrap().burst_count,
            1
        );
        let initial = replay::state_fingerprint(&g)?;
        let mut seen = false;
        for _ in 0..38 {
            g.advance(STEP, Input::default())?;
            for agent in g
                .state
                .combat
                .as_ref()
                .unwrap()
                .navigation
                .as_ref()
                .unwrap()
            {
                if let Some(goal) = &agent.goal {
                    let native = goal.native.as_ref().unwrap();
                    assert_eq!(native.max_attack_time, 2.);
                    assert!(matches!(native.remaining, 0 | 1));
                    seen = true;
                }
            }
        }
        assert!(seen);
        g.restart();
        assert_eq!(initial, replay::state_fingerprint(&g)?);
        // A deliberately altered fixture proves this is data-driven, not a new literal one.
        g.combat.as_mut().unwrap().bot.as_mut().unwrap().burst_count = 3;
        let mut three = false;
        for _ in 0..38 {
            g.advance(STEP, Input::default())?;
            for agent in g
                .state
                .combat
                .as_ref()
                .unwrap()
                .navigation
                .as_ref()
                .unwrap()
            {
                if let Some(goal) = &agent.goal {
                    let native = goal.native.as_ref().unwrap();
                    assert_eq!(native.max_attack_time, 6.);
                    assert!(matches!(native.remaining, 2 | 3));
                    three = true;
                }
            }
        }
        assert!(three);
        rows.push(serde_json::json!({"level":level,"original_budget":1,"max_attack_time":2.,"native_goal_seen":seen,"synthetic_three_uses_six_seconds":three,"restart_exact":true}));
    }
    fs::write(
        out.join("bot-goals-validation.json"),
        serde_json::to_vec_pretty(
            &serde_json::json!({"passed":true,"levels":rows,"scope":"Actual scene host budget selection; synthetic count-three fixture; arithmetic/order boundaries covered by library tests. Prepared host geometry and scheduling remain adapters."}),
        )?,
    )?;
    Ok(())
}
