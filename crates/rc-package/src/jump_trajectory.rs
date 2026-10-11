//! Prepared AActor::CalcTrajectoryVelocity (engine.dll 103bbfa0).
//!
//! The caller supplies the RSQRTSS seed boundary. This is trajectory planning,
//! not collision-aware flight integration or a complete JumpTo goal.
use serde::{Deserialize, Serialize};

#[derive(Clone, Copy, Debug, Deserialize, Serialize)]
pub struct Input {
    pub origin: [f32; 3],
    pub target: [f32; 3],
    pub horizontal_speed: f32,
    pub gravity_z: f32,
    pub allow_flat_arcs: bool,
}

#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Trajectory {
    pub velocity: [f32; 3],
    pub time: f32,
    pub extensions: u8,
}

fn inverse(s: f32, seed: &mut impl FnMut(f32) -> f32) -> f32 {
    let r = seed(s);
    (3.0 - (r * s) * r) * (r * 0.5)
}

fn normalized(v: [f32; 3], seed: &mut impl FnMut(f32) -> f32) -> [f32; 3] {
    let s = (v[2] * v[2] + v[1] * v[1]) + v[0] * v[0];
    if s == 0.0 {
        v
    } else {
        let r = inverse(s, seed);
        v.map(|x| x * r)
    }
}

/// Preserve scalar SSE operation order, zero masking, and strict arc decisions.
/// The original does not validate inputs. Intended prepared inputs are finite
/// positions/gravity and a positive horizontal speed, without float overflow.
pub fn calculate(input: Input, mut seed: impl FnMut(f32) -> f32) -> Trajectory {
    let gravity = input.gravity_z * -0.5;
    let d = std::array::from_fn::<_, 3, _>(|i| input.target[i] - input.origin[i]);
    let squared = d[0] * d[0] + d[1] * d[1];
    // RSQRTSS runs even at zero; ANDPS subsequently masks the NaN to +0.
    let horizontal = inverse(squared, &mut seed) * squared;
    let horizontal = if squared == 0.0 { 0.0 } else { horizontal };
    let candidate = horizontal / input.horizontal_speed;
    let mut time = if candidate >= 0.5 { candidate } else { 0.5 };
    let velocity = |t: f32| {
        let r = 1.0 / t;
        [d[0] * r, d[1] * r, d[2] * r + t * gravity]
    };
    let mut v = velocity(time);
    let descending = v[2] < 0.0;
    if !descending {
        if input.allow_flat_arcs {
            return Trajectory {
                velocity: v,
                time,
                extensions: 0,
            };
        }
        let n = normalized(v, &mut seed);
        let direction = normalized(d, &mut seed);
        let dot = (direction[0] * n[0] + n[1] * direction[1]) + n[2] * direction[2];
        if dot.partial_cmp(&0.95) != Some(std::cmp::Ordering::Greater) {
            return Trajectory {
                velocity: v,
                time,
                extensions: 0,
            };
        }
    }
    // The original repeats this normalization, including in descending mode.
    let direction = normalized(d, &mut seed);
    let mut extensions = 0;
    for _ in 0..4 {
        time *= 1.5;
        extensions += 1;
        v = velocity(time);
        if descending {
            if v[2] > 0.0 {
                break;
            }
        } else {
            let n = normalized(v, &mut seed);
            let dot = (n[1] * direction[1] + n[2] * direction[2]) + n[0] * direction[0];
            if dot < 0.95 {
                break;
            }
        }
    }
    Trajectory {
        velocity: v,
        time,
        extensions,
    }
}

/// Portable seed adapter; this does not emulate a processor's RSQRTSS estimate.
pub fn portable(input: Input) -> Trajectory {
    calculate(input, |s| 1.0 / s.sqrt())
}

#[cfg(test)]
mod tests {
    use super::*;
    fn input(target: [f32; 3]) -> Input {
        Input {
            origin: [0.0; 3],
            target,
            horizontal_speed: 600.0,
            gravity_z: -1500.0,
            allow_flat_arcs: false,
        }
    }
    #[test]
    fn zero_horizontal_masks_seed_nan_and_keeps_minimum_time() {
        let mut calls = vec![];
        let result = calculate(input([0.0; 3]), |s| {
            calls.push(s);
            1.0 / s.sqrt()
        });
        assert_eq!(calls[0].to_bits(), 0);
        assert_eq!(result.time, 0.5);
        assert_eq!(result.velocity, [0.0, 0.0, 375.0]);
    }
    #[test]
    fn descending_ignores_flat_arc_permission_and_limits_extensions() {
        let mut i = input([0.0, 0.0, -100000.0]);
        let result = portable(i);
        assert_eq!(result.extensions, 4);
        assert!(result.velocity[2] < 0.0);
        i.allow_flat_arcs = true;
        assert_eq!(portable(i), result);
    }
    #[test]
    fn flat_permission_returns_initial_nonnegative_arc() {
        let mut i = input([500.0, 0.0, 0.0]);
        i.gravity_z = 0.0;
        assert_eq!(portable(i).extensions, 4);
        i.allow_flat_arcs = true;
        assert_eq!(portable(i).extensions, 0);
    }
}
