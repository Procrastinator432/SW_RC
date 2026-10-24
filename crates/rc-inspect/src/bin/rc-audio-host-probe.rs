//! Mixed real-asset host regression. No device IO; no original PCM in reports.
use rc_package::{
    audio_buffer_bank as pcm, audio_driver as driver, audio_resource_host::*,
    audio_source_tick as tick, audio_stream_bridge as bridge, audio_upload::Environment,
    audio_vorbis_host::Tokens,
};
use std::{fs, path::Path};
fn prepared(
    template: &tick::Session,
    sound: u32,
    buffer: u32,
    hardware: u32,
    stream: bool,
) -> tick::Session {
    let mut t = template.clone();
    t.driver.node.source.sound = sound;
    t.driver.node.source.original_sound = sound;
    t.driver.node.source.flags = if stream { 0x104 } else { 0x100 };
    t.driver.node.hardware = hardware;
    t.driver.node.buffer = buffer;
    t.driver.buffers[0].handle = buffer;
    t.driver.buffers[0].stream_token = 0;
    t.driver.sounds[0].handle = sound;
    t.driver.sounds[0].flags = if stream { 3 } else { 0 };
    t
}
fn step(h: &Host, key: u32) -> bridge::Tick {
    let s = h.stream(key).unwrap();
    bridge::Tick {
        environment: Environment {
            category: 0,
            category_gain_bits: 1f32.to_bits(),
            editor: false,
            eax_enabled: false,
            eax_version: 3,
            listener_bits: [0; 3],
            rsqrt_seed_bits: None,
        },
        al: bridge::AlReply {
            state: 0x1012,
            processed: 1,
            queued: 4,
            unqueued: s.tick.driver.buffers[0].storage[s.tick.progress[0].indices[1] as usize],
        },
        delta_bits: (1f32 / 60.).to_bits(),
    }
}
fn run() -> Result<(), Box<dyn std::error::Error>> {
    let a: Vec<_> = std::env::args_os().collect();
    if a.len() != 6 {
        return Err(
            "Usage: rc-audio-host-probe TEMPLATE.json PCM.wav OGG_A.ogg OGG_B.ogg REPORT.json"
                .into(),
        );
    }
    let template: tick::Session = serde_json::from_slice(&fs::read(&a[1])?)?;
    let wave = fs::read(&a[2])?;
    let mut h = Host::new(1)?;
    h.register_pcm(
        &wave,
        &pcm::Registration {
            sound: 100,
            handle: 200,
            allocation: 300,
            generated: 400,
            allow_stereo: true,
            generation_error: 0,
            upload_error: 0,
        },
    )?;
    for key in [1, 2] {
        let mut t = prepared(&template, 100, 200, key, false);
        h.attach_static(key, t.driver.node, t.driver.sounds.remove(0))?;
        h.control_static(
            key,
            &driver::Command::Play,
            &driver::Reply {
                source_state: 0x1012,
                unqueued: [0; 4],
            },
        )?;
    }
    for (n, path) in [(1, &a[3]), (2, &a[4])] {
        let t = prepared(&template, 1000 + n, 2000 + n, 3000 + n, true);
        h.register_stream(
            &fs::read(path)?,
            t,
            &StreamRegistration {
                key: 10 + n,
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
        )?;
        h.control_stream(
            10 + n,
            &driver::Command::Play,
            &driver::Reply {
                source_state: 0x1012,
                unqueued: [0; 4],
            },
        )?;
    }
    let mut services = 0;
    let mut uploads = 0;
    let mut cold = Host::restore(&h.checkpoint()?)?;
    for _ in 0..8 {
        let a = h.service_round(2)?;
        if a != cold.service_round(2)? {
            return Err("Cold service round mismatch".into());
        }
        services += a.len();
        for s in a {
            let t = step(&h, s.source);
            let a = h.tick_stream(s.source, &t)?;
            if a != cold.tick_stream(s.source, &t)? {
                return Err("Cold driver tick mismatch".into());
            }
            uploads += a.uploads.len();
        }
        let saved = h.checkpoint()?;
        if saved != cold.checkpoint()? {
            return Err("Mixed resource checkpoint mismatch".into());
        }
        cold = Host::restore(&saved)?;
    }
    let unregistered = h.unregister(100, None)?;
    if unregistered != cold.unregister(100, None)? {
        return Err("Cold shared PCM disposal mismatch".into());
    }
    let replies = [11, 12].map(|source| {
        let s = h.stream(source).unwrap();
        StreamReply {
            source,
            reply: driver::Reply {
                source_state: 0x1012,
                unqueued: s.tick.driver.buffers[0].storage[..4].try_into().unwrap(),
            },
        }
    });
    let shutdown = h.shutdown(&replies)?;
    if shutdown != cold.shutdown(&replies)?
        || h.checkpoint()? != cold.checkpoint()?
        || !h.is_closed()
    {
        return Err("Cold mixed shutdown mismatch".into());
    }
    let report = serde_json::json!({"passed":true,"static_registrations":1,"shared_static_voices":2,"vorbis_streams":2,"rounds":8,"services":services,"refill_uploads":uploads,"cold_restore_after_every_round":true,"pcm_unregister_events":unregistered.len(),"shutdown_events":shutdown.len(),"final_counts":h.counts(),"closed":h.is_closed(),"real_audio_device":false});
    fs::write(Path::new(&a[5]), serde_json::to_vec_pretty(&report)?)?;
    println!("{}", report);
    Ok(())
}
fn main() {
    if let Err(e) = run() {
        eprintln!("{e}");
        std::process::exit(1);
    }
}
