//! Isolated free-flight arithmetic, not safe world movement or live original execution.
use rc_package::{
    falling::{free_fall, FreeFallOptions},
    movement_profile::MovementProfile,
};
use std::{env, fs, path::Path};
#[path = "../defaults.rs"]
mod defaults;
fn main() -> Result<(), Box<dyn std::error::Error>> {
    let args: Vec<_> = env::args_os().collect();
    if args.len() != 3 {
        return Err("Usage: rc-falling-probe <GameData> <report.json>".into());
    }
    let profile = MovementProfile::read(&defaults::load(Path::new(&args[1]))?)?;
    let options = FreeFallOptions {
        acceleration_rate: profile.controller.acceleration,
        air_control: profile.controller.air_control,
        ground_speed: profile.controller.speed,
        terminal_speed: profile.physics.terminal_speed,
        gravity: [0.0, 0.0, -profile.physics.gravity],
        zone_velocity: [0.0; 3],
    };
    let mut probes = Vec::new();
    for (name, gravity, probe_hit) in [
        ("base_clear", profile.physics.gravity, false),
        ("weak_clear", 110.0, false),
        ("blocked_lookahead", profile.physics.gravity, true),
    ] {
        let options = FreeFallOptions {
            gravity: [0.0, 0.0, -gravity],
            ..options
        };
        let mut velocity = [0.0, 0.0, profile.controller.jump_speed];
        let mut position = [0.0; 3];
        let mut frames = Vec::new();
        for tick in 0..360 {
            let acceleration = if tick < 180 {
                [options.acceleration_rate, 0.0]
            } else if tick < 240 {
                [0.0, options.acceleration_rate]
            } else {
                [0.0; 2]
            };
            let result = free_fall(velocity, acceleration, 1.0 / 60.0, Some(probe_hit), options)?;
            velocity = result.velocity;
            for (p, d) in position.iter_mut().zip(result.displacement) {
                *p += d;
            }
            frames.push(serde_json::json!({"tick":tick,"position":position,"requested_acceleration":acceleration,"result":result}));
        }
        probes.push(serde_json::json!({"name":name,"options":options,"preset_lookahead_hit":probe_hit,"frames":frames,"final_position":position,"final_velocity":velocity}));
    }
    let terminal_case = free_fall(
        [6000.0, 8000.0, -12000.0],
        [0.0; 2],
        1.0 / 60.0,
        Some(false),
        options,
    )?;
    let split_case = free_fall([0.0; 3], [0.0; 2], 0.12, Some(false), options)?;
    let unknown_probe = free_fall([0.0; 3], [0.0; 2], 1.0 / 60.0, None, options).err();
    let budget_error = free_fall([0.0; 3], [0.0; 2], 1.0, Some(false), options).err();
    let out = Path::new(&args[2]);
    if let Some(parent) = out.parent() {
        fs::create_dir_all(parent)?;
    }
    fs::write(
        out,
        serde_json::to_vec_pretty(
            &serde_json::json!({"profile":profile,"probes":probes,"terminal_case":terminal_case,"split_case":split_case,"unknown_probe_error":unknown_probe,"budget_error":budget_error,"scope":"1080 isolated ticks seeded with JumpZ475, plus terminal and substep cases. Original base-profile scalars; gravity110 controlled variant. Lookahead hit/clear is preset, not a world query; no collision, landing, damage, ModifyVelocity/apex callbacks, native flags, live volume changes, SSE-f32 parity, safe position candidate or Android integration. Existing controller/walking bridge unchanged."}),
        )?,
    )?;
    println!("3 isolated free-flight sequences; 1080 ticks plus terminal/substep cases");
    Ok(())
}
