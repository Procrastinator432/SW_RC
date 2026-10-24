//! Original audio curve state, 103d1cf0 / 103d1dc0 and GetFadeMultiplier's advance.
//! Transcendental helper rounding is an explicit math-policy boundary.
use serde::{Deserialize, Serialize};

pub trait CurveMath {
    fn transform(&mut self, kind: u8, encode: bool, value: f32) -> Result<f32, String>;
}
/// Implements the observed original helper formulas using portable f64 math.
/// Native fixtures check its f32 results on recorded probes; this is not a
/// universal replacement for the bundled CRT/x87 rounding on every CPU/input.
#[derive(Default)]
pub struct PortableCurveMath;
impl CurveMath for PortableCurveMath {
    fn transform(&mut self, kind: u8, encode: bool, value: f32) -> Result<f32, String> {
        if !value.is_finite() || !(1..=3).contains(&kind) {
            return Err("Invalid nonlinear audio curve input".into());
        }
        if value <= 0. {
            return Ok(0.);
        }
        if value >= 1. {
            return Ok(1.);
        }
        let x = f64::from(value);
        let result = match kind {
            1 if encode => x.asin() * f64::from(f32::from_bits(0x3f22f983)),
            1 => (x * f64::from(f32::from_bits(0x3fc90fdb))).sin(),
            2 | 3 => {
                // Both the encode AND decode helpers use this original power.
                let power = 10f64.powf(((x * 100.) - 100.) * f64::from(0.05f32));
                if kind == 2 {
                    power
                } else {
                    f64::from(value + value) - power
                }
            }
            _ => unreachable!(),
        } as f32;
        if result.is_finite() {
            Ok(result)
        } else {
            Err("Audio curve helper overflow".into())
        }
    }
}

/// Original six binary32 fields: position, encoded target, last evaluated
/// position, slope, cached value, requested value. Raw words survive JSON exactly.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Curve {
    pub words: [u32; 6],
    pub kind: u8,
}
impl Curve {
    pub fn new(value: f32) -> Result<Self, String> {
        gain(value)?;
        Ok(Self {
            words: [
                (-1f32).to_bits(),
                (-1f32).to_bits(),
                (-1f32).to_bits(),
                0,
                value.to_bits(),
                value.to_bits(),
            ],
            kind: 0,
        })
    }
    pub fn validate(&self) -> Result<(), String> {
        if self.kind > 3
            || self
                .words
                .iter()
                .map(|&v| f32::from_bits(v))
                .any(|v| !v.is_finite() || v.abs() > 1e12)
        {
            return Err("Invalid saved audio curve".into());
        }
        gain(f32::from_bits(self.words[5]))
    }
    pub fn cached(&self) -> Result<f32, String> {
        self.validate()?;
        Ok(f32::from_bits(self.words[4]))
    }
    pub fn set(
        &mut self,
        target: f32,
        duration: f32,
        kind: u8,
        math: &mut impl CurveMath,
    ) -> Result<(), String> {
        self.validate()?;
        gain(target)?;
        time(duration)?;
        if kind > 3 {
            return Err("Unsupported audio curve kind".into());
        }
        let mut next = self.clone();
        next.kind = kind;
        next.words[5] = target.to_bits();
        if duration == 0. {
            next.words = [
                (-1f32).to_bits(),
                (-1f32).to_bits(),
                (-1f32).to_bits(),
                0,
                target.to_bits(),
                target.to_bits(),
            ];
        } else {
            let end = if kind == 0 {
                target
            } else {
                math.transform(kind, true, target)?
            };
            let value = self.cached()?;
            let start = if kind == 0 {
                value
            } else {
                math.transform(kind, true, value)?
            };
            next.words[0] = start.to_bits();
            next.words[1] = end.to_bits();
            next.words[2] = (-1f32).to_bits();
            next.words[3] = ((end - start) / duration).to_bits();
        }
        next.validate()?;
        *self = next;
        Ok(())
    }
    pub fn value(&mut self, math: &mut impl CurveMath) -> Result<f32, String> {
        self.validate()?;
        let mut next = self.clone();
        let [position, _, last, slope, _, target] = next.words.map(f32::from_bits);
        if position != last {
            let mut value = if next.kind == 0 {
                position
            } else {
                math.transform(next.kind, false, position)?
            };
            if (slope <= 0. && value < target) || (slope > 0. && value > target) {
                value = target;
            }
            next.words[4] = value.to_bits();
            next.words[2] = position.to_bits();
        }
        next.validate()?;
        let result = next.cached()?;
        *self = next;
        Ok(result)
    }
    /// Same bounded delta domain as the advancing branch of GetFadeMultiplier.
    pub fn step(&mut self, delta: f32, math: &mut impl CurveMath) -> Result<f32, String> {
        self.validate()?;
        time(delta)?;
        if delta >= 1. {
            return Err("Curve advance requires delta below one second".into());
        }
        let mut next = self.clone();
        let [position, end, _, slope, value, target] = next.words.map(f32::from_bits);
        if value != target {
            let mut p = position + slope * delta;
            if (slope <= 0. && p < end) || (slope > 0. && p > end) {
                p = end;
            }
            next.words[0] = p.to_bits();
        }
        let result = next.value(math)?;
        *self = next;
        Ok(result)
    }
}
pub(crate) fn gain(value: f32) -> Result<(), String> {
    if !value.is_finite() || !(0. ..=1.).contains(&value) {
        Err("Invalid audio gain".into())
    } else {
        Ok(())
    }
}
pub(crate) fn time(value: f32) -> Result<(), String> {
    if !value.is_finite() || !(0. ..=60.).contains(&value) {
        Err("Invalid audio controller time".into())
    } else {
        Ok(())
    }
}
