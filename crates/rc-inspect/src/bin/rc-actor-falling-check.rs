//! Check the prepared original Actor falling SSE arithmetic bit for bit.
use serde::Deserialize;
use std::{env, fs};
#[derive(Deserialize)]
struct Expected {
    velocity: [u32; 3],
    displacement: [u32; 3],
}
#[derive(Deserialize)]
struct Case {
    velocity: [u32; 3],
    acceleration: [u32; 3],
    gravity: [u32; 3],
    zone_velocity: [u32; 3],
    buoyancy: u32,
    mass: u32,
    fluid_friction: u32,
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
        return Err("usage: rc-actor-falling-check ORACLE REPORT".into());
    }
    let oracle: Oracle = serde_json::from_slice(&fs::read(&args[1])?)?;
    if oracle.cases.len() != 4096 {
        return Err("Unexpected original falling oracle count".into());
    }
    for (i, c) in oracle.cases.iter().enumerate() {
        let environment = rc_package::actor_falling::Environment {
            gravity: c.gravity.map(f32::from_bits),
            zone_velocity: c.zone_velocity.map(f32::from_bits),
            buoyancy: f32::from_bits(c.buoyancy),
            mass: f32::from_bits(c.mass),
            fluid_friction: f32::from_bits(c.fluid_friction),
        };
        let (velocity, displacement) = rc_package::actor_falling::step(
            c.velocity.map(f32::from_bits),
            c.acceleration.map(f32::from_bits),
            environment,
            f32::from_bits(c.dt),
        );
        if velocity.map(f32::to_bits) != c.expected.velocity
            || displacement.map(f32::to_bits) != c.expected.displacement
        {
            return Err(format!("Native falling mismatch case {i}").into());
        }
    }
    fs::write(
        &args[2],
        serde_json::to_vec_pretty(&serde_json::json!({
            "passed":true,"cases":oracle.cases.len(),"bit_exact":true,
            "scope":"Prepared SSE arithmetic 10490c63..10490d71; GetNetBuoyancy and volume supplied; no MoveActor or landing claim"
        }))?,
    )?;
    println!(
        "{} original actor falling cases matched bit for bit",
        oracle.cases.len()
    );
    Ok(())
}
