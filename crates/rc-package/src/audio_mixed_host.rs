//! Mixed prepared static/stream pool. Not the queue/global Update or a device.
use crate::{
    audio_activation as activation, audio_buffer_bank as pcm, audio_census as census,
    audio_curve::CurveMath,
    audio_driver as driver,
    audio_duck::{Ducking, Settings},
    audio_resource_host as resources,
    audio_source::Source,
    audio_source_runtime as runtime, audio_source_tick as tick, audio_stream_bridge as bridge,
};
use serde::{Deserialize, Serialize};
use std::collections::BTreeSet;
pub struct Host {
    resources: resources::Host,
    pool: activation::Pool,
    keys: Vec<u32>,
    duck: Ducking,
    status: census::Status,
    eax: crate::audio_upload::EaxCache,
}
#[derive(Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub(crate) struct Saved {
    schema: u32,
    resources: resources::Saved,
    pool: activation::Pool,
    keys: Vec<u32>,
    duck: Ducking,
    status: census::Status,
    #[serde(default = "default_eax")]
    eax: crate::audio_upload::EaxCache,
}
fn default_eax() -> crate::audio_upload::EaxCache {
    crate::audio_upload::EaxCache {
        initialized: false,
        value: 0,
    }
}
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub enum Event {
    Activation(activation::Event),
    Resource(resources::Event),
    Driver {
        node: usize,
        event: driver::Event,
    },
    Base {
        node: usize,
        event: runtime::Event,
    },
    StaticTick {
        node: usize,
        event: tick::Event,
    },
    StreamTick {
        node: usize,
        outcome: bridge::Outcome,
    },
    Census(census::Event),
    Finished(usize),
    Parameter(crate::audio_upload::Event),
}
#[derive(Debug)]
pub struct Admission {
    pub node: Option<usize>,
    pub events: Vec<Event>,
    pub uploads: Vec<bridge::Upload>,
}
#[derive(Clone, Debug, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Frame {
    /// One frame for every physical node, including the currently free transient.
    pub base: runtime::Frame,
    pub base_delta_bits: u32,
    pub cleanup: driver::Reply,
    pub late: bridge::Tick,
}
impl Host {
    pub fn new(
        owner: u32,
        hardware: &[u32],
        keys: Vec<u32>,
        settings: Settings,
    ) -> Result<Self, String> {
        if !(2..=4097).contains(&hardware.len()) {
            return Err("Mixed pool needs slots and transient".into());
        }
        let mut counter = 0;
        let pool = activation::Pool {
            nodes: hardware
                .iter()
                .map(|&hardware| activation::Node {
                    source: Source::new(&mut counter),
                    hardware,
                    buffer: 0,
                })
                .collect(),
            slots: (0..hardware.len() - 1).map(Some).collect(),
            transient: Some(hardware.len() - 1),
        };
        let h = Self {
            resources: resources::Host::new(owner)?,
            pool,
            keys,
            duck: Ducking::new(settings)?,
            status: census::Status {
                pending: 0,
                current: 0,
                ducking_voices: 0,
                any_playing: false,
            },
            eax: default_eax(),
        };
        h.validate()?;
        Ok(h)
    }
    pub fn resources(&self) -> &resources::Host {
        &self.resources
    }
    pub fn pool(&self) -> &activation::Pool {
        &self.pool
    }
    pub fn status(&self) -> &census::Status {
        &self.status
    }
    pub fn ducking(&self) -> &Ducking {
        &self.duck
    }
    pub(crate) fn eax_cache(&self) -> &crate::audio_upload::EaxCache {
        &self.eax
    }
    pub fn key(&self, node: usize) -> Result<u32, String> {
        self.keys
            .get(node)
            .copied()
            .ok_or_else(|| "Unknown mixed node".into())
    }
    fn owned_node(&self, n: usize) -> Option<&activation::Node> {
        self.resources
            .voice(self.keys[n])
            .map(|v| &v.driver.node)
            .or_else(|| {
                self.resources
                    .stream(self.keys[n])
                    .map(|s| &s.tick.driver.node)
            })
    }
    pub fn validate(&self) -> Result<(), String> {
        self.resources.validate()?;
        self.pool.validate()?;
        self.duck.validate()?;
        self.status.validate()?;
        let len = self.pool.nodes.len();
        if len != self.pool.slots.len() + 1
            || len < 2
            || self.keys.len() != len
            || self.keys.contains(&0)
            || self.keys.iter().collect::<BTreeSet<_>>().len() != len
            || self.pool.nodes.iter().any(|n| n.hardware == 0)
            || self
                .pool
                .nodes
                .iter()
                .map(|n| n.hardware)
                .collect::<BTreeSet<_>>()
                .len()
                != len
            || self
                .pool
                .slots
                .iter()
                .chain(std::iter::once(&self.pool.transient))
                .flatten()
                .copied()
                .collect::<BTreeSet<_>>()
                .len()
                != len
            || self.pool.slots.iter().any(Option::is_none)
            || self.pool.transient.is_none()
            || self.resources.is_closed()
        {
            return Err("Invalid mixed physical graph".into());
        }
        let transient = self.pool.transient.unwrap();
        for n in 0..len {
            if let Some(owned) = self.owned_node(n) {
                if n == transient
                    || serde_json::to_value(owned).map_err(|e| e.to_string())?
                        != serde_json::to_value(&self.pool.nodes[n]).map_err(|e| e.to_string())?
                {
                    return Err("Mixed pool/owner diverged".into());
                }
            } else if self.pool.nodes[n].source.allocated || self.pool.nodes[n].buffer != 0 {
                return Err("Unowned mixed source".into());
            }
        }
        if self
            .resources
            .static_keys()
            .iter()
            .chain(self.resources.stream_keys().iter())
            .any(|k| !self.keys.contains(k))
        {
            return Err("Untracked mixed resource".into());
        }
        Ok(())
    }
    pub(crate) fn saved(&self) -> Result<Saved, String> {
        self.validate()?;
        Ok(Saved {
            schema: 1,
            resources: self.resources.saved()?,
            pool: self.pool.clone(),
            keys: self.keys.clone(),
            duck: self.duck.clone(),
            status: self.status.clone(),
            eax: self.eax.clone(),
        })
    }
    pub(crate) fn from_saved(s: Saved) -> Result<Self, String> {
        if s.schema != 1 {
            return Err("Unsupported mixed snapshot".into());
        }
        let h = Self {
            resources: resources::Host::restore_state(s.resources)?,
            pool: s.pool,
            keys: s.keys,
            duck: s.duck,
            status: s.status,
            eax: s.eax,
        };
        h.validate()?;
        Ok(h)
    }
    fn transaction<T>(
        &mut self,
        f: impl FnOnce(&mut Self) -> Result<T, String>,
    ) -> Result<T, String> {
        let before = self.saved()?;
        let out = f(self).and_then(|out| {
            self.validate()?;
            Ok(out)
        });
        if out.is_err() {
            *self = Self::from_saved(before)?;
        }
        out
    }
    pub fn register_pcm(
        &mut self,
        bytes: &[u8],
        r: &pcm::Registration,
    ) -> Result<resources::Registered, String> {
        self.transaction(|h| h.resources.register_pcm(bytes, r))
    }
    pub(crate) fn replace_source(&mut self, node: usize, source: Source) -> Result<(), String> {
        let key = self.keys[node];
        if self.resources.voice(key).is_some() {
            self.resources.replace_static_source(key, source.clone())?;
        } else if self.resources.stream(key).is_some() {
            self.resources.replace_stream_source(key, source.clone())?;
        }
        self.pool.nodes[node].source = source;
        Ok(())
    }
    fn release(&mut self, node: usize, reply: &driver::Reply) -> Result<Vec<Event>, String> {
        let key = self.key(node)?;
        let events = if let Some(s) = self.resources.stream(key) {
            let sound = s.tick.driver.node.source.sound;
            self.resources.unregister(sound, Some(reply))?
        } else if self.resources.voice(key).is_some() {
            self.resources.detach_static(key)?
        } else {
            vec![]
        };
        self.pool.nodes[node].source.clear();
        self.pool.nodes[node].buffer = 0;
        Ok(events.into_iter().map(Event::Resource).collect())
    }
    fn prefix(
        &mut self,
        incoming: &Source,
        buffer: u32,
        replies: &[driver::Reply],
    ) -> Result<(Option<activation::Prepared>, Vec<Event>), String> {
        if !incoming.allocated || replies.len() != self.pool.nodes.len() {
            return Err("Invalid prepared admission/replies".into());
        }
        let mut plan = self.pool.clone();
        let (prepared, prefix) = plan.prepare(
            incoming,
            activation::Request {
                engine_present: true,
                sound_valid: true,
                buffer,
                queued: 1,
            },
        )?;
        let mut events = vec![];
        for event in prefix {
            events.push(Event::Activation(event.clone()));
            if let activation::Event::Stop(n) = event {
                events.extend(self.release(n, &replies[n])?);
                plan.nodes[n] = self.pool.nodes[n].clone();
            }
        }
        if prepared.is_none() {
            let t = plan.transient.unwrap();
            plan.nodes[t].buffer = 0;
        }
        self.pool = plan;
        Ok((prepared, events))
    }
    pub fn admit_static(
        &mut self,
        incoming: &Source,
        sound: &driver::Sound,
        replies: &[driver::Reply],
    ) -> Result<Admission, String> {
        self.admit_static_mode(incoming, sound, replies, None)
    }
    pub fn play_static(
        &mut self,
        incoming: &Source,
        sound: &driver::Sound,
        replies: &[driver::Reply],
        env: &crate::audio_upload::Environment,
    ) -> Result<Admission, String> {
        self.admit_static_mode(incoming, sound, replies, Some(env))
    }
    fn admit_static_mode(
        &mut self,
        incoming: &Source,
        sound: &driver::Sound,
        replies: &[driver::Reply],
        env: Option<&crate::audio_upload::Environment>,
    ) -> Result<Admission, String> {
        self.transaction(|h| {
            if incoming.flags & 4 != 0 || sound.flags & 2 != 0 || incoming.sound != sound.handle {
                return Err("Expected matching static admission".into());
            }
            let buffer = h
                .resources
                .bank()
                .entries
                .iter()
                .find(|e| e.sound == incoming.sound)
                .ok_or("Static PCM not registered")?
                .buffer
                .handle;
            let (p, mut events) = h.prefix(incoming, buffer, replies)?;
            if let Some(p) = &p {
                h.resources.attach_static(
                    h.keys[p.node],
                    h.pool.nodes[p.node].clone(),
                    sound.clone(),
                )?;
                events.extend(if let Some(env) = env {
                    h.start_uploaded(p.node, p.pause, env)?
                } else {
                    h.start(p.node, p.pause)?
                });
            }
            Ok(Admission {
                node: p.map(|p| p.node),
                events,
                uploads: vec![],
            })
        })
    }
    pub fn admit_stream(
        &mut self,
        bytes: &[u8],
        template: &tick::Session,
        registration: &resources::StreamRegistration,
        replies: &[driver::Reply],
    ) -> Result<Admission, String> {
        self.admit_stream_mode(bytes, template, registration, replies, None)
    }
    pub fn play_stream(
        &mut self,
        bytes: &[u8],
        template: &tick::Session,
        registration: &resources::StreamRegistration,
        replies: &[driver::Reply],
        env: &crate::audio_upload::Environment,
    ) -> Result<Admission, String> {
        self.admit_stream_mode(bytes, template, registration, replies, Some(env))
    }
    fn admit_stream_mode(
        &mut self,
        bytes: &[u8],
        template: &tick::Session,
        registration: &resources::StreamRegistration,
        replies: &[driver::Reply],
        env: Option<&crate::audio_upload::Environment>,
    ) -> Result<Admission, String> {
        self.transaction(|h| {
            let incoming = &template.driver.node.source;
            if incoming.flags & 4 == 0
                || h.resources.stream_keys().iter().any(|&k| {
                    h.resources
                        .stream(k)
                        .is_some_and(|s| s.tick.driver.node.source.sound == incoming.sound)
                })
            {
                return Err("Stream requires a fresh unique sound owner".into());
            }
            let (p, mut events) = h.prefix(incoming, template.driver.node.buffer, replies)?;
            let mut uploads = vec![];
            if let Some(p) = &p {
                if registration.key != h.keys[p.node] {
                    return Err("Stream key must match selected transient".into());
                }
                let mut tick = template.clone();
                tick.driver.node = h.pool.nodes[p.node].clone();
                let out = h.resources.register_stream(bytes, tick, registration)?;
                uploads = out.uploads;
                if let Some(env) = env {
                    events.extend(h.start_uploaded(p.node, p.pause, env)?);
                } else {
                    events.extend(out.events.into_iter().map(Event::Resource));
                    events.extend(h.start(p.node, p.pause)?);
                }
            }
            Ok(Admission {
                node: p.map(|p| p.node),
                events,
                uploads,
            })
        })
    }
    fn start_uploaded(
        &mut self,
        node: usize,
        pause: i32,
        env: &crate::audio_upload::Environment,
    ) -> Result<Vec<Event>, String> {
        use crate::audio_upload as upload;
        let key = self.keys[node];
        let buffers = if let Some(v) = self.resources.voice(key) {
            v.driver.buffers[0].storage.clone()
        } else {
            self.resources
                .stream(key)
                .ok_or("Missing upload owner")?
                .tick
                .driver
                .buffers[0]
                .storage
                .clone()
        };
        let mut plan = upload::Session {
            pool: self.pool.clone(),
            buffers: vec![upload::Buffer {
                handle: self.pool.nodes[node].buffer,
                buffers,
                counters: [0; 2],
            }],
            eax_cache: self.eax.clone(),
        };
        let mut events = vec![];
        for event in plan.upload(&activation::Prepared { node, pause }, env)? {
            events.push(Event::Parameter(event.clone()));
            match event {
                upload::Event::Play { .. } => {
                    events.extend(self.control_inner(
                        node,
                        &driver::Command::Play,
                        &driver::Reply {
                            source_state: 0,
                            unqueued: [0; 4],
                        },
                    )?);
                    let mut s = self.pool.nodes[node].source.clone();
                    s.lifetime_words = [0; 2];
                    self.replace_source(node, s)?;
                }
                upload::Event::Pause { request, .. } => events.extend(self.control_inner(
                    node,
                    &driver::Command::Pause(request),
                    &driver::Reply {
                        source_state: 0,
                        unqueued: [0; 4],
                    },
                )?),
                _ => {}
            }
        }
        self.eax = plan.eax_cache;
        Ok(events)
    }
    fn start(&mut self, node: usize, pause: i32) -> Result<Vec<Event>, String> {
        let mut events = self.control_inner(
            node,
            &driver::Command::Play,
            &driver::Reply {
                source_state: 0,
                unqueued: [0; 4],
            },
        )?;
        let mut source = self.pool.nodes[node].source.clone();
        source.lifetime_words = [0; 2];
        self.replace_source(node, source)?;
        if pause != 0 {
            events.extend(self.control_inner(
                node,
                &driver::Command::Pause(pause),
                &driver::Reply {
                    source_state: 0,
                    unqueued: [0; 4],
                },
            )?);
        }
        Ok(events)
    }
    fn control_inner(
        &mut self,
        node: usize,
        command: &driver::Command,
        reply: &driver::Reply,
    ) -> Result<Vec<Event>, String> {
        if matches!(command, driver::Command::Stop) {
            return self.release(node, reply);
        }
        let key = self.key(node)?;
        let out = if self.resources.voice(key).is_some() {
            self.resources.control_static(key, command, reply)?
        } else if self.resources.stream(key).is_some() {
            self.resources.control_stream(key, command, reply)?
        } else {
            return Ok(vec![]);
        };
        if let Some(n) = self.owned_node(node) {
            self.pool.nodes[node] = n.clone();
        }
        Ok(out
            .events
            .into_iter()
            .map(|event| Event::Driver { node, event })
            .collect())
    }
    pub fn control(
        &mut self,
        node: usize,
        command: &driver::Command,
        reply: &driver::Reply,
    ) -> Result<Vec<Event>, String> {
        self.transaction(|h| h.control_inner(node, command, reply))
    }
    pub fn service_round(&mut self, budget: usize) -> Result<Vec<resources::Served>, String> {
        self.transaction(|h| h.resources.service_round(budget))
    }
    pub fn update(
        &mut self,
        global_delta: f32,
        frames: &[Frame],
        params: &[census::Params],
        math: &mut impl CurveMath,
    ) -> Result<Vec<Event>, String> {
        self.transaction(|h| h.update_inner(global_delta, frames, params, math))
    }
    fn update_inner(
        &mut self,
        global_delta: f32,
        frames: &[Frame],
        params: &[census::Params],
        math: &mut impl CurveMath,
    ) -> Result<Vec<Event>, String> {
        let mut events = self.begin_phase(global_delta, frames)?;
        events.extend(self.base_phase(frames, math)?);
        events.extend(self.late_phase(frames, params)?);
        Ok(events)
    }
    pub(crate) fn begin_phase(
        &mut self,
        global_delta: f32,
        frames: &[Frame],
    ) -> Result<Vec<Event>, String> {
        if !global_delta.is_finite() || frames.len() != self.pool.nodes.len() {
            return Err("Mixed update shape mismatch".into());
        }
        self.duck.step(
            self.status.ducking_voices as i32,
            global_delta.clamp(0., 1.),
        )?;
        let slots = self
            .pool
            .slots
            .iter()
            .map(|n| n.unwrap())
            .collect::<Vec<_>>();
        let mut events = vec![];
        for &node in &slots {
            events.push(Event::Finished(node));
            let key = self.keys[node];
            let out = if self.resources.voice(key).is_some() {
                Some(self.resources.control_static(
                    key,
                    &driver::Command::IsFinished,
                    &frames[node].cleanup,
                )?)
            } else if self.resources.stream(key).is_some() {
                Some(self.resources.control_stream(
                    key,
                    &driver::Command::IsFinished,
                    &frames[node].cleanup,
                )?)
            } else {
                None
            };
            if let Some(out) = out {
                events.extend(
                    out.events
                        .into_iter()
                        .map(|event| Event::Driver { node, event }),
                );
                if out.result != 0 {
                    events.extend(self.release(node, &frames[node].cleanup)?);
                }
            }
        }
        Ok(events)
    }
    pub(crate) fn base_phase(
        &mut self,
        frames: &[Frame],
        math: &mut impl CurveMath,
    ) -> Result<Vec<Event>, String> {
        let slots = self
            .pool
            .slots
            .iter()
            .map(|n| n.unwrap())
            .collect::<Vec<_>>();
        let mut events = vec![];
        for &node in &slots {
            let source = self.pool.nodes[node].source.clone();
            if source.sound == 0 || source.cursor == 0 {
                continue;
            }
            let mut frame = frames[node].base.clone();
            frame.base.paused = [source.requested_pause != 0; 2];
            frame.queued = false;
            let mut runtime = runtime::Runtime {
                ducking: self.duck.clone(),
                sources: vec![source],
            };
            let mut stops = vec![];
            let out = runtime.step_controlled(
                0,
                f32::from_bits(frames[node].base_delta_bits),
                &[frame],
                math,
                &mut |_, s: &mut Source| {
                    self.replace_source(node, s.clone())?;
                    stops = self.release(node, &frames[node].cleanup)?;
                    s.clear();
                    Ok(())
                },
                false,
            )?;
            for event in out.events {
                let stop = matches!(event, runtime::Event::Stop(_));
                events.push(Event::Base { node, event });
                if stop {
                    events.append(&mut stops);
                }
            }
            self.replace_source(node, runtime.sources.remove(0))?;
        }
        Ok(events)
    }
    pub(crate) fn late_phase(
        &mut self,
        frames: &[Frame],
        params: &[census::Params],
    ) -> Result<Vec<Event>, String> {
        let slots = self
            .pool
            .slots
            .iter()
            .map(|n| n.unwrap())
            .collect::<Vec<_>>();
        let mut events = vec![];
        let mut sources = slots
            .iter()
            .map(|&n| self.pool.nodes[n].source.clone())
            .collect::<Vec<_>>();
        let mut census_frames = vec![];
        for &node in &slots {
            let playing = self.pool.nodes[node].source.cursor as i32;
            let key = self.keys[node];
            if playing != 0 {
                if self.resources.voice(key).is_some() {
                    let out = self.resources.tick_static(
                        key,
                        &frames[node].late.environment,
                        frames[node].late.delta_bits,
                    )?;
                    events.extend(
                        out.events
                            .into_iter()
                            .map(|event| Event::StaticTick { node, event }),
                    );
                } else {
                    let outcome = self.resources.tick_stream(key, &frames[node].late)?;
                    events.push(Event::StreamTick { node, outcome });
                }
                self.pool.nodes[node] = self
                    .owned_node(node)
                    .ok_or("Mixed tick lost source")?
                    .clone();
            }
            census_frames.push(census::Frame {
                playing,
                seconds_bits: frames[node].late.delta_bits,
                after: self.pool.nodes[node].source.clone(),
            });
        }
        events.extend(
            self.status
                .recount(&mut sources, &census_frames, params)?
                .into_iter()
                .map(Event::Census),
        );
        Ok(events)
    }
    pub fn refresh_cues(&mut self) -> Result<Vec<Event>, String> {
        self.transaction(|h| {
            let sources = h
                .pool
                .slots
                .iter()
                .map(|n| h.pool.nodes[n.unwrap()].source.clone())
                .collect::<Vec<_>>();
            let replies = sources
                .iter()
                .map(|s| [s.cursor as i32; 2])
                .collect::<Vec<_>>();
            Ok(h.status
                .refresh(&sources, &replies)?
                .into_iter()
                .map(Event::Census)
                .collect())
        })
    }
    pub fn set_pending(&mut self, mask: u32) {
        self.status.pending = mask;
    }
    pub fn unregister(
        &mut self,
        sound: u32,
        replies: &[driver::Reply],
    ) -> Result<Vec<Event>, String> {
        self.transaction(|h| {
            if replies.len() != h.pool.nodes.len() {
                return Err("Unregister reply shape mismatch".into());
            }
            let mut events = vec![];
            for (n, reply) in replies.iter().enumerate() {
                if h.pool.nodes[n].source.sound == sound {
                    events.extend(h.release(n, reply)?);
                }
            }
            events.extend(
                h.resources
                    .unregister(sound, None)?
                    .into_iter()
                    .map(Event::Resource),
            );
            Ok(events)
        })
    }
    pub fn checkpoint(&self) -> Result<Vec<u8>, String> {
        let bytes = serde_json::to_vec(&self.saved()?).map_err(|e| e.to_string())?;
        if bytes.len() > crate::audio_stream_data::MAX_BYTES {
            return Err("Mixed snapshot too large".into());
        }
        Ok(bytes)
    }
    pub fn restore(bytes: &[u8]) -> Result<Self, String> {
        if bytes.len() > crate::audio_stream_data::MAX_BYTES {
            return Err("Mixed snapshot too large".into());
        }
        Self::from_saved(serde_json::from_slice(bytes).map_err(|e| e.to_string())?)
    }
    pub fn load(&mut self, bytes: &[u8]) -> Result<(), String> {
        let h = Self::restore(bytes)?;
        *self = h;
        Ok(())
    }
}
