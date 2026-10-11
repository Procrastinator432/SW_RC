//! Compare the portable decision block with recorded original x86 branches.
use rc_package::geonosian_attack::choose;
use serde::Deserialize;
use std::{env, fs};
#[derive(Deserialize)]
struct Case {
    mask: u32,
    probability_bits: u32,
    random: [u16; 2],
    maneuver: u32,
    draws: usize,
}
#[derive(Deserialize)]
struct Recording {
    provenance: serde_json::Value,
    cases: Vec<Case>,
}
fn main() -> Result<(), Box<dyn std::error::Error>> {
    let args: Vec<_> = env::args_os().collect();
    if args.len() != 3 {
        return Err("usage: rc-geonosian-check RECORDING OUTPUT".into());
    }
    let recording: Recording = serde_json::from_slice(&fs::read(&args[1])?)?;
    if recording.cases.is_empty() {
        return Err("Empty native recording".into());
    }
    for (i, case) in recording.cases.iter().enumerate() {
        let mut draws = 0;
        let actual = choose(case.mask, f32::from_bits(case.probability_bits), || {
            let value = case.random[draws];
            draws += 1;
            value
        });
        if actual as u32 != case.maneuver || draws != case.draws {
            return Err(format!("Native decision mismatch at case {i}").into());
        }
    }
    fs::write(
        &args[2],
        serde_json::to_vec_pretty(&serde_json::json!({
            "passed": true, "cases": recording.cases.len(),
            "checked": ["maneuver", "conditional random consumption"],
            "provenance": recording.provenance,
            "scope": "prepared x86 decision block; navigation, RNG generator, goals and gameplay are not executed"
        }))?,
    )?;
    Ok(())
}
