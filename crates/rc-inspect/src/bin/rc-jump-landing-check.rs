use rc_package::jump_goal::{Decision, State};
use serde::Deserialize;
use serde_json::{json, Value};
use std::{env, fs};
#[derive(Deserialize)]
struct Case {
    started: bool,
    completed: bool,
    physics: u8,
    position: [f32; 3],
    target: [f32; 3],
    tolerance_squared: f32,
    decision: String,
}
#[derive(Deserialize)]
struct Recording {
    provenance: Value,
    cases: Vec<Case>,
}
fn main() -> Result<(), Box<dyn std::error::Error>> {
    let a: Vec<_> = env::args_os().collect();
    if a.len() != 4 {
        return Err(
            "usage: rc-jump-landing-check LANDING_RECORDING TRANSLATION_RECORDING OUTPUT".into(),
        );
    }
    let r: Recording = serde_json::from_slice(&fs::read(&a[1])?)?;
    if r.cases.is_empty() {
        return Err("Empty landing recording".into());
    }
    let mut counts = [0; 3];
    for (i, c) in r.cases.iter().enumerate() {
        let mut s = State {
            started: c.started,
            completed: c.completed,
        };
        let result = s.pre_update(c.physics, c.position, c.target, c.tolerance_squared);
        let index = match result {
            Decision::Active => 0,
            Decision::Land => 1,
            Decision::Fail => 2,
        };
        counts[index] += 1;
        if format!("{result:?}") != c.decision {
            return Err(format!("Landing mismatch at {i}").into());
        }
    }
    if counts.contains(&0) {
        return Err("Missing landing outcome".into());
    }
    let translations: Value = serde_json::from_slice(&fs::read(&a[2])?)?;
    let cases = translations["cases"]
        .as_array()
        .ok_or("Missing translations")?;
    for (i, c) in cases.iter().enumerate() {
        let first = serde_json::from_value(c["first"].clone())?;
        let last = serde_json::from_value(c["last"].clone())?;
        let scale = serde_json::from_value(c["scale_bits"].clone())?;
        let expected: [u32; 3] = serde_json::from_value(c["expected"].clone())?;
        if rc_package::jump_goal::sequence_translation(first, last, scale).map(f32::to_bits)
            != expected
        {
            return Err(format!("Translation mismatch at {i}").into());
        }
    }
    if cases.is_empty() {
        return Err("Empty translation recording".into());
    }
    fs::write(
        &a[3],
        serde_json::to_vec_pretty(
            &json!({"passed":true,"translations":cases.len(),"translation_provenance":translations["provenance"],"cases":r.cases.len(),"outcomes":counts,"provenance":r.provenance}),
        )?,
    )?;
    println!("Matched {} original landing branches", r.cases.len());
    Ok(())
}
