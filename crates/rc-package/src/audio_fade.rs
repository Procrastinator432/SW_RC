//! Original general fade controller, 103d2810; selected StopSound dispatch rules.
use crate::{
    audio_curve::{self, Curve, CurveMath},
    world_audio::{Volume, VolumeUpdate},
};
use serde::{Deserialize, Serialize};
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Fade {
    pub curve: Curve,
    pub duration_bits: u32,
    pub elapsed_bits: u32,
    /// Original 0 idle, 1 fade-in, 2 fade-out.
    pub mode: u8,
    pub held_bits: u32,
}
#[derive(Clone, Copy, Debug)]
pub struct VolumeInput {
    pub base: f32,
    pub duck: f32,
    pub flags: u32,
    pub queued: bool,
    pub delta: f32,
}
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Stop {
    Immediate,
    Fading,
}
impl Fade {
    pub fn new(value: f32) -> Result<Self, String> {
        Ok(Self {
            curve: Curve::new(value)?,
            duration_bits: 0,
            elapsed_bits: 0,
            mode: 0,
            held_bits: value.to_bits(),
        })
    }
    pub fn validate(&self) -> Result<(), String> {
        self.curve.validate()?;
        audio_curve::time(f32::from_bits(self.duration_bits))?;
        let elapsed = f32::from_bits(self.elapsed_bits);
        if self.mode > 2 || !elapsed.is_finite() || !(0. ..=61.).contains(&elapsed) {
            return Err("Invalid saved audio fade".into());
        }
        let held = f32::from_bits(self.held_bits);
        if !held.is_finite() || held.abs() > 1e12 {
            return Err("Invalid saved held fade".into());
        }
        Ok(())
    }
    pub fn begin(
        &mut self,
        mode: u8,
        duration: f32,
        kind: u8,
        math: &mut impl CurveMath,
    ) -> Result<(), String> {
        self.validate()?;
        audio_curve::time(duration)?;
        if !(1..=2).contains(&mode) {
            return Err("Invalid audio fade mode".into());
        }
        let mut next = self.clone();
        next.mode = mode;
        next.duration_bits = duration.to_bits();
        next.elapsed_bits = 0;
        next.curve
            .set(if mode == 1 { 1. } else { 0. }, duration, kind, math)?;
        next.validate()?;
        *self = next;
        Ok(())
    }
    /// Sound lookup/default duration are caller-owned. Non-streamed StopSound
    /// uses curve 0; streamed sources use the sound's supplied original curve.
    pub fn stop(
        &mut self,
        duration: f32,
        streamed: bool,
        stream_curve: u8,
        math: &mut impl CurveMath,
    ) -> Result<Stop, String> {
        self.validate()?;
        audio_curve::time(duration)?;
        if duration == 0. {
            return Ok(Stop::Immediate);
        }
        self.begin(2, duration, if streamed { stream_curve } else { 0 }, math)?;
        Ok(Stop::Fading)
    }
    pub fn step(&mut self, delta: f32, math: &mut impl CurveMath) -> Result<f32, String> {
        self.validate()?;
        audio_curve::time(delta)?;
        // Original returns 1 WITHOUT advancing or completing a fade at dt>=1.
        if self.mode == 0 || delta >= 1. {
            return Ok(1.);
        }
        let mut next = self.clone();
        let elapsed = f32::from_bits(next.elapsed_bits) + delta;
        next.elapsed_bits = elapsed.to_bits();
        let mut result = next.curve.step(delta, math)?;
        if f32::from_bits(next.duration_bits) <= elapsed {
            let target = if next.mode == 1 { 1. } else { 0. };
            next.curve.set(target, 0., 0, math)?;
            next.mode = 0;
            if target == 0. {
                result = 0.;
            }
        }
        next.validate()?;
        *self = next;
        Ok(result)
    }
    /// Replace UpdateVolume's former synthetic fade callback with this real
    /// controller. Driver stopping and a resolved duck factor stay caller-owned.
    pub fn update_volume(
        &mut self,
        input: VolumeInput,
        math: &mut impl CurveMath,
    ) -> Result<VolumeUpdate, String> {
        let VolumeInput {
            base,
            duck,
            flags,
            queued,
            delta,
        } = input;
        self.validate()?;
        audio_curve::time(delta)?;
        let mut next = self.clone();
        let before = next.mode;
        let fade = if before != 0 {
            next.step(delta, math)?
        } else {
            1.
        };
        let out = Volume {
            base_volume: base,
            duck,
            fade,
            held_fade: f32::from_bits(next.held_bits),
            mode_before: before,
            mode_after: next.mode,
            flags,
            queued,
        }
        .update_controller_factors()?;
        next.held_bits = out.held_fade.to_bits();
        next.validate()?;
        *self = next;
        Ok(out)
    }
}
