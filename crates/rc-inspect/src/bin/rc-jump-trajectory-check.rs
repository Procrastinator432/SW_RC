//! Compare prepared trajectories with original scalar instruction recordings.
use rc_package::jump_trajectory::{calculate, Input};
use serde::Deserialize;
use serde_json::{json, Value};
use std::{env, fs};

#[derive(Deserialize)]
struct Seed {
    input: u32,
    seed: u32,
}
#[derive(Deserialize)]
struct Expected {
    velocity: [u32; 3],
    time: u32,
    extensions: u8,
    seeds: Vec<Seed>,
}
#[derive(Deserialize)]
struct Case {
    input: Input,
    expected: Expected,
}
#[derive(Deserialize)]
struct Recording {
    provenance: Value,
    cases: Vec<Case>,
}

fn main() -> Result<(), Box<dyn std::error::Error>> {
    let args: Vec<_> = env::args_os().collect();
    if args.len() != 3 {
        return Err("usage: rc-jump-trajectory-check RECORDING OUTPUT".into());
    }
    let recording: Recording = serde_json::from_slice(&fs::read(&args[1])?)?;
    if recording.cases.is_empty() {
        return Err("Empty trajectory recording".into());
    }
    let mut extensions = [0usize; 5];
    let mut seed_calls = 0;
    for (i, c) in recording.cases.iter().enumerate() {
        let mut calls = 0;
        let mut mismatch = false;
        let actual = calculate(c.input, |s| {
            let Some(expected) = c.expected.seeds.get(calls) else {
                mismatch = true;
                return 0.0;
            };
            mismatch |= s.to_bits() != expected.input;
            calls += 1;
            f32::from_bits(expected.seed)
        });
        if mismatch
            || calls != c.expected.seeds.len()
            || actual.velocity.map(f32::to_bits) != c.expected.velocity
            || actual.time.to_bits() != c.expected.time
            || actual.extensions != c.expected.extensions
        {
            return Err(format!("Native trajectory mismatch at case {i}: actual {actual:?}, expected velocity {:?}, time {}, extensions {}, seed calls {calls}/{}",
                c.expected.velocity.map(f32::from_bits), f32::from_bits(c.expected.time), c.expected.extensions, c.expected.seeds.len()).into());
        }
        extensions[actual.extensions as usize] += 1;
        seed_calls += calls;
    }
    if extensions.contains(&0) {
        return Err("Missing extension branch coverage".into());
    }
    let receipt = json!({"passed":true,"cases":recording.cases.len(),"extensions":extensions,"seed_calls":seed_calls,"provenance":recording.provenance});
    fs::write(&args[2], serde_json::to_vec_pretty(&receipt)?)?;
    println!(
        "Matched {} original trajectories, including seed inputs and all extension branches",
        recording.cases.len()
    );
    Ok(())
}
