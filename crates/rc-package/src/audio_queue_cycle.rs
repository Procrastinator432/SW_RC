//! Bounded main-listener static update slice, not the full engine Update.
use crate::{
    audio_census,
    audio_curve::CurveMath,
    audio_driver, audio_playback_host as playback, audio_queue_order,
    audio_source::{Init, Source},
    audio_source_runtime as runtime, audio_upload,
};
use serde::{Deserialize, Serialize};
#[path = "audio_queue_control.rs"]
mod control;
pub use control::{Command, CommandResult};
#[derive(Clone, Debug, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Queued {
    pub source: Source,
    pub sound: audio_driver::Sound,
}
pub struct Host {
    playback: playback::Host,
    waiting: Vec<Queued>,
    counter: u32,
    reservation: Option<crate::audio_reservation::Config>,
    cue_id: u32,
}
#[derive(Clone, Debug, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct SourceFrame {
    pub base: runtime::Frame,
    pub delta_bits: u32,
}
#[derive(Clone, Debug, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct QueuedFrame {
    pub source: SourceFrame,
    pub environment: audio_upload::Environment,
    pub engine_present: bool,
    pub sound_valid: bool,
}
#[derive(Clone, Debug, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Frame {
    pub global_delta_bits: u32,
    pub cleanup: Vec<audio_driver::Reply>,
    pub active: Vec<SourceFrame>,
    pub queued: Vec<QueuedFrame>,
    pub drivers_by_node: Vec<playback::DriverFrame>,
    pub params: Vec<audio_census::Params>,
    pub context: u32,
}
#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub enum Candidate {
    Queued(usize),
    Active(usize),
}
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub enum Event {
    Duck,
    Playback(playback::Event),
    QueuedBase {
        index: usize,
        event: runtime::Event,
    },
    ActiveBase {
        node: usize,
        event: playback::Event,
    },
    Sorted(Vec<Candidate>),
    Cutoff(i32),
    Attempt {
        queue: usize,
        node: Option<usize>,
    },
    QueuedStop(usize),
    Discard {
        queue: usize,
        unique_id: u32,
    },
    ProcessContext(u32),
    SuspendContext(u32),
    Reservation {
        order: Vec<Candidate>,
        considered: u32,
    },
    Command(crate::audio_commands::Event),
    Resource(crate::audio_resource_host::Event),
}
#[derive(Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
struct Saved {
    schema: u32,
    playback: playback::Saved,
    waiting: Vec<Queued>,
    counter: u32,
    #[serde(default)]
    reservation: Option<crate::audio_reservation::Config>,
    #[serde(default)]
    cue_id: u32,
}
impl Host {
    pub fn new(playback: playback::Host, counter: u32) -> Result<Self, String> {
        let h = Self {
            playback,
            waiting: vec![],
            counter,
            reservation: None,
            cue_id: 0,
        };
        h.validate()?;
        Ok(h)
    }
    pub fn playback(&self) -> &playback::Host {
        &self.playback
    }
    pub fn waiting(&self) -> &[Queued] {
        &self.waiting
    }
    pub fn counter(&self) -> u32 {
        self.counter
    }
    pub fn validate(&self) -> Result<(), String> {
        self.playback.validate()?;
        if self.waiting.len() > 4096 {
            return Err("Audio queue too large".into());
        }
        for q in &self.waiting {
            q.source.validate()?;
            if q.sound.flags & 2 != 0
                || q.source.flags & 4 != 0
                || (q.source.allocated && q.source.sound != q.sound.handle)
            {
                return Err("Queue cycle supports matching static sources".into());
            }
            audio_driver::Session {
                node: crate::audio_activation::Node {
                    source: q.source.clone(),
                    hardware: 0,
                    buffer: 0,
                },
                buffers: vec![],
                sounds: vec![q.sound.clone()],
            }
            .validate()?;
        }
        if let Some(config) = &self.reservation {
            config.validate()?;
        }
        for s in self
            .waiting
            .iter()
            .map(|q| &q.source)
            .chain(self.playback.pool().nodes.iter().map(|n| &n.source))
        {
            if s.flags & 0x140 == 0x40 {
                self.reservation
                    .as_ref()
                    .ok_or("Spatial soft-reservation branch needs original sound names")?
                    .name(s.sound)?;
            }
        }
        Ok(())
    }
    pub fn enqueue(
        &mut self,
        input: &Init,
        sound: audio_driver::Sound,
        math: &mut impl CurveMath,
    ) -> Result<usize, String> {
        self.validate()?;
        if self.waiting.len() == 4096
            || input.flags & 4 != 0
            || sound.flags & 2 != 0
            || sound.handle != input.sound
        {
            return Err("Invalid static queue request".into());
        }
        let mut counter = self.counter;
        let mut source = Source::new(&mut counter);
        if !source.initialize(input, math)? {
            return Err("No engine for queued source".into());
        }
        self.waiting.push(Queued { source, sound });
        if let Err(e) = self.validate() {
            self.waiting.pop();
            return Err(e);
        }
        self.counter = counter;
        Ok(self.waiting.len() - 1)
    }
    pub fn pause(&mut self, index: usize, request: i32) -> Result<(), String> {
        let q = self
            .waiting
            .get_mut(index)
            .ok_or("Unknown pending source")?;
        q.source.pause(request);
        Ok(())
    }
    pub fn cancel(&mut self, index: usize) -> Result<(), String> {
        let q = self
            .waiting
            .get_mut(index)
            .ok_or("Unknown pending source")?;
        q.source.clear();
        Ok(())
    }
    fn saved(&self) -> Result<Saved, String> {
        self.validate()?;
        Ok(Saved {
            schema: 1,
            playback: self.playback.saved()?,
            waiting: self.waiting.clone(),
            counter: self.counter,
            reservation: self.reservation.clone(),
            cue_id: self.cue_id,
        })
    }
    fn from_saved(s: Saved) -> Result<Self, String> {
        if s.schema != 1 {
            return Err("Unsupported queue cycle schema".into());
        }
        let h = Self {
            playback: playback::Host::from_saved(s.playback)?,
            waiting: s.waiting,
            counter: s.counter,
            reservation: s.reservation,
            cue_id: s.cue_id,
        };
        h.validate()?;
        Ok(h)
    }
    fn source(&self, c: Candidate) -> &Source {
        match c {
            Candidate::Queued(i) => &self.waiting[i].source,
            Candidate::Active(n) => &self.playback.pool().nodes[n].source,
        }
    }
    pub fn cycle(
        &mut self,
        frame: &Frame,
        math: &mut impl CurveMath,
    ) -> Result<Vec<Event>, String> {
        let before = self.saved()?;
        match self.run(frame, math).and_then(|out| {
            self.validate()?;
            Ok(out)
        }) {
            Ok(out) => Ok(out),
            Err(e) => {
                *self = Self::from_saved(before)?;
                Err(e)
            }
        }
    }
    fn run(&mut self, f: &Frame, math: &mut impl CurveMath) -> Result<Vec<Event>, String> {
        let slots = self
            .playback
            .pool()
            .slots
            .iter()
            .map(|n| n.unwrap())
            .collect::<Vec<_>>();
        let count = slots.len();
        let global = f32::from_bits(f.global_delta_bits);
        if !global.is_finite()
            || f.context == 0
            || f.cleanup.len() != count
            || f.active.len() != count
            || f.queued.len() != self.waiting.len()
            || f.drivers_by_node.len() != self.playback.pool().nodes.len()
        {
            return Err("Queue cycle frame shape mismatch".into());
        }
        self.playback.advance_duck(global.clamp(0., 1.))?;
        let mut events = vec![Event::Duck];
        events.extend(
            self.playback
                .cleanup_finished(&f.cleanup)?
                .into_iter()
                .map(Event::Playback),
        );
        let mut candidates = (0..self.waiting.len())
            .map(Candidate::Queued)
            .collect::<Vec<_>>();
        candidates.extend(
            slots
                .iter()
                .filter(|&&n| {
                    let s = &self.playback.pool().nodes[n].source;
                    s.sound != 0 && s.cursor != 0
                })
                .copied()
                .map(Candidate::Active),
        );
        if candidates.len() > 4096 {
            return Err("Combined audio list exceeds 4096".into());
        }
        for i in 0..self.waiting.len() {
            let input = &f.queued[i].source;
            let mut base = input.base.clone();
            base.base.paused = [false; 2];
            base.queued = true;
            let mut r = runtime::Runtime {
                ducking: self.playback.ducking().clone(),
                sources: vec![self.waiting[i].source.clone()],
            };
            let out = r.step_controlled(
                0,
                f32::from_bits(input.delta_bits),
                &[base],
                math,
                &mut |_, s: &mut Source| {
                    s.clear();
                    Ok(())
                },
                false,
            )?;
            self.waiting[i].source = r.sources.remove(0);
            events.extend(
                out.events
                    .into_iter()
                    .map(|event| Event::QueuedBase { index: i, event }),
            );
        }
        for (i, &node) in slots.iter().enumerate() {
            if candidates.contains(&Candidate::Active(node)) {
                let input = &f.active[i];
                events.extend(
                    self.playback
                        .base_nodes(
                            f32::from_bits(input.delta_bits),
                            &[node],
                            std::slice::from_ref(&input.base),
                            math,
                            false,
                        )?
                        .into_iter()
                        .map(|event| Event::ActiveBase { node, event }),
                );
            }
        }
        let priorities = candidates
            .iter()
            .map(|&c| self.source(c).priority_bits)
            .collect::<Vec<_>>();
        let order = audio_queue_order::order(&priorities)?;
        let mut ordered = order.iter().map(|&i| candidates[i]).collect::<Vec<_>>();
        let words = ordered
            .iter()
            .map(|&c| self.source(c).priority_bits)
            .collect::<Vec<_>>();
        let cutoff = audio_queue_order::cutoff(&words, count)?;
        events.push(Event::Sorted(ordered.clone()));
        events.push(Event::Cutoff(cutoff));
        let mut remaining = count as i32;
        for &node in &slots {
            let s = &self.playback.pool().nodes[node].source;
            if s.cursor != 0
                && f32::from_bits(s.priority_bits) <= cutoff as f32
                && s.identity & 1 == 0
            {
                events.extend(
                    self.playback
                        .stop_node(node)?
                        .into_iter()
                        .map(Event::Playback),
                );
            } else if s.cursor != 0 {
                remaining -= 1;
            }
        }
        if let Some(config) = &self.reservation {
            let entries = ordered
                .iter()
                .map(|&c| {
                    let s = self.source(c);
                    Ok(crate::audio_reservation::Entry {
                        queued: matches!(c, Candidate::Queued(_)),
                        flags: s.flags,
                        priority_bits: s.priority_bits,
                        name: if s.flags & 0x140 == 0x40 {
                            config.name(s.sound)?.to_owned()
                        } else {
                            "unused".into()
                        },
                    })
                })
                .collect::<Result<Vec<_>, String>>()?;
            let plan = crate::audio_reservation::plan(&entries, count, remaining, config.limit)?;
            for &index in &plan.stopped {
                if let Candidate::Queued(i) = ordered[index] {
                    self.waiting[i].source.stop();
                    events.push(Event::QueuedStop(i));
                }
            }
            ordered = plan.order.iter().map(|&i| ordered[i]).collect();
            events.push(Event::Reservation {
                order: ordered.clone(),
                considered: plan.considered,
            });
        }
        for &candidate in ordered.iter().take(count) {
            let source = self.source(candidate);
            if !source.allocated || f32::from_bits(source.priority_bits) <= cutoff as f32 {
                continue;
            }
            let Candidate::Queued(i) = candidate else {
                continue;
            };
            let q = &mut self.waiting[i];
            if q.source.flags & 0xb3e == 0
                && f32::from_bits(q.sound.duration_bits) > 2.
                && f32::from_bits(q.source.radii.radius_bits[1]) < f32::from_bits(0x3a83126f)
            {
                q.source.stop();
                events.push(Event::QueuedStop(i));
                continue;
            }
            let input = &f.queued[i];
            let out = self.playback.play(&playback::Request {
                incoming: q.source.clone(),
                sound: q.sound.clone(),
                engine_present: input.engine_present,
                sound_valid: input.sound_valid,
                queued: 1,
                environment: input.environment.clone(),
            })?;
            events.push(Event::Attempt {
                queue: i,
                node: out.node,
            });
            events.extend(out.events.into_iter().map(Event::Playback));
        }
        let late = self
            .playback
            .pool()
            .slots
            .iter()
            .map(|n| f.drivers_by_node[n.unwrap()].clone())
            .collect::<Vec<_>>();
        events.extend(
            self.playback
                .late_tick(&late, &f.params)?
                .into_iter()
                .map(Event::Playback),
        );
        for (i, q) in self.waiting.iter().enumerate() {
            events.push(Event::Discard {
                queue: i,
                unique_id: q.source.unique_id,
            });
        }
        self.waiting.clear();
        events.push(Event::ProcessContext(f.context));
        events.push(Event::SuspendContext(f.context));
        Ok(events)
    }
    pub fn checkpoint(&self) -> Result<Vec<u8>, String> {
        let bytes = serde_json::to_vec(&self.saved()?).map_err(|e| e.to_string())?;
        if bytes.len() > crate::audio_stream_data::MAX_BYTES {
            return Err("Queue cycle snapshot too large".into());
        }
        Ok(bytes)
    }
    pub fn restore(bytes: &[u8]) -> Result<Self, String> {
        if bytes.len() > crate::audio_stream_data::MAX_BYTES {
            return Err("Queue cycle snapshot too large".into());
        }
        Self::from_saved(serde_json::from_slice(bytes).map_err(|e| e.to_string())?)
    }
    pub fn load(&mut self, bytes: &[u8]) -> Result<(), String> {
        let h = Self::restore(bytes)?;
        *self = h;
        Ok(())
    }
}
