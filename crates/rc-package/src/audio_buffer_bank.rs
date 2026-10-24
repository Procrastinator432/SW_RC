//! Owned static PCM registrations with explicit device replies and disposal plans.
use crate::{
    audio_buffer as raw, audio_stream_bridge::Upload, audio_stream_data::MAX_BYTES, audio_wave::Pcm,
};
use serde::{Deserialize, Serialize};
#[derive(Clone, Debug, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Entry {
    pub sound: u32,
    pub buffer: raw::Buffer,
    pub pcm: Pcm,
}
#[derive(Clone, Debug, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Bank {
    pub owner: u32,
    pub entries: Vec<Entry>,
    pub sources: Vec<raw::Source>,
}
#[derive(Clone, Debug, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Registration {
    pub sound: u32,
    pub handle: u32,
    pub allocation: u32,
    pub generated: u32,
    pub allow_stereo: bool,
    pub generation_error: i32,
    pub upload_error: i32,
}
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub enum Event {
    GetHandle { sound: u32 },
    Generate { buffer: u32 },
    Check { stage: u8, error: i32 },
    Upload(Upload),
    Unload { sound: u32 },
    SetHandle { sound: u32, handle: u32 },
    Cleanup(raw::Event),
    FreeObject { handle: u32 },
}
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct Outcome {
    pub handle: u32,
    pub cached: bool,
    pub events: Vec<Event>,
}
#[derive(Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
struct Saved {
    schema: u32,
    bank: Bank,
}
impl Bank {
    pub fn new(owner: u32) -> Self {
        Self {
            owner,
            entries: vec![],
            sources: vec![],
        }
    }
    pub fn validate(&self) -> Result<(), String> {
        if self.owner == 0 || self.entries.len() > 4096 || self.sources.len() > 4096 {
            return Err("Invalid PCM bank".into());
        }
        let (mut sounds, mut handles, mut ids, mut allocations) = (
            std::collections::BTreeSet::new(),
            std::collections::BTreeSet::new(),
            std::collections::BTreeSet::new(),
            std::collections::BTreeSet::new(),
        );
        let mut total = 0usize;
        for e in &self.entries {
            e.pcm.validate()?;
            e.buffer.validate()?;
            let b = &e.buffer;
            if e.sound == 0
                || !sounds.insert(e.sound)
                || !handles.insert(b.handle)
                || !allocations.insert(b.allocation)
                || b.sound != e.sound
                || b.owner != self.owner
                || b.count_word != 1
                || b.storage.len() != 1
                || b.storage[0] == 0
                || !ids.insert(b.storage[0])
                || b.stream != 0
                || b.staging != 0
            {
                return Err("Invalid static PCM registration".into());
            }
            total = total.saturating_add(e.pcm.bytes.len());
        }
        handles.clear();
        if self
            .sources
            .iter()
            .any(|s| s.handle == 0 || !handles.insert(s.handle))
            || total > MAX_BYTES
        {
            return Err("PCM bank byte/source budget exceeded".into());
        }
        Ok(())
    }
    /// Cached handles win before reading the supplied WAVE or device reply.
    /// Generation-check errors are ignored by the inspected native WAV path;
    /// upload-check errors cause a compensating deletion in this portable owner.
    pub fn register(&mut self, wave: &[u8], r: &Registration) -> Result<Outcome, String> {
        self.validate()?;
        let mut events = vec![Event::GetHandle { sound: r.sound }];
        if let Some(e) = self.entries.iter().find(|e| e.sound == r.sound) {
            return Ok(Outcome {
                handle: e.buffer.handle,
                cached: true,
                events,
            });
        }
        let pcm = Pcm::from_wave(wave)?;
        if pcm.info.channels != 1 && !r.allow_stereo {
            return Err("Stereo PCM requires nonpositional permission".into());
        }
        if r.sound == 0
            || r.handle == 0
            || r.allocation == 0
            || r.generated == 0
            || self.entries.iter().any(|e| {
                e.buffer.handle == r.handle
                    || e.buffer.allocation == r.allocation
                    || e.buffer.storage.contains(&r.generated)
            })
        {
            return Err("Invalid or duplicate PCM registration tokens".into());
        }
        // Preflight limits before publishing a device-side request plan.
        if self.entries.len() >= 4096
            || self
                .entries
                .iter()
                .fold(pcm.bytes.len(), |n, e| n.saturating_add(e.pcm.bytes.len()))
                > MAX_BYTES
        {
            return Err("PCM bank capacity exceeded".into());
        }
        events.extend([
            Event::Generate {
                buffer: r.generated,
            },
            Event::Check {
                stage: 0,
                error: r.generation_error,
            },
            Event::Upload(Upload {
                buffer: r.generated,
                format: pcm.format(),
                frequency: pcm.info.frequency,
                bytes: pcm.bytes.clone(),
            }),
            Event::Unload { sound: r.sound },
            Event::Check {
                stage: 1,
                error: r.upload_error,
            },
        ]);
        if r.upload_error != 0 {
            events.extend([
                Event::SetHandle {
                    sound: r.sound,
                    handle: 0,
                },
                Event::Cleanup(raw::Event::Delete {
                    buffer: r.generated,
                }),
            ]);
            return Ok(Outcome {
                handle: 0,
                cached: false,
                events,
            });
        }
        let mut buffer = raw::Buffer::new(r.handle, self.owner);
        buffer.allocation = r.allocation;
        buffer.count_word = 1;
        buffer.storage = vec![r.generated];
        buffer.sound = r.sound;
        events.push(Event::SetHandle {
            sound: r.sound,
            handle: r.handle,
        });
        let mut next = self.clone();
        next.entries.push(Entry {
            sound: r.sound,
            buffer,
            pcm,
        });
        next.validate()?;
        *self = next;
        Ok(Outcome {
            handle: r.handle,
            cached: false,
            events,
        })
    }
    pub fn unregister(&mut self, sound: u32, reply: &raw::Reply) -> Result<Outcome, String> {
        self.validate()?;
        let mut events = vec![Event::GetHandle { sound }];
        let Some(i) = self.entries.iter().position(|e| e.sound == sound) else {
            if !reply.stops.is_empty() {
                return Err("Unused unregister Stop replies".into());
            }
            return Ok(Outcome {
                handle: 0,
                cached: false,
                events,
            });
        };
        let mut next = self.clone();
        let entry = next.entries.remove(i);
        if reply.reallocated != 0 {
            return Err("Owned bank expects null zero-size reallocation".into());
        }
        let owned = entry.buffer.storage[0];
        let handle = entry.buffer.handle;
        let mut state = raw::Session {
            buffer: entry.buffer,
            sources: next.sources,
        };
        let cleanup = state.release(reply, true)?;
        if cleanup
            .iter()
            .filter_map(|e| {
                if let raw::Event::Delete { buffer } = e {
                    Some(*buffer)
                } else {
                    None
                }
            })
            .collect::<Vec<_>>()
            != [owned]
            || state.sources.iter().any(|s| s.buffer == handle)
        {
            return Err("Stop reply would orphan a PCM buffer or live binding".into());
        }
        events.extend(cleanup.into_iter().map(Event::Cleanup));
        next.sources = state.sources;
        events.push(Event::FreeObject {
            handle: state.buffer.handle,
        });
        next.validate()?;
        *self = next;
        Ok(Outcome {
            handle: 0,
            cached: false,
            events,
        })
    }
    pub fn checkpoint(&self) -> Result<Vec<u8>, String> {
        self.validate()?;
        let bytes = serde_json::to_vec(&Saved {
            schema: 1,
            bank: self.clone(),
        })
        .map_err(|e| e.to_string())?;
        if bytes.len() > MAX_BYTES {
            return Err("PCM bank snapshot too large".into());
        }
        Ok(bytes)
    }
    pub fn load(&mut self, bytes: &[u8]) -> Result<(), String> {
        if bytes.len() > MAX_BYTES {
            return Err("PCM bank snapshot too large".into());
        }
        let s: Saved = serde_json::from_slice(bytes).map_err(|e| e.to_string())?;
        if s.schema != 1 {
            return Err("Unsupported PCM bank schema".into());
        }
        s.bank.validate()?;
        *self = s.bank;
        Ok(())
    }
}
