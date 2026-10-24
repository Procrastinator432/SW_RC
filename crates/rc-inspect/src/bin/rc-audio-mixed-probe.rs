//! Mixed prepared pool exercise with real PCM/Vorbis inputs; no device output.
use rc_package::{
    audio_buffer_bank::Registration,
    audio_census::Params,
    audio_curve::PortableCurveMath,
    audio_driver::{self as driver, Reply, Sound},
    audio_duck::Settings,
    audio_mixed_host::*,
    audio_resource_host::StreamRegistration,
    audio_source::{Init, Source},
    audio_source_runtime as runtime, audio_source_tick as tick,
    audio_source_update::BaseInput,
    audio_stream_bridge as bridge,
    audio_upload::Environment,
    audio_vorbis_host::Tokens,
};
use std::fs;
fn source(sound: u32, flags: u32, id: u32, priority: f32) -> Result<Source, String> {
    let mut counter = id;
    let mut s = Source::new(&mut counter);
    s.initialize(
        &Init {
            engine_present: true,
            sound,
            original_sound: sound,
            actor: 0,
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
            ambient_priority: priority,
            radius_caps: [0.; 3],
            voice_exempt: true,
        },
        &mut PortableCurveMath,
    )?;
    Ok(s)
}
fn replies(h: &Host) -> Vec<Reply> {
    (0..h.pool().nodes.len())
        .map(|n| Reply {
            source_state: 0x1012,
            unqueued: h.resources().stream(h.key(n).unwrap()).map_or([0; 4], |s| {
                s.tick.driver.buffers[0].storage[..4].try_into().unwrap()
            }),
        })
        .collect()
}
fn frames(h: &Host) -> Vec<Frame> {
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
            Frame {
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
fn run() -> Result<(), Box<dyn std::error::Error>> {
    let args: Vec<_> = std::env::args_os().collect();
    if args.len() != 6 {
        return Err(
            "Usage: rc-audio-mixed-probe TEMPLATE.json PCM.wav OGG_A.ogg OGG_B.ogg REPORT.json"
                .into(),
        );
    }
    let template: tick::Session = serde_json::from_slice(&fs::read(&args[1])?)?;
    let mut h = Host::new(
        1,
        &[10, 11, 12, 13],
        vec![1, 2, 3, 4],
        Settings {
            attack: 0.3,
            release: 0.4,
            levels: [0.4, 0.5, 0.6],
        },
    )?;
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
    h.admit_static(
        &source(100, 0x100, 6, 1.)?,
        &Sound {
            handle: 100,
            flags: 0,
            duration_bits: 0,
            native_handle: 200,
        },
        &replies(&h),
    )?;
    let mut initial_uploads = 0;
    for (n, path) in [(1, &args[3]), (2, &args[4])] {
        let mut t = template.clone();
        t.driver.node.source = source(1000 + n, 0x104, 6 + 16 * n, n as f32 + 1.)?;
        t.driver.node.buffer = 2000 + n;
        t.driver.buffers[0].handle = 2000 + n;
        t.driver.sounds[0] = Sound {
            handle: 1000 + n,
            flags: 3,
            duration_bits: 0,
            native_handle: 0,
        };
        let r = StreamRegistration {
            key: h.key(h.pool().transient.unwrap())?,
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
        let out = h.admit_stream(&fs::read(path)?, &t, &r, &replies(&h))?;
        if out.node.is_none() {
            return Err("Stream admission refused".into());
        }
        initial_uploads += out.uploads.len();
    }
    if h.resources().counts() != (1, 1, 2) || initial_uploads != 8 {
        return Err("Mixed initial ownership mismatch".into());
    }
    let params = [
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
    ];
    let mut services = 0;
    let mut stream_ticks = 0;
    for _ in 0..8 {
        let mut cold = Host::restore(&h.checkpoint()?)?;
        let served = h.service_round(1)?;
        if served != cold.service_round(1)? {
            return Err("Cold mixed worker mismatch".into());
        }
        services += served.len();
        let f = frames(&h);
        let out = h.update(0.05, &f, &params, &mut PortableCurveMath)?;
        if out != cold.update(0.05, &f, &params, &mut PortableCurveMath)?
            || h.checkpoint()? != cold.checkpoint()?
        {
            return Err("Cold mixed update mismatch".into());
        }
        stream_ticks += out
            .iter()
            .filter(|e| matches!(e, Event::StreamTick { .. }))
            .count();
        if h.status().ducking_voices != 3 {
            return Err("Common census must count all three sources".into());
        }
    }
    if !h.ducking().active {
        return Err("Common census did not drive ducking".into());
    }
    let stream_node = h
        .pool()
        .slots
        .iter()
        .flatten()
        .copied()
        .find(|&n| h.pool().nodes[n].source.flags & 4 != 0)
        .ok_or("Missing stream node")?;
    h.control(
        stream_node,
        &driver::Command::Pause(1),
        &replies(&h)[stream_node],
    )?;
    let clock = h.pool().nodes[stream_node].source.lifetime_words;
    h.update(0.05, &frames(&h), &params, &mut PortableCurveMath)?;
    if h.pool().nodes[stream_node].source.lifetime_words != clock {
        return Err("Paused stream clock advanced".into());
    }
    h.control(
        stream_node,
        &driver::Command::Pause(0),
        &replies(&h)[stream_node],
    )?;
    let before = h.checkpoint()?;
    if h.update(0.05, &frames(&h), &[], &mut PortableCurveMath)
        .is_ok()
        || h.checkpoint()? != before
    {
        return Err("Late census failure was not atomic".into());
    }
    let bad = Reply {
        source_state: 0x1014,
        unqueued: [0; 4],
    };
    if h.control(stream_node, &driver::Command::Stop, &bad).is_ok() || h.checkpoint()? != before {
        return Err("Bad disposal changed mixed state".into());
    }
    let replacement = h.admit_static(
        &source(100, 0x100, 54, 100.)?,
        &Sound {
            handle: 100,
            flags: 0,
            duration_bits: 0,
            native_handle: 200,
        },
        &replies(&h),
    )?;
    if replacement.node.is_none() {
        return Err("Mixed eviction failed".into());
    }
    let mut release_events = 0;
    for sound in [1001, 1002, 100] {
        release_events += h.unregister(sound, &replies(&h))?.len();
    }
    if h.resources().counts() != (0, 0, 0) {
        return Err("Mixed teardown leaked owners".into());
    }
    let report = serde_json::json!({"passed":true,"channels":3,"initial_static":1,"initial_streams":2,"initial_uploads":initial_uploads,"rounds":8,"worker_budget":1,"services":services,"stream_ticks":stream_ticks,"common_census":3,"ducking_active":true,"cold_restore_each_round":true,"paused_clock_stable":true,"late_error_rollback":true,"bad_disposal_rollback":true,"mixed_eviction":true,"release_events":release_events,"final_counts":h.resources().counts(),"real_audio_device":false});
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
