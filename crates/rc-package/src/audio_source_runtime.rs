//! Staged, serializable composition of proven PC audio kernels. Not an Android
//! driver or the engine's global voice counter; frames supply those boundaries.
use crate::{
    audio_curve::CurveMath,
    audio_duck::{Ducking, Source as DuckSource},
    audio_fade::VolumeInput,
    audio_geometry::SavedRadii,
    audio_pool::{self, Incoming, Slot},
    audio_priority::RunningPriority,
    audio_source::Source,
    audio_source_update::{self, BaseInput, BaseOutcome, UpdateHost},
};
use serde::{Deserialize, Serialize};
#[derive(Clone, Debug, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Runtime {
    pub ducking: Ducking,
    pub sources: Vec<Source>,
}
#[derive(Clone, Debug, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Frame {
    pub base: BaseInput,
    pub now: f32,
    pub occlusion: Option<f32>,
    pub listener_location: [f32; 3],
    pub radius_multiplier: f32,
    pub rsqrt_seed_bits: Option<u32>,
    pub category: u8,
    pub special_level: bool,
    pub ducker: bool,
    pub queued: bool,
}
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum Event {
    Volume(usize),
    Stop(usize),
    Radii(usize),
    Priority(usize),
}
#[derive(Debug)]
pub struct Tick {
    pub outcomes: Vec<BaseOutcome>,
    pub events: Vec<Event>,
}
#[derive(Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
struct Snapshot<T> {
    schema: u32,
    state: T,
}
impl Runtime {
    /// Independent core snapshot, not the application's checkpoint schema.
    pub fn checkpoint(&self) -> Result<Vec<u8>, String> {
        self.validate()?;
        serde_json::to_vec(&Snapshot {
            schema: 1,
            state: self,
        })
        .map_err(|e| e.to_string())
    }
    pub fn restore(bytes: &[u8]) -> Result<Self, String> {
        if bytes.len() > 16 * 1024 * 1024 {
            return Err("Audio core snapshot too large".into());
        }
        let restored: Snapshot<Self> = serde_json::from_slice(bytes).map_err(|e| e.to_string())?;
        if restored.schema != 1 {
            return Err("Unsupported audio core snapshot schema".into());
        }
        restored.state.validate()?;
        Ok(restored.state)
    }
    pub fn load(&mut self, bytes: &[u8]) -> Result<(), String> {
        let restored = Self::restore(bytes)?;
        *self = restored;
        Ok(())
    }
    pub fn validate(&self) -> Result<(), String> {
        self.ducking.validate()?;
        if self.sources.len() > 4096 {
            return Err("Too many runtime audio sources".into());
        }
        for s in &self.sources {
            s.validate()?;
        }
        Ok(())
    }
    /// Pool policy chooses a destination; installing an owned portable snapshot
    /// has no external Stop/Attach side effects and is not a native allocator.
    pub fn admit(&mut self, incoming: Source) -> Result<Option<usize>, String> {
        self.validate()?;
        incoming.validate()?;
        if !incoming.allocated {
            return Err("Cannot admit an uninitialized source".into());
        }
        let pool: Vec<_> = self
            .sources
            .iter()
            .map(|s| Slot {
                occupied: s.allocated,
                identity: s.identity,
                priority: f32::from_bits(s.priority_bits),
            })
            .collect();
        let selected = audio_pool::select(
            Incoming {
                identity: incoming.identity,
                priority: f32::from_bits(incoming.priority_bits),
            },
            &pool,
        )?;
        if let Some(i) = selected {
            self.sources[i] = incoming;
        }
        Ok(selected)
    }
    /// All computations commit together. Math-policy side effects are outside
    /// this state transaction; no external driver callbacks occur in this API.
    pub fn step(
        &mut self,
        voices: i32,
        delta: f32,
        frames: &[Frame],
        math: &mut impl CurveMath,
    ) -> Result<Tick, String> {
        self.validate()?;
        if frames.len() != self.sources.len() {
            return Err("Audio frame/source count mismatch".into());
        }
        for f in frames {
            f.validate()?;
        }
        let mut next = self.clone();
        next.ducking.step(voices, delta)?;
        let mut events = vec![];
        let mut outcomes = vec![];
        for (i, (source, frame)) in next.sources.iter_mut().zip(frames).enumerate() {
            let mut host = Host {
                frame,
                ducking: &next.ducking,
                math,
                index: i,
                events: &mut events,
            };
            outcomes.push(audio_source_update::update(
                source,
                &frame.base,
                delta,
                &mut host,
            )?);
        }
        next.validate()?;
        *self = next;
        Ok(Tick { outcomes, events })
    }
}
impl Frame {
    pub fn validate(&self) -> Result<(), String> {
        self.base.validate()?;
        crate::audio_source::vector(self.listener_location)?;
        if !self.now.is_finite()
            || !(0. ..=1e12).contains(&self.now)
            || self
                .occlusion
                .is_some_and(|v| !v.is_finite() || !(0. ..=1.).contains(&v))
            || !self.radius_multiplier.is_finite()
            || !(0.0001..=1e6).contains(&self.radius_multiplier)
            || self.rsqrt_seed_bits.is_some_and(|w| {
                let v = f32::from_bits(w);
                v.is_nan() || v <= 0.
            })
        {
            return Err("Invalid audio runtime frame".into());
        }
        Ok(())
    }
}
struct Host<'a, M> {
    frame: &'a Frame,
    ducking: &'a Ducking,
    math: &'a mut M,
    index: usize,
    events: &'a mut Vec<Event>,
}
impl<M: CurveMath> UpdateHost for Host<'_, M> {
    fn volume(&mut self, s: &mut Source, delta: f32) -> Result<(), String> {
        self.events.push(Event::Volume(self.index));
        let duck = self.ducking.factor(DuckSource {
            category: self.frame.category,
            slot: s.slot(),
            voice_exempt: s.voice_exempt,
            special_level: self.frame.special_level,
            ducker: self.frame.ducker,
        })?;
        let out = s.fade.update_volume(
            VolumeInput {
                base: f32::from_bits(s.volume_bits),
                duck,
                flags: s.flags,
                queued: self.frame.queued,
                delta,
            },
            self.math,
        )?;
        // Commit the native +94 output; the default Clear preserves this cell.
        if out.stop {
            self.events.push(Event::Stop(self.index));
            s.clear();
        }
        s.output_volume_bits = out.volume.to_bits();
        Ok(())
    }
    fn radii(&mut self, s: &mut Source, _: f32) -> Result<(), String> {
        self.events.push(Event::Radii(self.index));
        // Explicit geometry callback result; actor tracing itself stays external.
        let radii = s.radii.restore()?.update(
            f32::from_bits(s.base_radius_bits),
            if s.flags & 0x100 != 0 {
                None
            } else {
                self.frame.occlusion
            },
            self.frame.now,
        )?;
        s.radii = SavedRadii::capture(&radii)?;
        Ok(())
    }
    fn priority(&mut self, s: &mut Source, _: f32) -> Result<(), String> {
        self.events.push(Event::Priority(self.index));
        s.priority_bits = RunningPriority {
            location: s.location_bits.map(f32::from_bits),
            listener: self.frame.listener_location,
            volume: f32::from_bits(s.volume_bits),
            radius: s.radii.restore()?.priority_radius,
            radius_multiplier: self.frame.radius_multiplier,
            flags: s.flags,
            slot: s.slot(),
        }
        .update(
            f32::from_bits(s.priority_bits),
            self.frame.rsqrt_seed_bits.map(f32::from_bits),
        )?
        .to_bits();
        Ok(())
    }
}
