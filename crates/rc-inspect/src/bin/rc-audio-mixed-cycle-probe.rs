//! Mixed owned queue/upload/control exercise with original files, no device.
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
use rc_package::audio_mixed_cycle::{Command, Event, Frame, Host, QueuedFrame};
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
fn command(h: &mut Host, c: &Command) -> Result<usize, String> {
    let bank = [
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
    ];
    let mut cold = Host::restore(&h.checkpoint()?)?;
    let r = replies(h.mixed());
    let out = h.dispatch(c, &bank, &mut PortableCurveMath, &r)?;
    if out.events != cold.dispatch(c, &bank, &mut PortableCurveMath, &r)?.events
        || h.checkpoint()? != cold.checkpoint()?
    {
        return Err("Cold command diverged".into());
    }
    Ok(out.events.len())
}
fn run() -> Result<(), Box<dyn std::error::Error>> {
    let args: Vec<_> = std::env::args_os().collect();
    if args.len() != 6 {
        return Err("Usage: rc-audio-mixed-cycle-probe TEMPLATE.json PCM.wav OGG_A.ogg OGG_B.ogg REPORT.json".into());
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
    let sound = Sound {
        handle: 100,
        flags: 0,
        duration_bits: 0,
        native_handle: 200,
    };
    h.enqueue_static(
        &input(100, 0x100, 6, 1.),
        sound.clone(),
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
        let r = StreamRegistration {
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
        };
        h.enqueue_stream(
            &input(1000 + n, 0x104, 6 + 16 * n, n as f32 + 1.),
            fs::read(path)?,
            t,
            r,
            &mut PortableCurveMath,
        )?;
    }
    h.set_cue_id(h.waiting()[2].source.unique_id);
    h.set_pending_cues(8);
    h.enqueue_static(&input(100, 0x100, 54, 0.5), sound, &mut PortableCurveMath)?;
    let mut cold = Host::restore(&h.checkpoint()?)?;
    let f = frame(&h);
    let first = h.cycle(&f, &mut PortableCurveMath)?;
    if first != cold.cycle(&f, &mut PortableCurveMath)? || h.checkpoint()? != cold.checkpoint()? {
        return Err("Cold queued activation diverged".into());
    }
    let uploads = first
        .iter()
        .filter(|e| matches!(e, Event::Upload(_)))
        .count();
    let bindings = first
        .iter()
        .filter(|e| {
            matches!(
                e,
                Event::Mixed(rc_package::audio_mixed_host::Event::Parameter(
                    rc_package::audio_upload::Event::Queue { .. }
                ))
            )
        })
        .count();
    let admitted = first
        .iter()
        .filter(|e| matches!(e, Event::Attempt { node: Some(_), .. }))
        .count();
    let deleted = first
        .iter()
        .filter(|e| matches!(e, Event::Delete { .. }))
        .count();
    if uploads != 8
        || bindings != 2
        || admitted != 3
        || deleted != 4
        || h.mixed().resources().counts() != (1, 1, 2)
    {
        return Err("Queued ownership/upload counts disagree".into());
    }
    let mut services = 0;
    for _ in 0..8 {
        let mut cold = Host::restore(&h.checkpoint()?)?;
        let served = h.service_round(1)?;
        if served != cold.service_round(1)? {
            return Err("Cold queue worker diverged".into());
        }
        services += served.len();
        let f = frame(&h);
        if h.cycle(&f, &mut PortableCurveMath)? != cold.cycle(&f, &mut PortableCurveMath)?
            || h.checkpoint()? != cold.checkpoint()?
        {
            return Err("Cold queue round diverged".into());
        }
        if h.mixed().status().ducking_voices != 3 {
            return Err("Mixed queue census lost a voice".into());
        }
    }
    let mut command_events = command(
        &mut h,
        &Command::PauseAll {
            request: -7,
            exclude: 0,
        },
    )?;
    for sound in [100, 1001, 1002] {
        if h.is_sound_paused(rc_package::audio_source_lookup::Query {
            actor: 42,
            sound,
            auto_actor: false,
            engine_actor: 0,
        })? != -7
        {
            return Err("Mixed pause query disagrees".into());
        }
    }
    let clock = h
        .mixed()
        .pool()
        .nodes
        .iter()
        .map(|n| n.source.lifetime_words)
        .collect::<Vec<_>>();
    h.cycle(&frame(&h), &mut PortableCurveMath)?;
    if clock
        != h.mixed()
            .pool()
            .nodes
            .iter()
            .map(|n| n.source.lifetime_words)
            .collect::<Vec<_>>()
    {
        return Err("Paused mixed clocks advanced".into());
    }
    command_events += command(
        &mut h,
        &Command::PauseAll {
            request: 0,
            exclude: u32::MAX,
        },
    )?;
    let before = h.checkpoint()?;
    let mut bad = frame(&h);
    bad.params.clear();
    if h.cycle(&bad, &mut PortableCurveMath).is_ok() || h.checkpoint()? != before {
        return Err("Late queue failure was not atomic".into());
    }
    let mut bad: serde_json::Value = serde_json::from_slice(&before)?;
    bad["mixed"]["pool"]["slots"][0] = bad["mixed"]["pool"]["transient"].clone();
    if h.load(&serde_json::to_vec(&bad)?).is_ok() || h.checkpoint()? != before {
        return Err("Bad mixed queue restore changed state".into());
    }
    command_events += command(&mut h, &Command::StopAll { exclude: 0x80 })?;
    h.refresh_cues()?;
    if h.mixed().resources().counts() != (1, 0, 1) || h.mixed().status().current != 9 {
        return Err("Mixed cue protection failed".into());
    }
    command_events += command(&mut h, &Command::StopVoices { actor: 42 })?;
    let mut release_events = 0;
    for sound in [1001, 1002, 100] {
        release_events += h.unregister(sound, &replies(h.mixed()))?.len();
    }
    if h.mixed().resources().counts() != (0, 0, 0) || !h.waiting().is_empty() {
        return Err("Mixed queue teardown leaked owners".into());
    }
    let report = serde_json::json!({"passed":true,"channels":3,"queued_requests":4,"successful_admissions":admitted,"deleted_requests":deleted,"initial_data_uploads":uploads,"initial_queue_bindings":bindings,"rounds":8,"services":services,"common_census":3,"command_phases":4,"command_events":command_events,"cue_protected_stream":true,"cue_mask":9,"cold_pending_live_and_commands":true,"paused_clocks_stable":true,"late_error_rollback":true,"bad_restore_rollback":true,"release_events":release_events,"final_counts":h.mixed().resources().counts(),"real_audio_device":false});
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
