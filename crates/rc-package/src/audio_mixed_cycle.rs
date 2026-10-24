//! Owned mixed queue over the prepared pool; no device/global scene executor.
use crate::{
    audio_census, audio_commands,
    audio_curve::CurveMath,
    audio_driver as driver, audio_mixed_host as mixed, audio_queue_order,
    audio_reservation as reservation, audio_resource_host as resources,
    audio_source::{Init, Source},
    audio_source_runtime as runtime, audio_source_tick as tick, audio_upload,
};
use serde::{Deserialize, Serialize};
#[path = "audio_mixed_control.rs"]
mod control;
pub use crate::audio_queue_cycle::Command;
pub use control::CommandResult;
#[derive(Clone, Debug, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub enum Content {
    Static(driver::Sound),
    Stream {
        bytes: Vec<u8>,
        template: Box<tick::Session>,
        registration: resources::StreamRegistration,
    },
}
impl Content {
    fn sound(&self) -> &driver::Sound {
        match self {
            Self::Static(s) => s,
            Self::Stream { template, .. } => &template.driver.sounds[0],
        }
    }
    fn bytes(&self) -> usize {
        match self {
            Self::Static(_) => 0,
            Self::Stream { bytes, .. } => bytes.len(),
        }
    }
}
#[derive(Clone, Debug, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Pending {
    pub source: Source,
    pub content: Content,
}
pub struct Host {
    mixed: mixed::Host,
    waiting: Vec<Pending>,
    counter: u32,
    cue_id: u32,
    reservation: Option<reservation::Config>,
}
#[derive(Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
struct Saved {
    schema: u32,
    mixed: mixed::Saved,
    waiting: Vec<Pending>,
    counter: u32,
    cue_id: u32,
    reservation: Option<reservation::Config>,
}
#[derive(Clone, Debug, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct QueuedFrame {
    pub base: runtime::Frame,
    pub delta_bits: u32,
    pub environment: audio_upload::Environment,
}
#[derive(Clone, Debug, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Frame {
    pub global_delta_bits: u32,
    pub nodes: Vec<mixed::Frame>,
    pub queued: Vec<QueuedFrame>,
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
    Mixed(mixed::Event),
    QueuedBase {
        index: usize,
        event: runtime::Event,
    },
    Sorted(Vec<Candidate>),
    Cutoff(i32),
    Reservation {
        order: Vec<Candidate>,
        considered: u32,
    },
    QueuedStop(usize),
    Attempt {
        index: usize,
        node: Option<usize>,
    },
    Upload(crate::audio_stream_bridge::Upload),
    Delete {
        index: usize,
        unique_id: u32,
    },
    ProcessContext(u32),
    SuspendContext(u32),
    Command(audio_commands::Event),
}
impl Host {
    pub fn new(mixed: mixed::Host, counter: u32) -> Result<Self, String> {
        let h = Self {
            mixed,
            waiting: vec![],
            counter,
            cue_id: 0,
            reservation: None,
        };
        h.validate()?;
        Ok(h)
    }
    pub fn mixed(&self) -> &mixed::Host {
        &self.mixed
    }
    pub fn waiting(&self) -> &[Pending] {
        &self.waiting
    }
    pub fn counter(&self) -> u32 {
        self.counter
    }
    pub fn pause(&mut self, index: usize, request: i32) -> Result<(), String> {
        self.waiting
            .get_mut(index)
            .ok_or("Unknown mixed pending source")?
            .source
            .pause(request);
        Ok(())
    }
    pub fn cancel(&mut self, index: usize) -> Result<(), String> {
        self.waiting
            .get_mut(index)
            .ok_or("Unknown mixed pending source")?
            .source
            .clear();
        Ok(())
    }
    pub fn service_round(&mut self, budget: usize) -> Result<Vec<resources::Served>, String> {
        self.transaction(|h| h.mixed.service_round(budget))
    }
    pub fn validate(&self) -> Result<(), String> {
        self.mixed.validate()?;
        if self.waiting.len() > 4096 {
            return Err("Mixed queue full".into());
        }
        if let Some(config) = &self.reservation {
            config.validate()?;
        }
        let mut stream_sounds = std::collections::BTreeSet::new();
        for n in &self.mixed.pool().nodes {
            if n.source.flags & 4 != 0 {
                stream_sounds.insert(n.source.sound);
            }
        }
        let mut bytes = self.mixed.resources().owned_bytes();
        for q in &self.waiting {
            q.source.validate()?;
            match &q.content {
                Content::Static(sound) => {
                    if sound.flags & 2 != 0
                        || q.source.flags & 4 != 0
                        || (q.source.allocated && q.source.sound != sound.handle)
                    {
                        return Err("Invalid queued static metadata".into());
                    }
                    driver::Session {
                        node: crate::audio_activation::Node {
                            source: q.source.clone(),
                            hardware: 0,
                            buffer: 0,
                        },
                        buffers: vec![],
                        sounds: vec![sound.clone()],
                    }
                    .validate()?;
                }
                Content::Stream {
                    bytes: encoded,
                    template,
                    registration: r,
                } => {
                    template.validate()?;
                    if template.driver.sounds.len() != 1 || template.driver.buffers.len() != 1 {
                        return Err("Queue stream needs one sound/buffer".into());
                    }
                    let sound = &template.driver.sounds[0];
                    if !encoded.starts_with(b"OggS")
                        || encoded.len() > crate::audio_stream_data::MAX_BYTES
                        || sound.flags & 2 == 0
                        || (sound.flags & 1 != 0) != r.looping
                        || !stream_sounds.insert(sound.handle)
                        || (q.source.allocated
                            && (q.source.sound != sound.handle || q.source.flags & 4 == 0))
                        || r.allocation == 0
                        || r.tokens.stream == 0
                        || r.tokens.decoder == 0
                        || r.tokens.staging == 0
                        || r.tokens.buffers.contains(&0)
                        || r.tokens
                            .buffers
                            .iter()
                            .collect::<std::collections::BTreeSet<_>>()
                            .len()
                            != 4
                    {
                        return Err("Invalid or duplicate queued stream owner".into());
                    }
                }
            }
            bytes = bytes.saturating_add(q.content.bytes());
        }
        if bytes > crate::audio_stream_data::MAX_BYTES {
            return Err("Combined pending/live audio bytes exceed budget".into());
        }
        for s in self
            .waiting
            .iter()
            .map(|q| &q.source)
            .chain(self.mixed.pool().nodes.iter().map(|n| &n.source))
        {
            if s.flags & 0x140 == 0x40 {
                self.reservation
                    .as_ref()
                    .ok_or("Missing spatial reservation names")?
                    .name(s.sound)?;
            }
        }
        Ok(())
    }
    fn enqueue(
        &mut self,
        input: &Init,
        content: Content,
        math: &mut impl CurveMath,
    ) -> Result<usize, String> {
        self.validate()?;
        let mut counter = self.counter;
        let mut source = Source::new(&mut counter);
        if !source.initialize(input, math)? {
            return Err("Queued source has no engine".into());
        }
        self.waiting.push(Pending { source, content });
        if let Err(e) = self.validate() {
            self.waiting.pop();
            return Err(e);
        }
        self.counter = counter;
        Ok(self.waiting.len() - 1)
    }
    pub fn enqueue_static(
        &mut self,
        input: &Init,
        sound: driver::Sound,
        math: &mut impl CurveMath,
    ) -> Result<usize, String> {
        self.enqueue(input, Content::Static(sound), math)
    }
    pub fn enqueue_stream(
        &mut self,
        input: &Init,
        bytes: Vec<u8>,
        template: tick::Session,
        registration: resources::StreamRegistration,
        math: &mut impl CurveMath,
    ) -> Result<usize, String> {
        self.enqueue(
            input,
            Content::Stream {
                bytes,
                template: Box::new(template),
                registration,
            },
            math,
        )
    }
    fn saved(&self) -> Result<Saved, String> {
        self.validate()?;
        Ok(Saved {
            schema: 1,
            mixed: self.mixed.saved()?,
            waiting: self.waiting.clone(),
            counter: self.counter,
            cue_id: self.cue_id,
            reservation: self.reservation.clone(),
        })
    }
    fn from_saved(s: Saved) -> Result<Self, String> {
        if s.schema != 1 {
            return Err("Unsupported mixed cycle schema".into());
        }
        let h = Self {
            mixed: mixed::Host::from_saved(s.mixed)?,
            waiting: s.waiting,
            counter: s.counter,
            cue_id: s.cue_id,
            reservation: s.reservation,
        };
        h.validate()?;
        Ok(h)
    }
    fn transaction<T>(
        &mut self,
        f: impl FnOnce(&mut Self) -> Result<T, String>,
    ) -> Result<T, String> {
        let before = self.saved()?;
        let out = f(self).and_then(|v| {
            self.validate()?;
            Ok(v)
        });
        if out.is_err() {
            *self = Self::from_saved(before)?;
        }
        out
    }
    fn source(&self, c: Candidate) -> &Source {
        match c {
            Candidate::Queued(i) => &self.waiting[i].source,
            Candidate::Active(n) => &self.mixed.pool().nodes[n].source,
        }
    }
    pub fn cycle(&mut self, f: &Frame, math: &mut impl CurveMath) -> Result<Vec<Event>, String> {
        self.transaction(|h| h.run(f, math))
    }
    fn run(&mut self, f: &Frame, math: &mut impl CurveMath) -> Result<Vec<Event>, String> {
        if f.context == 0 || f.queued.len() != self.waiting.len() {
            return Err("Mixed queue frame mismatch".into());
        }
        let mut events = self
            .mixed
            .begin_phase(f32::from_bits(f.global_delta_bits), &f.nodes)?
            .into_iter()
            .map(Event::Mixed)
            .collect::<Vec<_>>();
        let slots = self
            .mixed
            .pool()
            .slots
            .iter()
            .map(|n| n.unwrap())
            .collect::<Vec<_>>();
        let count = slots.len();
        let mut candidates = (0..self.waiting.len())
            .map(Candidate::Queued)
            .collect::<Vec<_>>();
        candidates.extend(
            slots
                .iter()
                .copied()
                .filter(|&n| {
                    let s = &self.mixed.pool().nodes[n].source;
                    s.sound != 0 && s.cursor != 0
                })
                .map(Candidate::Active),
        );
        if candidates.len() > 4096 {
            return Err("Combined mixed source list too large".into());
        }
        for (i, q) in self.waiting.iter_mut().enumerate() {
            let mut frame = f.queued[i].base.clone();
            frame.base.paused = [false; 2];
            frame.queued = true;
            let mut runtime = runtime::Runtime {
                ducking: self.mixed.ducking().clone(),
                sources: vec![q.source.clone()],
            };
            let out = runtime.step_controlled(
                0,
                f32::from_bits(f.queued[i].delta_bits),
                &[frame],
                math,
                &mut |_, s: &mut Source| {
                    s.clear();
                    Ok(())
                },
                false,
            )?;
            q.source = runtime.sources.remove(0);
            events.extend(
                out.events
                    .into_iter()
                    .map(|event| Event::QueuedBase { index: i, event }),
            );
        }
        events.extend(
            self.mixed
                .base_phase(&f.nodes, math)?
                .into_iter()
                .map(Event::Mixed),
        );
        let priorities = candidates
            .iter()
            .map(|&c| self.source(c).priority_bits)
            .collect::<Vec<_>>();
        let mut ordered = audio_queue_order::order(&priorities)?
            .into_iter()
            .map(|i| candidates[i])
            .collect::<Vec<_>>();
        let cutoff = audio_queue_order::cutoff(
            &ordered
                .iter()
                .map(|&c| self.source(c).priority_bits)
                .collect::<Vec<_>>(),
            count,
        )?;
        events.push(Event::Sorted(ordered.clone()));
        events.push(Event::Cutoff(cutoff));
        let mut remaining = count as i32;
        for &n in &slots {
            let s = &self.mixed.pool().nodes[n].source;
            if s.cursor != 0 {
                if f32::from_bits(s.priority_bits) <= cutoff as f32 && s.identity & 1 == 0 {
                    events.extend(
                        self.mixed
                            .control(n, &driver::Command::Stop, &f.nodes[n].cleanup)?
                            .into_iter()
                            .map(Event::Mixed),
                    );
                } else {
                    remaining -= 1;
                }
            }
        }
        if let Some(config) = &self.reservation {
            let entries = ordered
                .iter()
                .map(|&c| {
                    let s = self.source(c);
                    Ok(reservation::Entry {
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
            let plan = reservation::plan(&entries, count, remaining, config.limit)?;
            for &j in &plan.stopped {
                if let Candidate::Queued(i) = ordered[j] {
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
        let replies = f
            .nodes
            .iter()
            .map(|n| n.cleanup.clone())
            .collect::<Vec<_>>();
        for &c in ordered.iter().take(count) {
            let s = self.source(c);
            if !s.allocated || f32::from_bits(s.priority_bits) <= cutoff as f32 {
                continue;
            }
            let Candidate::Queued(i) = c else {
                continue;
            };
            let q = &mut self.waiting[i];
            if q.source.flags & 0xb3e == 0
                && f32::from_bits(q.content.sound().duration_bits) > 2.
                && f32::from_bits(q.source.radii.radius_bits[1]) < f32::from_bits(0x3a83126f)
            {
                q.source.stop();
                events.push(Event::QueuedStop(i));
                continue;
            }
            let out = match &q.content {
                Content::Static(sound) => {
                    self.mixed
                        .play_static(&q.source, sound, &replies, &f.queued[i].environment)?
                }
                Content::Stream {
                    bytes,
                    template,
                    registration,
                } => {
                    let mut t = (**template).clone();
                    t.driver.node.source = q.source.clone();
                    let mut r = registration.clone();
                    r.key = self.mixed.key(self.mixed.pool().transient.unwrap())?;
                    self.mixed
                        .play_stream(bytes, &t, &r, &replies, &f.queued[i].environment)?
                }
            };
            events.push(Event::Attempt {
                index: i,
                node: out.node,
            });
            let mut uploads = Some(out.uploads);
            for event in out.events {
                let precache = matches!(
                    event,
                    mixed::Event::Activation(crate::audio_activation::Event::Precache)
                );
                events.push(Event::Mixed(event));
                if precache {
                    events.extend(
                        uploads
                            .take()
                            .unwrap_or_default()
                            .into_iter()
                            .map(Event::Upload),
                    );
                }
            }
        }
        events.extend(
            self.mixed
                .late_phase(&f.nodes, &f.params)?
                .into_iter()
                .map(Event::Mixed),
        );
        events.extend(
            self.waiting
                .iter()
                .enumerate()
                .map(|(index, q)| Event::Delete {
                    index,
                    unique_id: q.source.unique_id,
                }),
        );
        self.waiting.clear();
        events.push(Event::ProcessContext(f.context));
        events.push(Event::SuspendContext(f.context));
        Ok(events)
    }
    pub fn checkpoint(&self) -> Result<Vec<u8>, String> {
        let b = serde_json::to_vec(&self.saved()?).map_err(|e| e.to_string())?;
        if b.len() > crate::audio_stream_data::MAX_BYTES {
            return Err("Mixed queue snapshot too large".into());
        }
        Ok(b)
    }
    pub fn restore(bytes: &[u8]) -> Result<Self, String> {
        if bytes.len() > crate::audio_stream_data::MAX_BYTES {
            return Err("Mixed queue snapshot too large".into());
        }
        Self::from_saved(serde_json::from_slice(bytes).map_err(|e| e.to_string())?)
    }
    pub fn load(&mut self, bytes: &[u8]) -> Result<(), String> {
        *self = Self::restore(bytes)?;
        Ok(())
    }
}
