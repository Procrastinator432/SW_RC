//! Byte-owning adapter between decoded PCM service and the original source tick.
//! AL status/unqueue answers remain external. No audio device or Vorbis decoder.
use crate::{audio_source_tick as tick, audio_stream_data as data, audio_upload::Environment};
use serde::{Deserialize, Serialize};
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Upload {
    pub buffer: u32,
    pub format: u32,
    pub frequency: u32,
    pub bytes: Vec<u8>,
}
#[derive(Clone, Debug, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Session {
    pub tick: tick::Session,
    pub worker: Option<data::State>,
    pub pcm: Option<data::Pcm>,
    pub heap: data::Heap,
    pub uploads: Vec<Upload>,
}
#[derive(Clone, Debug, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct AlReply {
    pub state: i32,
    pub processed: i32,
    pub queued: i32,
    pub unqueued: u32,
}
#[derive(Clone, Debug, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Tick {
    pub environment: Environment,
    pub al: AlReply,
    pub delta_bits: u32,
}
#[derive(Clone, Debug, Serialize, Deserialize)]
pub enum Command {
    Service,
    Tick(Tick),
}
#[derive(Clone, Debug, Default, PartialEq, Eq, Serialize, Deserialize)]
pub struct Outcome {
    pub driver: Option<tick::Outcome>,
    pub worker: Vec<data::Outcome>,
    pub uploads: Vec<Upload>,
}
#[derive(Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
struct Snapshot {
    schema: u32,
    state: Session,
}
impl Session {
    pub fn validate(&self) -> Result<(), String> {
        self.tick.validate()?;
        self.heap.validate()?;
        if self.uploads.len() > 4096 {
            return Err("Too many owned AL buffers".into());
        }
        let mut ids = std::collections::BTreeSet::new();
        let mut bytes = self
            .heap
            .regions
            .iter()
            .map(|r| r.bytes.len())
            .fold(0usize, usize::saturating_add);
        for u in &self.uploads {
            if u.buffer == 0 || !ids.insert(u.buffer) || u.bytes.len() > data::MAX_BYTES {
                return Err("Invalid owned buffer".into());
            }
            bytes = bytes.saturating_add(u.bytes.len());
        }
        match (&self.worker, &self.pcm) {
            (Some(w), Some(p)) => {
                w.validate()?;
                p.validate()?;
                bytes = bytes.saturating_add(p.bytes.len());
                let b = self
                    .tick
                    .driver
                    .buffers
                    .iter()
                    .find(|b| b.handle == self.tick.driver.node.buffer)
                    .ok_or("Bridge buffer missing")?;
                let progress = self
                    .tick
                    .progress
                    .iter()
                    .find(|p| p.buffer == b.handle)
                    .ok_or("Bridge progress missing")?;
                let stream = self
                    .tick
                    .streams
                    .iter()
                    .find(|s| s.handle == w.handle)
                    .ok_or("Bridge stream metadata missing")?;
                if w.handle != b.stream_token
                    || w.chunk_bytes < 0
                    || w.chunk_bytes as u32 != stream.chunk_bytes
                {
                    return Err("Worker/driver stream mismatch".into());
                }
                if !self.heap.regions.iter().any(|r| r.base == progress.staging) || w.requested < 0
                {
                    return Err("Invalid owned stream allocation".into());
                }
                self.heap
                    .read(progress.staging, stream.chunk_bytes as usize)?;
                if w.requested > 0 {
                    self.heap.read(w.cursor, stream.chunk_bytes as usize)?;
                }
            }
            (None, None) => {
                if self
                    .tick
                    .driver
                    .buffers
                    .iter()
                    .any(|b| b.handle == self.tick.driver.node.buffer && b.stream_token != 0)
                {
                    return Err("Live stream lacks worker".into());
                }
            }
            _ => return Err("PCM provider/worker mismatch".into()),
        }
        if bytes > data::MAX_BYTES {
            return Err("Owned PCM byte budget exceeded".into());
        }
        Ok(())
    }
    pub fn checkpoint(&self) -> Result<Vec<u8>, String> {
        self.validate()?;
        let b = serde_json::to_vec(&Snapshot {
            schema: 1,
            state: self.clone(),
        })
        .map_err(|e| e.to_string())?;
        if b.len() > data::MAX_BYTES {
            return Err("Bridge snapshot too large".into());
        }
        Ok(b)
    }
    pub fn load(&mut self, bytes: &[u8]) -> Result<(), String> {
        if bytes.len() > data::MAX_BYTES {
            return Err("Bridge snapshot too large".into());
        }
        let s: Snapshot = serde_json::from_slice(bytes).map_err(|e| e.to_string())?;
        if s.schema != 1 {
            return Err("Unsupported PCM bridge schema".into());
        }
        s.state.validate()?;
        *self = s.state;
        Ok(())
    }
    pub fn execute(&mut self, command: &Command) -> Result<Outcome, String> {
        Ok(self.batch(std::slice::from_ref(command))?.remove(0))
    }
    pub fn batch(&mut self, commands: &[Command]) -> Result<Vec<Outcome>, String> {
        self.validate()?;
        if commands.len() > 4096 {
            return Err("Bridge command batch too large".into());
        }
        let mut next = self.clone();
        let mut out = vec![];
        for c in commands {
            out.push(next.apply(c)?);
            next.validate()?;
        }
        *self = next;
        Ok(out)
    }
    fn worker_action(&mut self, action: &data::Action) -> Result<data::Outcome, String> {
        let state = self.worker.as_ref().ok_or("No active PCM worker")?.clone();
        let mut worker = data::Session {
            state,
            heap: self.heap.clone(),
        };
        let out = worker.execute(action)?;
        self.worker = Some(worker.state);
        self.heap = worker.heap;
        Ok(out)
    }
    fn apply(&mut self, command: &Command) -> Result<Outcome, String> {
        let mut result = Outcome::default();
        match command {
            Command::Service => {
                let state = self.worker.as_ref().ok_or("No active PCM worker")?.clone();
                let pcm = self.pcm.as_mut().ok_or("No decoded PCM provider")?;
                let mut worker = data::Session {
                    state,
                    heap: self.heap.clone(),
                };
                result.worker.push(worker.service_pcm(1, pcm)?);
                self.worker = Some(worker.state);
                self.heap = worker.heap;
            }
            Command::Tick(step) => {
                let (available, busy) = self
                    .worker
                    .as_ref()
                    .map_or((1, 0), |w| (i32::from(w.ended == 0), w.requested));
                let outcome = self.tick.execute(&tick::Step {
                    environment: step.environment.clone(),
                    delta_bits: step.delta_bits,
                    reply: tick::Reply {
                        state: step.al.state,
                        processed: step.al.processed,
                        queued: step.al.queued,
                        unqueued: step.al.unqueued,
                        available,
                        busy,
                    },
                })?;
                for e in &outcome.events {
                    match e {
                        tick::Event::Stream(tick::StreamEvent::QueryStream { stream }) => {
                            if self.worker.as_ref().map(|w| w.handle) != Some(*stream) {
                                return Err("Unknown worker query".into());
                            }
                            let q = self.worker_action(&data::Action::Query)?;
                            if q.result != Some(available) || q.busy != Some(busy) {
                                return Err("Worker reply changed during tick".into());
                            }
                            result.worker.push(q);
                        }
                        tick::Event::Stream(tick::StreamEvent::Data {
                            buffer,
                            format,
                            staging,
                            bytes,
                            frequency,
                        }) => {
                            let upload = Upload {
                                buffer: *buffer,
                                format: *format,
                                frequency: *frequency,
                                bytes: self.heap.read(*staging, *bytes as usize)?.to_vec(),
                            };
                            if let Some(old) = self.uploads.iter_mut().find(|u| u.buffer == *buffer)
                            {
                                *old = upload.clone();
                            } else {
                                self.uploads.push(upload.clone());
                            }
                            result.uploads.push(upload);
                        }
                        tick::Event::Stream(tick::StreamEvent::RequestChunks {
                            stream,
                            count,
                            staging,
                        }) => {
                            if self.worker.as_ref().map(|w| w.handle) != Some(*stream) {
                                return Err("Unknown worker request".into());
                            }
                            result
                                .worker
                                .push(self.worker_action(&data::Action::Request {
                                    chunks:
                                        i32::try_from(*count).map_err(|_| "Chunk count overflow")?,
                                    destination: *staging,
                                })?);
                        }
                        tick::Event::Stream(tick::StreamEvent::DestroyStream { stream, wait }) => {
                            if *wait != 0 || self.worker.as_ref().map(|w| w.handle) != Some(*stream)
                            {
                                return Err("Unsupported worker destruction".into());
                            }
                            self.worker = None;
                            self.pcm = None;
                        }
                        tick::Event::Stream(tick::StreamEvent::Free { staging }) => {
                            self.heap.free(*staging)?;
                        }
                        _ => {}
                    }
                }
                result.driver = Some(outcome);
            }
        }
        Ok(result)
    }
}
