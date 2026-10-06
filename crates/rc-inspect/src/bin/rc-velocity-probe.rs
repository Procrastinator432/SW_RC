//! Compare isolated reconstructed walking arithmetic using original profile scalars.
use rc_package::{
    movement_profile::MovementProfile,
    pawn_velocity::{walking_velocity, WalkingVelocityOptions},
    properties::Value,
};
use std::{env, fs, path::Path};
#[path = "../defaults.rs"]
mod defaults;
fn main() -> Result<(), Box<dyn std::error::Error>> {
    let args: Vec<_> = env::args_os().collect();
    if args.len() != 3 {
        return Err("Usage: rc-velocity-probe <GameData> <report.json>".into());
    }
    let profile = MovementProfile::read(&defaults::load(Path::new(&args[1]))?)?;
    let friction = match profile.properties.get("GroundFriction").map(|p| &p.value) {
        Some(Value::Float(f)) => f64::from(*f),
        _ => return Err("missing GroundFriction".into()),
    };
    let mut probes = Vec::new();
    for (name, gravity, friction) in [
        ("base_gravity", profile.physics.gravity, friction),
        ("weak_gravity", 110.0, friction),
        ("zero_friction", profile.physics.gravity, 0.0),
    ] {
        let options = WalkingVelocityOptions {
            acceleration_limit: profile.controller.acceleration,
            speed_limit: profile.controller.speed,
            ground_friction: friction,
            gravity_magnitude: gravity,
        };
        let mut velocity = [0.0; 2];
        let mut frames = Vec::new();
        for tick in 0..420 {
            let acceleration = if tick < 60 {
                [profile.controller.acceleration, 0.0]
            } else if tick < 120 {
                [0.0, profile.controller.acceleration]
            } else {
                [0.0; 2]
            };
            velocity = walking_velocity(velocity, acceleration, 1.0 / 60.0, options)?;
            frames.push(serde_json::json!({"tick":tick,"acceleration":acceleration,"velocity":velocity,"speed":velocity[0].hypot(velocity[1])}));
        }
        probes.push(serde_json::json!({"name":name,"options":options,"frames":frames,"final_velocity":velocity}));
    }
    let out = Path::new(&args[2]);
    if let Some(parent) = out.parent() {
        fs::create_dir_all(parent)?;
    }
    fs::write(
        out,
        serde_json::to_vec_pretty(
            &serde_json::json!({"profile":profile,"probes":probes,"scope":"Isolated XY walking calcVelocity arithmetic at 1/60: 60 forward acceleration, 60 perpendicular acceleration, 300 no acceleration. Caller-supplied effective acceleration and speed limits. Original base profile scalars; weak gravity110 and zero friction are controlled diagnostic variants. No collision, runtime SpeedFactor/MaximumDesiredSpeed/walking flags, ModifyVelocity callbacks, input-to-acceleration mapping, SSE float parity, world trajectory or Android integration. Existing controller unchanged."}),
        )?,
    )?;
    println!("3 isolated walking velocity probes; 1260 steps");
    Ok(())
}
