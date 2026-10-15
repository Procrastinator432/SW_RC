//! Prepared objective5 acceptance. This does not claim a playable integrated scene.
use rc_game::demolition::{Assets, Effect, Phase, State};
use std::{env, fs, path::Path};
fn main() -> Result<(), Box<dyn std::error::Error>> {
    let args: Vec<_> = env::args_os().collect();
    if args.len() != 2 {
        return Err("usage: rc-demolition-flow-check OUTPUT".into());
    }
    let out = Path::new(&args[1]);
    let a: Assets = serde_json::from_slice(&fs::read(out.join("demolition-assets.json"))?)?;
    let mut runs = vec![];
    for hz in [20, 60, 144] {
        for detonate in [true, false] {
            let mut s = State::new(&a)?;
            assert!(!s.enable(&a, false)?);
            assert!(s.command(&a, 62, true)?.is_empty());
            s.enable(&a, true)?;
            assert!(s.command(&a, 40, true)?.is_empty());
            let mut events = vec![(0., s.command(&a, 62, true)?)];
            // Time alone while travelling cannot place the charge.
            for _ in 0..hz * 2 {
                assert!(s.advance(&a, 1. / hz as f64)?.is_empty());
            }
            assert_eq!(s.phase, Phase::Moving);
            assert!(!s.arrived(&a, 40)?);
            assert!(s.arrived(&a, 62)?);
            assert!(s.notify(&a, 40, "MDplaceCharge")?.is_empty());
            assert!(s.detonate(&a, true)?.is_empty());
            let mut armed_at = None;
            let mut exploded_at = None;
            for frame in 1..=hz * 70 {
                let t = frame as f64 / hz as f64;
                let e = s.advance(&a, 1. / hz as f64)?;
                if e.contains(&Effect::Event {
                    tag: "DemoChargeArmed".into(),
                }) {
                    armed_at = Some(t);
                }
                if !e.is_empty() {
                    events.push((t, e));
                }
                if detonate && s.phase == Phase::Armed && exploded_at.is_none() {
                    let before = s.clone();
                    assert!(s.detonate(&a, false)?.is_empty());
                    assert_eq!(s, before);
                    assert!(s.detonate(&a, true)?.is_empty());
                    assert!(!s.select_plunger(&a, a.player_range + 1.)?);
                    assert!(s.select_plunger(&a, a.player_range)?);
                    events.push((t, s.detonate(&a, true)?));
                    exploded_at = Some(t);
                    assert!(s.detonate(&a, true)?.is_empty());
                }
            }
            let sounds = events
                .iter()
                .flat_map(|(_, e)| e)
                .filter_map(|e| {
                    if let Effect::Sound { source } = e {
                        Some(source.clone())
                    } else {
                        None
                    }
                })
                .collect::<Vec<_>>();
            assert!(sounds.contains(&a.placed_sound));
            assert!(sounds.contains(&a.armed_sound));
            let armed = armed_at.ok_or("No arming event")?;
            let exact = a.start_seconds * a.place_fraction + a.setup_seconds;
            assert!((armed - exact).abs() <= 1. / hz as f64 + 1e-8);
            if detonate {
                assert_eq!(s.phase, Phase::Exploded);
                assert!(s.objective_completed);
                assert!(sounds.contains(&a.explosion_sound));
                assert!(!sounds.iter().any(|s| ["D62G1515", "D62G1516", "D62G1517"]
                    .iter()
                    .any(|n| s.ends_with(n))));
                assert!(events
                    .iter()
                    .flat_map(|(_, e)| e)
                    .any(|e| matches!(e,Effect::Event{tag}if tag=="Conversation05")));
            } else {
                assert_eq!(s.phase, Phase::Armed);
                assert!(!s.objective_completed);
                assert!(!sounds.contains(&a.explosion_sound));
                for name in ["D62G1515", "D62G1516"] {
                    assert!(sounds.iter().any(|s| s.ends_with(name)));
                }
            }
            assert_eq!(
                events
                    .iter()
                    .flat_map(|(_, e)| e)
                    .filter(|e| matches!(e, Effect::Voice { enabled: true }))
                    .count(),
                1
            );
            runs.push(serde_json::json!({"hz":hz,"detonate":detonate,"armed_at":armed,"exploded_at":exploded_at,"state":s,"effects":events}));
        }
    }
    fs::write(
        out.join("flow-validation.json"),
        serde_json::to_vec_pretty(
            &serde_json::json!({"passed":true,"scope":"Prepared marker/charge event host; physical arrival is simulated explicitly. No scene, input, movement, damage or Android acceptance.","runs":runs}),
        )?,
    )?;
    println!("Objective5 selected host: six original-data scenarios at20/60/144Hz passed; explicit detonation, reminder cancellation, no automatic explosion");
    Ok(())
}
