//! Device-session executor contract. Physical handles are never checkpointed.
use serde::{Deserialize, Serialize};
use std::collections::BTreeMap;
#[derive(Clone, Copy, Debug, PartialEq, Eq, PartialOrd, Ord, Serialize, Deserialize)]
pub enum Kind {
    Source,
    Buffer,
    Context,
}
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct Capabilities {
    pub eax: bool,
    pub context: bool,
    pub music: bool,
}
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub enum Op {
    Upload {
        buffer: u32,
        format: u32,
        frequency: u32,
        bytes: Vec<u8>,
    },
    Float {
        source: u32,
        parameter: u32,
        value: u32,
    },
    Integer {
        source: u32,
        parameter: u32,
        value: u32,
    },
    Vector {
        source: u32,
        parameter: u32,
        value: [u32; 3],
    },
    Eax {
        source: u32,
        version: u8,
        property: u32,
        value: i32,
    },
    Queue {
        source: u32,
        buffers: Vec<u32>,
    },
    Play(u32),
    Pause(u32),
    Stop(u32),
    DeleteBuffer(u32),
    Query {
        source: u32,
        parameter: u32,
        expected: i32,
    },
    Unqueue {
        source: u32,
        expected: Vec<u32>,
        exact_order: bool,
    },
    Process(u32),
    Suspend(u32),
    MusicPause(bool),
    MusicStop,
    Check(String),
    ClearError,
}
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum Reply {
    Unit,
    Scalar(i32),
    Buffers(Vec<u32>),
}
pub trait Backend {
    fn listener(
        &mut self,
        _position: [f32; 3],
        _forward: [f32; 3],
        _up: [f32; 3],
    ) -> Result<(), String> {
        Err("Backend has no listener support".into())
    }
    fn capabilities(&self) -> Capabilities;
    fn reset(&mut self) -> Result<(), String>;
    fn create(&mut self, kind: Kind) -> Result<u32, String>;
    /// Query/unqueue must return actual backend values, independently of expected.
    fn execute(&mut self, op: &Op) -> Result<Reply, String>;
    fn supports_render(&self) -> bool {
        false
    }
    fn render(&mut self, _frames: usize) -> Result<Rendered, String> {
        Err("Backend has no controlled render clock".into())
    }
}
pub struct Rendered {
    pub frequency: u32,
    pub samples: Vec<i16>,
}
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Fault {
    pub sequence: u64,
    pub failed_index: usize,
    pub completed: usize,
    pub reason: String,
}
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Receipt {
    pub epoch: u64,
    pub sequence: u64,
    pub replies: Vec<Reply>,
}
#[derive(Clone)]
struct Registry {
    epoch: u64,
    bindings: BTreeMap<(Kind, u32), u32>,
}
impl Registry {
    fn resolve(&self, k: Kind, id: u32) -> Result<u32, String> {
        self.bindings
            .get(&(k, id))
            .copied()
            .ok_or_else(|| format!("Unbound {k:?} token {id}"))
    }
    fn can_insert(&self, k: Kind, id: u32) -> Result<(), String> {
        let max = match k {
            Kind::Source => 4097,
            Kind::Buffer => 16384,
            Kind::Context => 16,
        };
        if id == 0
            || self.bindings.contains_key(&(k, id))
            || self.bindings.keys().filter(|(kind, _)| *kind == k).count() >= max
        {
            return Err("Duplicate/invalid device token or binding limit".into());
        }
        Ok(())
    }
    fn insert(&mut self, k: Kind, id: u32, physical: u32) -> Result<(), String> {
        self.can_insert(k, id)?;
        if physical == 0
            || self
                .bindings
                .iter()
                .any(|(&(kind, _), &value)| kind == k && value == physical)
        {
            return Err("Invalid/duplicate backend handle".into());
        }
        self.bindings.insert((k, id), physical);
        Ok(())
    }
}
pub struct Program {
    session: u64,
    epoch: u64,
    revision: u64,
    sequence: u64,
    ops: Vec<Op>,
    after: Registry,
    logical_unqueues: BTreeMap<usize, Vec<(u32, u32)>>,
}
impl Program {
    pub fn commands(&self) -> &[Op] {
        &self.ops
    }
}
#[derive(Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
struct Manifest {
    schema: u32,
    epoch: u64,
    objects: Vec<(Kind, u32)>,
}
static NEXT_SESSION: std::sync::atomic::AtomicU64 = std::sync::atomic::AtomicU64::new(1);
pub struct Executor<B: Backend> {
    session: u64,
    backend: B,
    registry: Registry,
    revision: u64,
    next_sequence: u64,
    fault: Option<Fault>,
}
impl<B: Backend> Executor<B> {
    pub fn listener(
        &mut self,
        position: [f32; 3],
        forward: [f32; 3],
        up: [f32; 3],
    ) -> Result<(), String> {
        self.ready()?;
        if position.iter().any(|v| !v.is_finite() || v.abs() > 1e6)
            || forward.iter().chain(&up).any(|v| !v.is_finite())
            || (forward.iter().map(|v| v * v).sum::<f32>() - 1.).abs() > 0.001
            || (up.iter().map(|v| v * v).sum::<f32>() - 1.).abs() > 0.001
            || forward
                .iter()
                .zip(up)
                .map(|(a, b)| a * b)
                .sum::<f32>()
                .abs()
                > 0.001
        {
            return Err("Invalid listener pose".into());
        }
        self.revision = self
            .revision
            .checked_add(1)
            .ok_or("Device revision exhausted")?;
        let result = self.backend.listener(position, forward, up);
        if let Err(reason) = &result {
            self.fault = Some(Fault {
                sequence: self.next_sequence,
                failed_index: 0,
                completed: 0,
                reason: reason.clone(),
            });
        }
        result
    }
    pub fn new(backend: B, epoch: u64) -> Result<Self, String> {
        if epoch == 0 {
            return Err("Device epoch must be nonzero".into());
        }
        let session = NEXT_SESSION
            .fetch_update(
                std::sync::atomic::Ordering::Relaxed,
                std::sync::atomic::Ordering::Relaxed,
                |n| n.checked_add(1),
            )
            .map_err(|_| "Device session IDs exhausted")?;
        Ok(Self {
            session,
            backend,
            registry: Registry {
                epoch,
                bindings: BTreeMap::new(),
            },
            revision: 0,
            next_sequence: 0,
            fault: None,
        })
    }
    pub fn backend(&self) -> &B {
        &self.backend
    }
    pub fn epoch(&self) -> u64 {
        self.registry.epoch
    }
    pub fn fault(&self) -> Option<&Fault> {
        self.fault.as_ref()
    }
    pub fn physical(&self, kind: Kind, logical: u32) -> Result<u32, String> {
        if self.fault.is_some() {
            return Err("Device session needs reset".into());
        }
        self.registry.resolve(kind, logical)
    }
    fn ready(&self) -> Result<(), String> {
        if self.fault.is_some() {
            Err("Device session quarantined after uncertain execution".into())
        } else {
            Ok(())
        }
    }
    pub fn create(&mut self, kind: Kind, logical: u32) -> Result<u32, String> {
        self.ready()?;
        self.registry.can_insert(kind, logical)?;
        let revision = self
            .revision
            .checked_add(1)
            .ok_or("Device revision exhausted")?;
        let result = self.backend.create(kind).and_then(|physical| {
            self.registry.insert(kind, logical, physical)?;
            Ok(physical)
        });
        self.revision = revision;
        if let Err(reason) = &result {
            self.fault = Some(Fault {
                sequence: self.next_sequence,
                failed_index: 0,
                completed: 0,
                reason: reason.clone(),
            });
        }
        result
    }
    pub fn prepare(&self, logical: &[Op]) -> Result<Program, String> {
        self.ready()?;
        if logical.len() > 65536 {
            return Err("Too many device operations".into());
        }
        self.revision
            .checked_add(1)
            .ok_or("Device revision exhausted")?;
        self.next_sequence
            .checked_add(1)
            .ok_or("Device sequence exhausted")?;
        let mut after = self.registry.clone();
        let mut ops = vec![];
        let mut bytes = 0usize;
        let mut unqueues = BTreeMap::new();
        for (index, op) in logical.iter().enumerate() {
            if let Op::Upload { bytes: data, .. } = op {
                bytes = bytes.saturating_add(data.len());
            }
            if bytes > crate::audio_stream_data::MAX_BYTES {
                return Err("Device upload batch exceeds byte budget".into());
            }
            let resolved = translate(op, &mut after, self.backend.capabilities())?;
            if let (
                Op::Unqueue { expected: ids, .. },
                Op::Unqueue {
                    expected: physical, ..
                },
            ) = (op, &resolved)
            {
                unqueues.insert(
                    index,
                    physical.iter().copied().zip(ids.iter().copied()).collect(),
                );
            }
            ops.push(resolved);
        }
        Ok(Program {
            session: self.session,
            epoch: self.registry.epoch,
            revision: self.revision,
            sequence: self.next_sequence,
            ops,
            after,
            logical_unqueues: unqueues,
        })
    }
    pub fn observe(&mut self, source: u32, parameter: u32) -> Result<i32, String> {
        self.ready()?;
        let physical = self.registry.resolve(Kind::Source, source)?;
        if ![0x1010, 0x1015, 0x1016, 0x1025].contains(&parameter) {
            return Err("Unsupported device observation".into());
        }
        self.revision = self
            .revision
            .checked_add(1)
            .ok_or("Device revision exhausted")?;
        let result = self
            .backend
            .execute(&Op::Query {
                source: physical,
                parameter,
                expected: 0,
            })
            .and_then(|reply| match reply {
                Reply::Scalar(n)
                    if (parameter == 0x1010 && (0x1011..=0x1014).contains(&n))
                        || (parameter == 0x1025 && n >= 0)
                        || ([0x1015, 0x1016].contains(&parameter) && (0..=4096).contains(&n)) =>
                {
                    Ok(n)
                }
                _ => Err("Invalid actual device observation".into()),
            });
        if let Err(reason) = &result {
            self.fault = Some(Fault {
                sequence: self.next_sequence,
                failed_index: 0,
                completed: 0,
                reason: reason.clone(),
            });
        }
        result
    }
    pub fn render(&mut self, frames: usize) -> Result<Rendered, String> {
        self.ready()?;
        if !self.backend.supports_render() || !(1..=1_000_000).contains(&frames) {
            return Err("Unsupported or oversized controlled render".into());
        }
        self.revision = self
            .revision
            .checked_add(1)
            .ok_or("Device revision exhausted")?;
        let result = self.backend.render(frames).and_then(|r| {
            if r.frequency == 0 || r.frequency > 1_000_000 || r.samples.len() != frames * 2 {
                return Err("Invalid stereo render receipt".into());
            }
            Ok(r)
        });
        if let Err(reason) = &result {
            self.fault = Some(Fault {
                sequence: self.next_sequence,
                failed_index: 0,
                completed: 0,
                reason: reason.clone(),
            });
        }
        result
    }
    pub fn apply(&mut self, program: &Program) -> Result<Receipt, Fault> {
        let reject = |reason: &str| Fault {
            sequence: program.sequence,
            failed_index: 0,
            completed: 0,
            reason: reason.into(),
        };
        if self.fault.is_some() {
            return Err(reject("Device session quarantined"));
        }
        if program.session != self.session
            || program.epoch != self.registry.epoch
            || program.revision != self.revision
            || program.sequence != self.next_sequence
        {
            return Err(reject("Foreign, stale or already applied device program"));
        }
        self.revision += 1;
        self.next_sequence += 1;
        let mut replies = vec![];
        for (index, op) in program.ops.iter().enumerate() {
            let result = self
                .backend
                .execute(op)
                .and_then(|reply| validate_reply(op, reply))
                .map(|mut reply| {
                    if let Reply::Buffers(ids) = &mut reply {
                        let mapping = &program.logical_unqueues[&index];
                        for id in ids {
                            *id = mapping.iter().find(|(p, _)| p == id).unwrap().1;
                        }
                    }
                    reply
                });
            match result {
                Ok(reply) => replies.push(reply),
                Err(reason) => {
                    let fault = Fault {
                        sequence: program.sequence,
                        failed_index: index,
                        completed: replies.len(),
                        reason,
                    };
                    self.fault = Some(fault.clone());
                    return Err(fault);
                }
            }
        }
        self.registry = program.after.clone();
        Ok(Receipt {
            epoch: self.registry.epoch,
            sequence: program.sequence,
            replies,
        })
    }
    /// Only logical requirements survive process/device loss, not queued audio.
    pub fn checkpoint_manifest(&self) -> Result<Vec<u8>, String> {
        serde_json::to_vec(&Manifest {
            schema: 1,
            epoch: self.registry.epoch,
            objects: self.registry.bindings.keys().copied().collect(),
        })
        .map_err(|e| e.to_string())
    }
    pub fn restart(&mut self, bytes: &[u8]) -> Result<(), String> {
        if bytes.len() > 1024 * 1024 {
            return Err("Device manifest too large".into());
        }
        let m: Manifest = serde_json::from_slice(bytes).map_err(|e| e.to_string())?;
        if m.schema != 1 || m.epoch == 0 || m.objects.len() > 20497 {
            return Err("Invalid device manifest".into());
        }
        let epoch = m
            .epoch
            .max(self.registry.epoch)
            .checked_add(1)
            .ok_or("Device epoch exhausted")?;
        let mut planned = Registry {
            epoch,
            bindings: BTreeMap::new(),
        };
        for &(kind, id) in &m.objects {
            planned.insert(kind, id, id)?;
        }
        planned.bindings.clear();
        self.fault = Some(Fault {
            sequence: self.next_sequence,
            failed_index: 0,
            completed: 0,
            reason: "Device reset/rebind incomplete".into(),
        });
        self.backend.reset()?;
        for (kind, id) in m.objects {
            let physical = self.backend.create(kind)?;
            planned.insert(kind, id, physical)?;
        }
        self.registry = planned;
        self.revision = 0;
        self.next_sequence = 0;
        self.fault = None;
        Ok(())
    }
}
fn translate(op: &Op, r: &mut Registry, caps: Capabilities) -> Result<Op, String> {
    use Kind::*;
    let s = |id| r.resolve(Source, id);
    let b = |id| r.resolve(Buffer, id);
    Ok(match op {
        Op::Upload {
            buffer,
            format,
            frequency,
            bytes,
        } => {
            let width = match format {
                0x1100 => 1,
                0x1101 | 0x1102 => 2,
                0x1103 => 4,
                _ => return Err("Unsupported PCM format".into()),
            };
            if *frequency == 0 || *frequency > 1_000_000 || bytes.len() % width != 0 {
                return Err("Invalid PCM rate/alignment".into());
            }
            Op::Upload {
                buffer: b(*buffer)?,
                format: *format,
                frequency: *frequency,
                bytes: bytes.clone(),
            }
        }
        Op::Float {
            source,
            parameter,
            value,
        } => {
            let f = f32::from_bits(*value);
            if ![0x100a, 0x1003, 0x1020, 0x1021].contains(parameter)
                || !f.is_finite()
                || !(0.0..=1e6).contains(&f)
            {
                return Err("Unsupported/nonfinite source float".into());
            }
            Op::Float {
                source: s(*source)?,
                parameter: *parameter,
                value: *value,
            }
        }
        Op::Integer {
            source,
            parameter,
            value,
        } => {
            if ![0x1009, 0x1007, 0x202, 0x1025].contains(parameter)
                || ([0x1007, 0x202].contains(parameter) && *value > 1)
                || (*parameter == 0x1025 && *value > i32::MAX as u32)
            {
                return Err("Unsupported source integer".into());
            }
            Op::Integer {
                source: s(*source)?,
                parameter: *parameter,
                value: if *parameter == 0x1009 && *value != 0 {
                    b(*value)?
                } else {
                    *value
                },
            }
        }
        Op::Vector {
            source,
            parameter,
            value,
        } => {
            if *parameter != 0x1004
                || value
                    .iter()
                    .any(|&v| !f32::from_bits(v).is_finite() || f32::from_bits(v).abs() > 1e6)
            {
                return Err("Invalid source vector".into());
            }
            Op::Vector {
                source: s(*source)?,
                parameter: *parameter,
                value: *value,
            }
        }
        Op::Eax {
            source,
            version,
            property,
            value,
        } => {
            if !caps.eax || !matches!((*version, *property), (2, 7 | 9) | (3, 7 | 11)) {
                return Err("Backend lacks required EAX capability/property".into());
            }
            Op::Eax {
                source: s(*source)?,
                version: *version,
                property: *property,
                value: *value,
            }
        }
        Op::Queue { source, buffers } => {
            ids(buffers)?;
            Op::Queue {
                source: s(*source)?,
                buffers: buffers.iter().map(|&id| b(id)).collect::<Result<_, _>>()?,
            }
        }
        Op::Unqueue {
            source,
            expected,
            exact_order,
        } => {
            ids(expected)?;
            Op::Unqueue {
                source: s(*source)?,
                expected: expected.iter().map(|&id| b(id)).collect::<Result<_, _>>()?,
                exact_order: *exact_order,
            }
        }
        Op::Query {
            source,
            parameter,
            expected,
        } => {
            if ![0x1010, 0x1015, 0x1016, 0x1025].contains(parameter)
                || (*parameter == 0x1010 && !matches!(expected, 0x1011..=0x1014))
                || (*parameter == 0x1025 && *expected < 0)
                || (![0x1010, 0x1025].contains(parameter) && !(0..=4096).contains(expected))
            {
                return Err("Invalid AL read expectation".into());
            }
            Op::Query {
                source: s(*source)?,
                parameter: *parameter,
                expected: *expected,
            }
        }
        Op::Play(id) => Op::Play(s(*id)?),
        Op::Pause(id) => Op::Pause(s(*id)?),
        Op::Stop(id) => Op::Stop(s(*id)?),
        Op::DeleteBuffer(id) => {
            let physical = b(*id)?;
            r.bindings.remove(&(Buffer, *id));
            Op::DeleteBuffer(physical)
        }
        Op::Process(id) | Op::Suspend(id) => {
            if !caps.context {
                return Err("Backend lacks context capability".into());
            }
            let p = r.resolve(Context, *id)?;
            if matches!(op, Op::Process(_)) {
                Op::Process(p)
            } else {
                Op::Suspend(p)
            }
        }
        Op::MusicPause(_) | Op::MusicStop => {
            if !caps.music {
                return Err("Backend lacks music capability".into());
            }
            op.clone()
        }
        Op::Check(label) => {
            if label.len() > 128 {
                return Err("Device check label too long".into());
            }
            op.clone()
        }
        Op::ClearError => Op::ClearError,
    })
}
fn ids(v: &[u32]) -> Result<(), String> {
    if v.is_empty()
        || v.len() > 4096
        || v.contains(&0)
        || v.iter().collect::<std::collections::BTreeSet<_>>().len() != v.len()
    {
        return Err("Invalid/duplicate AL buffer list".into());
    }
    Ok(())
}
fn validate_reply(op: &Op, reply: Reply) -> Result<Reply, String> {
    match (op, &reply) {
        (Op::Query { expected, .. }, Reply::Scalar(value)) if expected == value => {}
        (
            Op::Unqueue {
                expected,
                exact_order,
                ..
            },
            Reply::Buffers(actual),
        ) => {
            let mut a = actual.clone();
            let mut e = expected.clone();
            if !exact_order {
                a.sort_unstable();
                e.sort_unstable();
            }
            if a != e {
                return Err("Actual AL unqueue disagrees with planned buffer ownership".into());
            }
        }
        (Op::Query { .. } | Op::Unqueue { .. }, _) => {
            return Err("Actual AL read disagrees with plan".into())
        }
        (_, Reply::Unit) => {}
        _ => return Err("Unexpected backend reply type".into()),
    }
    Ok(reply)
}
