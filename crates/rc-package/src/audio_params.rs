//! Selected PC PlayUSound parameter arithmetic (103d5e56..103d5f2c).
use serde::{Deserialize, Serialize};
#[derive(Clone, Debug, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Params {
    pub source: String,
    pub min_volume: f32,
    pub max_volume: f32,
    pub min_pitch: f32,
    pub max_pitch: f32,
    pub radius: f32,
    pub non_positional: bool,
    pub disable_falloff: bool,
    pub disable_occlusion: bool,
    pub category: u8,
}
#[derive(Clone, Debug, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Playback {
    pub volume: f32,
    pub pitch: f32,
    pub radius: f32,
    pub category: u8,
}
impl Params {
    pub fn validate(&self) -> Result<(), String> {
        if [
            self.min_volume,
            self.max_volume,
            self.min_pitch,
            self.max_pitch,
        ]
        .iter()
        .any(|v| !v.is_finite() || !(0. ..=16.).contains(v))
            || self.min_volume > self.max_volume
            || self.min_pitch > self.max_pitch
            || !self.radius.is_finite()
            || !(0. ..=1e6).contains(&self.radius)
            || self.category > 2
        {
            return Err("Invalid original audio parameters".into());
        }
        Ok(())
    }
    pub fn select(&self, seed: &mut u32) -> Option<Playback> {
        let random = crate::sound_multiple::random(seed);
        let volume = ((self.max_volume - self.min_volume) * random + self.min_volume).min(1.);
        if volume <= 0.0001 {
            return None;
        }
        let random = crate::sound_multiple::random(seed);
        let pitch = ((self.max_pitch - self.min_pitch) * random + self.min_pitch).clamp(0.5, 2.);
        Some(Playback {
            volume,
            pitch,
            radius: self.radius,
            category: self.category,
        })
    }
}
impl Playback {
    pub fn validate(&self) -> Result<(), String> {
        if !self.volume.is_finite()
            || !(0.0001..=1.).contains(&self.volume)
            || self.volume == 0.0001
            || !self.pitch.is_finite()
            || !(0.5..=2.).contains(&self.pitch)
            || !self.radius.is_finite()
            || !(0. ..=1e6).contains(&self.radius)
            || self.category > 2
        {
            return Err("Invalid saved audio parameters".into());
        }
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn matches_800_original_instruction_cases_and_random_consumption() {
        #[derive(Deserialize)]
        struct Expected {
            accepted: bool,
            seed: u32,
            draws: u32,
            volume_bits: Option<u32>,
            pitch_bits: Option<u32>,
        }
        #[derive(Deserialize)]
        struct Case {
            params: Params,
            seed: u32,
            expected: Expected,
        }
        #[derive(Deserialize)]
        struct Fixture {
            cases: Vec<Case>,
        }
        let data: Fixture =
            serde_json::from_str(include_str!("../../../fixtures/audio-params-native.json"))
                .unwrap();
        assert_eq!(data.cases.len(), 800);
        for (i, c) in data.cases.into_iter().enumerate() {
            c.params.validate().unwrap();
            let mut seed = c.seed;
            let result = c.params.select(&mut seed);
            assert_eq!(result.is_some(), c.expected.accepted, "case {i}");
            assert_eq!(seed, c.expected.seed, "case {i}");
            let mut drawn = c.seed;
            for _ in 0..c.expected.draws {
                crate::sound_multiple::random(&mut drawn);
            }
            assert_eq!(seed, drawn, "case {i}");
            if let Some(p) = result {
                p.validate().unwrap();
                assert_eq!(
                    p.volume.to_bits(),
                    c.expected.volume_bits.unwrap(),
                    "case {i}"
                );
                assert_eq!(
                    p.pitch.to_bits(),
                    c.expected.pitch_bits.unwrap(),
                    "case {i}"
                );
            }
        }
    }
}
