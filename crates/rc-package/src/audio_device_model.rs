//! Strict reference backend for contract tests. No clock, DSP or audio output.
use crate::audio_device::*;
use std::collections::BTreeMap;
#[derive(Default)]
struct Source {
    sample_offset: i32,
    state: i32,
    bound: Option<u32>,
    queue: Vec<u32>,
    processed: usize,
}
pub struct Model {
    caps: Capabilities,
    generation: u32,
    next: BTreeMap<Kind, u32>,
    sources: BTreeMap<u32, Source>,
    buffers: BTreeMap<u32, Option<Vec<u8>>>,
    contexts: BTreeMap<u32, bool>,
    calls: Vec<Op>,
    attempts: usize,
    fail_at: Option<usize>,
    pub fail_reset: bool,
}
impl Default for Model {
    fn default() -> Self {
        Self::new(Capabilities {
            eax: true,
            context: true,
            music: true,
        })
    }
}
impl Model {
    pub fn new(caps: Capabilities) -> Self {
        Self {
            caps,
            generation: 0,
            next: BTreeMap::new(),
            sources: BTreeMap::new(),
            buffers: BTreeMap::new(),
            contexts: BTreeMap::new(),
            calls: vec![],
            attempts: 0,
            fail_at: None,
            fail_reset: false,
        }
    }
    pub fn fail_on_attempt(&mut self, attempt: usize) {
        self.fail_at = Some(attempt);
    }
    pub fn calls(&self) -> &[Op] {
        &self.calls
    }
    pub fn buffer_count(&self) -> usize {
        self.buffers.len()
    }
    pub fn uploaded_bytes(&self) -> usize {
        self.buffers.values().flatten().map(Vec::len).sum()
    }
    fn source(&mut self, id: u32) -> Result<&mut Source, String> {
        self.sources
            .get_mut(&id)
            .ok_or_else(|| "Unknown physical source".into())
    }
}
impl Backend for Model {
    fn listener(
        &mut self,
        _position: [f32; 3],
        _forward: [f32; 3],
        _up: [f32; 3],
    ) -> Result<(), String> {
        Ok(())
    }
    fn capabilities(&self) -> Capabilities {
        self.caps
    }
    fn reset(&mut self) -> Result<(), String> {
        if self.fail_reset {
            return Err("Injected reset failure".into());
        }
        self.generation = self
            .generation
            .checked_add(1)
            .ok_or("Model generation exhausted")?;
        self.next.clear();
        self.sources.clear();
        self.buffers.clear();
        self.contexts.clear();
        self.fail_at = None;
        Ok(())
    }
    fn create(&mut self, kind: Kind) -> Result<u32, String> {
        let next = self.next.entry(kind).or_insert(0);
        *next += 1;
        let id = self
            .generation
            .checked_mul(100000)
            .and_then(|g| g.checked_add(*next))
            .ok_or("Model handle exhausted")?;
        match kind {
            Kind::Source => {
                self.sources.insert(
                    id,
                    Source {
                        state: 0x1011,
                        ..Source::default()
                    },
                );
            }
            Kind::Buffer => {
                self.buffers.insert(id, None);
            }
            Kind::Context => {
                self.contexts.insert(id, false);
            }
        }
        Ok(id)
    }
    fn execute(&mut self, op: &Op) -> Result<Reply, String> {
        let attempt = self.attempts;
        self.attempts += 1;
        if self.fail_at == Some(attempt) {
            return Err("Injected uncertain device operation failure".into());
        }
        self.calls.push(op.clone());
        match op {
            Op::Upload { buffer, bytes, .. } => {
                if self
                    .sources
                    .values()
                    .any(|s| s.queue.contains(buffer) || s.bound == Some(*buffer))
                {
                    return Err("Upload to referenced buffer".into());
                }
                *self
                    .buffers
                    .get_mut(buffer)
                    .ok_or("Unknown physical buffer")? = Some(bytes.clone());
            }
            Op::Integer {
                source,
                parameter,
                value,
            } => {
                if *parameter == 0x1009 {
                    if *value != 0 && self.buffers.get(value).is_none_or(Option::is_none) {
                        return Err("Binding an unuploaded buffer".into());
                    }
                    let s = self.source(*source)?;
                    if s.state == 0x1012 || s.state == 0x1013 {
                        return Err("Cannot replace a playing binding".into());
                    }
                    s.bound = if *value == 0 { None } else { Some(*value) };
                    s.sample_offset = 0;
                    if *value == 0 {
                        s.queue.clear();
                        s.processed = 0;
                    }
                } else if *parameter == 0x1025 {
                    let s = self.source(*source)?;
                    if s.bound.is_none() {
                        return Err("Seeking an unbound source".into());
                    }
                    s.sample_offset = *value as i32;
                } else {
                    self.source(*source)?;
                }
            }
            Op::Float { source, .. } | Op::Vector { source, .. } | Op::Eax { source, .. } => {
                self.source(*source)?;
            }
            Op::Queue { source, buffers } => {
                if buffers
                    .iter()
                    .any(|id| self.buffers.get(id).is_none_or(Option::is_none))
                {
                    return Err("Queueing an unuploaded buffer".into());
                }
                let s = self.source(*source)?;
                if s.bound.is_some() || buffers.iter().any(|id| s.queue.contains(id)) {
                    return Err("Duplicate/mixed source queue binding".into());
                }
                s.queue.extend(buffers);
            }
            Op::Play(id) => {
                let s = self.source(*id)?;
                if s.bound.is_none() && s.queue.is_empty() {
                    return Err("Playing empty source".into());
                }
                s.state = 0x1012;
            }
            Op::Pause(id) => {
                self.source(*id)?.state = 0x1013;
            }
            Op::Stop(id) => {
                let s = self.source(*id)?;
                s.state = 0x1014;
                s.sample_offset = 0;
                s.processed = s.queue.len();
            }
            Op::Query {
                source, parameter, ..
            } => {
                let s = self.source(*source)?;
                return Ok(Reply::Scalar(match parameter {
                    0x1010 => s.state,
                    0x1015 => s.queue.len() as i32,
                    0x1016 => s.processed as i32,
                    0x1025 => s.sample_offset,
                    _ => return Err("Unknown physical query".into()),
                }));
            }
            Op::Unqueue {
                source, expected, ..
            } => {
                let s = self.source(*source)?;
                if expected.len() > s.processed {
                    return Err("Unqueue exceeds actually processed buffers".into());
                }
                let ids = s.queue.drain(..expected.len()).collect();
                s.processed -= expected.len();
                return Ok(Reply::Buffers(ids));
            }
            Op::DeleteBuffer(id) => {
                if self
                    .sources
                    .values()
                    .any(|s| s.bound == Some(*id) || s.queue.contains(id))
                {
                    return Err("Delete referenced physical buffer".into());
                }
                self.buffers.remove(id).ok_or("Unknown physical delete")?;
            }
            Op::Process(id) | Op::Suspend(id) => {
                *self
                    .contexts
                    .get_mut(id)
                    .ok_or("Unknown physical context")? = matches!(op, Op::Process(_));
            }
            Op::MusicPause(_) | Op::MusicStop | Op::Check(_) | Op::ClearError => {}
        }
        Ok(Reply::Unit)
    }
}
