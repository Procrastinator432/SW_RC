//! Shared portable resource/control host. No device, thread or source allocator.
use crate::{
    audio_activation::Node, audio_buffer as raw, audio_buffer_bank as pcm, audio_driver as driver,
    audio_source_tick as tick, audio_stream_bridge as bridge, audio_stream_data::MAX_BYTES,
    audio_vorbis::Provider, audio_vorbis_host as vorbis,
};
use serde::{Deserialize, Serialize};
use std::collections::BTreeSet;
#[derive(Clone, Debug, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Voice {
    pub key: u32,
    pub sound: u32,
    pub driver: driver::Session,
}
pub struct Stream {
    key: u32,
    sound: u32,
    buffer: u32,
    allocation: u32,
    tokens: vorbis::Tokens,
    host: vorbis::Session,
}
pub struct Host {
    bank: pcm::Bank,
    voices: Vec<Voice>,
    streams: Vec<Stream>,
    cursor: usize,
    closed: bool,
}
#[derive(Clone, Debug, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct StreamRegistration {
    pub key: u32,
    pub allocation: u32,
    pub tokens: vorbis::Tokens,
    pub looping: bool,
}
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub enum Event {
    Pcm(pcm::Event),
    Driver {
        source: u32,
        event: driver::Event,
    },
    StreamDispose {
        source: u32,
        event: vorbis::DisposalEvent,
    },
    Parameter {
        source: u32,
        event: crate::audio_upload::Event,
    },
}
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct Registered {
    pub handle: u32,
    pub source: Option<u32>,
    pub cached: bool,
    pub uploads: Vec<bridge::Upload>,
    pub events: Vec<Event>,
}
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct Served {
    pub source: u32,
    pub outcome: bridge::Outcome,
}
#[derive(Clone, Debug, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct StreamReply {
    pub source: u32,
    pub reply: driver::Reply,
}
#[derive(Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
struct SavedStream {
    key: u32,
    sound: u32,
    buffer: u32,
    allocation: u32,
    tokens: vorbis::Tokens,
    host: vorbis::Saved,
}
#[derive(Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub(crate) struct Saved {
    schema: u32,
    bank: pcm::Bank,
    voices: Vec<Voice>,
    streams: Vec<SavedStream>,
    cursor: usize,
    closed: bool,
}
impl Host {
    pub(crate) fn owned_bytes(&self) -> usize {
        self.bank
            .entries
            .iter()
            .map(|e| e.pcm.bytes.len())
            .sum::<usize>()
            + self
                .streams
                .iter()
                .map(|s| s.host.owned_bytes())
                .sum::<usize>()
    }
    pub(crate) fn stream_keys(&self) -> Vec<u32> {
        self.streams.iter().map(|s| s.key).collect()
    }
    pub(crate) fn replace_stream_source(
        &mut self,
        key: u32,
        source: crate::audio_source::Source,
    ) -> Result<(), String> {
        self.open()?;
        self.streams
            .iter_mut()
            .find(|s| s.key == key)
            .ok_or("Unknown stream source")?
            .host
            .replace_source(source)
    }
    pub(crate) fn static_keys(&self) -> Vec<u32> {
        self.voices.iter().map(|v| v.key).collect()
    }
    pub(crate) fn replace_static_source(
        &mut self,
        key: u32,
        source: crate::audio_source::Source,
    ) -> Result<(), String> {
        self.open()?;
        let mut voices = self.voices.clone();
        let v = voices
            .iter_mut()
            .find(|v| v.key == key)
            .ok_or("Unknown static source")?;
        v.driver.node.source = source;
        let mut bank = self.bank.clone();
        bank.sources = Self::bindings(&voices, &self.streams);
        Self::check(&bank, &voices, &self.streams, self.cursor, false)?;
        self.bank = bank;
        self.voices = voices;
        Ok(())
    }
    pub(crate) fn tick_static(
        &mut self,
        key: u32,
        environment: &crate::audio_upload::Environment,
        delta_bits: u32,
    ) -> Result<tick::Outcome, String> {
        self.open()?;
        let mut voices = self.voices.clone();
        let v = voices
            .iter_mut()
            .find(|v| v.key == key)
            .ok_or("Unknown static source")?;
        let mut t = tick::Session {
            driver: v.driver.clone(),
            progress: vec![],
            streams: vec![],
        };
        let out = t.execute(&tick::Step {
            environment: environment.clone(),
            delta_bits,
            reply: tick::Reply {
                state: 0,
                processed: 0,
                queued: 0,
                available: 0,
                busy: 0,
                unqueued: 0,
            },
        })?;
        v.driver = t.driver;
        Self::check(&self.bank, &voices, &self.streams, self.cursor, false)?;
        self.voices = voices;
        Ok(out)
    }
    pub fn new(owner: u32) -> Result<Self, String> {
        let h = Self {
            bank: pcm::Bank::new(owner),
            voices: vec![],
            streams: vec![],
            cursor: 0,
            closed: false,
        };
        h.validate()?;
        Ok(h)
    }
    pub fn counts(&self) -> (usize, usize, usize) {
        (
            self.bank.entries.len(),
            self.voices.len(),
            self.streams.len(),
        )
    }
    pub fn is_closed(&self) -> bool {
        self.closed
    }
    pub fn bank(&self) -> &pcm::Bank {
        &self.bank
    }
    pub fn voice(&self, key: u32) -> Option<&Voice> {
        self.voices.iter().find(|v| v.key == key)
    }
    pub fn stream(&self, key: u32) -> Option<&bridge::Session> {
        self.streams
            .iter()
            .find(|s| s.key == key)
            .map(|s| s.host.state())
    }
    fn open(&self) -> Result<(), String> {
        if self.closed {
            Err("Audio resource host is shut down".into())
        } else {
            self.validate()
        }
    }
    fn bindings(voices: &[Voice], streams: &[Stream]) -> Vec<raw::Source> {
        voices
            .iter()
            .map(|v| raw::Source {
                handle: v.key,
                buffer: v.driver.node.buffer,
            })
            .chain(streams.iter().map(|s| raw::Source {
                handle: s.key,
                buffer: s.host.state().tick.driver.node.buffer,
            }))
            .collect()
    }
    fn check(
        bank: &pcm::Bank,
        voices: &[Voice],
        streams: &[Stream],
        cursor: usize,
        closed: bool,
    ) -> Result<(), String> {
        bank.validate()?;
        if bank.entries.len() + streams.len() > 4096
            || voices.len() + streams.len() > 4096
            || (streams.is_empty() && cursor != 0)
            || (!streams.is_empty() && cursor >= streams.len())
            || (closed && (!bank.entries.is_empty() || !voices.is_empty() || !streams.is_empty()))
        {
            return Err("Invalid resource host counts/cursor".into());
        }
        if bank.sources != Self::bindings(voices, streams) {
            return Err("Resource host binding census mismatch".into());
        }
        let (
            mut sounds,
            mut buffers,
            mut allocations,
            mut al,
            mut keys,
            mut hardware,
            mut stream_ids,
            mut decoders,
        ) = (
            BTreeSet::new(),
            BTreeSet::new(),
            BTreeSet::new(),
            BTreeSet::new(),
            BTreeSet::new(),
            BTreeSet::new(),
            BTreeSet::new(),
            BTreeSet::new(),
        );
        let mut bytes = 0usize;
        for e in &bank.entries {
            sounds.insert(e.sound);
            buffers.insert(e.buffer.handle);
            allocations.insert(e.buffer.allocation);
            al.insert(e.buffer.storage[0]);
            bytes = bytes.saturating_add(e.pcm.bytes.len());
        }
        for v in voices {
            v.driver.validate()?;
            let e = bank
                .entries
                .iter()
                .find(|e| e.sound == v.sound)
                .ok_or("Voice has no PCM registration")?;
            if v.key == 0
                || !keys.insert(v.key)
                || v.driver.node.hardware == 0
                || !hardware.insert(v.driver.node.hardware)
                || v.driver.buffers.len() != 1
                || v.driver.sounds.len() != 1
            {
                return Err("Invalid static voice ownership".into());
            }
            let b = &v.driver.buffers[0];
            let sound = &v.driver.sounds[0];
            if b.handle != e.buffer.handle
                || b.storage != e.buffer.storage
                || b.count_word != 1
                || b.stream_token != 0
                || sound.handle != v.sound
                || sound.flags & 2 != 0
                || sound.native_handle as u32 != e.buffer.handle
                || (v.driver.node.source.allocated
                    && (v.driver.node.buffer != e.buffer.handle
                        || v.driver.node.source.sound != v.sound
                        || v.driver.node.source.flags & 4 != 0))
                || (!v.driver.node.source.allocated && v.driver.node.buffer != 0)
            {
                return Err("Static driver/registration mismatch".into());
            }
        }
        for s in streams {
            let h = s.host.state();
            h.validate()?;
            if s.sound == 0
                || !sounds.insert(s.sound)
                || s.key == 0
                || !keys.insert(s.key)
                || s.buffer == 0
                || !buffers.insert(s.buffer)
                || s.allocation == 0
                || !allocations.insert(s.allocation)
                || !allocations.insert(s.tokens.staging)
                || s.tokens.staging == 0
                || s.tokens.stream == 0
                || !stream_ids.insert(s.tokens.stream)
                || s.tokens.decoder == 0
                || !decoders.insert(s.tokens.decoder)
                || h.tick.driver.node.hardware == 0
                || !hardware.insert(h.tick.driver.node.hardware)
            {
                return Err("Conflicting stream resource tokens".into());
            }
            if h.tick.driver.buffers.len() != 1
                || h.tick.progress.len() != 1
                || h.tick.streams.len() != 1
                || h.tick.driver.sounds.len() != 1
            {
                return Err("Stream must own exactly one prepared driver".into());
            }
            let b = &h.tick.driver.buffers[0];
            let sound = &h.tick.driver.sounds[0];
            let own = h.uploads.iter().map(|u| u.buffer).collect::<BTreeSet<_>>();
            if b.handle != s.buffer
                || b.count_word != 4
                || b.storage.len() != 4
                || b.storage.iter().copied().collect::<BTreeSet<_>>() != own
                || own != s.tokens.buffers.into_iter().collect()
                || own.len() != 4
                || h.tick.driver.node.buffer != s.buffer
                || h.tick.driver.node.source.sound != s.sound
                || !h.tick.driver.node.source.allocated
                || h.tick.driver.node.source.flags & 4 == 0
                || sound.handle != s.sound
                || sound.flags & 2 == 0
                || h.tick.streams[0].handle != s.tokens.stream
                || h.worker.as_ref().is_some_and(|w| {
                    w.handle != s.tokens.stream
                        || w.decoder != s.tokens.decoder
                        || w.critical != s.tokens.critical
                        || w.kind != i32::from(sound.flags & 1 != 0)
                })
                || (h.worker.is_some()
                    && (h.tick.progress[0].staging != s.tokens.staging
                        || sound.native_handle as u32 != s.buffer))
                || (h.worker.is_none() && sound.native_handle != 0)
            {
                return Err("Stream registration/driver mismatch".into());
            }
            for id in own {
                if id == 0 || !al.insert(id) {
                    return Err("AL buffer is owned by multiple registrations".into());
                }
            }
            bytes = bytes.saturating_add(s.host.owned_bytes());
        }
        if bytes > MAX_BYTES {
            return Err("Combined audio resource byte budget exceeded".into());
        }
        Ok(())
    }
    pub fn validate(&self) -> Result<(), String> {
        Self::check(
            &self.bank,
            &self.voices,
            &self.streams,
            self.cursor,
            self.closed,
        )
    }
    pub fn register_pcm(
        &mut self,
        wave: &[u8],
        r: &pcm::Registration,
    ) -> Result<Registered, String> {
        self.open()?;
        if self.streams.iter().any(|s| s.sound == r.sound) {
            return Err("Sound is already registered as a stream".into());
        }
        if !self.bank.entries.iter().any(|e| e.sound == r.sound)
            && self.streams.iter().any(|s| {
                s.host
                    .state()
                    .uploads
                    .iter()
                    .any(|u| u.buffer == r.generated)
            })
        {
            return Err("Generated PCM ID already belongs to a stream".into());
        }
        let mut bank = self.bank.clone();
        let out = bank.register(wave, r)?;
        Self::check(&bank, &self.voices, &self.streams, self.cursor, false)?;
        self.bank = bank;
        Ok(Registered {
            handle: out.handle,
            source: None,
            cached: out.cached,
            uploads: vec![],
            events: out.events.into_iter().map(Event::Pcm).collect(),
        })
    }
    pub fn attach_static(
        &mut self,
        key: u32,
        node: Node,
        mut sound: driver::Sound,
    ) -> Result<Vec<Event>, String> {
        self.open()?;
        if key == 0
            || self.voices.iter().any(|v| v.key == key)
            || self.streams.iter().any(|s| s.key == key)
        {
            return Err("Source key already occupied".into());
        }
        let entry = self
            .bank
            .entries
            .iter()
            .find(|e| e.sound == node.source.sound)
            .ok_or("No static sound registration")?;
        if !node.source.allocated
            || node.buffer != entry.buffer.handle
            || sound.handle != entry.sound
        {
            return Err("Static source is not prepared for this registration".into());
        }
        sound.native_handle = entry.buffer.handle as i32;
        let v = Voice {
            key,
            sound: entry.sound,
            driver: driver::Session {
                node,
                buffers: vec![driver::Buffer {
                    handle: entry.buffer.handle,
                    stream_token: 0,
                    count_word: 1,
                    storage: entry.buffer.storage.clone(),
                }],
                sounds: vec![sound],
            },
        };
        let event = Event::Driver {
            source: key,
            event: driver::Event::Integer {
                hardware: v.driver.node.hardware,
                parameter: 0x1009,
                value: entry.buffer.storage[0],
            },
        };
        let mut voices = self.voices.clone();
        voices.push(v);
        let mut bank = self.bank.clone();
        bank.sources = Self::bindings(&voices, &self.streams);
        Self::check(&bank, &voices, &self.streams, self.cursor, false)?;
        self.bank = bank;
        self.voices = voices;
        Ok(vec![
            Event::Driver {
                source: key,
                event: driver::Event::Integer {
                    hardware: self
                        .voices
                        .last()
                        .ok_or("Missing attached voice")?
                        .driver
                        .node
                        .hardware,
                    parameter: 0x1009,
                    value: 0,
                },
            },
            event,
        ])
    }
    pub fn control_static(
        &mut self,
        key: u32,
        command: &driver::Command,
        reply: &driver::Reply,
    ) -> Result<driver::Outcome, String> {
        self.open()?;
        let mut voices = self.voices.clone();
        let v = voices
            .iter_mut()
            .find(|v| v.key == key)
            .ok_or("Unknown static voice")?;
        let out = v.driver.execute(command, reply)?;
        let mut bank = self.bank.clone();
        bank.sources = Self::bindings(&voices, &self.streams);
        Self::check(&bank, &voices, &self.streams, self.cursor, false)?;
        self.bank = bank;
        self.voices = voices;
        Ok(out)
    }
    pub fn detach_static(&mut self, key: u32) -> Result<Vec<Event>, String> {
        self.open()?;
        let mut voices = self.voices.clone();
        let i = voices
            .iter()
            .position(|v| v.key == key)
            .ok_or("Unknown static voice")?;
        let out = voices[i].driver.execute(
            &driver::Command::Stop,
            &driver::Reply {
                source_state: 0,
                unqueued: [0; 4],
            },
        )?;
        voices.remove(i);
        let mut bank = self.bank.clone();
        bank.sources = Self::bindings(&voices, &self.streams);
        Self::check(&bank, &voices, &self.streams, self.cursor, false)?;
        self.bank = bank;
        self.voices = voices;
        Ok(out
            .events
            .into_iter()
            .map(|event| Event::Driver { source: key, event })
            .collect())
    }
    pub fn register_stream(
        &mut self,
        bytes: &[u8],
        tick: tick::Session,
        r: &StreamRegistration,
    ) -> Result<Registered, String> {
        self.open()?;
        let sound = tick.driver.node.source.sound;
        if self.bank.entries.iter().any(|e| e.sound == sound) {
            return Err("Sound is already registered as static PCM".into());
        }
        if let Some(s) = self.streams.iter().find(|s| s.sound == sound) {
            if s.host.state().worker.as_ref().is_none_or(|w| w.ended != 0) {
                return Err("Ended stream must be unregistered before reopening".into());
            }
            return Ok(Registered {
                handle: s.buffer,
                source: Some(s.key),
                cached: true,
                uploads: vec![],
                events: vec![],
            });
        }
        let metadata = tick
            .driver
            .sounds
            .iter()
            .find(|s| s.handle == sound)
            .ok_or("Missing stream sound")?;
        if metadata.flags & 2 == 0 || (metadata.flags & 1 != 0) != r.looping {
            return Err("Stream flags and loop request disagree".into());
        }
        let buffer = tick.driver.node.buffer;
        let host = vorbis::Session::register(
            tick,
            Provider::new(bytes.to_vec())?,
            r.tokens.clone(),
            r.looping,
        )?;
        let uploads = host.state().uploads.clone();
        let source = r.key;
        self.streams.push(Stream {
            key: source,
            sound,
            buffer,
            allocation: r.allocation,
            tokens: r.tokens.clone(),
            host,
        });
        let mut bank = self.bank.clone();
        bank.sources = Self::bindings(&self.voices, &self.streams);
        if let Err(e) = Self::check(&bank, &self.voices, &self.streams, self.cursor, false) {
            self.streams.pop();
            return Err(e);
        }
        self.bank = bank;
        let hardware = self
            .streams
            .last()
            .ok_or("Missing registered stream")?
            .host
            .state()
            .tick
            .driver
            .node
            .hardware;
        let events = vec![
            Event::Driver {
                source,
                event: driver::Event::Integer {
                    hardware,
                    parameter: 0x1009,
                    value: 0,
                },
            },
            Event::Parameter {
                source,
                event: crate::audio_upload::Event::Queue {
                    hardware,
                    buffers: r.tokens.buffers.to_vec(),
                },
            },
            Event::Driver {
                source,
                event: driver::Event::Integer {
                    hardware,
                    parameter: 0x1007,
                    value: 0,
                },
            },
        ];
        Ok(Registered {
            handle: buffer,
            source: Some(source),
            cached: false,
            uploads,
            events,
        })
    }
    pub fn control_stream(
        &mut self,
        key: u32,
        command: &driver::Command,
        reply: &driver::Reply,
    ) -> Result<driver::Outcome, String> {
        self.open()?;
        self.streams
            .iter_mut()
            .find(|s| s.key == key)
            .ok_or("Unknown stream source")?
            .host
            .control(command, reply)
    }
    pub fn tick_stream(
        &mut self,
        key: u32,
        step: &bridge::Tick,
    ) -> Result<bridge::Outcome, String> {
        self.open()?;
        let s = self
            .streams
            .iter_mut()
            .find(|s| s.key == key)
            .ok_or("Unknown stream source")?;
        let h = s.host.state();
        if step.al.processed < 0
            || step.al.queued < 0
            || step.al.processed > 4096
            || step.al.queued > 4096
        {
            return Err("Invalid AL queue counts".into());
        }
        if h.tick.driver.node.source.requested_pause == 0
            && h.worker.as_ref().is_some_and(|w| w.requested == 0)
            && step.al.processed != 0
        {
            let expected = h.tick.driver.buffers[0].storage[h.tick.progress[0].indices[1] as usize];
            if step.al.unqueued != expected {
                return Err("AL unqueue disagrees with owned ring order".into());
            }
        }
        s.host.execute(&bridge::Command::Tick(step.clone()))
    }
    /// At most one chunk per ready stream and at most 64 chunks per call.
    /// No timers/threads. Normal service does not replay already decoded audio.
    pub fn service_round(&mut self, budget: usize) -> Result<Vec<Served>, String> {
        self.open()?;
        if budget > 64 {
            return Err("Stream round budget exceeds 64".into());
        }
        if self.streams.is_empty() || budget == 0 {
            return Ok(vec![]);
        }
        let mut selected = vec![];
        let mut cursor = self.cursor;
        for _ in 0..self.streams.len() {
            let i = cursor;
            cursor = (cursor + 1) % self.streams.len();
            if self.streams[i]
                .host
                .state()
                .worker
                .as_ref()
                .is_some_and(|w| w.requested > 0)
            {
                selected.push(i);
                if selected.len() == budget {
                    break;
                }
            }
        }
        let mut undo = vec![];
        for &i in &selected {
            undo.push((i, self.streams[i].host.saved()?));
        }
        let mut outcomes = vec![];
        for &i in &selected {
            match self.streams[i].host.execute(&bridge::Command::Service) {
                Ok(outcome) => outcomes.push(Served {
                    source: self.streams[i].key,
                    outcome,
                }),
                Err(e) => {
                    let restored = undo
                        .into_iter()
                        .map(|(i, s)| Ok((i, vorbis::Session::restore_saved(s)?)))
                        .collect::<Result<Vec<_>, String>>()?;
                    for (i, h) in restored {
                        self.streams[i].host = h;
                    }
                    return Err(e);
                }
            }
        }
        self.cursor = cursor;
        Ok(outcomes)
    }
    pub fn unregister(
        &mut self,
        sound: u32,
        reply: Option<&driver::Reply>,
    ) -> Result<Vec<Event>, String> {
        self.open()?;
        if let Some(i) = self.streams.iter().position(|s| s.sound == sound) {
            let s = &mut self.streams[i];
            let key = s.key;
            let events = s.host.dispose(
                &vorbis::DisposalTokens {
                    owner: self.bank.owner,
                    allocation: s.allocation,
                    source: key,
                },
                reply.ok_or("Stream disposal needs an AL reply")?,
            )?;
            self.streams.remove(i);
            if self.streams.is_empty() {
                self.cursor = 0;
            } else if i < self.cursor {
                self.cursor -= 1;
            } else {
                self.cursor %= self.streams.len();
            }
            self.bank.sources = Self::bindings(&self.voices, &self.streams);
            return Ok(events
                .into_iter()
                .map(|event| Event::StreamDispose { source: key, event })
                .collect());
        }
        let mut bank = self.bank.clone();
        let mut voices = self.voices.clone();
        let mut stops = vec![];
        let mut drivers = vec![];
        if let Some(e) = bank.entries.iter().find(|e| e.sound == sound) {
            for v in voices
                .iter_mut()
                .filter(|v| v.driver.node.buffer == e.buffer.handle)
            {
                let out = v.driver.execute(
                    &driver::Command::Stop,
                    &driver::Reply {
                        source_state: 0,
                        unqueued: [0; 4],
                    },
                )?;
                stops.push(raw::StopReply {
                    detached: v.driver.node.buffer == 0,
                    storage: None,
                });
                drivers.push((v.key, out.events));
            }
        }
        let outcome = bank.unregister(
            sound,
            &raw::Reply {
                stops,
                reallocated: 0,
            },
        )?;
        let mut events = vec![];
        for e in outcome.events {
            let stop = if let pcm::Event::Cleanup(raw::Event::Stop { source }) = &e {
                Some(*source)
            } else {
                None
            };
            events.push(Event::Pcm(e));
            if let Some(key) = stop {
                let d = drivers
                    .iter()
                    .find(|(k, _)| *k == key)
                    .ok_or("Missing stopped static driver")?;
                events.extend(
                    d.1.iter()
                        .cloned()
                        .map(|event| Event::Driver { source: key, event }),
                );
            }
        }
        voices.retain(|v| v.sound != sound);
        bank.sources = Self::bindings(&voices, &self.streams);
        Self::check(&bank, &voices, &self.streams, self.cursor, false)?;
        self.bank = bank;
        self.voices = voices;
        Ok(events)
    }
    pub fn shutdown(&mut self, replies: &[StreamReply]) -> Result<Vec<Event>, String> {
        self.validate()?;
        if replies.len() != self.streams.len()
            || replies
                .iter()
                .map(|r| r.source)
                .collect::<BTreeSet<_>>()
                .len()
                != replies.len()
            || replies
                .iter()
                .any(|r| !self.streams.iter().any(|s| s.key == r.source))
        {
            return Err("Shutdown needs exactly one reply per stream".into());
        }
        if self.closed {
            return Ok(vec![]);
        }
        // Rare all-resource transaction; decode replay is deliberately outside
        // a realtime callback. No state or request prefix escapes on failure.
        let mut next = Self::restore_state(self.saved()?)?;
        let mut events = vec![];
        let sounds = next
            .bank
            .entries
            .iter()
            .map(|e| e.sound)
            .collect::<Vec<_>>();
        for sound in sounds {
            events.extend(next.unregister(sound, None)?);
        }
        let streams = next
            .streams
            .iter()
            .map(|s| (s.sound, s.key))
            .collect::<Vec<_>>();
        for (sound, key) in streams {
            let r = replies
                .iter()
                .find(|r| r.source == key)
                .ok_or("Missing shutdown reply")?;
            events.extend(next.unregister(sound, Some(&r.reply))?);
        }
        next.closed = true;
        next.validate()?;
        *self = next;
        Ok(events)
    }
    pub(crate) fn saved(&self) -> Result<Saved, String> {
        self.validate()?;
        let streams = self
            .streams
            .iter()
            .map(|s| {
                Ok(SavedStream {
                    key: s.key,
                    sound: s.sound,
                    buffer: s.buffer,
                    allocation: s.allocation,
                    tokens: s.tokens.clone(),
                    host: s.host.saved()?,
                })
            })
            .collect::<Result<Vec<_>, String>>()?;
        Ok(Saved {
            schema: 1,
            bank: self.bank.clone(),
            voices: self.voices.clone(),
            streams,
            cursor: self.cursor,
            closed: self.closed,
        })
    }
    pub fn checkpoint(&self) -> Result<Vec<u8>, String> {
        let b = serde_json::to_vec(&self.saved()?).map_err(|e| e.to_string())?;
        if b.len() > MAX_BYTES {
            return Err("Resource host snapshot too large".into());
        }
        Ok(b)
    }
    pub fn restore(bytes: &[u8]) -> Result<Self, String> {
        if bytes.len() > MAX_BYTES {
            return Err("Resource host snapshot too large".into());
        }
        let saved: Saved = serde_json::from_slice(bytes).map_err(|e| e.to_string())?;
        Self::restore_state(saved)
    }
    pub(crate) fn restore_state(saved: Saved) -> Result<Self, String> {
        if saved.schema != 1 {
            return Err("Unsupported resource host schema".into());
        }
        let streams = saved
            .streams
            .into_iter()
            .map(|s| {
                Ok(Stream {
                    key: s.key,
                    sound: s.sound,
                    buffer: s.buffer,
                    allocation: s.allocation,
                    tokens: s.tokens,
                    host: vorbis::Session::restore_saved(s.host)?,
                })
            })
            .collect::<Result<Vec<_>, String>>()?;
        let h = Self {
            bank: saved.bank,
            voices: saved.voices,
            streams,
            cursor: saved.cursor,
            closed: saved.closed,
        };
        h.validate()?;
        Ok(h)
    }
    pub fn load(&mut self, bytes: &[u8]) -> Result<(), String> {
        let next = Self::restore(bytes)?;
        *self = next;
        Ok(())
    }
}
