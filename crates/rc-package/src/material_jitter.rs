//! UTexOscillator::GetMatrix OT_Jitter branches, engine.dll 104539c9..10453c39.
use crate::{
    material_uv::UvTransform,
    properties::{Properties, Value},
};
use serde::{Deserialize, Serialize};
#[derive(Clone, Copy, Debug, Serialize, Deserialize, PartialEq)]
pub struct JitterState {
    pub last: [f32; 2],
    pub current: [f32; 2],
}
#[derive(Clone, Copy, Debug, Serialize, Deserialize)]
pub struct TexJitter {
    pub rate: [f32; 2],
    pub phase: [f32; 2],
    pub amplitude: [f32; 2],
    pub kind: [u8; 2],
    pub pivot: [f32; 2],
}
impl TexJitter {
    pub fn from_properties(
        defaults: &Properties,
        instance: &Properties,
    ) -> Result<(Self, JitterState), String> {
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
        let number = |name: &str, zero: bool| match find(name) {
            Some(Value::Float(v)) if v.is_finite() => Ok(*v),
            None if zero => Ok(0.),
            _ => Err(format!("Invalid oscillator {name}")),
        };
        let kind = |name: &str| match find(name) {
            Some(Value::Byte(v)) => Ok(*v),
            None => Ok(0),
            _ => Err(format!("Invalid oscillator {name}")),
        };
        Ok((
            Self {
                rate: [
                    number("UOscillationRate", false)?,
                    number("VOscillationRate", false)?,
                ],
                phase: [
                    number("UOscillationPhase", true)?,
                    number("VOscillationPhase", true)?,
                ],
                amplitude: [
                    number("UOscillationAmplitude", false)?,
                    number("VOscillationAmplitude", false)?,
                ],
                kind: [kind("UOscillationType")?, kind("VOscillationType")?],
                pivot: [number("UOffset", true)?, number("VOffset", true)?],
            },
            JitterState {
                last: [number("LastSu", true)?, number("LastSv", true)?],
                current: [
                    number("CurrentUJitter", true)?,
                    number("CurrentVJitter", true)?,
                ],
            },
        ))
    }
    /// Random callback is the host's shared CRT rand stream (0..32767).
    /// Completed state writes survive callback failure; random calls cannot roll back.
    pub fn step(
        self,
        time: f32,
        state: &mut JitterState,
        random: &mut dyn FnMut() -> Result<u16, String>,
    ) -> Result<UvTransform, String> {
        if self.kind != [3; 2] {
            return Err("Only dual-axis OT_Jitter is supported".into());
        }
        if self.pivot != [0.; 2] {
            return Err("Oscillator pivot matrix composition is not supported".into());
        }
        if !time.is_finite()
            || self
                .rate
                .iter()
                .chain(&self.phase)
                .chain(&self.amplitude)
                .chain(&self.pivot)
                .chain(&state.last)
                .chain(&state.current)
                .any(|v| !v.is_finite())
        {
            return Err("Nonfinite oscillator input/state".into());
        }
        let s = self.rate.map(|r| r * time);
        let mut reset = [0.; 2];
        for axis in 0..2 {
            // LDMXCSR 0x3f80 selects round toward negative infinity, not nearest.
            if !s[axis].is_finite()
                || f64::from(s[axis]) < f64::from(i32::MIN)
                || f64::from(s[axis]) >= 2147483648.
            {
                return Err("Oscillator integer phase outside native range".into());
            }
            reset[axis] = (s[axis].floor() as i32) as f32 + self.phase[axis];
            if !reset[axis].is_finite() {
                return Err("Oscillator reset overflow".into());
            }
        }
        for axis in 0..2 {
            // V deliberately compares LastSu after U has been processed.
            if state.last[axis] < 1. || state.last[0] > s[axis] + 1. {
                state.last[axis] = reset[axis];
            }
            if s[axis] - state.last[axis] > 1. {
                let value = random()?;
                if value > 32767 {
                    return Err("CRT rand sample outside 0..32767".into());
                }
                let current = (value as f32 * self.amplitude[axis]) * f32::from_bits(0x38000100);
                if !current.is_finite() {
                    return Err("Oscillator jitter overflow".into());
                }
                state.current[axis] = current;
                state.last[axis] = reset[axis];
            }
        }
        Ok(UvTransform {
            scale: [1.; 2],
            offset: state.current,
        })
    }
}
#[cfg(test)]
mod tests {
    use super::*;
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
    fn serialized_state_and_explicit_zero_override_class_defaults() {
        let defaults = props(&[
            ("UOscillationRate", Value::Float(1.)),
            ("VOscillationRate", Value::Float(1.)),
            ("UOscillationAmplitude", Value::Float(0.1)),
            ("VOscillationAmplitude", Value::Float(0.1)),
        ]);
        let instance = props(&[
            ("UOscillationRate", Value::Float(80.)),
            ("VOscillationRate", Value::Float(0.)),
            ("UOscillationType", Value::Byte(3)),
            ("VOscillationType", Value::Byte(3)),
            ("LastSu", Value::Float(30720.)),
            ("CurrentUJitter", Value::Float(0.05)),
            ("CurrentVJitter", Value::Float(0.03)),
            ("M", Value::Float(f32::NAN)),
        ]);
        let (p, s) = TexJitter::from_properties(&defaults, &instance).unwrap();
        assert_eq!(p.rate, [80., 0.]);
        assert_eq!(p.kind, [3; 2]);
        assert_eq!(s.last, [30720., 0.]);
        assert_eq!(s.current, [0.05, 0.03]);
    }
    #[test]
    fn invalid_serialized_state_or_missing_defaults_is_rejected() {
        let defaults = props(&[
            ("UOscillationRate", Value::Float(1.)),
            ("VOscillationRate", Value::Float(1.)),
            ("UOscillationAmplitude", Value::Float(0.1)),
            ("VOscillationAmplitude", Value::Float(0.1)),
        ]);
        assert!(TexJitter::from_properties(
            &defaults,
            &props(&[("LastSu", Value::Float(f32::NAN))])
        )
        .is_err());
        assert!(TexJitter::from_properties(&props(&[]), &props(&[])).is_err());
    }
    fn fixture() -> (TexJitter, JitterState) {
        (
            TexJitter {
                rate: [1.; 2],
                phase: [0.; 2],
                amplitude: [1.; 2],
                kind: [3; 2],
                pivot: [0.; 2],
            },
            JitterState {
                last: [1.; 2],
                current: [0.25, 0.5],
            },
        )
    }
    #[test]
    fn strict_threshold_preserves_current_and_consumes_no_random() {
        let (p, mut s) = fixture();
        let before = s;
        let t = p
            .step(2., &mut s, &mut || panic!("unexpected rand"))
            .unwrap();
        assert_eq!(s, before);
        assert_eq!(t.offset, before.current);
    }
    #[test]
    fn u_then_v_consume_shared_random_even_for_zero_amplitude() {
        let (mut p, mut s) = fixture();
        p.amplitude[1] = 0.;
        let mut calls = 0;
        p.step(3., &mut s, &mut || {
            calls += 1;
            Ok(if calls == 1 { 32767 } else { 123 })
        })
        .unwrap();
        assert_eq!(calls, 2);
        assert_eq!(s.last, [3.; 2]);
        assert_eq!(s.current, [1., 0.]);
    }
    #[test]
    fn v_reset_uses_u_last_and_negative_floor_not_nearest() {
        let (mut p, mut s) = fixture();
        p.rate = [1., -1.];
        p.phase = [1.5, 1.5];
        s.last = [10., 1.];
        p.step(0.25, &mut s, &mut || panic!("rand")).unwrap();
        assert_eq!(s.last, [1.5, 0.5]);
    }
    #[test]
    fn callback_failure_preserves_completed_u_state() {
        let (p, mut s) = fixture();
        let mut calls = 0;
        assert!(p
            .step(3., &mut s, &mut || {
                calls += 1;
                if calls == 1 {
                    Ok(32767)
                } else {
                    Err("host".into())
                }
            })
            .is_err());
        assert_eq!(s.last, [3., 1.]);
        assert_eq!(s.current, [1., 0.5]);
    }
    #[test]
    fn invalid_input_and_unhandled_modes_do_not_consume_random_or_mutate() {
        let (mut p, mut s) = fixture();
        let before = s;
        p.kind[0] = 0;
        assert!(p.step(0., &mut s, &mut || panic!("rand")).is_err());
        assert_eq!(s, before);
        p.kind = [3; 2];
        assert!(p.step(f32::NAN, &mut s, &mut || panic!("rand")).is_err());
        assert_eq!(s, before);
        p.rate[0] = f32::MAX;
        assert!(p.step(2., &mut s, &mut || panic!("rand")).is_err());
        assert_eq!(s, before);
    }
}
