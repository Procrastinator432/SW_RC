//! Check the prepared original native toss arithmetic bit for bit.
use serde::Deserialize;
use std::{env, fs};
#[derive(Deserialize)]
struct Expected {
    velocity: [u32; 3],
}
#[derive(Deserialize)]
struct Case {
    direction: [u32; 3],
    pawn_velocity: [u32; 3],
    speed: u32,
    toss_z: u32,
    expected: Expected,
}
#[derive(Deserialize)]
struct Oracle {
    cases: Vec<Case>,
}
fn main() -> Result<(), Box<dyn std::error::Error>> {
    let args: Vec<_> = env::args_os().collect();
    if args.len() != 3 {
        return Err("usage: rc-detonator-toss-check ORACLE REPORT".into());
    }
    let oracle: Oracle = serde_json::from_slice(&fs::read(&args[1])?)?;
    if oracle.cases.len() != 2052 {
        return Err("Unexpected original toss oracle count".into());
    }
    for (i, c) in oracle.cases.iter().enumerate() {
        let v = rc_package::projectile_motion::toss_velocity(
            c.direction.map(f32::from_bits),
            c.pawn_velocity.map(f32::from_bits),
            f32::from_bits(c.speed),
            f32::from_bits(c.toss_z),
        );
        if v.map(f32::to_bits) != c.expected.velocity {
            return Err(format!("Native toss mismatch case {i}").into());
        }
    }
    fs::write(
        &args[2],
        serde_json::to_vec_pretty(
            &serde_json::json!({"passed":true,"cases":oracle.cases.len(),"bit_exact":true,"scope":"Prepared SSE arithmetic 104ed7e0..104ed86c; pawn/VM parameters and FRotator::Vector supplied, no complete native physics claim"}),
        )?,
    )?;
    println!(
        "{} original toss arithmetic cases matched bit for bit",
        oracle.cases.len()
    );
    Ok(())
}
