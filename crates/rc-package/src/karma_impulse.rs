//! Selected engine.dll KAddImpulse wrapper and x87 accumulator arithmetic.
//! VM dispatch, model lookup and the MathEngine time integrator are boundaries.
use serde::{Deserialize, Serialize};
pub const SCALE: f32 = 0.02;
pub const WORLD_UNITS: f64 = 50.;
#[derive(Clone, Copy, Debug, Default, Serialize, Deserialize, PartialEq)]
pub struct Accumulator {
    pub linear: [f32; 3],
    pub angular: [f32; 3],
}
pub fn accumulate(
    old: Accumulator,
    impulse: [f32; 3],
    position: [f32; 3],
    center: [f32; 3],
) -> Result<Accumulator, String> {
    if old
        .linear
        .iter()
        .chain(&old.angular)
        .chain(&impulse)
        .chain(&position)
        .chain(&center)
        .any(|v| !v.is_finite() || v.abs() > 1e8)
    {
        return Err("Invalid Karma impulse accumulator".into());
    }
    // All arguments here are already in Karma units. The first two torque
    // components spill to float before accumulation; Z remains on the x87 stack.
    let r: [f64; 3] = std::array::from_fn(|i| f64::from(position[i]) - f64::from(center[i]));
    let j = impulse.map(f64::from);
    let torque = [
        r[1] * j[2] - r[2] * j[1],
        r[2] * j[0] - r[0] * j[2],
        r[0] * j[1] - r[1] * j[0],
    ];
    Ok(Accumulator {
        linear: std::array::from_fn(|i| (f64::from(old.linear[i]) + j[i]) as f32),
        angular: std::array::from_fn(|i| {
            (f64::from(old.angular[i])
                + if i < 2 {
                    f64::from(torque[i] as f32)
                } else {
                    torque[i]
                }) as f32
        }),
    })
}
pub fn world_impulse(
    impulse: [f32; 3],
    position: [f32; 3],
    center: [f32; 3],
) -> Result<Accumulator, String> {
    accumulate(
        Accumulator::default(),
        impulse.map(|v| v * SCALE),
        position.map(|v| v * SCALE),
        center.map(|v| v * SCALE),
    )
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn exported_original_x87_accumulator_fixtures_preserve_spill_rounding() {
        let oracle: serde_json::Value =
            serde_json::from_str(include_str!("../tests/fixtures/karma-impulse.json")).unwrap();
        assert_eq!(
            oracle["scale_bits"].as_u64().unwrap(),
            u64::from(SCALE.to_bits())
        );
        let vector = |v: &serde_json::Value| {
            std::array::from_fn(|i| f32::from_bits(v[i].as_u64().unwrap() as u32))
        };
        let frames = oracle["frames"].as_array().unwrap();
        assert_eq!(frames.len(), 256);
        for f in frames {
            let a = &f["input"];
            let actual = accumulate(
                Accumulator {
                    linear: vector(&a[0]),
                    angular: vector(&a[1]),
                },
                vector(&a[2]),
                vector(&a[3]),
                vector(&a[4]),
            )
            .unwrap();
            assert_eq!(
                actual.linear.map(f32::to_bits),
                vector(&f["linear"]).map(f32::to_bits)
            );
            assert_eq!(
                actual.angular.map(f32::to_bits),
                vector(&f["angular"]).map(f32::to_bits)
            );
        }
    }
    #[test]
    fn off_center_impulse_has_correct_units_and_axis_and_invalid_input_is_rejected() {
        let a = world_impulse([200., 0., 0.], [0., 100., 0.], [0.; 3]).unwrap();
        assert_eq!(a.linear, [4., 0., 0.]);
        assert_eq!(a.angular, [0., 0., -8.]);
        assert!(world_impulse([f32::NAN, 0., 0.], [0.; 3], [0.; 3]).is_err());
    }
}
