//! Core FRotator::Vector and engine UTexPanner::GetMatrix (10452050).
use crate::{
    material_uv::UvTransform,
    properties::{Properties, Value},
};
use serde::{Deserialize, Serialize};
const SINE: &[u8; 65536] = include_bytes!("material_sine_table.bin");
fn sin_tab(angle: i32) -> f32 {
    let index = ((angle >> 2) & 16383) as usize * 4;
    f32::from_bits(u32::from_le_bytes(
        SINE[index..index + 4].try_into().unwrap(),
    ))
}
/// Quantized native rotator lookup. Roll does not affect this direction vector.
pub fn rotator_direction(rotation: [i32; 3]) -> [f32; 3] {
    let cosine_pitch = sin_tab(rotation[0].wrapping_add(16384));
    [
        sin_tab(rotation[1].wrapping_add(16384)) * cosine_pitch,
        sin_tab(rotation[1]) * cosine_pitch,
        sin_tab(rotation[0]),
    ]
}
#[derive(Clone, Copy, Debug, Serialize, Deserialize)]
pub struct TexPanner {
    pub direction: [i32; 3],
    pub rate: f32,
}
impl TexPanner {
    pub fn from_properties(defaults: &Properties, instance: &Properties) -> Result<Self, String> {
        let find = |name: &str| {
            instance
                .values
                .iter()
                .find(|p| p.name == name && p.array_index == 0)
                .or_else(|| {
                    defaults
                        .values
                        .iter()
                        .find(|p| p.name == name && p.array_index == 0)
                })
                .map(|p| &p.value)
        };
        let rate = match find("PanRate") {
            Some(Value::Float(v)) if v.is_finite() => *v,
            _ => return Err("Missing or invalid PanRate".into()),
        };
        let direction = match find("PanDirection") {
            Some(Value::Rotator(v)) => *v,
            None => [0; 3],
            _ => return Err("Invalid PanDirection".into()),
        };
        Ok(Self { direction, rate })
    }
    pub fn at(self, time: f32) -> Result<UvTransform, String> {
        if !self.rate.is_finite() || !time.is_finite() {
            return Err("Invalid TexPanner rate/time".into());
        }
        let direction = rotator_direction(self.direction);
        let mut offset = [0.; 2];
        for axis in 0..2 {
            // Three separate MULSS instructions, then original x87 reduction.
            let phase = ((self.rate * direction[axis]) * time) * 0.0009765625;
            if !phase.is_finite() {
                return Err("TexPanner phase overflow".into());
            }
            let rounded = ((f64::from(phase) + 103_079_215_104.).to_bits() as u32 as i32) >> 16;
            offset[axis] = (phase - rounded as f32) * 1024.;
            if !offset[axis].is_finite() {
                return Err("TexPanner offset overflow".into());
            }
        }
        Ok(UvTransform {
            scale: [1.; 2],
            offset,
        })
    }
}
#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn quantization_wrap_negative_angles_and_roll_ignored() {
        let a = rotator_direction([1232, -5676, 0]);
        assert_eq!(a, rotator_direction([1235, -5673, i32::MAX]));
        assert_eq!(a, rotator_direction([1232 + 65536, -5676 - 65536, 1]));
        assert_eq!(rotator_direction([0, 0, 0]), [1., 0., 0.]);
    }
    #[test]
    fn original_table_retains_nonzero_quarter_turn_residual() {
        let d = rotator_direction([0, 16384, 0]);
        assert_eq!(d[1], 1.);
        assert!(d[0] < 0. && d[0].abs() < 0.000001);
    }
    #[test]
    fn periodic_shift_has_1024_unit_period() {
        let p = TexPanner {
            direction: [0; 3],
            rate: 1.,
        };
        assert_eq!(p.at(1024.25).unwrap().offset, [0.25, 0.]);
        assert_eq!(p.at(-0.25).unwrap().offset, [1023.75, 0.]);
    }
    #[test]
    fn invalid_inputs_and_overflow_are_bounded() {
        let p = TexPanner {
            direction: [0; 3],
            rate: f32::MAX,
        };
        assert!(p.at(2.).is_err());
        assert!(p.at(f32::NAN).is_err());
        assert!(TexPanner {
            rate: f32::INFINITY,
            ..p
        }
        .at(0.)
        .is_err());
    }
    fn props(items: &[(&str, Value)]) -> Properties {
        Properties {
            native_offset: 0,
            values: items
                .iter()
                .map(|(name, value)| crate::properties::Property {
                    name: (*name).into(),
                    kind: 4,
                    array_index: 0,
                    struct_name: None,
                    bytes: 4,
                    declared_bytes: 4,
                    payload_offset: 0,
                    value: value.clone(),
                })
                .collect(),
        }
    }
    #[test]
    fn original_default_rate_and_instance_rotator_override() {
        let default = props(&[("PanRate", Value::Float(0.1))]);
        let instance = props(&[
            ("PanDirection", Value::Rotator([123, 456, 789])),
            ("PanRate", Value::Float(0.)),
            ("M", Value::Float(f32::NAN)),
        ]);
        let p = TexPanner::from_properties(&default, &instance).unwrap();
        assert_eq!(p.direction, [123, 456, 789]);
        assert_eq!(p.rate, 0.);
        assert_eq!(
            TexPanner::from_properties(&default, &props(&[]))
                .unwrap()
                .direction,
            [0; 3]
        );
    }
    #[test]
    fn invalid_override_is_not_replaced_by_defaults() {
        let default = props(&[("PanRate", Value::Float(0.1))]);
        assert!(TexPanner::from_properties(
            &default,
            &props(&[("PanRate", Value::Float(f32::NAN))])
        )
        .is_err());
        assert!(
            TexPanner::from_properties(&default, &props(&[("PanDirection", Value::Int(0))]))
                .is_err()
        );
        assert!(TexPanner::from_properties(&props(&[]), &props(&[])).is_err());
    }
}
