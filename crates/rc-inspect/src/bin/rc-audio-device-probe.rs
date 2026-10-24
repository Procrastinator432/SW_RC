//! Device-session contract with original PCM/Vorbis and a reference backend only.
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
                            s.tick.driver.buffers[0].storage[s.tick.progress[0].indices[1] as usize]
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
    audio_device::{Executor, Kind, Op},
    audio_device_model::Model,
    audio_device_session::Session,
};
fn observed_frame(s: &mut Session<Model>) -> Result<Frame, String> {
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
fn run() -> Result<(), Box<dyn std::error::Error>> {
    let args: Vec<_> = std::env::args_os().collect();
    if args.len() != 6 {
        return Err(
            "Usage: rc-audio-device-probe TEMPLATE.json PCM.wav OGG_A.ogg OGG_B.ogg REPORT.json"
                .into(),
        );
    }
    let template: tick::Session = serde_json::from_slice(&fs::read(&args[1])?)?;
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
        &fs::read(&args[2])?,
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
    for (n, path) in [(1, &args[3]), (2, &args[4])] {
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
    let mut device = Executor::new(Model::default(), 1)?;
    for id in [10, 11, 12, 13] {
        device.create(Kind::Source, id)?;
    }
    for id in std::iter::once(400).chain(7004..=7011) {
        device.create(Kind::Buffer, id)?;
    }
    device.create(Kind::Context, 42)?;
    let mut s = Session::new(h, device)?;
    let manifest = s.manifest()?;
    let envs = frame(s.owner())
        .nodes
        .iter()
        .map(|n| n.late.environment.clone())
        .collect::<Vec<_>>();
    s.restart(&manifest, &envs)?;
    let f = observed_frame(&mut s)?;
    s.cycle(&f, &mut PortableCurveMath)?;
    let calls = s.device().backend().calls();
    let initial_uploads = calls
        .iter()
        .filter(|o| matches!(o, Op::Upload { .. }))
        .count();
    let initial_plays = calls.iter().filter(|o| matches!(o, Op::Play(_))).count();
    let initial_queues = calls
        .iter()
        .filter(|o| matches!(o, Op::Queue { .. }))
        .count();
    if (initial_uploads, initial_plays, initial_queues) != (9, 3, 2) {
        return Err("Executor duplicated or lost initial device work".into());
    }
    let mut services = 0;
    for _ in 0..8 {
        services += s.service_round(1)?.len();
        let f = observed_frame(&mut s)?;
        s.cycle(&f, &mut PortableCurveMath)?;
        if s.owner().mixed().status().ducking_voices != 3 {
            return Err("Executor/owner census diverged".into());
        }
    }
    let before = s.owner().checkpoint()?;
    let mut bad = observed_frame(&mut s)?;
    let stream = s
        .owner()
        .mixed()
        .pool()
        .nodes
        .iter()
        .position(|n| n.source.flags & 4 != 0)
        .ok_or("Missing stream")?;
    bad.nodes[stream].late.al.queued = 3;
    if s.cycle(&bad, &mut PortableCurveMath).is_ok()
        || s.owner().checkpoint()? != before
        || s.device().fault().is_none()
    {
        return Err("Actual backend disagreement failed to quarantine and roll back owner".into());
    }
    let prefix = s.device().fault().unwrap().completed;
    if prefix == 0 {
        return Err("Expected a real executed prefix before disagreement".into());
    }
    let manifest = s.manifest()?;
    let epoch = s.device().epoch();
    s.restart(&manifest, &envs)?;
    if s.device().epoch() <= epoch || s.owner().checkpoint()? != before {
        return Err("Recovery lost logical owner or epoch boundary".into());
    }
    s.dispatch(
        &Command::PauseAll {
            request: -7,
            exclude: 0,
        },
        &bank(),
        &mut PortableCurveMath,
        &replies(s.owner().mixed()),
    )?;
    let before_pause = s.owner().checkpoint()?;
    let manifest = s.manifest()?;
    s.restart(&manifest, &envs)?;
    if s.owner().checkpoint()? != before_pause {
        return Err("Paused recovery changed owner".into());
    }
    let clock = s
        .owner()
        .mixed()
        .pool()
        .nodes
        .iter()
        .map(|n| n.source.lifetime_words)
        .collect::<Vec<_>>();
    let f = observed_frame(&mut s)?;
    s.cycle(&f, &mut PortableCurveMath)?;
    if clock
        != s.owner()
            .mixed()
            .pool()
            .nodes
            .iter()
            .map(|n| n.source.lifetime_words)
            .collect::<Vec<_>>()
    {
        return Err("Paused clocks changed".into());
    }
    s.dispatch(
        &Command::PauseAll {
            request: 0,
            exclude: u32::MAX,
        },
        &bank(),
        &mut PortableCurveMath,
        &replies(s.owner().mixed()),
    )?;
    s.dispatch(
        &Command::StopAll { exclude: 0x80 },
        &bank(),
        &mut PortableCurveMath,
        &replies(s.owner().mixed()),
    )?;
    if s.owner().mixed().resources().counts() != (1, 0, 1) {
        return Err("Cue-protected stream was not preserved".into());
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
        || s.device().backend().buffer_count() != 0
    {
        return Err("Device/owner buffer cleanup diverged".into());
    }
    let report = serde_json::json!({"passed":true,"backend":"strict reference model","real_audio_device":false,"initial_uploads":initial_uploads,"initial_plays":initial_plays,"initial_queues":initial_queues,"rounds":8,"services":services,"census":3,"actual_read_disagreement_quarantined":true,"acknowledged_prefix_before_fault":prefix,"owner_rollback":true,"active_and_paused_rebuild":true,"final_epoch":s.device().epoch(),"reference_backend_calls":s.device().backend().calls().len(),"remaining_device_buffers":s.device().backend().buffer_count(),"final_counts":s.owner().mixed().resources().counts()});
    fs::write(&args[5], serde_json::to_vec_pretty(&report)?)?;
    println!("{report}");
    Ok(())
}
fn main() {
    if let Err(e) = run() {
        eprintln!("{e}");
        std::process::exit(1);
    }
}
