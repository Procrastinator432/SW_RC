//! Whole FALSource driver Update (10006270). Ordered requests, not a device.
//! PCM bytes and stream-manager/allocator work remain explicit callback boundaries.
use crate::{
    audio_driver as driver,
    audio_upload::{self, Environment},
};
use serde::{Deserialize, Serialize};
#[derive(Clone, Debug, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Progress {
    pub buffer: u32,
    pub staging: u32,
    pub indices: [u32; 2],
}
#[derive(Clone, Debug, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Stream {
    pub handle: u32,
    pub chunk_bytes: u32,
    pub format: u32,
    pub frequency: u32,
}
#[derive(Clone, Debug, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Session {
    pub driver: driver::Session,
    pub progress: Vec<Progress>,
    pub streams: Vec<Stream>,
}
#[derive(Clone, Debug, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Reply {
    pub state: i32,
    pub processed: i32,
    pub queued: i32,
    pub available: i32,
    pub busy: i32,
    pub unqueued: u32,
}
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub enum StreamEvent {
    QueryStream {
        stream: u32,
    },
    Unqueue {
        hardware: u32,
        buffers: Vec<u32>,
    },
    Data {
        buffer: u32,
        format: u32,
        staging: u32,
        bytes: u32,
        frequency: u32,
    },
    RequestChunks {
        stream: u32,
        count: u32,
        staging: u32,
    },
    DestroyStream {
        stream: u32,
        wait: u32,
    },
    Free {
        staging: u32,
    },
}
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(untagged)]
pub enum Event {
    Parameter(audio_upload::Event),
    Driver(driver::Event),
    Stream(StreamEvent),
}
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct Outcome {
    pub result: u32,
    pub events: Vec<Event>,
}
#[derive(Clone, Debug, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Step {
    pub environment: Environment,
    pub reply: Reply,
    pub delta_bits: u32,
}
#[derive(Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
struct Snapshot {
    schema: u32,
    state: Session,
}
impl Session {
    pub fn validate(&self) -> Result<(), String> {
        self.driver.validate()?;
        if self.progress.len() > 4096 || self.streams.len() > 4096 {
            return Err("Tick resource bank too large".into());
        }
        let mut ids = std::collections::BTreeSet::new();
        for p in &self.progress {
            let buffer = self
                .driver
                .buffers
                .iter()
                .find(|b| b.handle == p.buffer)
                .ok_or("Unknown tick progress buffer")?;
            let count = buffer.count_word & 0x1fffffff;
            if !ids.insert(p.buffer)
                || p.indices
                    .iter()
                    .any(|&i| if count == 0 { i != 0 } else { i >= count })
            {
                return Err("Invalid tick buffer indices".into());
            }
        }
        ids.clear();
        for s in &self.streams {
            if s.handle == 0
                || !ids.insert(s.handle)
                || s.chunk_bytes > 16 * 1024 * 1024
                || s.frequency > 1_000_000
            {
                return Err("Invalid tick stream description".into());
            }
        }
        Ok(())
    }
    pub fn checkpoint(&self) -> Result<Vec<u8>, String> {
        self.validate()?;
        serde_json::to_vec(&Snapshot {
            schema: 1,
            state: self.clone(),
        })
        .map_err(|e| e.to_string())
    }
    pub fn load(&mut self, bytes: &[u8]) -> Result<(), String> {
        if bytes.len() > 16 * 1024 * 1024 {
            return Err("Tick snapshot too large".into());
        }
        let snapshot: Snapshot = serde_json::from_slice(bytes).map_err(|e| e.to_string())?;
        if snapshot.schema != 1 {
            return Err("Unsupported tick snapshot schema".into());
        }
        snapshot.state.validate()?;
        *self = snapshot.state;
        Ok(())
    }
    pub fn execute(&mut self, step: &Step) -> Result<Outcome, String> {
        Ok(self.batch(std::slice::from_ref(step))?.remove(0))
    }
    /// Atomic planning. The caller must obtain real AL/stream replies and execute
    /// emitted requests when connecting this kernel to a real host.
    pub fn batch(&mut self, steps: &[Step]) -> Result<Vec<Outcome>, String> {
        self.validate()?;
        if steps.len() > 4096 {
            return Err("Tick batch too large".into());
        }
        let mut next = self.clone();
        let mut outcomes = Vec::with_capacity(steps.len());
        for step in steps {
            outcomes.push(next.apply(step)?);
            next.validate()?;
        }
        *self = next;
        Ok(outcomes)
    }
    fn apply(&mut self, step: &Step) -> Result<Outcome, String> {
        let mut events = vec![];
        let source = &self.driver.node.source;
        if !source.allocated || source.requested_pause != 0 {
            return Ok(Outcome { result: 0, events });
        }
        let delta = f32::from_bits(step.delta_bits);
        if !delta.is_finite() || !(0. ..=1e6).contains(&delta) {
            return Err("Invalid driver tick delta".into());
        }
        step.environment.validate()?;
        let hw = self.driver.node.hardware;
        let env = &step.environment;
        events.push(Event::Parameter(audio_upload::Event::Params {
            sound: source.sound,
        }));
        events.push(Event::Parameter(audio_upload::Event::Category {
            index: env.category,
        }));
        let gain = (f32::from_bits(source.output_volume_bits)
            * f32::from_bits(env.category_gain_bits))
        .clamp(f32::from_bits(0x38d1b717), f32::from_bits(0x3f7ff972));
        events.push(Event::Parameter(audio_upload::Event::Float {
            hardware: hw,
            parameter: 0x100a,
            value: gain.to_bits(),
        }));
        if source.flags & 0x100 == 0 && !env.editor {
            let position = source.location_bits.map(f32::from_bits);
            let scale = f32::from_bits(0x3c23d70a);
            let radius = f32::from_bits(source.radii.radius_bits[1]);
            let reference = if source.flags & 0x1000 == 0 {
                radius * scale
            } else {
                events.push(Event::Parameter(audio_upload::Event::Listener {
                    index: source.engine_index,
                }));
                let [x, y, z] =
                    std::array::from_fn(|i| position[i] - f32::from_bits(env.listener_bits[i]));
                let q = (x * x + y * y) + z * z;
                let root = if q == 0. {
                    0.
                } else {
                    let seed = env
                        .rsqrt_seed_bits
                        .map(f32::from_bits)
                        .ok_or("Driver tick distance needs RSQRTSS seed")?;
                    if !seed.is_finite() || seed <= 0. {
                        return Err("Invalid tick distance seed".into());
                    }
                    ((3. - (seed * q) * seed) * (seed * 0.5)) * q
                };
                root * scale
            };
            if !reference.is_finite() {
                return Err("Driver tick distance overflow".into());
            }
            events.push(Event::Parameter(audio_upload::Event::Vector {
                hardware: hw,
                parameter: 0x1004,
                value: [
                    (position[0] * scale).to_bits(),
                    (position[2] * scale).to_bits(),
                    (position[1] * scale).to_bits(),
                ],
            }));
            events.push(Event::Parameter(audio_upload::Event::Float {
                hardware: hw,
                parameter: 0x1020,
                value: reference.to_bits(),
            }));
            if env.eax_enabled && source.flags & 0x800 == 0 {
                // Unlike initial PlaySource: current radius / uploaded reference.
                let attenuation = audio_upload::attenuation(radius / reference)?;
                events.push(Event::Parameter(audio_upload::Event::Eax {
                    version: if env.eax_version < 3 { 2 } else { 3 },
                    property: if env.eax_version < 3 { 9 } else { 11 },
                    hardware: hw,
                    value: attenuation,
                }));
            }
        }
        if source.flags & 4 != 0 {
            let bi = self
                .driver
                .buffers
                .iter()
                .position(|b| b.handle == self.driver.node.buffer)
                .ok_or("Missing tick buffer")?;
            let token = self.driver.buffers[bi].stream_token;
            if token != 0 {
                let pi = self
                    .progress
                    .iter()
                    .position(|p| p.buffer == self.driver.node.buffer)
                    .ok_or("Missing stream buffer progress")?;
                let si = self
                    .streams
                    .iter()
                    .position(|s| s.handle == token)
                    .ok_or("Missing tick stream")?;
                for parameter in [0x1010, 0x1016, 0x1015] {
                    events.push(Event::Driver(driver::Event::GetState {
                        hardware: hw,
                        parameter,
                    }));
                }
                events.push(Event::Stream(StreamEvent::QueryStream { stream: token }));
                if step.reply.processed != 0 && step.reply.busy == 0 {
                    let b = &mut self.driver.buffers[bi];
                    let p = &mut self.progress[pi];
                    let s = &self.streams[si];
                    let count = b.count_word & 0x1fffffff;
                    if count == 0 {
                        return Err("Cannot refill zero-count stream buffer".into());
                    }
                    let [write, read] = p.indices.map(|v| v as usize);
                    events.push(Event::Stream(StreamEvent::Unqueue {
                        hardware: hw,
                        buffers: vec![b.storage[read]],
                    }));
                    b.storage[read] = step.reply.unqueued;
                    events.push(Event::Stream(StreamEvent::Data {
                        buffer: b.storage[write],
                        format: s.format,
                        staging: p.staging,
                        bytes: s.chunk_bytes,
                        frequency: s.frequency,
                    }));
                    events.push(Event::Parameter(audio_upload::Event::Queue {
                        hardware: hw,
                        buffers: vec![b.storage[write]],
                    }));
                    events.push(Event::Stream(StreamEvent::RequestChunks {
                        stream: token,
                        count: 1,
                        staging: p.staging,
                    }));
                    p.indices = p.indices.map(|i| (i + 1) % count);
                    if step.reply.state == 0x1014 {
                        self.restart(&mut events)?;
                    }
                } else if step.reply.available == 0 {
                    let staging = self.progress[pi].staging;
                    events.push(Event::Stream(StreamEvent::DestroyStream {
                        stream: token,
                        wait: 0,
                    }));
                    self.driver.buffers[bi].stream_token = 0;
                    events.push(Event::Stream(StreamEvent::Free { staging }));
                    self.progress[pi].staging = 0;
                    let sound = self
                        .driver
                        .sounds
                        .iter_mut()
                        .find(|s| s.handle == self.driver.node.source.sound)
                        .ok_or("Missing sound for stream completion")?;
                    events.push(Event::Driver(driver::Event::SetHandle {
                        sound: sound.handle,
                        request: 0,
                        previous: sound.native_handle,
                        buffer_before: self.driver.node.buffer,
                        allocated_before: self.driver.node.source.allocated,
                    }));
                    sound.native_handle = 0;
                } else if step.reply.state == 0x1014 {
                    self.restart(&mut events)?;
                }
            }
        }
        let source = &mut self.driver.node.source;
        let time = f64::from_bits(
            u64::from(source.lifetime_words[0]) | (u64::from(source.lifetime_words[1]) << 32),
        );
        let next = (time + f64::from(delta)).to_bits();
        source.lifetime_words = [next as u32, (next >> 32) as u32];
        Ok(Outcome { result: 1, events })
    }
    fn restart(&mut self, events: &mut Vec<Event>) -> Result<(), String> {
        let outcome = self.driver.execute(
            &driver::Command::Play,
            &driver::Reply {
                source_state: 0,
                unqueued: [0; 4],
            },
        )?;
        events.extend(outcome.events.into_iter().map(Event::Driver));
        Ok(())
    }
}
