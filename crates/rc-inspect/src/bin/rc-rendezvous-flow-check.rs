//! Exercise extracted original script fragments, without claiming Game integration.
use rc_game::rendezvous::{Assets, Effect, Operation, State};
use std::{env, fs, path::Path};

fn duration(a: &Assets, trigger: &str) -> f64 {
    a.programs
        .iter()
        .find(|p| p.trigger == trigger)
        .unwrap()
        .actions
        .iter()
        .map(|a| match a.operation {
            Operation::Wait { seconds } | Operation::Sound { seconds, .. } => seconds,
            _ => 0.,
        })
        .sum()
}
fn main() -> Result<(), Box<dyn std::error::Error>> {
    let args: Vec<_> = env::args_os().collect();
    if args.len() != 3 {
        return Err("usage: rc-rendezvous-flow-check ASSETS OUTPUT".into());
    }
    let a: Assets = serde_json::from_slice(&fs::read(&args[1])?)?;
    let expected = duration(&a, "Add62ToSquad") + duration(&a, "DialogDemolition");
    let mut runs = vec![];
    let mut reference = None;
    for hz in [20, 60, 144] {
        let mut s = State::new(&a)?;
        let mut effects = s.event(&a, "levelstart")?;
        effects.extend(s.event(&a, "SpawnSmallBunker")?);
        effects.extend(s.event(&a, "Conversation04")?);
        for _ in 0..hz * 120 {
            effects.extend(s.advance(&a, 1. / hz as f64)?);
        }
        if s.squad_joined || s.objective_completed || s.charge_enabled {
            return Err("Ambient tail completed rendezvous without player contact".into());
        }
        effects.extend(s.observe(&a, a.join.position, [40., 40., 84.])?);
        let mut completed = None;
        for tick in 1..=hz * 120 {
            effects.extend(s.advance(&a, 1. / hz as f64)?);
            if s.objective_completed && completed.is_none() {
                completed = Some(tick as f64 / hz as f64);
            }
        }
        let completed = completed.ok_or("Rendezvous never completed")?;
        if (completed - expected).abs() > 1. / hz as f64 + 1e-8
            || !s.squad_joined
            || !s.charge_enabled
        {
            return Err("Original dialogue boundary mismatch".into());
        }
        let at = |test: fn(&Effect) -> bool| {
            effects
                .iter()
                .position(test)
                .ok_or("Missing expected effect")
        };
        let squad = at(|e| matches!(e, Effect::AddSquadMember { .. }))?;
        let objective = at(|e| matches!(e, Effect::ObjectiveCompleted { objective: 4 }))?;
        let marker = at(|e| matches!(e, Effect::ShowMarker { .. }))?;
        if !(squad < objective && objective < marker) {
            return Err("Squad/objective/marker order mismatch".into());
        }
        let sounds = effects
            .iter()
            .filter(|e| matches!(e, Effect::PlaySound { .. }))
            .count();
        if sounds != 8
            || effects
                .iter()
                .filter(|e| matches!(e, Effect::ObjectiveCompleted { .. }))
                .count()
                != 1
        {
            return Err("Original sound/action count mismatch".into());
        }
        for event in [
            "levelstart",
            "SpawnSmallBunker",
            "Conversation04",
            "Add62ToSquad",
            "DialogDemolition",
            "ActivateDemoCharge",
        ] {
            if !s.event(&a, event)?.is_empty() {
                return Err("Duplicate event replayed a completed lane".into());
            }
        }
        s.observe(&a, [0.; 3], [40., 40., 84.])?;
        if !s.observe(&a, a.join.position, [40., 40., 84.])?.is_empty() {
            return Err("Player reentry duplicated recruitment".into());
        }
        if let Some(prior) = &reference {
            if prior != &effects {
                return Err("Effect order changes with frame rate".into());
            }
        } else {
            reference = Some(effects.clone());
        }
        runs.push(serde_json::json!({"hz":hz,"completed_seconds_after_contact":completed,"expected_seconds":expected,"sounds":sounds,"effects":effects,"state":s}));
    }
    let out = Path::new(&args[2]);
    fs::write(
        out,
        serde_json::to_vec_pretty(
            &serde_json::json!({"passed":true,"runs":runs,"scope":"Selected original script host only; no playable Game, companion AI or Android integration"}),
        )?,
    )?;
    println!("Original rendezvous flow passed at 20/60/144 Hz; objective4 after {expected:.6}s");
    Ok(())
}
