//! Portable compressed-file owner over the original PCM worker and driver tick.
//! Caller supplies AL buffer IDs; uploads are receipts, not device operations.
use crate::{
    audio_source_tick as tick, audio_stream_bridge as bridge, audio_stream_data as data,
    audio_vorbis::{Metadata, Provider, CHUNK_BYTES},
};
use serde::{Deserialize, Serialize};
#[derive(Clone, Debug, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Tokens {
    pub stream: u32,
    pub critical: u32,
    pub decoder: u32,
    pub staging: u32,
    pub buffers: [u32; 4],
}
pub struct Session {
    bridge: bridge::Session,
    decoder: Option<Provider>,
}
#[derive(Clone, Debug, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct DisposalTokens {
    pub owner: u32,
    pub allocation: u32,
    pub source: u32,
}
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub enum DisposalEvent {
    Buffer(crate::audio_buffer::Event),
    Driver(crate::audio_driver::Event),
}
#[derive(Clone, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub(crate) struct Saved {
    schema: u32,
    bridge: bridge::Session,
    decoder: Option<crate::audio_vorbis::Saved>,
}
impl Session {
    pub(crate) fn replace_source(
        &mut self,
        source: crate::audio_source::Source,
    ) -> Result<(), String> {
        let old = std::mem::replace(&mut self.bridge.tick.driver.node.source, source);
        if let Err(e) = self.validate() {
            self.bridge.tick.driver.node.source = old;
            return Err(e);
        }
        Ok(())
    }
    pub(crate) fn owned_bytes(&self) -> usize {
        self.decoder.as_ref().map_or(0, Provider::encoded_len)
            + self
                .bridge
                .uploads
                .iter()
                .map(|u| u.bytes.len())
                .sum::<usize>()
            + self
                .bridge
                .heap
                .regions
                .iter()
                .map(|r| r.bytes.len())
                .sum::<usize>()
    }
    /// Stop requires dispose, so that queued buffers and decoder remain owned.
    pub fn control(
        &mut self,
        command: &crate::audio_driver::Command,
        reply: &crate::audio_driver::Reply,
    ) -> Result<crate::audio_driver::Outcome, String> {
        if matches!(command, crate::audio_driver::Command::Stop) {
            return Err("Use stream disposal for Stop".into());
        }
        self.validate()?;
        let before = self.bridge.tick.driver.clone();
        let out = self.bridge.tick.driver.execute(command, reply)?;
        if let Err(e) = self.validate() {
            self.bridge.tick.driver = before;
            return Err(e);
        }
        Ok(out)
    }
    pub(crate) fn saved(&self) -> Result<Saved, String> {
        self.validate()?;
        Ok(Saved {
            schema: 1,
            bridge: self.bridge.clone(),
            decoder: self.decoder.as_ref().map(Provider::saved),
        })
    }
    pub(crate) fn restore_saved(saved: Saved) -> Result<Self, String> {
        if saved.schema != 1 {
            return Err("Unsupported Vorbis host schema".into());
        }
        let next = Self {
            bridge: saved.bridge,
            decoder: saved.decoder.map(Provider::restore_saved).transpose()?,
        };
        next.validate()?;
        Ok(next)
    }
    /// Explicit unregister: caller supplies opaque subsystem/list/source tokens
    /// and actual AL unqueue replies. Returns requests; performs no device IO.
    pub fn dispose(
        &mut self,
        tokens: &DisposalTokens,
        reply: &crate::audio_driver::Reply,
    ) -> Result<Vec<DisposalEvent>, String> {
        use crate::{audio_buffer as raw, audio_driver as driver};
        self.validate()?;
        if self.bridge.uploads.is_empty() && self.bridge.tick.driver.buffers.is_empty() {
            return Ok(vec![]);
        }
        if tokens.allocation == 0 {
            return Err("Missing buffer array allocation token".into());
        }
        let mut next = self.bridge.clone();
        if next.tick.driver.buffers.len() != 1 || next.tick.progress.len() != 1 {
            return Err("Disposal requires a single owned stream registration".into());
        }
        let b = next.tick.driver.buffers[0].clone();
        let p = next.tick.progress[0].clone();
        if b.handle != p.buffer
            || (next.tick.driver.node.buffer != 0 && next.tick.driver.node.buffer != b.handle)
        {
            return Err("Disposal buffer ownership mismatch".into());
        }
        let sound = next
            .tick
            .driver
            .sounds
            .iter()
            .find(|s| s.handle == next.tick.driver.node.source.sound)
            .ok_or("Missing disposed sound")?
            .handle;
        let bound = next.tick.driver.node.buffer == b.handle;
        let owned = next
            .uploads
            .iter()
            .map(|u| u.buffer)
            .collect::<std::collections::BTreeSet<_>>();
        let deleting = b
            .storage
            .iter()
            .take((b.count_word & 0x1fffffff) as usize)
            .copied()
            .collect::<std::collections::BTreeSet<_>>();
        if owned != deleting {
            return Err("Disposal would orphan an owned AL buffer".into());
        }
        let mut raw = raw::Session {
            buffer: raw::Buffer {
                handle: b.handle,
                owner: tokens.owner,
                stream: b.stream_token,
                staging: p.staging,
                sound,
                allocation: tokens.allocation,
                count_word: b.count_word,
                storage: b.storage.clone(),
                indices: p.indices,
            },
            sources: vec![raw::Source {
                handle: tokens.source,
                buffer: if bound { b.handle } else { 0 },
            }],
        };
        raw.validate()?;
        let stopped = if bound && b.count_word & 0x1fffffff != 0 {
            Some(next.tick.driver.execute(&driver::Command::Stop, reply)?)
        } else {
            None
        };
        if owned
            != next.tick.driver.buffers[0]
                .storage
                .iter()
                .take((b.count_word & 0x1fffffff) as usize)
                .copied()
                .collect()
        {
            return Err("Unqueue reply lost owned AL buffers".into());
        }
        let stop_replies = if stopped.is_some() {
            vec![raw::StopReply {
                detached: next.tick.driver.node.buffer == 0,
                storage: Some(next.tick.driver.buffers[0].storage.clone()),
            }]
        } else {
            vec![]
        };
        let events = raw.release(
            &raw::Reply {
                stops: stop_replies,
                reallocated: 0,
            },
            true,
        )?;
        let mut out = vec![];
        for e in events {
            let stop = matches!(e, raw::Event::Stop { .. });
            out.push(DisposalEvent::Buffer(e));
            if stop {
                if let Some(s) = &stopped {
                    out.extend(s.events.iter().cloned().map(DisposalEvent::Driver));
                }
            }
        }
        out.push(DisposalEvent::Buffer(raw::Event::Free {
            allocation: b.handle,
        }));
        next.uploads.clear();
        next.heap.regions.clear();
        next.worker = None;
        next.pcm = None;
        next.tick.driver.buffers.clear();
        next.tick.progress.clear();
        next.tick.streams.clear();
        next.tick.driver.node.buffer = 0;
        for s in &mut next.tick.driver.sounds {
            if s.handle == sound {
                s.native_handle = 0;
            }
        }
        next.validate()?;
        self.bridge = next;
        self.decoder = None;
        self.validate()?;
        Ok(out)
    }
    /// Four 64-KiB prefill uploads and one outstanding staging request.
    /// This composes inspected registration rules; it is not all of Precache.
    pub fn register(
        mut tick: tick::Session,
        mut decoder: Provider,
        tokens: Tokens,
        looping: bool,
    ) -> Result<Self, String> {
        tick.validate()?;
        if tokens.stream == 0
            || tokens.decoder == 0
            || tokens.staging == 0
            || tokens.buffers.contains(&0)
            || tokens
                .buffers
                .iter()
                .collect::<std::collections::BTreeSet<_>>()
                .len()
                != 4
        {
            return Err("Invalid Vorbis resource tokens".into());
        }
        if tick.driver.node.source.flags & 4 == 0 {
            return Err("Source is not streaming".into());
        }
        let meta = decoder.metadata().clone();
        let buffer = tick
            .driver
            .buffers
            .iter_mut()
            .find(|b| b.handle == tick.driver.node.buffer)
            .ok_or("Missing registration buffer")?;
        if buffer.stream_token != 0 {
            return Err("Registration buffer already has a stream".into());
        }
        buffer.stream_token = tokens.stream;
        buffer.count_word = 4;
        buffer.storage = tokens.buffers.to_vec();
        let buffer_handle = buffer.handle;
        let sound = tick
            .driver
            .sounds
            .iter_mut()
            .find(|s| s.handle == tick.driver.node.source.sound)
            .ok_or("Missing registration sound")?;
        sound.duration_bits = meta.duration().to_bits();
        sound.native_handle = buffer_handle as i32;
        // Registration input is fresh. Reject hidden preexisting stream ownership.
        if !tick.progress.is_empty() || !tick.streams.is_empty() {
            return Err("Registration requires fresh tick resources".into());
        }
        tick.progress.push(tick::Progress {
            buffer: buffer_handle,
            staging: tokens.staging,
            indices: [0, 0],
        });
        tick.streams.push(tick::Stream {
            handle: tokens.stream,
            chunk_bytes: CHUNK_BYTES as u32,
            format: meta.format,
            frequency: meta.frequency,
        });
        let mut worker = data::Session {
            state: data::State {
                handle: tokens.stream,
                critical: tokens.critical,
                decoder: tokens.decoder,
                cursor: tokens.staging,
                chunk_bytes: CHUNK_BYTES as i32,
                requested: 4,
                loops: 0,
                bitstream: 0,
                ended: 0,
                kind: i32::from(looping),
            },
            heap: data::Heap {
                regions: vec![data::Region {
                    base: tokens.staging,
                    bytes: vec![0; 4 * CHUNK_BYTES],
                }],
            },
        };
        worker.validate()?;
        worker.fill(4, &mut decoder)?;
        let mut uploads = vec![];
        for (i, buffer) in tokens.buffers.into_iter().enumerate() {
            uploads.push(bridge::Upload {
                buffer,
                format: meta.format,
                frequency: meta.frequency,
                bytes: worker.heap.regions[0].bytes[i * CHUNK_BYTES..(i + 1) * CHUNK_BYTES]
                    .to_vec(),
            });
        }
        worker.heap.regions[0].bytes = vec![0; CHUNK_BYTES];
        worker.execute(&data::Action::Request {
            chunks: 1,
            destination: tokens.staging,
        })?;
        // PCM is an empty compatibility marker; the private host exclusively
        // services the real decoder. It is never passed to bridge::Service.
        let bridge = bridge::Session {
            tick,
            worker: Some(worker.state),
            pcm: Some(data::Pcm {
                bytes: vec![],
                cursor: 0,
                max_read: CHUNK_BYTES,
                bitstream: 0,
            }),
            heap: worker.heap,
            uploads,
        };
        let result = Self {
            bridge,
            decoder: Some(decoder),
        };
        result.validate()?;
        Ok(result)
    }
    pub fn state(&self) -> &bridge::Session {
        &self.bridge
    }
    pub fn metadata(&self) -> Option<&Metadata> {
        self.decoder.as_ref().map(Provider::metadata)
    }
    pub fn decoded_cursor(&self) -> Option<u64> {
        self.decoder.as_ref().map(Provider::cursor)
    }
    fn validate(&self) -> Result<(), String> {
        self.bridge.validate()?;
        match (&self.decoder, &self.bridge.worker, &self.bridge.pcm) {
            (Some(d), Some(w), Some(p)) => {
                if !p.bytes.is_empty()
                    || p.cursor != 0
                    || p.max_read != CHUNK_BYTES
                    || p.bitstream != 0
                    || !(0..=1).contains(&w.kind)
                    || w.chunk_bytes != CHUNK_BYTES as i32
                    || w.requested > 1
                    || w.decoder == 0
                {
                    return Err("Invalid external Vorbis worker".into());
                }
                let s = self
                    .bridge
                    .tick
                    .streams
                    .iter()
                    .find(|s| s.handle == w.handle)
                    .ok_or("Missing Vorbis metadata")?;
                if s.format != d.metadata().format || s.frequency != d.metadata().frequency {
                    return Err("Vorbis/driver format mismatch".into());
                }
                let owned = self
                    .bridge
                    .uploads
                    .iter()
                    .map(|u| u.bytes.len())
                    .sum::<usize>()
                    + self
                        .bridge
                        .heap
                        .regions
                        .iter()
                        .map(|r| r.bytes.len())
                        .sum::<usize>();
                if owned.saturating_add(d.encoded_len()) > data::MAX_BYTES {
                    return Err("Vorbis host byte budget exceeded".into());
                }
            }
            (None, None, None) => {}
            _ => return Err("Vorbis worker/decoder lifetime mismatch".into()),
        }
        Ok(())
    }
    pub fn execute(&mut self, command: &bridge::Command) -> Result<bridge::Outcome, String> {
        self.validate()?;
        let before = self.bridge.clone();
        let cursor = self.decoder.as_ref().map(Provider::cursor);
        let result = (|| {
            let out = match command {
                bridge::Command::Service => {
                    let mut worker = data::Session {
                        state: self
                            .bridge
                            .worker
                            .as_ref()
                            .ok_or("No live Vorbis worker")?
                            .clone(),
                        heap: self.bridge.heap.clone(),
                    };
                    let out =
                        worker.fill(1, self.decoder.as_mut().ok_or("No live Vorbis decoder")?)?;
                    worker.validate()?;
                    self.bridge.worker = Some(worker.state);
                    self.bridge.heap = worker.heap;
                    bridge::Outcome {
                        driver: None,
                        worker: vec![out],
                        uploads: vec![],
                    }
                }
                bridge::Command::Tick(_) => self.bridge.execute(command)?,
            };
            // Defer decoder destruction until successful bridge validation.
            if self.bridge.worker.is_some() {
                self.validate()?;
            } else {
                self.bridge.validate()?;
            }
            Ok(out)
        })();
        match result {
            Ok(out) => {
                if self.bridge.worker.is_none() {
                    self.decoder = None;
                }
                Ok(out)
            }
            Err(error) => {
                self.bridge = before;
                if let (Some(d), Some(at)) = (&mut self.decoder, cursor) {
                    d.seek_byte(at)?;
                }
                Err(error)
            }
        }
    }
    pub fn checkpoint(&self) -> Result<Vec<u8>, String> {
        self.validate()?;
        let out = serde_json::to_vec(&Saved {
            schema: 1,
            bridge: self.bridge.clone(),
            decoder: self.decoder.as_ref().map(Provider::saved),
        })
        .map_err(|e| e.to_string())?;
        if out.len() > data::MAX_BYTES {
            return Err("Vorbis host snapshot budget exceeded".into());
        }
        Ok(out)
    }
    pub fn restore(bytes: &[u8]) -> Result<Self, String> {
        if bytes.len() > data::MAX_BYTES {
            return Err("Vorbis host snapshot budget exceeded".into());
        }
        let saved: Saved = serde_json::from_slice(bytes).map_err(|e| e.to_string())?;
        if saved.schema != 1 {
            return Err("Unsupported Vorbis host snapshot schema".into());
        }
        let next = Self {
            bridge: saved.bridge,
            decoder: saved.decoder.map(Provider::restore_saved).transpose()?,
        };
        next.validate()?;
        Ok(next)
    }
}
