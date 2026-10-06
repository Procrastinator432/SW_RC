//! Collision-free subset of APawn::physFalling10496dc0; no runtime Pawn state machine.
use serde::Serialize;

/// Original engine.dll VA10665bbc (shared AirControl threshold and maximum substep).
pub const FALLING_STEP: f64 = 0.05_f32 as f64;
/// Original engine.dll VA10668e18.
pub const LOW_AIR_SPEED: f64 = 10.0;

#[derive(Debug, Clone, Copy, Serialize)]
pub struct FreeFallOptions {
    pub acceleration_rate: f64,
    pub air_control: f64,
    pub ground_speed: f64,
    pub terminal_speed: f64,
    pub gravity: [f64; 3],
    pub zone_velocity: [f64; 3],
}
#[derive(Debug, Serialize)]
pub struct FreeFallSubstep {
    pub dt: f64,
    pub velocity_before_terminal: [f64; 3],
    /// Native move uses velocity before the terminal cap; cap follows the move.
    pub displacement: [f64; 3],
    pub velocity: [f64; 3],
}
#[derive(Debug, Serialize)]
pub struct FreeFallFrame {
    pub effective_air_control: f64,
    pub lookahead_displacement: Option<[f64; 3]>,
    pub acceleration_limit: f64,
    pub acceleration: [f64; 3],
    pub horizontal_speed_cap: Option<f64>,
    pub substeps: Vec<FreeFallSubstep>,
    pub displacement: [f64; 3],
    pub velocity: [f64; 3],
}
fn magnitude(v: [f64; 3]) -> f64 {
    v[0].hypot(v[1]).hypot(v[2])
}

/// Plan an unobstructed tick. Caller must supply the native lookahead outcome when required.
/// `Some(true)` means actor hit, `Some(false)` means clear; None is not a clear fallback.
/// Collision/landings, callbacks, volume changes, native flags and SSE-f32 parity are absent.
pub fn free_fall(
    velocity: [f64; 3],
    requested_acceleration: [f64; 2],
    dt: f64,
    air_control_probe_hit: Option<bool>,
    options: FreeFallOptions,
) -> Result<FreeFallFrame, String> {
    if !dt.is_finite()
        || dt <= 0.0
        || dt > 1.0
        || velocity
            .iter()
            .chain(requested_acceleration.iter())
            .chain(options.gravity.iter())
            .chain(options.zone_velocity.iter())
            .any(|v| !v.is_finite() || v.abs() > 1e6)
        || [options.acceleration_rate, options.ground_speed]
            .iter()
            .any(|v| !v.is_finite() || !(0.0..=1e6).contains(v))
        || !options.terminal_speed.is_finite()
        || options.terminal_speed <= 0.0
        || options.terminal_speed > 1e6
        || !options.air_control.is_finite()
        || !(0.0..=1.0).contains(&options.air_control)
    {
        return Err("invalid free-fall diagnostic input/options".into());
    }
    let length = requested_acceleration[0].hypot(requested_acceleration[1]);
    let direction = if length > 0.0 {
        requested_acceleration.map(|v| v / length)
    } else {
        [0.0; 2]
    };
    let mut air = options.air_control;
    let lookahead_displacement = if air > FALLING_STEP {
        let hit = air_control_probe_hit.ok_or("required air-control lookahead result unknown")?;
        let displacement = [
            (velocity[0] + direction[0] * options.acceleration_rate * air) * dt,
            (velocity[1] + direction[1] * options.acceleration_rate * air) * dt,
            0.0,
        ];
        if hit {
            air = 0.0;
        }
        Some(displacement)
    } else {
        None
    };
    let speed = velocity[0].hypot(velocity[1]);
    let mut acceleration_limit = options.acceleration_rate * air;
    let mut horizontal_speed_cap = None;
    if speed < LOW_AIR_SPEED && air > 0.0 {
        acceleration_limit += (LOW_AIR_SPEED - speed) / dt;
    } else if speed >= options.ground_speed {
        if air <= FALLING_STEP {
            acceleration_limit = 1.0;
        } else {
            horizontal_speed_cap = Some(speed);
        }
    }
    let limited = length.min(acceleration_limit);
    let acceleration = [direction[0] * limited, direction[1] * limited, 0.0];
    let mut velocity = velocity;
    let mut remaining = dt;
    let mut displacement = [0.0; 3];
    let mut substeps = Vec::new();
    while remaining > 0.0 {
        // Native counter starts at zero here; partial budget exhaustion is an explicit error.
        if substeps.len() == 8 {
            return Err("native free-fall eight-substep budget exhausted".into());
        }
        let step = if remaining > FALLING_STEP {
            (remaining * 0.5).min(FALLING_STEP)
        } else {
            remaining
        };
        let mut updated =
            std::array::from_fn(|i| velocity[i] + (options.gravity[i] + acceleration[i]) * step);
        if let Some(cap) = horizontal_speed_cap {
            let speed = updated[0].hypot(updated[1]);
            if speed > cap {
                updated[0] *= cap / speed;
                updated[1] *= cap / speed;
            }
        }
        let moved: [f64; 3] =
            std::array::from_fn(|i| (updated[i] + options.zone_velocity[i]) * step);
        for i in 0..3 {
            displacement[i] += moved[i];
        }
        let total_speed = magnitude(updated);
        velocity = if total_speed > options.terminal_speed {
            updated.map(|v| v * options.terminal_speed / total_speed)
        } else {
            updated
        };
        substeps.push(FreeFallSubstep {
            dt: step,
            velocity_before_terminal: updated,
            displacement: moved,
            velocity,
        });
        remaining -= step;
    }
    Ok(FreeFallFrame {
        effective_air_control: air,
        lookahead_displacement,
        acceleration_limit,
        acceleration,
        horizontal_speed_cap,
        substeps,
        displacement,
        velocity,
    })
}

