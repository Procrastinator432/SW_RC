//! UTexPanner2D::GetMatrix (engine.dll 10452210), scalar SSE order.
use crate::properties::{Properties, Value};
use serde::{Deserialize, Serialize};

#[derive(Clone, Copy, Debug, Serialize, Deserialize, PartialEq)]
pub struct UvTransform {
    pub scale: [f32; 2],
    pub offset: [f32; 2],
}
impl UvTransform {
    pub fn apply(self, uv: [f32; 2]) -> [f32; 2] {
        [
            uv[0] * self.scale[0] + self.offset[0],
            uv[1] * self.scale[1] + self.offset[1],
        ]
    }
}
#[derive(Clone, Copy, Debug, Serialize, Deserialize)]
pub struct TexPanner2D {
    pub speed: [f32; 2],
    pub offset: [f32; 2],
    pub scale: [f32; 2],
    pub clamped_size: [f32; 2],
}
impl TexPanner2D {
    /// Serialized class delta over zero initialization, then instance overrides.
    /// The transient cached M property is deliberately not used by GetMatrix.
    pub fn from_properties(defaults: &Properties, instance: &Properties) -> Result<Self, String> {
        let get = |name: &str, zero: bool| -> Result<f32, String> {
            let value = instance
                .values
                .iter()
                .find(|p| p.name == name && p.array_index == 0)
                .or_else(|| {
                    defaults
                        .values
                        .iter()
                        .find(|p| p.name == name && p.array_index == 0)
                });
            match value.map(|p| &p.value) {
                Some(Value::Float(v)) if v.is_finite() => Ok(*v),
                None if zero => Ok(0.),
                _ => Err(format!("Missing or invalid TexPanner2D {name}")),
            }
        };
        Ok(Self {
            speed: [get("SpeedU", false)?, get("SpeedV", false)?],
            offset: [get("OffsetU", true)?, get("OffsetV", true)?],
            scale: [get("ScaleU", false)?, get("ScaleV", false)?],
            clamped_size: [get("ClampedSizeU", false)?, get("ClampedSizeV", false)?],
        })
    }
    pub fn at(self, time: f32) -> Result<UvTransform, String> {
        if !time.is_finite()
            || self
                .speed
                .iter()
                .chain(&self.offset)
                .chain(&self.scale)
                .chain(&self.clamped_size)
                .any(|v| !v.is_finite())
            || self.clamped_size.contains(&0.)
        {
            return Err("Invalid TexPanner2D parameters/time".into());
        }
        let mut offset = [0.; 2];
        for (axis, result) in offset.iter_mut().enumerate() {
            let size = self.clamped_size[axis];
            let phase = (self.speed[axis] / size) * time;
            if !phase.is_finite() {
                return Err("TexPanner2D phase overflow".into());
            }
            // Original FLD f32, FADD double 0x4238000000000000, FSTP double,
            // take low signed dword then SAR 16. This is NOT floor/fract.
            let rounded = ((f64::from(phase) + 103_079_215_104.).to_bits() as u32 as i32) >> 16;
            let mut value = (phase - rounded as f32) * size + self.offset[axis];
            if size > 1. && value > size - 1. {
                value -= size;
            }
            if !value.is_finite() {
                return Err("TexPanner2D offset overflow".into());
            }
            *result = value;
        }
        Ok(UvTransform {
            scale: self.scale,
            offset,
        })
    }
}
#[cfg(test)]
mod tests {
    use super::*;
    fn panner() -> TexPanner2D {
        TexPanner2D {
            speed: [1., -1.],
            offset: [0.; 2],
            scale: [2., 3.],
            clamped_size: [1.; 2],
        }
    }
    #[test]
    fn positive_and_negative_phase_use_native_signed_wrap() {
        assert_eq!(panner().at(0.75).unwrap().offset, [0.75, 0.25]);
        assert_eq!(panner().at(1.25).unwrap().offset, [0.25, 0.75]);
    }
    #[test]
    fn clamp_branch_is_strict_and_runs_once() {
        let mut p = panner();
        p.speed = [0.; 2];
        p.clamped_size = [4.; 2];
        p.offset = [3., 9.];
        assert_eq!(p.at(0.).unwrap().offset, [3., 5.]);
    }
    #[test]
    fn transform_keeps_scale_and_offset_separate() {
        let t = panner().at(0.25).unwrap();
        assert_eq!(t.apply([2., 4.]), [4.25, 12.75]);
    }
    #[test]
    fn invalid_inputs_and_intermediate_overflow_are_rejected() {
        assert!(panner().at(f32::NAN).is_err());
        let mut p = panner();
        p.clamped_size[0] = 0.;
        assert!(p.at(1.).is_err());
        p = panner();
        p.speed[0] = f32::MAX;
        assert!(p.at(2.).is_err());
    }
    fn properties(items: &[(&str, f32)]) -> Properties {
        Properties {
            native_offset: 0,
            values: items
                .iter()
                .map(|(name, v)| crate::properties::Property {
                    name: (*name).into(),
                    kind: 4,
                    array_index: 0,
                    struct_name: None,
                    bytes: 4,
                    declared_bytes: 4,
                    payload_offset: 0,
                    value: Value::Float(*v),
                })
                .collect(),
        }
    }
    #[test]
    fn instance_zero_overrides_nonzero_class_delta_and_transient_matrix_is_ignored() {
        let defaults = properties(&[
            ("SpeedU", 0.5),
            ("SpeedV", 0.5),
            ("ScaleU", 1.),
            ("ScaleV", 1.),
            ("ClampedSizeU", 1.),
            ("ClampedSizeV", 1.),
        ]);
        let instance = properties(&[("SpeedU", 0.), ("OffsetV", 0.25), ("M", f32::NAN)]);
        let p = TexPanner2D::from_properties(&defaults, &instance).unwrap();
        assert_eq!(p.speed, [0., 0.5]);
        assert_eq!(p.offset, [0., 0.25]);
    }
    #[test]
    fn missing_default_and_invalid_override_do_not_fall_back() {
        let defaults = properties(&[]);
        assert!(TexPanner2D::from_properties(&defaults, &defaults).is_err());
        let defaults = properties(&[
            ("SpeedU", 0.5),
            ("SpeedV", 0.5),
            ("ScaleU", 1.),
            ("ScaleV", 1.),
            ("ClampedSizeU", 1.),
            ("ClampedSizeV", 1.),
        ]);
        assert!(
            TexPanner2D::from_properties(&defaults, &properties(&[("SpeedU", f32::NAN)])).is_err()
        );
    }
}
