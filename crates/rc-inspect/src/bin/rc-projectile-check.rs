//! Compare prepared native instruction receipts against the portable Rust motion policy.
use serde::Deserialize;
use std::{env, fs};
#[derive(Deserialize)]
struct Expected {
    velocity: [u32; 3],
    delta: [u32; 3],
}
#[derive(Deserialize)]
struct Case {
    velocity: [u32; 3],
    acceleration: [u32; 3],
    max_speed: u32,
    dt: u32,
    expected: Expected,
}
#[derive(Deserialize)]
struct Oracle {
    cases: Vec<Case>,
}
fn main() -> Result<(), Box<dyn std::error::Error>> {
    let args: Vec<_> = env::args_os().collect();
    if args.len() != 3 {
        return Err("usage: rc-projectile-check ORACLE OUTPUT".into());
    }
    let oracle: Oracle = serde_json::from_slice(&fs::read(&args[1])?)?;
    for (i, c) in oracle.cases.iter().enumerate() {
        let (v, d) = rc_package::projectile_motion::step(
            c.velocity.map(f32::from_bits),
            c.acceleration.map(f32::from_bits),
            f32::from_bits(c.max_speed),
            f32::from_bits(c.dt),
        );
        if v.map(f32::to_bits) != c.expected.velocity || d.map(f32::to_bits) != c.expected.delta {
            return Err(format!(
                "Prepared native mismatch case {i}: {:?} {:?}, expected {:?} {:?}",
                v.map(f32::to_bits),
                d.map(f32::to_bits),
                c.expected.velocity,
                c.expected.delta
            )
            .into());
        }
    }
    fs::write(
        &args[2],
        serde_json::to_vec_pretty(
            &serde_json::json!({"passed":true,"cases":oracle.cases.len(),"scope":"Prepared original SSE arithmetic with portable sqrt seed boundary; no full native physics/collision or hardware RSQRT parity"}),
        )?,
    )?;
    println!(
        "{} prepared native motion cases matched",
        oracle.cases.len()
    );
    Ok(())
}
