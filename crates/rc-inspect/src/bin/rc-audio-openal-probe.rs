#[cfg(windows)]
mod windows {
    // Actual OpenAL loopback: render original PCM/Vorbis to a local WAV, no speakers.
    use rc_package::{
        audio_buffer_bank::Registration,
        audio_census::Params,
        audio_curve::PortableCurveMath,
        audio_driver::{Reply, Sound},
        audio_duck::Settings,
        audio_mixed_host::{Frame as NodeFrame, Host as MixedHost},
        audio_resource_host::StreamRegistration,
        audio_source::Init,
        audio_source_runtime as runtime, audio_source_tick as tick,
        audio_source_update::BaseInput,
        audio_stream_bridge as bridge,
        audio_upload::Environment,
        audio_vorbis_host::Tokens,
    };
    use std::fs;
    fn input(sound: u32, flags: u32, id: u32, p: f32) -> Init {
        Init {
            engine_present: true,
            sound,
            original_sound: sound,
            actor: 42,
            flags,
            identity: id,
            engine_index: -1,
            location: [0.; 3],
            volume: 1.,
            pitch: 1.,
            initial_output: 1.,
            fade_seconds: 0.,
            fade_curve: 0,
            radius: 40.,
            ambient_priority: p,
            radius_caps: [0.; 3],
            voice_exempt: true,
        }
    }
    fn replies(h: &MixedHost) -> Vec<Reply> {
        (0..h.pool().nodes.len())
            .map(|n| Reply {
                source_state: 0x1012,
                unqueued: h.resources().stream(h.key(n).unwrap()).map_or([0; 4], |s| {
                    s.tick.driver.buffers[0].storage[..4].try_into().unwrap()
                }),
            })
            .collect()
    }
    fn frames(h: &MixedHost) -> Vec<NodeFrame> {
        let replies = replies(h);
        (0..h.pool().nodes.len())
            .map(|n| {
                let stream = h.resources().stream(h.key(n).unwrap());
                let env = Environment {
                    category: 0,
                    category_gain_bits: 1f32.to_bits(),
                    editor: false,
                    eax_enabled: false,
                    eax_version: 3,
                    listener_bits: [0; 3],
                    rsqrt_seed_bits: None,
                };
                NodeFrame {
                    cleanup: replies[n].clone(),
                    base_delta_bits: 0.05f32.to_bits(),
                    base: runtime::Frame {
                        base: BaseInput {
                            paused: [false; 2],
                            actor_valid: true,
                            engine_present: true,
                            listener_velocity: [0.; 3],
                            actor_location: [0.; 3],
                            actor_velocity: [0.; 3],
                        },
                        now: 1.,
                        occlusion: None,
                        listener_location: [0.; 3],
                        radius_multiplier: 1.,
                        rsqrt_seed_bits: None,
                        category: 0,
                        special_level: false,
                        ducker: false,
                        queued: false,
                    },
                    late: bridge::Tick {
                        environment: env,
                        delta_bits: 0.05f32.to_bits(),
                        al: bridge::AlReply {
                            state: 0x1012,
                            processed: i32::from(stream.is_some()),
                            queued: 4,
                            unqueued: stream.map_or(0, |s| {
                                s.tick.driver.buffers[0].storage
                                    [s.tick.progress[0].indices[1] as usize]
                            }),
                        },
                    },
                }
            })
            .collect()
    }
    use rc_package::audio_mixed_cycle::{Command, Frame, Host, QueuedFrame};
    fn frame(h: &Host) -> Frame {
        let nodes = frames(h.mixed());
        Frame {
            global_delta_bits: 0.05f32.to_bits(),
            queued: h
                .waiting()
                .iter()
                .map(|_| QueuedFrame {
                    base: nodes[0].base.clone(),
                    delta_bits: 0.05f32.to_bits(),
                    environment: nodes[0].late.environment.clone(),
                })
                .collect(),
            nodes,
            params: vec![
                Params {
                    sound: 100,
                    flags: 1,
                },
                Params {
                    sound: 1001,
                    flags: 1,
                },
                Params {
                    sound: 1002,
                    flags: 1,
                },
            ],
            context: 42,
        }
    }
    use rc_package::{
        audio_device::{Executor, Kind},
        audio_device_session::Session,
        audio_openal::OpenAl,
    };
    fn observed_frame(s: &mut Session<OpenAl>) -> Result<Frame, String> {
        let mut f = frame(s.owner());
        let active = s
            .owner()
            .mixed()
            .pool()
            .nodes
            .iter()
            .enumerate()
            .filter(|(_, n)| n.source.allocated)
            .map(|(i, n)| (i, n.hardware, n.source.flags & 4 != 0))
            .collect::<Vec<_>>();
        for (i, hardware, stream) in active {
            let state = s.observe(hardware, 0x1010)?;
            f.nodes[i].cleanup.source_state = state;
            f.nodes[i].late.al.state = state;
            if stream {
                f.nodes[i].late.al.processed = s.observe(hardware, 0x1016)?;
                f.nodes[i].late.al.queued = s.observe(hardware, 0x1015)?;
            }
        }
        Ok(f)
    }
    fn bank() -> Vec<rc_package::audio_commands::Sound> {
        vec![
            rc_package::audio_commands::Sound {
                handle: 100,
                default_fade: 0.2,
                streamed: false,
                curve: 0,
                category: 0,
            },
            rc_package::audio_commands::Sound {
                handle: 1001,
                default_fade: 0.2,
                streamed: true,
                curve: 1,
                category: 1,
            },
            rc_package::audio_commands::Sound {
                handle: 1002,
                default_fade: 0.2,
                streamed: true,
                curve: 0,
                category: 1,
            },
        ]
    }
    fn rms(samples: &[i16]) -> f64 {
        (samples.iter().map(|&s| f64::from(s).powi(2)).sum::<f64>() / samples.len().max(1) as f64)
            .sqrt()
    }
    fn write_wave(
        path: &std::ffi::OsStr,
        samples: &[i16],
    ) -> Result<(), Box<dyn std::error::Error>> {
        let size = samples.len().checked_mul(2).ok_or("WAV length overflow")?;
        if size > 16 * 1024 * 1024 {
            return Err("Output WAV too large".into());
        }
        let mut b = Vec::with_capacity(size + 44);
        b.extend(b"RIFF");
        b.extend((size as u32 + 36).to_le_bytes());
        b.extend(b"WAVEfmt ");
        b.extend(16u32.to_le_bytes());
        b.extend(1u16.to_le_bytes());
        b.extend(2u16.to_le_bytes());
        b.extend(48000u32.to_le_bytes());
        b.extend(192000u32.to_le_bytes());
        b.extend(4u16.to_le_bytes());
        b.extend(16u16.to_le_bytes());
        b.extend(b"data");
        b.extend((size as u32).to_le_bytes());
        for s in samples {
            b.extend(s.to_le_bytes());
        }
        fs::write(path, b)?;
        Ok(())
    }
    pub fn run() -> Result<(), Box<dyn std::error::Error>> {
        let args: Vec<_> = std::env::args_os().collect();
        if args.len() != 8 && args.len() != 9 {
            return Err("Usage: rc-audio-openal-probe DLL TEMPLATE.json PCM.wav OGG_A.ogg OGG_B.ogg OUTPUT.wav REPORT.json".into());
        }
        let template: tick::Session = serde_json::from_slice(&fs::read(&args[2])?)?;
        let mixed = MixedHost::new(
            1,
            &[10, 11, 12, 13],
            vec![1, 2, 3, 4],
            Settings {
                attack: 0.3,
                release: 0.4,
                levels: [0.4, 0.5, 0.6],
            },
        )?;
        let mut h = Host::new(mixed, 100)?;
        h.register_pcm(
            &fs::read(&args[3])?,
            &Registration {
                sound: 100,
                handle: 200,
                allocation: 300,
                generated: 400,
                allow_stereo: true,
                generation_error: 0,
                upload_error: 0,
            },
        )?;
        h.enqueue_static(
            &input(100, 0x100, 6, 1.),
            Sound {
                handle: 100,
                flags: 0,
                duration_bits: 0,
                native_handle: 200,
            },
            &mut PortableCurveMath,
        )?;
        for (n, path) in [(1, &args[4]), (2, &args[5])] {
            let mut t = template.clone();
            t.driver.node.source.sound = 1000 + n;
            t.driver.node.source.original_sound = 1000 + n;
            t.driver.node.source.flags = 0x104;
            t.driver.node.buffer = 2000 + n;
            t.driver.buffers[0].handle = 2000 + n;
            t.driver.sounds[0] = Sound {
                handle: 1000 + n,
                flags: 3,
                duration_bits: 0,
                native_handle: 0,
            };
            h.enqueue_stream(
                &input(1000 + n, 0x104, 6 + 16 * n, n as f32 + 1.),
                fs::read(path)?,
                t,
                StreamRegistration {
                    key: 0,
                    allocation: 4000 + n,
                    tokens: Tokens {
                        stream: 5000 + n,
                        critical: 1,
                        decoder: 6000 + n,
                        staging: 0x10000 + n * 0x10000,
                        buffers: std::array::from_fn(|i| 7000 + n * 4 + i as u32),
                    },
                    looping: true,
                },
                &mut PortableCurveMath,
            )?;
        }
        h.set_cue_id(h.waiting()[2].source.unique_id);
        h.set_pending_cues(8);
        let backend = OpenAl::open_loopback(std::path::Path::new(&args[1]), 48000)?;
        let version = backend.version().to_owned();
        let mut d = Executor::new(backend, 1)?;
        for id in [10, 11, 12, 13] {
            d.create(Kind::Source, id)?;
        }
        for id in std::iter::once(400).chain(7004..=7011) {
            d.create(Kind::Buffer, id)?;
        }
        d.create(Kind::Context, 42)?;
        let mut s = Session::new(h, d)?;
        let envs = frame(s.owner())
            .nodes
            .iter()
            .map(|n| n.late.environment.clone())
            .collect::<Vec<_>>();
        let manifest = s.manifest()?;
        if args.len() == 9 {
            let f = frame(s.owner());
            let bundle = rc_package::audio_mixed_live::Bundle {
                schema: 1,
                owner: serde_json::from_slice(&s.owner().checkpoint()?)?,
                manifest: serde_json::from_slice(&manifest)?,
                base: f.nodes[0].base.clone(),
                environment: f.nodes[0].late.environment.clone(),
                params: f.params,
                bank: bank(),
                context: 42,
                limit_frames: 480000,
                rendered_frames: 0,
                next_control: 0,
                pending: vec![],
                actors: vec![],
                services: 0,
                cycles: 0,
            };
            fs::write(&args[8], bundle.encode()?)?;
        }
        s.restart(&manifest, &envs)?;
        let f = observed_frame(&mut s)?;
        s.cycle(&f, &mut PortableCurveMath)?;
        let mut output = vec![];
        let mut census = vec![s.owner().mixed().status().ducking_voices];
        let mut services = 0;
        for _ in 0..64 {
            output.extend(s.render(2400)?.samples);
            services += s.service_round(2)?.len();
            let f = observed_frame(&mut s)?;
            s.cycle(&f, &mut PortableCurveMath)?;
            census.push(s.owner().mixed().status().ducking_voices);
        }
        if rms(&output) < 10. || s.device().backend().stats().unqueues < 2 {
            return Err("Real rendering produced silence or no stream consumption".into());
        }
        let natural_wav_end = !s
            .owner()
            .mixed()
            .pool()
            .nodes
            .iter()
            .any(|n| n.source.allocated && n.source.sound == 100);
        for sound in [100, 1001, 1002] {
            s.dispatch(
                &Command::PauseSound {
                    actor: 42,
                    sound,
                    request: 1,
                },
                &bank(),
                &mut PortableCurveMath,
                &replies(s.owner().mixed()),
            )?;
        }
        s.render(512)?;
        let silence = s.render(4800)?.samples;
        let paused_rms = rms(&silence);
        if paused_rms > 2. {
            return Err("Paused AL sources still produce signal".into());
        }
        output.extend(silence);
        let before = s.owner().checkpoint()?;
        let manifest = s.manifest()?;
        let epoch = s.device().epoch();
        s.restart(&manifest, &envs)?;
        if s.device().epoch() <= epoch || s.owner().checkpoint()? != before {
            return Err("Actual context rebuild changed logical owner".into());
        }
        let silence = s.render(4800)?.samples;
        if rms(&silence) > 2. {
            return Err("Rebuilt paused sources resumed unexpectedly".into());
        }
        output.extend(silence);
        for sound in [100, 1001, 1002] {
            s.dispatch(
                &Command::PauseSound {
                    actor: 42,
                    sound,
                    request: 0,
                },
                &bank(),
                &mut PortableCurveMath,
                &replies(s.owner().mixed()),
            )?;
        }
        for _ in 0..8 {
            output.extend(s.render(2400)?.samples);
            s.service_round(2)?;
            let f = observed_frame(&mut s)?;
            s.cycle(&f, &mut PortableCurveMath)?;
        }
        s.dispatch(
            &Command::StopAll { exclude: 0x90 },
            &bank(),
            &mut PortableCurveMath,
            &replies(s.owner().mixed()),
        )?;
        if s.owner().mixed().resources().counts() != (1, 0, 1) {
            return Err("Cue-protected real stream did not survive StopAll".into());
        }
        s.dispatch(
            &Command::StopVoices { actor: 42 },
            &bank(),
            &mut PortableCurveMath,
            &replies(s.owner().mixed()),
        )?;
        for sound in [1001, 1002, 100] {
            s.unregister(sound, &replies(s.owner().mixed()))?;
        }
        if s.owner().mixed().resources().counts() != (0, 0, 0)
            || s.device().backend().live_objects().1 != 0
        {
            return Err("Real backend cleanup leaked buffers".into());
        }
        write_wave(&args[6], &output)?;
        let stats = s.device().backend().stats();
        let report = serde_json::json!({"passed":true,"backend":"OpenAL Soft loopback","version":version,"actual_al_renderer":true,"speaker_output":false,"frames":output.len()/2,"frequency":48000,"rms":rms(&output),"paused_rms":paused_rms,"rounds":64,"services":services,"natural_wav_end":natural_wav_end,"census_samples":census,"al_uploads":stats.uploads,"al_unqueues":stats.unqueues,"al_commands":stats.commands,"active_paused_context_rebuild":true,"final_epoch":s.device().epoch(),"remaining_al_buffers":s.device().backend().live_objects().1,"final_counts":s.owner().mixed().resources().counts()});
        fs::write(&args[7], serde_json::to_vec_pretty(&report)?)?;
        println!("{report}");
        Ok(())
    }
}
#[cfg(windows)]
fn main() {
    if let Err(e) = windows::run() {
        eprintln!("{e}");
        std::process::exit(1);
    }
}
#[cfg(not(windows))]
fn main() {
    eprintln!("Windows loopback backend is unavailable on this host");
    std::process::exit(1);
}
