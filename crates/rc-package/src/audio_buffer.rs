//! FALBuffer constructor/release/destructor. Callback effects are explicit.
use serde::{Deserialize, Serialize};
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Buffer {
    pub handle: u32,
    pub owner: u32,
    pub stream: u32,
    pub staging: u32,
    pub sound: u32,
    pub allocation: u32,
    pub count_word: u32,
    pub storage: Vec<u32>,
    pub indices: [u32; 2],
}
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Source {
    pub handle: u32,
    pub buffer: u32,
}
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Session {
    pub buffer: Buffer,
    pub sources: Vec<Source>,
}
#[derive(Clone, Debug, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct StopReply {
    pub detached: bool,
    pub storage: Option<Vec<u32>>,
}
#[derive(Clone, Debug, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Reply {
    pub stops: Vec<StopReply>,
    pub reallocated: u32,
}
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub enum Event {
    GetHandle {
        sound: u32,
    },
    Source {
        index: u32,
    },
    Stop {
        source: u32,
    },
    Delete {
        buffer: u32,
    },
    Reallocate {
        allocation: u32,
        bytes: u32,
        alignment: u32,
    },
    SetHandle {
        sound: u32,
        handle: u32,
    },
    DestroyStream {
        stream: u32,
        wait: u32,
    },
    Free {
        allocation: u32,
    },
}
impl Buffer {
    pub fn new(handle: u32, owner: u32) -> Self {
        Self {
            handle,
            owner,
            stream: 0,
            staging: 0,
            sound: 0,
            allocation: 0,
            count_word: 0,
            storage: vec![],
            indices: [0; 2],
        }
    }
    pub fn validate(&self) -> Result<(), String> {
        if self.handle == 0
            || self.owner == 0
            || self.storage.len() > 4096
            || self.count() as usize > self.storage.len()
            || (self.count() != 0 && self.allocation == 0)
        {
            return Err("Invalid FALBuffer".into());
        }
        Ok(())
    }
    pub fn count(&self) -> u32 {
        self.count_word & 0x1fffffff
    }
}
impl Session {
    /// Outer UnregisterSound with RemoveAll registry semantics. Registry memory
    /// allocation is a host detail; cleanup and object-free order is preserved.
    pub fn unregister(
        &mut self,
        registry: &mut Vec<u32>,
        sound: u32,
        reported: u32,
        reply: &Reply,
    ) -> Result<Vec<Event>, String> {
        self.validate()?;
        if registry.len() > 4096 || sound == 0 {
            return Err("Invalid unregister input".into());
        }
        let mut events = vec![Event::GetHandle { sound }];
        if reported == 0 {
            return Ok(events);
        }
        if reported != self.buffer.handle {
            return Err("Unknown reported buffer".into());
        }
        let mut next = self.clone();
        let mut list = registry.clone();
        list.retain(|&h| h != reported);
        events.extend(next.release(reply, true)?);
        events.push(Event::Free {
            allocation: reported,
        });
        *self = next;
        *registry = list;
        Ok(events)
    }
    pub fn validate(&self) -> Result<(), String> {
        self.buffer.validate()?;
        let mut ids = std::collections::BTreeSet::new();
        if self.sources.len() > 4096
            || self
                .sources
                .iter()
                .any(|s| s.handle == 0 || !ids.insert(s.handle))
        {
            return Err("Invalid buffer source list".into());
        }
        Ok(())
    }
    pub fn release(&mut self, reply: &Reply, destructor: bool) -> Result<Vec<Event>, String> {
        self.validate()?;
        let mut next = self.clone();
        let events = next.apply(reply, destructor)?;
        next.validate()?;
        *self = next;
        Ok(events)
    }
    fn apply(&mut self, reply: &Reply, destructor: bool) -> Result<Vec<Event>, String> {
        let mut events = vec![];
        let b = &mut self.buffer;
        let mut calls = 0;
        if b.count() != 0 {
            for (i, s) in self.sources.iter_mut().enumerate() {
                events.push(Event::Source { index: i as u32 });
                if s.buffer == b.handle {
                    events.push(Event::Stop { source: s.handle });
                    let r = reply.stops.get(calls).ok_or("Missing source Stop reply")?;
                    calls += 1;
                    if let Some(v) = &r.storage {
                        if v.len() != b.storage.len() {
                            return Err("Stop changed buffer storage shape".into());
                        }
                        b.storage.clone_from(v);
                    }
                    if r.detached {
                        s.buffer = 0;
                    }
                }
            }
        }
        if calls != reply.stops.len() {
            return Err("Unused source Stop replies".into());
        }
        for &buffer in b.storage.iter().take(b.count() as usize) {
            events.push(Event::Delete { buffer });
        }
        if b.allocation == 0 || b.count_word & 0x20000000 != 0 {
            b.allocation = 0;
        } else if b.count_word as i32 >= 0 && b.count() != 0 {
            events.push(Event::Reallocate {
                allocation: b.allocation,
                bytes: 0,
                alignment: 0,
            });
            b.allocation = reply.reallocated;
        }
        b.count_word &= 0xe0000000;
        if b.sound != 0 {
            events.push(Event::SetHandle {
                sound: b.sound,
                handle: 0,
            });
            b.sound = 0;
        }
        if b.stream != 0 {
            events.push(Event::DestroyStream {
                stream: b.stream,
                wait: 0,
            });
            b.stream = 0;
        }
        events.push(Event::Free {
            allocation: b.staging,
        });
        b.staging = 0;
        if destructor {
            if b.count_word & 0x20000000 == 0 && b.allocation != 0 {
                events.push(Event::Free {
                    allocation: b.allocation,
                });
            }
            b.allocation = 0;
            b.count_word &= 0xe0000000;
        }
        Ok(events)
    }
}
