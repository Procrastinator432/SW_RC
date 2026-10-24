//! Original UpdateDuckVolume (103d1210) / GetDuckMultiplier (103d2920).
use crate::audio_curve::{gain, time};
use serde::{Deserialize, Serialize};
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Settings {
    pub attack: f32,
    pub release: f32,
    pub levels: [f32; 3],
}
impl Settings {
    pub fn validate(&self) -> Result<(), String> {
        time(self.attack)?;
        time(self.release)?;
        if self.attack == 0. || self.release == 0. {
            return Err("Ducking times must be positive".into());
        }
        for level in self.levels {
            gain(level)?;
        }
        Ok(())
    }
}
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Ducking {
    pub settings: Settings,
    /// Per category: current, start, target. Native +9c..+bc.
    pub words: [u32; 9],
    pub active: bool,
    pub direction_bits: u32,
    pub remaining_bits: u32,
}
#[derive(Clone, Copy, Debug)]
pub struct Source {
    pub category: u8,
    pub slot: u8,
    /// Native source+3c, copied from Params+2c bit 0. No streamed-class inference.
    pub voice_exempt: bool,
    pub special_level: bool,
    pub ducker: bool,
}
impl Ducking {
    pub fn new(settings: Settings) -> Result<Self, String> {
        settings.validate()?;
        Ok(Self {
            settings,
            words: [1f32.to_bits(); 9],
            active: false,
            direction_bits: 0,
            remaining_bits: 0,
        })
    }
    pub fn validate(&self) -> Result<(), String> {
        self.settings.validate()?;
        if self
            .words
            .iter()
            .map(|&v| f32::from_bits(v))
            .any(|v| !v.is_finite() || v.abs() > 1e6)
            || ![-1., 0., 1., 2.].contains(&f32::from_bits(self.direction_bits))
        {
            return Err("Invalid saved ducking controller".into());
        }
        time(f32::from_bits(self.remaining_bits))
    }
    pub fn step(&mut self, voices: i32, delta: f32) -> Result<(), String> {
        self.validate()?;
        time(delta)?;
        let mut next = self.clone();
        let active = voices > 0;
        if next.active != active {
            next.active = active;
            next.direction_bits = if active {
                1f32.to_bits()
            } else {
                (-1f32).to_bits()
            };
            next.remaining_bits = if active {
                next.settings.attack
            } else {
                next.settings.release
            }
            .to_bits();
            for i in 0..3 {
                next.words[i * 3 + 1] = next.words[i * 3];
                next.words[i * 3 + 2] = if active { next.settings.levels[i] } else { 1. }.to_bits();
            }
        }
        let remaining = (f32::from_bits(next.remaining_bits) - delta).max(0.);
        next.remaining_bits = remaining.to_bits();
        let direction = f32::from_bits(next.direction_bits);
        if direction != 0. {
            // Preserve the native comparison against ONE, not against zero.
            let factor = if direction > 1. {
                remaining / next.settings.attack
            } else {
                1. - remaining / next.settings.release
            };
            for i in 0..3 {
                let from = f32::from_bits(next.words[i * 3 + 1]);
                let target = f32::from_bits(next.words[i * 3 + 2]);
                next.words[i * 3] = ((target - from) * factor + from).to_bits();
            }
        }
        if remaining < f32::from_bits(0x38d1b717) && direction != 0. {
            next.direction_bits = 0;
        }
        next.validate()?;
        *self = next;
        Ok(())
    }
    pub fn factor(&self, source: Source) -> Result<f32, String> {
        self.validate()?;
        if source.slot > 7 {
            return Err("Invalid ducking source slot".into());
        }
        if source.special_level
            || source.slot == 5
            || source.ducker
            || (source.category == 1 && source.voice_exempt)
        {
            return Ok(1.);
        }
        let category = match source.category {
            0 => 0,
            1 => 1,
            _ => 2,
        };
        Ok(f32::from_bits(self.words[category * 3]))
    }
}
