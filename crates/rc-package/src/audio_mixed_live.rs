//! Bounded adapter from the general mixed host to a controlled renderer clock.
//! Actor/listener input is explicit; no scene or campaign ownership is inferred.
use crate::{
    audio_curve::PortableCurveMath,
    audio_device::{Backend, Executor},
    audio_device_session::Session,
    audio_driver::Reply,
    audio_mixed_cycle as cycle, audio_mixed_host as mixed,
    audio_pcm::Packet,
    audio_source::Source,
    audio_source_runtime as runtime,
    audio_upload::Environment,
};
use serde::{Deserialize, Serialize};
use serde_json::Value;
pub const MAX_BUNDLE: usize = 16 * 1024 * 1024;
pub const MAX_CONTROL: usize = 8192;
#[derive(Clone, Debug, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Actor {
    pub id: u32,
    pub location: [f32; 3],
    pub velocity: [f32; 3],
    pub valid: bool,
}
#[derive(Clone, Debug, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub enum Action {
    Pause {
        actor: u32,
        sound: u32,
        paused: bool,
    },
    Actor(Actor),
    Listener {
        location: [f32; 3],
        velocity: [f32; 3],
    },
    StopAll,
}
#[derive(Clone, Debug, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Control {
    pub sequence: u64,
    pub action: Action,
}
impl Control {
    fn validate(&self) -> Result<(), String> {
        if self.sequence >= i64::MAX as u64 {
            return Err("Control counter exhausted".into());
        }
        match &self.action {
            Action::Actor(a) => {
                if a.id == 0 {
                    return Err("Actor ID zero".into());
                }
                crate::audio_source::vector(a.location)?;
                crate::audio_source::vector(a.velocity)?;
            }
            Action::Listener { location, velocity } => {
                crate::audio_source::vector(*location)?;
                crate::audio_source::vector(*velocity)?;
            }
            _ => {}
        }
        Ok(())
    }
}
#[derive(Clone, Debug, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Bundle {
    pub schema: u32,
    pub owner: Value,
    pub manifest: Value,
    pub base: runtime::Frame,
    pub environment: Environment,
    pub params: Vec<crate::audio_census::Params>,
    pub bank: Vec<crate::audio_commands::Sound>,
    pub context: u32,
    pub limit_frames: u64,
    pub rendered_frames: u64,
    pub next_control: u64,
    pub pending: Vec<Control>,
    pub actors: Vec<Actor>,
    pub services: u64,
    pub cycles: u64,
}
impl Bundle {
    pub fn parse(bytes: &[u8]) -> Result<Self, String> {
        if bytes.len() > MAX_BUNDLE {
            return Err("Mixed audio bundle too large".into());
        }
        let b: Self = serde_json::from_slice(bytes).map_err(|e| e.to_string())?;
        b.validate()?;
        Ok(b)
    }
    pub fn encode(&self) -> Result<Vec<u8>, String> {
        self.validate()?;
        let bytes = serde_json::to_vec(self).map_err(|e| e.to_string())?;
        if bytes.len() > MAX_BUNDLE {
            return Err("Mixed snapshot too large".into());
        }
        Ok(bytes)
    }
    fn validate(&self) -> Result<(), String> {
        if self.schema != 1
            || self.limit_frames == 0
            || self.limit_frames > 480000
            || self.rendered_frames > self.limit_frames
            || self.pending.len() > 32
            || self.actors.len() > 64
            || self.bank.is_empty()
            || self.bank.len() > 64
            || self.params.len() > 64
            || self.next_control > i64::MAX as u64
            || self.context == 0
            || self.environment.eax_enabled
            || self.cycles > 480000
            || self.services > 960000
        {
            return Err("Invalid mixed bundle limits/capabilities".into());
        }
        self.base.base.validate()?;
        crate::audio_source::vector(self.base.listener_location)?;
        if !self.base.radius_multiplier.is_finite()
            || self.base.radius_multiplier < 0.
            || !self.base.now.is_finite()
        {
            return Err("Invalid mixed base".into());
        }
        let owner =
            cycle::Host::restore(&serde_json::to_vec(&self.owner).map_err(|e| e.to_string())?)?;
        if owner.mixed().pool().nodes.len() > 9 || owner.waiting().len() > 64 {
            return Err("Mixed pool exceeds adapter budget".into());
        }
        let mut sounds = std::collections::BTreeSet::new();
        for s in &self.bank {
            if s.handle == 0
                || !sounds.insert(s.handle)
                || s.category > 2
                || !s.default_fade.is_finite()
            {
                return Err("Invalid mixed sound bank".into());
            }
        }
        for s in owner
            .mixed()
            .pool()
            .nodes
            .iter()
            .map(|n| &n.source)
            .chain(owner.waiting().iter().map(|p| &p.source))
        {
            if s.sound != 0 && !sounds.contains(&s.sound) {
                return Err("Mixed source missing bank metadata".into());
            }
        }
        let mut actors = std::collections::BTreeSet::new();
        for a in &self.actors {
            Control {
                sequence: 0,
                action: Action::Actor(a.clone()),
            }
            .validate()?;
            if !actors.insert(a.id) {
                return Err("Duplicate actor".into());
            }
        }
        for c in &self.pending {
            if let Action::Actor(a) = &c.action {
                actors.insert(a.id);
            }
        }
        if actors.len() > 64 {
            return Err("Pending actor budget exhausted".into());
        }
        for (i, c) in self.pending.iter().enumerate() {
            c.validate()?;
            if c.sequence
                != self
                    .next_control
                    .checked_sub((self.pending.len() - i) as u64)
                    .ok_or("Pending sequence underflow")?
            {
                return Err("Pending control sequence".into());
            }
        }
        Ok(())
    }
}
pub struct Mixed<B: Backend> {
    session: Session<B>,
    state: Bundle,
    epoch: u64,
    sequence: u64,
    fault: bool,
}
impl<B: Backend> Mixed<B> {
    pub fn open(backend: B, bundle: Bundle, epoch: u64) -> Result<Self, String> {
        bundle.validate()?;
        if epoch == 0 || epoch > i64::MAX as u64 {
            return Err("Invalid mixed output epoch".into());
        }
        let owner =
            cycle::Host::restore(&serde_json::to_vec(&bundle.owner).map_err(|e| e.to_string())?)?;
        let environments = vec![bundle.environment.clone(); owner.mixed().pool().nodes.len()];
        let mut session = Session::new(owner, Executor::new(backend, epoch)?)?;
        session.restart(
            &serde_json::to_vec(&bundle.manifest).map_err(|e| e.to_string())?,
            &environments,
        )?;
        Ok(Self {
            session,
            state: bundle,
            epoch,
            sequence: 0,
            fault: false,
        })
    }
    pub fn remaining(&self) -> u64 {
        self.state.limit_frames - self.state.rendered_frames
    }
    pub fn next_control(&self) -> u64 {
        self.state.next_control
    }
    pub fn owner(&self) -> &cycle::Host {
        self.session.owner()
    }
    pub fn services(&self) -> u64 {
        self.state.services
    }
    pub fn submit(&mut self, bytes: &[u8]) -> Result<(), String> {
        if self.fault || bytes.len() > MAX_CONTROL || self.state.pending.len() >= 32 {
            return Err("Mixed control queue unavailable".into());
        }
        let c: Control = serde_json::from_slice(bytes).map_err(|e| e.to_string())?;
        c.validate()?;
        if c.sequence != self.state.next_control {
            return Err("Stale/out-of-order control".into());
        }
        if let Action::Actor(a) = &c.action {
            let mut ids = self
                .state
                .actors
                .iter()
                .map(|v| v.id)
                .collect::<std::collections::BTreeSet<_>>();
            for c in &self.state.pending {
                if let Action::Actor(p) = &c.action {
                    ids.insert(p.id);
                }
            }
            ids.insert(a.id);
            if ids.len() > 64 {
                return Err("Actor budget exhausted".into());
            }
        }
        self.state.next_control += 1;
        self.state.pending.push(c);
        Ok(())
    }
    fn base(&self, s: &Source) -> runtime::Frame {
        let mut base = self.state.base.clone();
        base.now = self.state.rendered_frames as f32 / 48000.;
        base.category = self
            .state
            .bank
            .iter()
            .find(|b| b.handle == s.sound)
            .map_or(0, |b| b.category);
        if let Some(a) = self.state.actors.iter().find(|a| a.id == s.actor) {
            base.base.actor_location = a.location;
            base.base.actor_velocity = a.velocity;
            base.base.actor_valid = a.valid;
        } else {
            base.base.actor_location = s.location_bits.map(f32::from_bits);
            base.base.actor_velocity = s.velocity_bits.map(f32::from_bits);
        }
        base
    }
    fn frame(&mut self, delta: f32) -> Result<cycle::Frame, String> {
        let pool = self.session.owner().mixed().pool();
        let mut nodes = pool
            .nodes
            .iter()
            .map(|n| {
                let key = self
                    .session
                    .owner()
                    .mixed()
                    .key(
                        pool.nodes
                            .iter()
                            .position(|p| p.hardware == n.hardware)
                            .unwrap(),
                    )
                    .unwrap();
                let stream = self.session.owner().mixed().resources().stream(key);
                mixed::Frame {
                    base: self.base(&n.source),
                    base_delta_bits: delta.to_bits(),
                    cleanup: Reply {
                        source_state: 0x1012,
                        unqueued: stream.map_or([0; 4], |s| {
                            s.tick.driver.buffers[0].storage[..4].try_into().unwrap()
                        }),
                    },
                    late: crate::audio_stream_bridge::Tick {
                        environment: Environment {
                            category: self
                                .state
                                .bank
                                .iter()
                                .find(|b| b.handle == n.source.sound)
                                .map_or(0, |b| b.category),
                            listener_bits: self.state.base.listener_location.map(f32::to_bits),
                            ..self.state.environment.clone()
                        },
                        delta_bits: delta.to_bits(),
                        al: crate::audio_stream_bridge::AlReply {
                            state: 0x1012,
                            processed: 0,
                            queued: 4,
                            unqueued: stream.map_or(0, |s| {
                                s.tick.driver.buffers[0].storage
                                    [s.tick.progress[0].indices[1] as usize]
                            }),
                        },
                    },
                }
            })
            .collect::<Vec<_>>();
        let active = pool
            .nodes
            .iter()
            .enumerate()
            .filter(|(_, n)| n.source.allocated)
            .map(|(i, n)| (i, n.hardware, n.source.flags & 4 != 0))
            .collect::<Vec<_>>();
        for (i, hardware, stream) in active {
            let state = self.session.observe(hardware, 0x1010)?;
            nodes[i].cleanup.source_state = state;
            nodes[i].late.al.state = state;
            if stream {
                nodes[i].late.al.processed = self.session.observe(hardware, 0x1016)?;
                nodes[i].late.al.queued = self.session.observe(hardware, 0x1015)?;
            }
        }
        let queued = self
            .session
            .owner()
            .waiting()
            .iter()
            .map(|q| cycle::QueuedFrame {
                base: self.base(&q.source),
                delta_bits: delta.to_bits(),
                environment: Environment {
                    category: self
                        .state
                        .bank
                        .iter()
                        .find(|b| b.handle == q.source.sound)
                        .map_or(0, |b| b.category),
                    listener_bits: self.state.base.listener_location.map(f32::to_bits),
                    ..self.state.environment.clone()
                },
            })
            .collect();
        Ok(cycle::Frame {
            global_delta_bits: delta.to_bits(),
            nodes,
            queued,
            params: self.state.params.clone(),
            context: self.state.context,
        })
    }
    fn apply_controls(&mut self) -> Result<(), String> {
        for c in std::mem::take(&mut self.state.pending) {
            match c.action {
                Action::Actor(a) => {
                    if let Some(old) = self.state.actors.iter_mut().find(|v| v.id == a.id) {
                        *old = a;
                    } else {
                        self.state.actors.push(a);
                    }
                }
                Action::Listener { location, velocity } => {
                    self.state.base.listener_location = location;
                    self.state.base.base.listener_velocity = velocity;
                }
                action => {
                    let command = match action {
                        Action::Pause {
                            actor,
                            sound,
                            paused,
                        } => cycle::Command::PauseSound {
                            actor,
                            sound,
                            request: i32::from(paused),
                        },
                        Action::StopAll => cycle::Command::StopAll { exclude: 0x90 },
                        _ => unreachable!(),
                    };
                    let replies = self
                        .frame(0.)?
                        .nodes
                        .into_iter()
                        .map(|n| n.cleanup)
                        .collect::<Vec<_>>();
                    self.session.dispatch(
                        &command,
                        &self.state.bank,
                        &mut PortableCurveMath,
                        &replies,
                    )?;
                }
            }
        }
        Ok(())
    }
    pub fn render(&mut self, frames: usize, epoch: u64, sequence: u64) -> Result<Vec<u8>, String> {
        if self.fault
            || !(1..=480).contains(&frames)
            || epoch != self.epoch
            || sequence != self.sequence
            || frames as u64 > self.remaining()
            || sequence >= i64::MAX as u64
        {
            return Err("Invalid mixed render request".into());
        }
        let out = (|| {
            self.apply_controls()?;
            self.state.services += self.session.service_round(2)?.len() as u64;
            let frame = self.frame(frames as f32 / 48000.)?;
            self.session.cycle(&frame, &mut PortableCurveMath)?;
            let pcm = self.session.render(frames)?;
            if pcm.frequency != 48000 {
                return Err("Mixed renderer rate mismatch".into());
            }
            Packet::from_rendered(pcm, epoch, sequence)?
                .encode()
                .map_err(str::to_owned)
        })();
        if out.is_err() {
            self.fault = true;
        } else {
            self.state.rendered_frames += frames as u64;
            self.state.cycles += 1;
            self.sequence += 1;
        }
        out
    }
    pub fn checkpoint(&self) -> Result<Vec<u8>, String> {
        if self.fault {
            return Err("Faulted mixed session cannot checkpoint".into());
        }
        let mut b = self.state.clone();
        b.owner = serde_json::from_slice(&self.session.owner().checkpoint()?)
            .map_err(|e| e.to_string())?;
        b.manifest =
            serde_json::from_slice(&self.session.manifest()?).map_err(|e| e.to_string())?;
        b.encode()
    }
}
