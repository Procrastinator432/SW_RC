//! Original request/query/chunk-fill controls with owned destination bytes.
//! Vorbis remains an explicit reply boundary; Pcm supplies owned decoded bytes.
use serde::{Deserialize, Serialize};
pub const MAX_BYTES: usize = 16 * 1024 * 1024;
#[derive(Clone, Debug, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Region {
    pub base: u32,
    pub bytes: Vec<u8>,
}
#[derive(Clone, Debug, Default, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Heap {
    pub regions: Vec<Region>,
}
impl Heap {
    pub fn validate(&self) -> Result<(), String> {
        if self.regions.len() > 4096
            || self
                .regions
                .iter()
                .map(|r| r.bytes.len())
                .fold(0usize, usize::saturating_add)
                > MAX_BYTES
        {
            return Err("PCM heap too large".into());
        }
        let mut ranges = vec![];
        for r in &self.regions {
            if r.base == 0 || r.bytes.is_empty() || r.bytes.len() > MAX_BYTES {
                return Err("Invalid PCM region".into());
            }
            let end = u64::from(r.base) + r.bytes.len() as u64;
            if end > u64::from(u32::MAX) {
                return Err("PCM address overflow".into());
            }
            ranges.push((u64::from(r.base), end));
        }
        ranges.sort_unstable();
        if ranges.windows(2).any(|p| p[0].1 > p[1].0) {
            return Err("Overlapping PCM regions".into());
        }
        Ok(())
    }
    fn locate(&self, at: u32, size: usize) -> Result<(usize, usize), String> {
        self.regions
            .iter()
            .enumerate()
            .find_map(|(i, r)| {
                let start = at.checked_sub(r.base)? as usize;
                let end = start.checked_add(size)?;
                if end <= r.bytes.len() {
                    Some((i, start))
                } else {
                    None
                }
            })
            .ok_or_else(|| "PCM access outside owned allocation".into())
    }
    pub fn read(&self, at: u32, size: usize) -> Result<&[u8], String> {
        let (i, p) = self.locate(at, size)?;
        Ok(&self.regions[i].bytes[p..p + size])
    }
    pub fn write(&mut self, at: u32, bytes: &[u8]) -> Result<(), String> {
        let (i, p) = self.locate(at, bytes.len())?;
        self.regions[i].bytes[p..p + bytes.len()].copy_from_slice(bytes);
        Ok(())
    }
    pub fn zero(&mut self, at: u32, size: usize) -> Result<(), String> {
        let (i, p) = self.locate(at, size)?;
        self.regions[i].bytes[p..p + size].fill(0);
        Ok(())
    }
    pub fn free(&mut self, base: u32) -> Result<(), String> {
        if base == 0 {
            return Ok(());
        }
        let i = self
            .regions
            .iter()
            .position(|r| r.base == base)
            .ok_or("Free of unknown PCM allocation")?;
        self.regions.remove(i);
        Ok(())
    }
}
#[derive(Clone, Debug, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct State {
    pub handle: u32,
    pub critical: u32,
    pub decoder: u32,
    pub cursor: u32,
    pub chunk_bytes: i32,
    pub requested: i32,
    pub loops: u32,
    pub bitstream: i32,
    pub ended: i32,
    pub kind: i32,
}
impl State {
    pub fn validate(&self) -> Result<(), String> {
        if self.handle == 0 || self.chunk_bytes.unsigned_abs() > MAX_BYTES as u32 {
            return Err("Invalid PCM stream metadata".into());
        }
        Ok(())
    }
}
#[derive(Clone, Debug, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Session {
    pub state: State,
    pub heap: Heap,
}
#[derive(Clone, Debug, Serialize, Deserialize)]
pub enum Decode {
    Data { bytes: Vec<u8>, bitstream: i32 },
    End,
    Error(i32),
}
#[derive(Clone, Debug, Serialize, Deserialize)]
pub enum Action {
    Request {
        chunks: i32,
        destination: u32,
    },
    Query,
    Read {
        chunks: i32,
        replies: Vec<Decode>,
        seek_reply: i32,
    },
    Seek {
        seconds_bits: u32,
        reply: i32,
    },
}
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub enum Event {
    Lock {
        critical: u32,
    },
    Unlock {
        critical: u32,
    },
    Read {
        decoder: u32,
        destination: u32,
        bytes: u32,
    },
    Seek {
        decoder: u32,
        seconds_words: [u32; 2],
    },
}
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct Outcome {
    pub result: Option<i32>,
    pub busy: Option<i32>,
    pub events: Vec<Event>,
    pub consumed: usize,
}
#[derive(Clone, Debug, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Pcm {
    pub bytes: Vec<u8>,
    pub cursor: usize,
    pub max_read: usize,
    pub bitstream: i32,
}
impl Pcm {
    pub fn validate(&self) -> Result<(), String> {
        if self.bytes.len() > MAX_BYTES
            || self.cursor > self.bytes.len()
            || self.max_read == 0
            || self.max_read > MAX_BYTES
        {
            return Err("Invalid decoded PCM provider".into());
        }
        Ok(())
    }
}
pub(crate) trait Decoder {
    fn read(&mut self, max: usize) -> Result<Decode, String>;
    fn rewind(&mut self) -> Result<(), String>;
}
struct Script<'a> {
    replies: &'a [Decode],
    used: usize,
}
impl Decoder for Script<'_> {
    fn read(&mut self, _: usize) -> Result<Decode, String> {
        let r = self
            .replies
            .get(self.used)
            .ok_or("Decoder reply script exhausted")?
            .clone();
        self.used += 1;
        Ok(r)
    }
    fn rewind(&mut self) -> Result<(), String> {
        Ok(())
    }
}
impl Decoder for Pcm {
    fn read(&mut self, max: usize) -> Result<Decode, String> {
        let count = max.min(self.max_read).min(self.bytes.len() - self.cursor);
        if count == 0 {
            return Ok(Decode::End);
        }
        let bytes = self.bytes[self.cursor..self.cursor + count].to_vec();
        self.cursor += count;
        Ok(Decode::Data {
            bytes,
            bitstream: self.bitstream,
        })
    }
    fn rewind(&mut self) -> Result<(), String> {
        self.cursor = 0;
        Ok(())
    }
}
#[derive(Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
struct Snapshot {
    schema: u32,
    state: Session,
}
impl Session {
    pub fn validate(&self) -> Result<(), String> {
        self.state.validate()?;
        self.heap.validate()
    }
    pub fn checkpoint(&self) -> Result<Vec<u8>, String> {
        self.validate()?;
        let bytes = serde_json::to_vec(&Snapshot {
            schema: 1,
            state: self.clone(),
        })
        .map_err(|e| e.to_string())?;
        if bytes.len() > MAX_BYTES {
            return Err("PCM snapshot too large".into());
        }
        Ok(bytes)
    }
    pub fn load(&mut self, bytes: &[u8]) -> Result<(), String> {
        if bytes.len() > MAX_BYTES {
            return Err("PCM snapshot too large".into());
        }
        let s: Snapshot = serde_json::from_slice(bytes).map_err(|e| e.to_string())?;
        if s.schema != 1 {
            return Err("Unsupported PCM snapshot schema".into());
        }
        s.state.validate()?;
        *self = s.state;
        Ok(())
    }
    pub fn execute(&mut self, action: &Action) -> Result<Outcome, String> {
        Ok(self.batch(std::slice::from_ref(action))?.remove(0))
    }
    pub fn batch(&mut self, actions: &[Action]) -> Result<Vec<Outcome>, String> {
        self.validate()?;
        if actions.len() > 4096 {
            return Err("PCM action batch too large".into());
        }
        let mut next = self.clone();
        let mut out = vec![];
        for a in actions {
            out.push(next.apply(a)?);
            next.validate()?;
        }
        *self = next;
        Ok(out)
    }
    pub fn service_pcm(&mut self, chunks: i32, provider: &mut Pcm) -> Result<Outcome, String> {
        self.validate()?;
        provider.validate()?;
        let mut next = self.clone();
        let mut pcm = provider.clone();
        let out = next.fill(chunks, &mut pcm)?;
        next.validate()?;
        pcm.validate()?;
        *self = next;
        *provider = pcm;
        Ok(out)
    }
    fn apply(&mut self, action: &Action) -> Result<Outcome, String> {
        let mut out = Outcome {
            result: None,
            busy: None,
            events: vec![],
            consumed: 0,
        };
        let lock = Event::Lock {
            critical: self.state.critical,
        };
        let unlock = Event::Unlock {
            critical: self.state.critical,
        };
        match action {
            Action::Request {
                chunks,
                destination,
            } => {
                out.events.push(lock);
                self.state.requested = *chunks;
                self.state.cursor = *destination;
                out.events.push(unlock);
            }
            Action::Query => {
                out.events.push(lock);
                out.result = Some(i32::from(self.state.ended == 0));
                out.busy = Some(self.state.requested);
                out.events.push(unlock);
            }
            Action::Seek {
                seconds_bits,
                reply,
            } => {
                let seconds = f32::from_bits(*seconds_bits);
                if !seconds.is_finite() {
                    return Err("Invalid PCM seek time".into());
                }
                out.events.push(lock);
                let bits = f64::from(seconds).to_bits();
                out.events.push(Event::Seek {
                    decoder: self.state.decoder,
                    seconds_words: [bits as u32, (bits >> 32) as u32],
                });
                out.result = Some(i32::from(*reply == 0));
                out.events.push(unlock);
            }
            Action::Read {
                chunks,
                replies,
                seek_reply: _,
            } => {
                if replies.len() > 65536
                    || replies
                        .iter()
                        .map(|r| match r {
                            Decode::Data { bytes, .. } => bytes.len(),
                            _ => 0,
                        })
                        .fold(0usize, usize::saturating_add)
                        > MAX_BYTES
                {
                    return Err("Decoder reply budget exceeded".into());
                }
                let mut script = Script { replies, used: 0 };
                return self.fill(*chunks, &mut script);
            }
        }
        Ok(out)
    }
    // Caller owns decoder rollback; public PCM/script entry points remain atomic.
    pub(crate) fn fill(
        &mut self,
        chunks: i32,
        decoder: &mut impl Decoder,
    ) -> Result<Outcome, String> {
        let mut out = Outcome {
            result: Some(0),
            busy: None,
            events: vec![],
            consumed: 0,
        };
        let s = &mut self.state;
        if s.requested == 0 {
            return Ok(out);
        }
        let at = s.cursor;
        let size = s.chunk_bytes.wrapping_mul(chunks);
        s.requested = s.requested.wrapping_sub(chunks).max(0);
        s.cursor = s.cursor.wrapping_add(size as u32);
        if at == 0 || size == 0 {
            return Ok(out);
        }
        if size < 0 {
            out.result = Some(1);
            return Ok(out);
        }
        let size = size as usize;
        if size > MAX_BYTES {
            return Err("PCM chunk request too large".into());
        }
        self.heap.read(at, size)?;
        let mut used = 0usize;
        while used < size {
            if out.consumed >= 65536 {
                return Err("PCM decoder made no bounded progress".into());
            }
            let destination = at.checked_add(used as u32).ok_or("PCM cursor overflow")?;
            let remaining = size - used;
            out.events.push(Event::Read {
                decoder: s.decoder,
                destination,
                bytes: remaining as u32,
            });
            out.consumed += 1;
            match decoder.read(remaining)? {
                Decode::Data { bytes, bitstream } => {
                    if bytes.is_empty() || bytes.len() > remaining {
                        return Err("Invalid decoded PCM block length".into());
                    }
                    self.heap.write(destination, &bytes)?;
                    used += bytes.len();
                    s.bitstream = bitstream;
                }
                Decode::End if s.kind == 1 => {
                    out.events.push(Event::Seek {
                        decoder: s.decoder,
                        seconds_words: [0, 0],
                    });
                    decoder.rewind()?;
                    s.loops = s.loops.wrapping_add(1);
                }
                Decode::End => {
                    self.heap.zero(destination, remaining)?;
                    s.ended = 1;
                    return Ok(out);
                }
                Decode::Error(code) => {
                    if code >= 0 {
                        return Err("Nonnegative decoder error".into());
                    }
                    self.heap.zero(destination, remaining)?;
                    return Ok(out);
                }
            }
        }
        out.result = Some(1);
        Ok(out)
    }
}
