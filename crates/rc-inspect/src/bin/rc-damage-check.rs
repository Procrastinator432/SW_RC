//! Compare prepared Rust damage variance with bounded original x86 recordings.
use rc_package::damage_variance::{calculate, Binding, Include, Variance};
use serde::Deserialize;
use std::{env, fs};
#[derive(Deserialize)]
struct Rule {
    class: Option<u32>,
    multiplier_bits: u32,
}
#[derive(Deserialize)]
struct Case {
    include: Vec<Rule>,
    exclude: Vec<Option<u32>>,
    ancestry: Vec<u32>,
    damage_bits: u32,
    actor: String,
    explicit: Option<String>,
    has_type: bool,
    override_include: Vec<Rule>,
    override_exclude: Vec<Option<u32>>,
    result_bits: u32,
}
#[derive(Deserialize)]
struct Recording {
    provenance: serde_json::Value,
    cases: Vec<Case>,
}
fn rules(input: &[Rule]) -> Vec<Include> {
    input
        .iter()
        .map(|r| Include {
            class: r.class,
            multiplier: f32::from_bits(r.multiplier_bits),
        })
        .collect()
}
fn binding<'a>(status: &str, variance: &'a Variance<'a>) -> Result<Binding<'a>, String> {
    match status {
        "valid" => Ok(Binding::Valid(variance)),
        "invalid" | "missing" => Ok(Binding::Invalid),
        _ => Err("Invalid recording binding".into()),
    }
}
fn main() -> Result<(), Box<dyn std::error::Error>> {
    let args: Vec<_> = env::args_os().collect();
    if args.len() != 3 {
        return Err("usage: rc-damage-check RECORDING OUTPUT".into());
    }
    let recording: Recording = serde_json::from_slice(&fs::read(&args[1])?)?;
    if recording.cases.is_empty() {
        return Err("Empty native recording".into());
    }
    for (i, c) in recording.cases.iter().enumerate() {
        let inc = rules(&c.include);
        let ov = rules(&c.override_include);
        let actor = Variance {
            include: &inc,
            exclude: &c.exclude,
        };
        let explicit = Variance {
            include: &ov,
            exclude: &c.override_exclude,
        };
        let result = calculate(
            f32::from_bits(c.damage_bits),
            c.has_type.then_some(c.ancestry.as_slice()),
            binding(&c.actor, &actor)?,
            c.explicit
                .as_ref()
                .map(|s| binding(s, &explicit))
                .transpose()?,
        );
        let expected = f32::from_bits(c.result_bits);
        if result.to_bits() != c.result_bits && !(result.is_nan() && expected.is_nan()) {
            return Err(format!(
                "Damage mismatch at case {i}: {} != {}",
                result.to_bits(),
                c.result_bits
            )
            .into());
        }
    }
    fs::write(
        &args[2],
        serde_json::to_vec_pretty(
            &serde_json::json!({"passed":true,"cases":recording.cases.len(),"provenance":recording.provenance,"comparison":"exact f32 bits; NaN payload/sign are not compared","scope":"prepared native variance block, not VM/default construction/TakeDamage/gameplay"}),
        )?,
    )?;
    Ok(())
}