#[cfg(test)]
mod tests {
    use super::*;
    fn options() -> FreeFallOptions {
        FreeFallOptions {
            acceleration_rate: 1024.0,
            air_control: 0.35,
            ground_speed: 450.0,
            terminal_speed: 12000.0,
            gravity: [0.0, 0.0, -1100.0],
            zone_velocity: [0.0; 3],
        }
    }
    fn near(a: f64, b: f64) {
        assert!((a - b).abs() < 1e-8, "{a} != {b}");
    }
    #[test]
    fn low_speed_boost_preserves_analog_acceleration_and_boundary() {
        let f = free_fall([0.0; 3], [1024.0, 0.0], 0.05, Some(false), options()).unwrap();
        near(f.acceleration_limit, 558.4);
        near(f.velocity[0], 27.92);
        let f = free_fall([0.0; 3], [100.0, 0.0], 0.05, Some(false), options()).unwrap();
        near(f.velocity[0], 5.0);
        let f = free_fall(
            [10.0, 0.0, 0.0],
            [1024.0, 0.0],
            0.05,
            Some(false),
            options(),
        )
        .unwrap();
        near(f.acceleration_limit, 358.4);
    }
    #[test]
    fn lookahead_unknown_is_not_clear_and_hit_removes_air_control() {
        assert!(free_fall([20.0, 0.0, 0.0], [1024.0, 0.0], 0.05, None, options()).is_err());
        let f = free_fall([20.0, 0.0, 0.0], [1024.0, 0.0], 0.05, Some(true), options()).unwrap();
        near(f.velocity[0], 20.0);
        near(f.effective_air_control, 0.0);
        near(f.lookahead_displacement.unwrap()[0], (20.0 + 358.4) * 0.05);
        let f = free_fall(
            [20.0, 0.0, 0.0],
            [1024.0, 0.0],
            0.05,
            None,
            FreeFallOptions {
                air_control: FALLING_STEP,
                ..options()
            },
        )
        .unwrap();
        assert!(f.lookahead_displacement.is_none());
    }
    #[test]
    fn high_speed_keeps_existing_speed_cap_and_low_control_unit_limit() {
        let f = free_fall(
            [600.0, 0.0, 0.0],
            [1024.0, 0.0],
            0.05,
            Some(false),
            options(),
        )
        .unwrap();
        assert_eq!(f.horizontal_speed_cap, Some(600.0));
        near(f.velocity[0], 600.0);
        let f = free_fall(
            [449.0, 0.0, 0.0],
            [1024.0, 0.0],
            0.05,
            Some(false),
            options(),
        )
        .unwrap();
        assert!(f.horizontal_speed_cap.is_none());
        near(f.velocity[0], 466.92);
        let f = free_fall(
            [600.0, 0.0, 0.0],
            [1024.0, 0.0],
            0.05,
            None,
            FreeFallOptions {
                air_control: 0.0,
                ..options()
            },
        )
        .unwrap();
        near(f.acceleration_limit, 1.0);
        near(f.velocity[0], 600.05);
    }
    #[test]
    fn terminal_cap_is_three_dimensional_and_follows_displacement() {
        let f = free_fall(
            [300.0, 400.0, -1200.0],
            [0.0; 2],
            0.05,
            None,
            FreeFallOptions {
                air_control: 0.0,
                gravity: [0.0; 3],
                terminal_speed: 1000.0,
                ..options()
            },
        )
        .unwrap();
        assert_eq!(f.displacement, [15.0, 20.0, -60.0]);
        near(magnitude(f.velocity), 1000.0);
        near(f.velocity[2], -1200.0 / 1.3);
        let f = free_fall(
            [0.0; 3],
            [0.0; 2],
            0.05,
            None,
            FreeFallOptions {
                air_control: 0.0,
                gravity: [0.0; 3],
                zone_velocity: [10.0, 20.0, 30.0],
                ..options()
            },
        )
        .unwrap();
        assert_eq!(f.displacement, [0.5, 1.0, 1.5]);
        assert_eq!(f.velocity, [0.0; 3]);
    }
    #[test]
    fn substeps_use_remaining_half_and_reject_budget_or_invalid_inputs() {
        let f = free_fall([0.0; 3], [0.0; 2], 0.12, Some(false), options()).unwrap();
        assert_eq!(f.substeps.len(), 3);
        near(f.substeps[0].dt, FALLING_STEP);
        near(f.substeps[1].dt, (0.12 - FALLING_STEP) / 2.0);
        near(f.velocity[2], -132.0);
        let expected_displacement = -1100.0
            * (0.12_f64.powi(2) + f.substeps.iter().map(|s| s.dt * s.dt).sum::<f64>())
            / 2.0;
        near(f.displacement[2], expected_displacement);
        assert!(free_fall([0.0; 3], [0.0; 2], 1.0, Some(false), options()).is_err());
        for dt in [0.0, -0.1, f64::NAN] {
            assert!(free_fall([0.0; 3], [0.0; 2], dt, Some(false), options()).is_err());
        }
        assert!(free_fall(
            [f64::INFINITY, 0.0, 0.0],
            [0.0; 2],
            0.01,
            Some(false),
            options()
        )
        .is_err());
    }
}
