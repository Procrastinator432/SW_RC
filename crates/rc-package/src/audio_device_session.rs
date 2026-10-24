//! Couple pure ownership plans to acknowledged backend effects and recovery.
use crate::{
    audio_curve::CurveMath,
    audio_device::{self as device, Backend, Executor, Kind, Receipt},
    audio_device_events as lower, audio_driver, audio_mixed_cycle as cycle,
    audio_mixed_host as mixed, audio_upload as upload,
};
pub struct Session<B: Backend> {
    owner: cycle::Host,
    device: Executor<B>,
    hydrated: bool,
}
impl<B: Backend> Session<B> {
    pub fn new(owner: cycle::Host, device: Executor<B>) -> Result<Self, String> {
        owner.validate()?;
        Ok(Self {
            owner,
            device,
            hydrated: false,
        })
    }
    pub fn owner(&self) -> &cycle::Host {
        &self.owner
    }
    pub fn device(&self) -> &Executor<B> {
        &self.device
    }
    pub fn observe(&mut self, source: u32, parameter: u32) -> Result<i32, String> {
        self.ready()?;
        self.device.observe(source, parameter)
    }
    pub fn render(&mut self, frames: usize) -> Result<device::Rendered, String> {
        self.ready()?;
        self.device.render(frames)
    }
    pub fn manifest(&self) -> Result<Vec<u8>, String> {
        self.device.checkpoint_manifest()
    }
    fn ready(&self) -> Result<(), String> {
        if !self.hydrated || self.device.fault().is_some() {
            Err("Device owner requires reset/materialization".into())
        } else {
            Ok(())
        }
    }
    pub fn create(&mut self, kind: Kind, logical: u32) -> Result<u32, String> {
        self.device.create(kind, logical)
    }
    pub fn enqueue_static(
        &mut self,
        input: &crate::audio_source::Init,
        sound: audio_driver::Sound,
        math: &mut impl CurveMath,
    ) -> Result<usize, String> {
        self.owner.enqueue_static(input, sound, math)
    }
    pub fn enqueue_stream(
        &mut self,
        input: &crate::audio_source::Init,
        bytes: Vec<u8>,
        template: crate::audio_source_tick::Session,
        registration: crate::audio_resource_host::StreamRegistration,
        math: &mut impl CurveMath,
    ) -> Result<usize, String> {
        self.owner
            .enqueue_stream(input, bytes, template, registration, math)
    }
    pub fn service_round(
        &mut self,
        budget: usize,
    ) -> Result<Vec<crate::audio_resource_host::Served>, String> {
        self.ready()?;
        self.owner.service_round(budget)
    }
    fn execute(
        &mut self,
        events: &[cycle::Event],
        reads: &[lower::Read],
    ) -> Result<Receipt, String> {
        let plan = lower::cycle(events, reads)?;
        let program = self.device.prepare(&plan.ops)?;
        self.device.apply(&program).map_err(|e| {
            format!(
                "Device operation {} failed after {} acknowledgements: {}",
                e.failed_index, e.completed, e.reason
            )
        })
    }
    pub fn cycle(
        &mut self,
        frame: &cycle::Frame,
        math: &mut impl CurveMath,
    ) -> Result<Receipt, String> {
        self.ready()?;
        let before = self.owner.checkpoint()?;
        let result = (|| {
            let events = self.owner.cycle(frame, math)?;
            let reads = lower::expected_cycle_reads(&events, frame)?;
            self.execute(&events, &reads)
        })();
        if result.is_err() {
            self.owner.load(&before)?;
            if self.device.fault().is_some() {
                self.hydrated = false;
            }
        }
        result
    }
    pub fn dispatch(
        &mut self,
        command: &cycle::Command,
        bank: &[crate::audio_commands::Sound],
        math: &mut impl CurveMath,
        replies: &[audio_driver::Reply],
    ) -> Result<Receipt, String> {
        self.ready()?;
        let before = self.owner.checkpoint()?;
        let result = (|| {
            let events = self.owner.dispatch(command, bank, math, replies)?.events;
            self.execute(&events, &[])
        })();
        if result.is_err() {
            self.owner.load(&before)?;
            if self.device.fault().is_some() {
                self.hydrated = false;
            }
        }
        result
    }
    pub fn unregister(
        &mut self,
        sound: u32,
        replies: &[audio_driver::Reply],
    ) -> Result<Receipt, String> {
        self.ready()?;
        let before = self.owner.checkpoint()?;
        let result = (|| {
            let events = self.owner.unregister(sound, replies)?;
            self.execute(&events, &[])
        })();
        if result.is_err() {
            self.owner.load(&before)?;
            if self.device.fault().is_some() {
                self.hydrated = false;
            }
        }
        result
    }
    /// Static audio restarts; streams restart at retained ring boundaries.
    /// Exact sample offsets and original elapsed wall time are not restored.
    pub fn restart(
        &mut self,
        manifest: &[u8],
        environments: &[upload::Environment],
    ) -> Result<Receipt, String> {
        if environments.len() != self.owner.mixed().pool().nodes.len() {
            return Err("Recovery environment/node mismatch".into());
        }
        self.hydrated = false;
        self.device.restart(manifest)?;
        let mut ops = vec![];
        for e in &self.owner.mixed().resources().bank().entries {
            ops.push(device::Op::Upload {
                buffer: e.buffer.storage[0],
                format: e.pcm.format(),
                frequency: e.pcm.info.frequency,
                bytes: e.pcm.bytes.clone(),
            });
        }
        let pool = self.owner.mixed().pool();
        let mut buffers = vec![];
        for (node, _) in environments.iter().enumerate() {
            let key = self.owner.mixed().key(node)?;
            if let Some(s) = self.owner.mixed().resources().stream(key) {
                for u in &s.uploads {
                    ops.push(device::Op::Upload {
                        buffer: u.buffer,
                        format: u.format,
                        frequency: u.frequency,
                        bytes: u.bytes.clone(),
                    });
                }
                let b = &s.tick.driver.buffers[0];
                let mut ids = b.storage.clone();
                ids.rotate_left(s.tick.progress[0].indices[1] as usize);
                buffers.push(upload::Buffer {
                    handle: b.handle,
                    buffers: ids,
                    counters: [0; 2],
                });
            } else if let Some(v) = self.owner.mixed().resources().voice(key) {
                let b = &v.driver.buffers[0];
                buffers.push(upload::Buffer {
                    handle: b.handle,
                    buffers: b.storage.clone(),
                    counters: [0; 2],
                });
            }
        }
        let mut plan = upload::Session {
            pool: pool.clone(),
            buffers,
            eax_cache: self.owner.mixed().eax_cache().clone(),
        };
        let mut events = vec![];
        for node in pool.slots.iter().flatten().copied() {
            let s = &pool.nodes[node].source;
            if !s.allocated || s.cursor == 0 {
                continue;
            }
            let p = crate::audio_activation::Prepared {
                node,
                pause: s.requested_pause,
            };
            for event in plan.upload(&p, &environments[node])? {
                events.push(cycle::Event::Mixed(mixed::Event::Parameter(event.clone())));
                let driver = match event {
                    upload::Event::Play { .. } => Some(audio_driver::Event::AlPlay {
                        hardware: pool.nodes[node].hardware,
                        playing_before: 0,
                        pause_before: 0,
                    }),
                    upload::Event::Pause { request, .. } => Some(audio_driver::Event::AlPause {
                        hardware: pool.nodes[node].hardware,
                        playing_before: 1,
                        pause_before: request,
                    }),
                    _ => None,
                };
                if let Some(event) = driver {
                    events.push(cycle::Event::Mixed(mixed::Event::Driver { node, event }));
                }
            }
        }
        ops.extend(lower::cycle(&events, &[])?.ops);
        let program = self.device.prepare(&ops)?;
        let receipt = self.device.apply(&program).map_err(|e| e.reason)?;
        self.hydrated = true;
        Ok(receipt)
    }
}
