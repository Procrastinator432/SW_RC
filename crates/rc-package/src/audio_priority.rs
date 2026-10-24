//! Running source priority, 103d50e0. Distinct from SoundPriority admission.
use serde::{Deserialize, Serialize};
#[derive(Clone, Debug, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct RunningPriority {
    pub location: [f32; 3],
    pub listener: [f32; 3],
    pub volume: f32,
    pub radius: f32,
    pub radius_multiplier: f32,
    pub flags: u32,
    pub slot: u8,
}
impl RunningPriority {
    pub fn validate(&self) -> Result<(), String> {
        if self
            .location
            .iter()
            .chain(&self.listener)
            .any(|v| !v.is_finite() || v.abs() > 1e6)
            || !self.volume.is_finite()
            || !(-16. ..=16.).contains(&self.volume)
            || !self.radius.is_finite()
            || !(0. ..=1e6).contains(&self.radius)
            || !self.radius_multiplier.is_finite()
            || !(0.0001..=1e6).contains(&self.radius_multiplier)
            || self.slot > 7
        {
            return Err("Invalid running sound priority input".into());
        }
        Ok(())
    }
    pub fn squared_distance(&self) -> Result<f32, String> {
        self.validate()?;
        let [x, y, z] = std::array::from_fn(|i| self.listener[i] - self.location[i]);
        Ok((z * z + y * y) + x * x)
    }
    /// Caller supplies the actual CPU's RSQRTSS seed, not a refined square root.
    /// Zero distance is explicitly masked to zero by the original instructions.
    pub fn update(&self, previous: f32, seed: Option<f32>) -> Result<f32, String> {
        self.validate()?;
        if !previous.is_finite() {
            return Err("Invalid previous sound priority".into());
        }
        if self.slot == 3 {
            return Ok(previous);
        }
        let distance = if self.radius == 0. || self.flags & 0x100 != 0 {
            1.
        } else {
            let squared = self.squared_distance()?;
            let root = if squared == 0. {
                0.
            } else {
                let s = seed.ok_or("Running priority requires an explicit reciprocal-sqrt seed")?;
                if s <= 0. || s.is_nan() || (squared.is_normal() && !s.is_finite()) {
                    return Err("Invalid running priority reciprocal-sqrt seed".into());
                }
                let product = (s * squared) * s;
                let half_seed = s * 0.5;
                ((3. - product) * half_seed) * squared
            };
            let raw = 1. - root / (self.radius * self.radius_multiplier);
            // Native ordered comparisons select 1 for an unordered raw result.
            if raw < 0.01 {
                0.01
            } else if raw < 1. {
                raw
            } else {
                1.
            }
        };
        let v = self.volume.clamp(0., 1.);
        let weight = if v == 0. {
            0.
        } else {
            v * 0.15 + f32::from_bits(0x3f59999a)
        };
        let mut result = ((self.flags >> 8) & 1) as f32 + ((self.flags >> 2) & 1) as f32;
        result += weight * distance;
        result += if self.slot == 7 { 3. } else { 0. };
        result += if self.slot == 5 { 2. } else { 0. };
        result += if self.slot == 0 { 1. } else { 0. };
        Ok(result)
    }
    /// Explicitly portable policy; not a claim of the PC RSQRTSS approximation.
    pub fn update_portable(&self, previous: f32) -> Result<f32, String> {
        let q = self.squared_distance()?;
        self.update(previous, if q == 0. { None } else { Some(1f32 / q.sqrt()) })
    }
}
