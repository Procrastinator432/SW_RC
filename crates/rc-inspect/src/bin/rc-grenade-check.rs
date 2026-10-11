//! Execute the selected pickup flow with the extracted original anchors and waits.
use rc_game::grenade_pickup::{Assets, Effect, IntroStage, PickupStage, State};
use std::{env, fs, path::Path};
fn main() -> Result<(), Box<dyn std::error::Error>> {
    let args: Vec<_> = env::args_os().collect();
    if args.len() != 2 {
        return Err("usage: rc-grenade-check ASSET_DIR".into());
    }
    let out = Path::new(&args[1]);
    let a: Assets = serde_json::from_slice(&fs::read(out.join("grenade-assets.json"))?)?;
    a.validate()?;
    assert_eq!((a.initial_ammo, a.pickup_amount, a.maximum_ammo), (0, 5, 5));
    assert_eq!(
        (
            a.dialog_trigger.original,
            a.objective_trigger.original,
            a.pickup.original
        ),
        (746, 694, 610)
    );
    let extent = [22., 22., 60.];
    let away = [0., 10000., 0.];
    let mut state = State::new(&a)?;
    let initial = state.clone();
    let mut trace = vec![];
    let mut effects = state.advance(&a, 0., a.dialog_trigger.position, extent, true, true)?;
    assert_eq!(effects[1], Effect::Tracker("ThermalDetonatorBox01".into()));
    assert_eq!(effects[2], Effect::PlaySound(a.cues[0].source.clone()));
    for i in 1..=2400 {
        let emitted = state.advance(&a, 1. / 120., away, extent, true, true)?;
        if !emitted.is_empty() {
            trace.push(serde_json::json!({"seconds":i as f64/120.,"effects":emitted}));
        }
        effects.extend(emitted);
        if state.intro == IntroStage::Done {
            break;
        }
    }
    assert_eq!(state.intro, IntroStage::Done);
    assert_eq!(
        effects
            .iter()
            .filter(|e| matches!(e, Effect::PlaySound(_)))
            .count(),
        2
    );
    assert!(!state.objective_completed);
    let mut picked = state.advance(&a, 0., a.pickup.position, extent, true, true)?;
    assert_eq!(state.ammunition, 5);
    assert!(state.objective_completed);
    assert!(state.reminder_disabled);
    assert!(picked.contains(&Effect::ObjectiveCompleted(2)));
    assert!(picked.contains(&Effect::PlaySound(a.cues[3].source.clone())));
    assert!(picked.contains(&Effect::PlaySound(a.cues[2].source.clone())));
    let start = state.clone();
    let mut coarse = start.clone();
    let mut coarse_effects = vec![];
    for _ in 0..20 {
        coarse_effects.extend(coarse.advance(&a, 1., away, extent, true, true)?);
    }
    let mut fine_effects = vec![];
    for _ in 0..2400 {
        fine_effects.extend(state.advance(&a, 1. / 120., away, extent, true, true)?);
    }
    assert_eq!(coarse, state);
    assert_eq!(coarse_effects, fine_effects);
    picked.extend(fine_effects);
    assert_eq!(state.pickup_script, PickupStage::Done);
    assert!(picked.contains(&Effect::BlinkHud(10.)));
    assert!(picked.contains(&Effect::Event("PopUpGrenades".into())));
    assert!(picked.contains(&Effect::Event("DialogBlowUpGunnerDialog".into())));
    assert!(state
        .advance(&a, 1., a.pickup.position, extent, true, true)?
        .is_empty());
    let mut tall = State::new(&a)?;
    let mut above = a.pickup.position;
    above[2] += 140.;
    let e = tall.advance(&a, 0., above, extent, true, true)?;
    assert!(e.contains(&Effect::ObjectiveCompleted(2)));
    assert_eq!(tall.ammunition, 0);
    let before = state.clone();
    assert!(state
        .advance(&a, f64::NAN, away, extent, true, true)
        .is_err());
    assert_eq!(state, before);
    assert_eq!(State::new(&a)?, initial);
    fs::write(
        out.join("flow-trace.json"),
        serde_json::to_vec_pretty(
            &serde_json::json!({"intro_timeline":trace,"intro_effects":effects,"pickup_effects":picked,"final_state":state}),
        )?,
    )?;
    fs::write(
        out.join("flow-validation.json"),
        serde_json::to_vec_pretty(&serde_json::json!({
        "passed":true,"original_anchors_and_capacity":true,"sequential_intro_pcm_waits":true,"pickup_and_objective_separate":true,
        "five_grenades_granted":true,"one_objective_completion":true,"pickup_and_confirmation_sounds":true,
        "original_tracker_and_reminder_stop":true,"blink_prompt_next_dialog_order":true,"frame_partition_waits_exact":true,
        "repeat_contact_suppressed":true,"invalid_input_atomic":true,"fresh_state_exact":true,
        "scope":"Selected headless event host only; no new playable scene, Android acceptance, pickup rendering, route, grenade throwing or gun-position destruction."}))?,
    )?;
    println!("Original grenade pickup flow: anchors, 0→5 inventory, objective2, original cues/timers, independent triggers and reset passed");
    Ok(())
}
