//! Lower owned host events once; ownership/virtual markers are never AL calls.
use crate::{
    audio_buffer as raw, audio_buffer_bank as pcm, audio_device::Op, audio_driver as driver,
    audio_mixed_cycle as cycle, audio_mixed_host as mixed, audio_resource_host as resource,
    audio_source_tick as tick, audio_stream_bridge as bridge, audio_upload as upload,
    audio_vorbis_host::DisposalEvent,
};
#[derive(Clone, Debug)]
pub struct Read {
    pub source: u32,
    pub parameter: u32,
    pub value: i32,
}
pub struct Plan {
    pub ops: Vec<Op>,
    pub markers: usize,
}
struct Lower<'a> {
    plan: Plan,
    reads: &'a [Read],
    next: usize,
    bytes: usize,
}
impl<'a> Lower<'a> {
    fn new(reads: &'a [Read]) -> Self {
        Self {
            plan: Plan {
                ops: vec![],
                markers: 0,
            },
            reads,
            next: 0,
            bytes: 0,
        }
    }
    fn push(&mut self, op: Op) -> Result<(), String> {
        if self.plan.ops.len() >= 65536 {
            return Err("Too many lowered device operations".into());
        }
        self.plan.ops.push(op);
        Ok(())
    }
    fn mark(&mut self) -> Result<(), String> {
        self.plan.markers += 1;
        if self.plan.markers > 262144 {
            return Err("Too many host markers".into());
        }
        Ok(())
    }
    fn upload(&mut self, u: &bridge::Upload) -> Result<(), String> {
        self.bytes = self.bytes.saturating_add(u.bytes.len());
        if self.bytes > crate::audio_stream_data::MAX_BYTES {
            return Err("Lowered upload bytes exceed budget".into());
        }
        self.push(Op::Upload {
            buffer: u.buffer,
            format: u.format,
            frequency: u.frequency,
            bytes: u.bytes.clone(),
        })
    }
    fn parameter(&mut self, e: &upload::Event) -> Result<(), String> {
        match e {
            upload::Event::Float {
                hardware,
                parameter,
                value,
            } => self.push(Op::Float {
                source: *hardware,
                parameter: *parameter,
                value: *value,
            }),
            upload::Event::Integer {
                hardware,
                parameter,
                value,
            } => self.push(Op::Integer {
                source: *hardware,
                parameter: *parameter,
                value: *value,
            }),
            upload::Event::Vector {
                hardware,
                parameter,
                value,
            } => self.push(Op::Vector {
                source: *hardware,
                parameter: *parameter,
                value: *value,
            }),
            upload::Event::Eax {
                hardware,
                version,
                property,
                value,
            } => self.push(Op::Eax {
                source: *hardware,
                version: *version,
                property: *property,
                value: *value,
            }),
            upload::Event::Queue { hardware, buffers } => self.push(Op::Queue {
                source: *hardware,
                buffers: buffers.clone(),
            }),
            upload::Event::Check(label) => self.push(Op::Check(label.clone())),
            // Play/Pause here are virtual boundaries; the nested Driver emits AL.
            _ => self.mark(),
        }
    }
    fn driver(&mut self, e: &driver::Event) -> Result<(), String> {
        match e {
            driver::Event::AlPlay { hardware, .. } => self.push(Op::Play(*hardware)),
            driver::Event::AlPause { hardware, .. } => self.push(Op::Pause(*hardware)),
            driver::Event::AlStop { hardware, .. } => self.push(Op::Stop(*hardware)),
            driver::Event::Integer {
                hardware,
                parameter,
                value,
            } => self.push(Op::Integer {
                source: *hardware,
                parameter: *parameter,
                value: *value,
            }),
            driver::Event::GetState {
                hardware,
                parameter,
            } => {
                let r = self
                    .reads
                    .get(self.next)
                    .ok_or("Missing actual-query expectation")?;
                if r.source != *hardware || r.parameter != *parameter {
                    return Err("Read expectations do not follow host query order".into());
                }
                self.next += 1;
                self.push(Op::Query {
                    source: *hardware,
                    parameter: *parameter,
                    expected: r.value,
                })
            }
            driver::Event::Unqueue {
                hardware,
                count,
                buffers,
            } => {
                if *count != 4 {
                    return Err("Unsupported full-stop unqueue count".into());
                }
                self.push(Op::Unqueue {
                    source: *hardware,
                    expected: buffers.to_vec(),
                    exact_order: false,
                })
            }
            _ => self.mark(),
        }
    }
    fn raw(&mut self, e: &raw::Event) -> Result<(), String> {
        if let raw::Event::Delete { buffer } = e {
            self.push(Op::DeleteBuffer(*buffer))
        } else {
            self.mark()
        }
    }
    fn resource(&mut self, e: &resource::Event) -> Result<(), String> {
        match e {
            resource::Event::Pcm(pcm::Event::Check { stage, error }) => {
                if *error != 0 {
                    return Err("Refuse PCM plan with a failed device check".into());
                }
                self.push(Op::Check(format!("PCM stage {stage}")))
            }
            resource::Event::Driver { event, .. } => self.driver(event),
            resource::Event::Parameter { event, .. } => self.parameter(event),
            resource::Event::StreamDispose { event, .. } => match event {
                DisposalEvent::Driver(e) => self.driver(e),
                DisposalEvent::Buffer(e) => self.raw(e),
            },
            resource::Event::Pcm(pcm::Event::Upload(u)) => self.upload(u),
            resource::Event::Pcm(pcm::Event::Cleanup(e)) => self.raw(e),
            _ => self.mark(),
        }
    }
    fn tick(
        &mut self,
        e: &tick::Event,
        uploads: &[bridge::Upload],
        used: &mut [bool],
    ) -> Result<(), String> {
        match e {
            tick::Event::Parameter(e) => self.parameter(e),
            tick::Event::Driver(e) => self.driver(e),
            tick::Event::Stream(tick::StreamEvent::Unqueue { hardware, buffers }) => {
                self.push(Op::Unqueue {
                    source: *hardware,
                    expected: buffers.clone(),
                    exact_order: true,
                })
            }
            tick::Event::Stream(tick::StreamEvent::Data {
                buffer,
                format,
                bytes,
                frequency,
                ..
            }) => {
                let i = uploads
                    .iter()
                    .enumerate()
                    .position(|(i, u)| {
                        !used[i]
                            && u.buffer == *buffer
                            && u.format == *format
                            && u.frequency == *frequency
                            && u.bytes.len() == *bytes as usize
                    })
                    .ok_or("Driver data event lacks matching owned PCM bytes")?;
                used[i] = true;
                self.upload(&uploads[i])
            }
            _ => self.mark(),
        }
    }
    fn mixed(&mut self, e: &mixed::Event) -> Result<(), String> {
        match e {
            mixed::Event::Parameter(e) => self.parameter(e),
            mixed::Event::Driver { event, .. } => self.driver(event),
            mixed::Event::Resource(e) => self.resource(e),
            mixed::Event::StaticTick { event, .. } => self.tick(event, &[], &mut []),
            mixed::Event::StreamTick { outcome, .. } => {
                let mut used = vec![false; outcome.uploads.len()];
                if let Some(d) = &outcome.driver {
                    for e in &d.events {
                        self.tick(e, &outcome.uploads, &mut used)?;
                    }
                }
                if used.iter().any(|b| !*b) {
                    return Err("Unmatched stream PCM upload receipt".into());
                }
                Ok(())
            }
            mixed::Event::Activation(crate::audio_activation::Event::ClearError) => {
                self.push(Op::ClearError)
            }
            _ => self.mark(),
        }
    }
    fn cycle(&mut self, e: &cycle::Event) -> Result<(), String> {
        match e {
            cycle::Event::Mixed(e) => self.mixed(e),
            cycle::Event::Upload(u) => self.upload(u),
            cycle::Event::ProcessContext(c) => self.push(Op::Process(*c)),
            cycle::Event::SuspendContext(c) => self.push(Op::Suspend(*c)),
            cycle::Event::Command(crate::audio_commands::Event::MusicPause(p)) => {
                self.push(Op::MusicPause(*p))
            }
            cycle::Event::Command(crate::audio_commands::Event::MusicStop) => {
                self.push(Op::MusicStop)
            }
            _ => self.mark(),
        }
    }
    fn finish(self) -> Result<Plan, String> {
        if self.next != self.reads.len() {
            return Err("Unused device read expectations".into());
        }
        Ok(self.plan)
    }
}
pub fn cycle(events: &[cycle::Event], reads: &[Read]) -> Result<Plan, String> {
    let mut l = Lower::new(reads);
    for e in events {
        l.cycle(e)?;
    }
    l.finish()
}
pub fn resources(events: &[resource::Event], reads: &[Read]) -> Result<Plan, String> {
    let mut l = Lower::new(reads);
    for e in events {
        l.resource(e)?;
    }
    l.finish()
}
/// Check predicted frame replies against the real backend at execution time.
pub fn expected_cycle_reads(
    events: &[cycle::Event],
    frame: &cycle::Frame,
) -> Result<Vec<Read>, String> {
    let mut reads = vec![];
    for event in events {
        match event {
            cycle::Event::Mixed(mixed::Event::Driver {
                node,
                event:
                    driver::Event::GetState {
                        hardware,
                        parameter,
                    },
            }) => {
                if *parameter != 0x1010 {
                    return Err("Unknown direct cycle query".into());
                }
                reads.push(Read {
                    source: *hardware,
                    parameter: *parameter,
                    value: frame
                        .nodes
                        .get(*node)
                        .ok_or("Missing query node frame")?
                        .cleanup
                        .source_state,
                });
            }
            cycle::Event::Mixed(mixed::Event::StreamTick { node, outcome }) => {
                let al = &frame
                    .nodes
                    .get(*node)
                    .ok_or("Missing stream query frame")?
                    .late
                    .al;
                if let Some(d) = &outcome.driver {
                    for e in &d.events {
                        if let tick::Event::Driver(driver::Event::GetState {
                            hardware,
                            parameter,
                        }) = e
                        {
                            let value = match parameter {
                                0x1010 => al.state,
                                0x1016 => al.processed,
                                0x1015 => al.queued,
                                _ => return Err("Unknown stream AL query".into()),
                            };
                            reads.push(Read {
                                source: *hardware,
                                parameter: *parameter,
                                value,
                            });
                        }
                    }
                }
            }
            _ => {}
        }
    }
    Ok(reads)
}
