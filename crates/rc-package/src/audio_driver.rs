//! FALSource controls and completion. Produces AL requests with native state
//! transitions, not audio output. PCM, allocation and streaming refill external.
use crate::audio_activation::Node;
use serde::{Deserialize, Serialize};
#[derive(Clone, Debug, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Buffer {
    pub handle: u32,
    /// Original buffer +4, treated only as a zero/nonzero completion gate.
    pub stream_token: u32,
    pub count_word: u32,
    /// Backing storage is distinct from logical count. Stop always unqueues 4
    /// in the non-single-buffer branch, even if logical count is 0, 2 or 3.
    pub storage: Vec<u32>,
}
#[derive(Clone, Debug, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Sound {
    pub handle: u32,
    /// Sound object byte +30, distinct from Params flags and source flags.
    pub flags: u8,
    pub duration_bits: u32,
    /// USound +64, reset by its actual SetHandle method during Stop.
    pub native_handle: i32,
}
#[derive(Clone, Debug, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Session {
    pub node: Node,
    pub buffers: Vec<Buffer>,
    pub sounds: Vec<Sound>,
}
#[derive(Clone, Debug, Serialize, Deserialize)]
pub enum Command {
    Play,
    Pause(i32),
    Stop,
    IsPlaying,
    IsPaused,
    IsQueued,
    IsStopped,
    IsFinished,
}
#[derive(Clone, Debug, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Reply {
    pub source_state: i32,
    pub unqueued: [u32; 4],
}
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub enum Event {
    AlPlay {
        hardware: u32,
        playing_before: u32,
        pause_before: i32,
    },
    AlPause {
        hardware: u32,
        playing_before: u32,
        pause_before: i32,
    },
    AlStop {
        hardware: u32,
        playing_before: u32,
        pause_before: i32,
    },
    Integer {
        hardware: u32,
        parameter: u32,
        value: u32,
    },
    Unqueue {
        hardware: u32,
        count: u32,
        buffers: [u32; 4],
    },
    GetState {
        hardware: u32,
        parameter: u32,
    },
    SetHandle {
        sound: u32,
        request: u32,
        previous: i32,
        buffer_before: u32,
        allocated_before: bool,
    },
    Duration {
        sound: u32,
    },
    Clear {
        buffer_before: u32,
    },
}
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Outcome {
    pub result: u32,
    pub events: Vec<Event>,
}
#[derive(Clone, Debug, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Step {
    pub command: Command,
    pub reply: Reply,
}
#[derive(Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
struct Snapshot<T> {
    schema: u32,
    state: T,
}
impl Session {
    pub fn validate(&self) -> Result<(), String> {
        self.node.source.validate()?;
        if self.buffers.len() > 4096
            || self.sounds.len() > 4096
            || self.buffers.iter().map(|b| b.storage.len()).sum::<usize>() > 65536
        {
            return Err("Driver resource bank too large".into());
        }
        let mut handles = std::collections::BTreeSet::new();
        for b in &self.buffers {
            if b.handle == 0
                || !handles.insert(b.handle)
                || b.storage.len() > 4096
                || (b.count_word & 0x1fffffff) as usize > b.storage.len()
            {
                return Err("Invalid driver buffer description".into());
            }
        }
        handles.clear();
        for s in &self.sounds {
            let duration = f32::from_bits(s.duration_bits);
            if s.handle == 0
                || !handles.insert(s.handle)
                || !duration.is_finite()
                || duration.abs() > 1e6
                // Keep duration + 0.5 exact in binary64, as in native x87.
                // Smaller nonzero durations need a separate extended-precision
                // boundary contract; do not silently accept different results.
                || (duration != 0. && duration.abs() < f32::from_bits(0x31000000))
            {
                return Err("Invalid driver sound description".into());
            }
        }
        Ok(())
    }
    pub fn checkpoint(&self) -> Result<Vec<u8>, String> {
        self.validate()?;
        serde_json::to_vec(&Snapshot {
            schema: 1,
            state: self,
        })
        .map_err(|e| e.to_string())
    }
    pub fn load(&mut self, bytes: &[u8]) -> Result<(), String> {
        if bytes.len() > 16 * 1024 * 1024 {
            return Err("Driver snapshot too large".into());
        }
        let next: Snapshot<Self> = serde_json::from_slice(bytes).map_err(|e| e.to_string())?;
        if next.schema != 1 {
            return Err("Unsupported driver snapshot schema".into());
        }
        next.state.validate()?;
        *self = next.state;
        Ok(())
    }
    /// Pure staged command execution. Caller still owns a real device and must
    /// obtain the actual AL query/unqueue replies when integrating this model.
    pub fn execute(&mut self, command: &Command, reply: &Reply) -> Result<Outcome, String> {
        let mut result = self.batch(&[Step {
            command: command.clone(),
            reply: reply.clone(),
        }])?;
        Ok(result.remove(0))
    }
    pub fn batch(&mut self, steps: &[Step]) -> Result<Vec<Outcome>, String> {
        self.validate()?;
        if steps.len() > 4096 {
            return Err("Driver command batch too large".into());
        }
        let mut next = self.clone();
        let mut outcomes = Vec::with_capacity(steps.len());
        for step in steps {
            outcomes.push(next.apply(&step.command, &step.reply)?);
        }
        next.validate()?;
        *self = next;
        Ok(outcomes)
    }
    fn play(&mut self, events: &mut Vec<Event>) -> u32 {
        let s = &mut self.node.source;
        if !s.allocated {
            return 0;
        }
        events.push(Event::AlPlay {
            hardware: self.node.hardware,
            playing_before: s.cursor,
            pause_before: s.requested_pause,
        });
        s.requested_pause = 0;
        s.cursor = 1;
        1
    }
    fn stopped(&self, reply: &Reply, events: &mut Vec<Event>) -> u32 {
        if !self.node.source.allocated {
            return 1;
        }
        events.push(Event::GetState {
            hardware: self.node.hardware,
            parameter: 0x1010,
        });
        u32::from(reply.source_state == 0x1014)
    }
    fn buffer(&self) -> Result<&Buffer, String> {
        self.buffers
            .iter()
            .find(|b| b.handle == self.node.buffer)
            .ok_or_else(|| "Missing driver buffer description".into())
    }
    fn sound(&self) -> Result<&Sound, String> {
        self.sounds
            .iter()
            .find(|s| s.handle == self.node.source.sound)
            .ok_or_else(|| "Missing driver sound description".into())
    }
    fn finished(&self, reply: &Reply, events: &mut Vec<Event>) -> Result<u32, String> {
        let s = &self.node.source;
        if !s.allocated || self.node.buffer == 0 {
            return Ok(1);
        }
        let buffer = self.buffer()?;
        if s.flags & 4 != 0 {
            if buffer.stream_token != 0 {
                if s.flags & 0x4000 != 0
                    && f32::from_bits(s.fade.held_bits) <= f32::from_bits(0x38d1b717)
                    && s.fade.mode != 1
                {
                    return Ok(1);
                }
                if self.sound()?.flags & 1 != 0 {
                    return Ok(0);
                }
            } else {
                let sound = self.sound()?;
                events.push(Event::Duration {
                    sound: sound.handle,
                });
                let lifetime = f64::from_bits(
                    u64::from(s.lifetime_words[0]) | (u64::from(s.lifetime_words[1]) << 32),
                );
                return Ok(u32::from(
                    lifetime > f64::from(f32::from_bits(sound.duration_bits)) + 0.5,
                ));
            }
        }
        Ok(self.stopped(reply, events))
    }
    fn apply(&mut self, command: &Command, reply: &Reply) -> Result<Outcome, String> {
        let mut events = vec![];
        let result = match command {
            Command::Play => self.play(&mut events),
            Command::Pause(request) => {
                if !self.node.source.allocated {
                    0
                } else {
                    self.node.source.requested_pause = *request;
                    if *request != 0 {
                        events.push(Event::AlPause {
                            hardware: self.node.hardware,
                            playing_before: self.node.source.cursor,
                            pause_before: *request,
                        });
                    } else {
                        self.play(&mut events);
                    }
                    1
                }
            }
            Command::Stop => {
                if !self.node.source.allocated {
                    0
                } else {
                    // Resolve/check before committing any external action plan.
                    let buffer = self.buffer()?;
                    let sound = self.sound()?.clone();
                    let single = buffer.count_word & 0x1fffffff == 1;
                    let first = if single {
                        [0; 4]
                    } else {
                        buffer
                            .storage
                            .get(..4)
                            .ok_or("Driver Stop needs four backing entries")?
                            .try_into()
                            .map_err(|_| "Invalid driver backing storage")?
                    };
                    events.push(Event::AlStop {
                        hardware: self.node.hardware,
                        playing_before: self.node.source.cursor,
                        pause_before: self.node.source.requested_pause,
                    });
                    if single {
                        events.push(Event::Integer {
                            hardware: self.node.hardware,
                            parameter: 0x1009,
                            value: 0,
                        });
                    } else {
                        events.push(Event::Unqueue {
                            hardware: self.node.hardware,
                            count: 4,
                            buffers: first,
                        });
                        let buffer = self
                            .buffers
                            .iter_mut()
                            .find(|b| b.handle == self.node.buffer)
                            .ok_or("Missing driver buffer")?;
                        buffer.storage[..4].copy_from_slice(&reply.unqueued);
                    }
                    if sound.flags & 2 != 0 {
                        events.push(Event::SetHandle {
                            sound: sound.handle,
                            request: 0,
                            previous: sound.native_handle,
                            buffer_before: self.node.buffer,
                            allocated_before: true,
                        });
                        self.sounds
                            .iter_mut()
                            .find(|s| s.handle == sound.handle)
                            .ok_or("Missing driver sound")?
                            .native_handle = 0;
                    }
                    self.node.buffer = 0;
                    events.push(Event::Clear { buffer_before: 0 });
                    self.node.source.clear();
                    1
                }
            }
            Command::IsPlaying => self.node.source.cursor,
            Command::IsPaused => self.node.source.requested_pause as u32,
            Command::IsQueued => 0,
            Command::IsStopped => self.stopped(reply, &mut events),
            Command::IsFinished => self.finished(reply, &mut events)?,
        };
        Ok(Outcome { result, events })
    }
}
