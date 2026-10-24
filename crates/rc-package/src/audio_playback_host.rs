//! Static-source activation/update composition over the shared resource owner.
//! Phases are explicit; this is not the engine's complete global Update order.
use crate::{
    audio_activation as activation, audio_buffer_bank as pcm, audio_census as census,
    audio_curve::CurveMath,
    audio_driver as driver,
    audio_duck::{Ducking, Settings},
    audio_resource_host as resources,
    audio_source::Source,
    audio_source_runtime as runtime, audio_source_tick as tick, audio_upload as upload,
};
use serde::{Deserialize, Serialize};
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub enum Event {
    Finished(usize),
    Stop(usize),
    Activation(activation::Event),
    Parameter(upload::Event),
    Driver { node: usize, event: driver::Event },
    Base(runtime::Event),
    Tick { node: usize, event: tick::Event },
    Census(census::Event),
}
#[derive(Clone, Debug, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Request {
    pub incoming: Source,
    pub sound: driver::Sound,
    pub engine_present: bool,
    pub sound_valid: bool,
    pub queued: i32,
    pub environment: upload::Environment,
}
#[derive(Clone, Debug, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct DriverFrame {
    pub environment: upload::Environment,
    pub delta_bits: u32,
}
#[derive(Debug)]
pub struct Activated {
    pub node: Option<usize>,
    pub events: Vec<Event>,
}
pub struct Host {
    resources: resources::Host,
    upload: upload::Session,
    keys: Vec<u32>,
    duck: Ducking,
    status: census::Status,
}
#[derive(Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub(crate) struct Saved {
    schema: u32,
    resources: resources::Saved,
    upload: upload::Session,
    keys: Vec<u32>,
    duck: Ducking,
    status: census::Status,
}
impl Host {
    pub(crate) fn replace_source(&mut self, node: usize, source: Source) -> Result<(), String> {
        source.validate()?;
        let key = *self.keys.get(node).ok_or("Unknown source node")?;
        if self.resources.voice(key).is_some() {
            self.resources.replace_static_source(key, source.clone())?;
        }
        self.upload.pool.nodes[node].source = source;
        Ok(())
    }
    pub(crate) fn pause_node(&mut self, node: usize, request: i32) -> Result<Vec<Event>, String> {
        let key = *self.keys.get(node).ok_or("Unknown paused node")?;
        let mut events = vec![];
        if self.resources.voice(key).is_some() {
            let result = self.resources.control_static(
                key,
                &driver::Command::Pause(request),
                &driver::Reply {
                    source_state: 0,
                    unqueued: [0; 4],
                },
            )?;
            events.extend(
                result
                    .events
                    .into_iter()
                    .map(|event| Event::Driver { node, event }),
            );
            self.upload.pool.nodes[node] = self
                .resources
                .voice(key)
                .ok_or("Missing paused voice")?
                .driver
                .node
                .clone();
        }
        Ok(events)
    }
    pub(crate) fn cleanup_one(
        &mut self,
        node: usize,
        reply: &driver::Reply,
    ) -> Result<Vec<Event>, String> {
        let key = *self.keys.get(node).ok_or("Unknown cleanup node")?;
        let mut out = vec![Event::Finished(node)];
        if self.resources.voice(key).is_some() {
            let result = self
                .resources
                .control_static(key, &driver::Command::IsFinished, reply)?;
            out.extend(
                result
                    .events
                    .into_iter()
                    .map(|event| Event::Driver { node, event }),
            );
            if result.result != 0 {
                out.extend(self.stop_node(node)?);
            }
        } else {
            out.push(Event::Stop(node));
        }
        Ok(out)
    }
    pub fn cleanup_finished(&mut self, replies: &[driver::Reply]) -> Result<Vec<Event>, String> {
        self.transaction(|h| {
            if replies.len() != h.upload.pool.slots.len() {
                return Err("Cleanup reply/slot mismatch".into());
            }
            let nodes = h
                .upload
                .pool
                .slots
                .iter()
                .map(|n| n.unwrap())
                .collect::<Vec<_>>();
            let mut events = vec![];
            for (node, r) in nodes.into_iter().zip(replies) {
                events.extend(h.cleanup_one(node, r)?);
            }
            Ok(events)
        })
    }
    pub(crate) fn stop_node(&mut self, node: usize) -> Result<Vec<Event>, String> {
        let key = *self.keys.get(node).ok_or("Unknown stopped node")?;
        let mut events = vec![Event::Stop(node)];
        if self.resources.voice(key).is_some() {
            let out = self.resources.control_static(
                key,
                &driver::Command::Stop,
                &driver::Reply {
                    source_state: 0,
                    unqueued: [0; 4],
                },
            )?;
            events.extend(
                out.events
                    .into_iter()
                    .map(|event| Event::Driver { node, event }),
            );
            self.upload.pool.nodes[node] = self
                .resources
                .voice(key)
                .ok_or("Missing stopped node")?
                .driver
                .node
                .clone();
        }
        Ok(events)
    }
    pub fn from_prepared(
        resources: resources::Host,
        pool: activation::Pool,
        keys: Vec<u32>,
        settings: Settings,
        eax_cache: upload::EaxCache,
    ) -> Result<Self, String> {
        let buffers = resources
            .bank()
            .entries
            .iter()
            .map(|e| upload::Buffer {
                handle: e.buffer.handle,
                buffers: e.buffer.storage.clone(),
                counters: [0; 2],
            })
            .collect();
        let h = Self {
            resources,
            upload: upload::Session {
                pool,
                buffers,
                eax_cache,
            },
            keys,
            duck: Ducking::new(settings)?,
            status: census::Status {
                pending: 0,
                current: 0,
                ducking_voices: 0,
                any_playing: false,
            },
        };
        h.validate()?;
        Ok(h)
    }
    /// Last physical node is the transient; preceding nodes are playable slots.
    pub fn new(
        owner: u32,
        hardware: &[u32],
        keys: Vec<u32>,
        settings: Settings,
    ) -> Result<Self, String> {
        if hardware.len() < 2 || hardware.len() > 4097 {
            return Err("Playback pool needs slots and one transient".into());
        }
        let mut counter = 0;
        let nodes = hardware
            .iter()
            .map(|&hardware| activation::Node {
                source: Source::new(&mut counter),
                hardware,
                buffer: 0,
            })
            .collect();
        let h = Self {
            resources: resources::Host::new(owner)?,
            upload: upload::Session {
                pool: activation::Pool {
                    nodes,
                    slots: (0..hardware.len() - 1).map(Some).collect(),
                    transient: Some(hardware.len() - 1),
                },
                buffers: vec![],
                eax_cache: upload::EaxCache {
                    initialized: false,
                    value: 0,
                },
            },
            keys,
            duck: Ducking::new(settings)?,
            status: census::Status {
                pending: 0,
                current: 0,
                ducking_voices: 0,
                any_playing: false,
            },
        };
        h.validate()?;
        Ok(h)
    }
    pub fn pool(&self) -> &activation::Pool {
        &self.upload.pool
    }
    pub fn resources(&self) -> &resources::Host {
        &self.resources
    }
    pub fn status(&self) -> &census::Status {
        &self.status
    }
    pub fn ducking(&self) -> &Ducking {
        &self.duck
    }
    pub fn set_pending(&mut self, pending: u32) {
        self.status.pending = pending;
    }
    fn same<T: Serialize>(a: &T, b: &T) -> Result<bool, String> {
        Ok(serde_json::to_value(a).map_err(|e| e.to_string())?
            == serde_json::to_value(b).map_err(|e| e.to_string())?)
    }
    pub fn validate(&self) -> Result<(), String> {
        self.resources.validate()?;
        self.upload.validate()?;
        self.duck.validate()?;
        self.status.validate()?;
        let p = &self.upload.pool;
        if self.resources.counts().2 != 0
            || self.resources.is_closed()
            || p.nodes.len() != p.slots.len() + 1
            || self.keys.len() != p.nodes.len()
            || self.keys.contains(&0)
            || self
                .keys
                .iter()
                .collect::<std::collections::BTreeSet<_>>()
                .len()
                != self.keys.len()
            || p.nodes.iter().any(|n| n.hardware == 0)
            || p.nodes
                .iter()
                .map(|n| n.hardware)
                .collect::<std::collections::BTreeSet<_>>()
                .len()
                != p.nodes.len()
            || p.transient.is_none()
            || p.nodes[p.transient.unwrap()].source.allocated
        {
            return Err("Invalid static playback graph".into());
        }
        if self
            .resources
            .static_keys()
            .iter()
            .any(|k| !self.keys.contains(k))
        {
            return Err("Untracked playback resource voice".into());
        }
        for (i, n) in p.nodes.iter().enumerate() {
            if let Some(v) = self.resources.voice(self.keys[i]) {
                if !Self::same(n, &v.driver.node)? {
                    return Err("Pool/driver source state diverged".into());
                }
            } else if n.source.allocated {
                return Err("Allocated pool source has no driver".into());
            }
            if n.buffer != 0
                && !self
                    .resources
                    .bank()
                    .entries
                    .iter()
                    .any(|e| e.buffer.handle == n.buffer)
            {
                return Err("Pool has an unregistered buffer token".into());
            }
        }
        if self.upload.buffers.len() != self.resources.bank().entries.len() {
            return Err("Upload/resource buffer count mismatch".into());
        }
        for b in &self.upload.buffers {
            let e = self
                .resources
                .bank()
                .entries
                .iter()
                .find(|e| e.buffer.handle == b.handle)
                .ok_or("Unknown upload buffer")?;
            if b.buffers != e.buffer.storage || b.counters != [0; 2] {
                return Err("Static buffer upload state mismatch".into());
            }
        }
        Ok(())
    }
    pub(crate) fn saved(&self) -> Result<Saved, String> {
        self.validate()?;
        Ok(Saved {
            schema: 1,
            resources: self.resources.saved()?,
            upload: self.upload.clone(),
            keys: self.keys.clone(),
            duck: self.duck.clone(),
            status: self.status.clone(),
        })
    }
    pub(crate) fn from_saved(s: Saved) -> Result<Self, String> {
        if s.schema != 1 {
            return Err("Unsupported playback host schema".into());
        }
        let h = Self {
            resources: resources::Host::restore_state(s.resources)?,
            upload: s.upload,
            keys: s.keys,
            duck: s.duck,
            status: s.status,
        };
        h.validate()?;
        Ok(h)
    }
    fn transaction<T>(
        &mut self,
        f: impl FnOnce(&mut Self) -> Result<T, String>,
    ) -> Result<T, String> {
        let before = self.saved()?;
        match f(self).and_then(|v| {
            self.validate()?;
            Ok(v)
        }) {
            Ok(v) => Ok(v),
            Err(e) => {
                *self = Self::from_saved(before)?;
                Err(e)
            }
        }
    }
    pub fn register_pcm(
        &mut self,
        wave: &[u8],
        r: &pcm::Registration,
    ) -> Result<resources::Registered, String> {
        self.transaction(|h| {
            let out = h.resources.register_pcm(wave, r)?;
            if out.handle != 0 && !h.upload.buffers.iter().any(|b| b.handle == out.handle) {
                let e = h
                    .resources
                    .bank()
                    .entries
                    .iter()
                    .find(|e| e.buffer.handle == out.handle)
                    .ok_or("Missing registered PCM")?;
                h.upload.buffers.push(upload::Buffer {
                    handle: out.handle,
                    buffers: e.buffer.storage.clone(),
                    counters: [0; 2],
                });
            }
            Ok(out)
        })
    }
    pub fn play(&mut self, request: &Request) -> Result<Activated, String> {
        self.transaction(|h| h.play_inner(request))
    }
    fn play_inner(&mut self, r: &Request) -> Result<Activated, String> {
        if r.incoming.flags & 4 != 0 || r.sound.flags & 2 != 0 || r.sound.handle != r.incoming.sound
        {
            return Err("Static activation requires matching static sound metadata".into());
        }
        let buffer = self
            .resources
            .bank()
            .entries
            .iter()
            .find(|e| e.sound == r.incoming.sound)
            .map_or(0, |e| e.buffer.handle);
        let before = self.upload.pool.nodes.clone();
        let mut plan = self.upload.clone();
        let playback = plan.play(
            &r.incoming,
            activation::Request {
                engine_present: r.engine_present,
                sound_valid: r.sound_valid,
                buffer,
                queued: r.queued,
            },
            &r.environment,
        )?;
        let mut events = vec![];
        let mut stopped = std::collections::BTreeMap::new();
        for e in playback.prefix {
            if let activation::Event::Stop(node) = &e {
                let node = *node;
                let key = self.keys[node];
                if self.resources.voice(key).is_some() {
                    let out = self.resources.control_static(
                        key,
                        &driver::Command::Stop,
                        &driver::Reply {
                            source_state: 0,
                            unqueued: [0; 4],
                        },
                    )?;
                    stopped.insert(
                        node,
                        self.resources
                            .voice(key)
                            .ok_or("Missing stopped source")?
                            .driver
                            .node
                            .clone(),
                    );
                    self.resources.detach_static(key)?;
                    events.push(Event::Activation(e));
                    events.extend(
                        out.events
                            .into_iter()
                            .map(|event| Event::Driver { node, event }),
                    );
                    continue;
                }
                stopped.entry(node).or_insert_with(|| before[node].clone());
            }
            events.push(Event::Activation(e));
        }
        for (i, n) in stopped {
            plan.pool.nodes[i] = n;
        }
        let selected = playback.prepared.map(|p| p.node);
        if let Some(node) = selected {
            let key = self.keys[node];
            let mut n = plan.pool.nodes[node].clone();
            n.source.requested_pause = r.incoming.requested_pause;
            n.source.cursor = r.incoming.cursor;
            n.source.lifetime_words = r.incoming.lifetime_words;
            if self.resources.voice(key).is_some() {
                self.resources.detach_static(key)?;
            }
            self.resources.attach_static(key, n, r.sound.clone())?;
            for e in playback.events {
                match &e {
                    upload::Event::Play { .. } => {
                        events.push(Event::Parameter(e));
                        let out = self.resources.control_static(
                            key,
                            &driver::Command::Play,
                            &driver::Reply {
                                source_state: 0,
                                unqueued: [0; 4],
                            },
                        )?;
                        events.extend(
                            out.events
                                .into_iter()
                                .map(|event| Event::Driver { node, event }),
                        );
                        let mut s = self
                            .resources
                            .voice(key)
                            .ok_or("Missing played source")?
                            .driver
                            .node
                            .source
                            .clone();
                        s.lifetime_words = [0; 2];
                        self.resources.replace_static_source(key, s)?;
                    }
                    upload::Event::Pause { request, .. } => {
                        let request = *request;
                        events.push(Event::Parameter(e));
                        let out = self.resources.control_static(
                            key,
                            &driver::Command::Pause(request),
                            &driver::Reply {
                                source_state: 0,
                                unqueued: [0; 4],
                            },
                        )?;
                        events.extend(
                            out.events
                                .into_iter()
                                .map(|event| Event::Driver { node, event }),
                        );
                    }
                    _ => events.push(Event::Parameter(e)),
                }
            }
            plan.pool.nodes[node] = self
                .resources
                .voice(key)
                .ok_or("Missing activated source")?
                .driver
                .node
                .clone();
        }
        self.upload = plan;
        Ok(Activated {
            node: selected,
            events,
        })
    }
    /// Per-slot base/controller phase. Driver pause replies come from owned state.
    pub fn update_base(
        &mut self,
        delta: f32,
        frames: &[runtime::Frame],
        math: &mut impl CurveMath,
    ) -> Result<Vec<Event>, String> {
        let nodes = self
            .upload
            .pool
            .slots
            .iter()
            .map(|n| n.unwrap())
            .collect::<Vec<_>>();
        self.base_nodes(delta, &nodes, frames, math, true)
    }
    pub(crate) fn advance_duck(&mut self, delta: f32) -> Result<(), String> {
        self.validate()?;
        self.duck.step(self.status.ducking_voices as i32, delta)
    }
    pub(crate) fn base_nodes(
        &mut self,
        delta: f32,
        nodes: &[usize],
        frames: &[runtime::Frame],
        math: &mut impl CurveMath,
        advance_duck: bool,
    ) -> Result<Vec<Event>, String> {
        self.transaction(|h| {
            if frames.len() != nodes.len()
                || nodes
                    .iter()
                    .any(|n| !h.upload.pool.slots.contains(&Some(*n)))
                || nodes
                    .iter()
                    .collect::<std::collections::BTreeSet<_>>()
                    .len()
                    != nodes.len()
            {
                return Err("Base frame/node mismatch".into());
            }
            let mut frames = frames.to_vec();
            let mut runtime = runtime::Runtime {
                ducking: h.duck.clone(),
                sources: nodes
                    .iter()
                    .map(|&i| h.upload.pool.nodes[i].source.clone())
                    .collect(),
            };
            let mut stops = vec![];
            for (i, f) in frames.iter_mut().enumerate() {
                let paused = h.upload.pool.nodes[nodes[i]].source.requested_pause != 0;
                f.base.paused = [paused; 2];
            }
            let result = runtime.step_controlled(
                h.status.ducking_voices as i32,
                delta,
                &frames,
                math,
                &mut |index, s: &mut Source| {
                    let node = nodes[index];
                    let key = h.keys[node];
                    h.resources.replace_static_source(key, s.clone())?;
                    let out = h.resources.control_static(
                        key,
                        &driver::Command::Stop,
                        &driver::Reply {
                            source_state: 0,
                            unqueued: [0; 4],
                        },
                    )?;
                    *s = h
                        .resources
                        .voice(key)
                        .ok_or("Missing stopped base source")?
                        .driver
                        .node
                        .source
                        .clone();
                    stops.push((index, out.events));
                    Ok(())
                },
                advance_duck,
            )?;
            let mut events = vec![];
            for e in result.events {
                let stop = if let runtime::Event::Stop(i) = &e {
                    Some(*i)
                } else {
                    None
                };
                events.push(Event::Base(e));
                if let Some(i) = stop {
                    let out = stops
                        .iter()
                        .find(|(j, _)| *j == i)
                        .ok_or("Missing base Stop events")?;
                    events.extend(out.1.iter().cloned().map(|event| Event::Driver {
                        node: nodes[i],
                        event,
                    }));
                }
            }
            for (&node, s) in nodes.iter().zip(runtime.sources) {
                if h.resources.voice(h.keys[node]).is_some() {
                    h.resources.replace_static_source(h.keys[node], s.clone())?;
                }
                h.upload.pool.nodes[node].source = s;
                h.upload.pool.nodes[node].buffer = h
                    .resources
                    .voice(h.keys[node])
                    .map_or(h.upload.pool.nodes[node].buffer, |v| v.driver.node.buffer);
            }
            h.duck = runtime.ducking;
            Ok(events)
        })
    }
    /// Explicit late-driver/census phase; caller determines global phase order.
    pub fn late_tick(
        &mut self,
        frames: &[DriverFrame],
        params: &[census::Params],
    ) -> Result<Vec<Event>, String> {
        self.transaction(|h| {
            if frames.len() != h.upload.pool.slots.len() {
                return Err("Driver frame/slot mismatch".into());
            }
            let nodes = h
                .upload
                .pool
                .slots
                .iter()
                .map(|i| i.ok_or("Null playback slot"))
                .collect::<Result<Vec<_>, _>>()?;
            let mut sources = nodes
                .iter()
                .map(|&i| h.upload.pool.nodes[i].source.clone())
                .collect::<Vec<_>>();
            let mut census = vec![];
            let mut ticks = vec![];
            for (&node, f) in nodes.iter().zip(frames) {
                let key = h.keys[node];
                let playing = h.upload.pool.nodes[node].source.cursor as i32;
                if playing != 0 {
                    let out = h.resources.tick_static(key, &f.environment, f.delta_bits)?;
                    ticks.push((node, out.events));
                    h.upload.pool.nodes[node] = h
                        .resources
                        .voice(key)
                        .ok_or("Missing ticked source")?
                        .driver
                        .node
                        .clone();
                }
                census.push(census::Frame {
                    playing,
                    seconds_bits: f.delta_bits,
                    after: h.upload.pool.nodes[node].source.clone(),
                });
            }
            let calls = h.status.recount(&mut sources, &census, params)?;
            let mut events = vec![];
            for e in calls {
                let index = if let census::Event::Update { index, .. } = &e {
                    Some(*index)
                } else {
                    None
                };
                events.push(Event::Census(e));
                if let Some(i) = index {
                    let out = ticks
                        .iter()
                        .find(|(n, _)| *n == nodes[i])
                        .ok_or("Missing census driver tick")?;
                    events.extend(out.1.iter().cloned().map(|event| Event::Tick {
                        node: nodes[i],
                        event,
                    }));
                }
            }
            Ok(events)
        })
    }
    pub fn refresh_cues(&mut self) -> Result<Vec<Event>, String> {
        self.transaction(|h| {
            let sources = h
                .upload
                .pool
                .slots
                .iter()
                .map(|n| h.upload.pool.nodes[n.unwrap()].source.clone())
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
    pub fn unregister(&mut self, sound: u32) -> Result<Vec<resources::Event>, String> {
        self.transaction(|h| {
            let buffer = h
                .resources
                .bank()
                .entries
                .iter()
                .find(|e| e.sound == sound)
                .map(|e| e.buffer.handle);
            let events = h.resources.unregister(sound, None)?;
            if let Some(buffer) = buffer {
                h.upload.buffers.retain(|b| b.handle != buffer);
                for (i, n) in h.upload.pool.nodes.iter_mut().enumerate() {
                    if n.buffer == buffer {
                        if let Some(v) = h.resources.voice(h.keys[i]) {
                            *n = v.driver.node.clone();
                        } else {
                            n.source.clear();
                            n.buffer = 0;
                        }
                    }
                }
            }
            Ok(events)
        })
    }
    pub fn checkpoint(&self) -> Result<Vec<u8>, String> {
        let b = serde_json::to_vec(&self.saved()?).map_err(|e| e.to_string())?;
        if b.len() > crate::audio_stream_data::MAX_BYTES {
            return Err("Playback snapshot too large".into());
        }
        Ok(b)
    }
    pub fn restore(bytes: &[u8]) -> Result<Self, String> {
        if bytes.len() > crate::audio_stream_data::MAX_BYTES {
            return Err("Playback snapshot too large".into());
        }
        Self::from_saved(serde_json::from_slice(bytes).map_err(|e| e.to_string())?)
    }
    pub fn load(&mut self, bytes: &[u8]) -> Result<(), String> {
        let h = Self::restore(bytes)?;
        *self = h;
        Ok(())
    }
}
