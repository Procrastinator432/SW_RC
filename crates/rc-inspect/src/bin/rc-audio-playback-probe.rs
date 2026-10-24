//! Real-WAV pool/update regression, with control requests only; no audio device.
use rc_package::{
    audio_buffer_bank as pcm, audio_census::Params, audio_curve::PortableCurveMath,
    audio_driver as driver, audio_duck::Settings, audio_fade::Fade, audio_geometry::SavedRadii,
    audio_playback_host::*, audio_source::Source, audio_source_runtime::Frame,
    audio_source_update::BaseInput, audio_upload::Environment, audio_wave::Pcm, world_audio::Radii,
};
use std::fs;
fn env() -> Environment {
    Environment {
        category: 0,
        category_gain_bits: 1f32.to_bits(),
        editor: false,
        eax_enabled: false,
        eax_version: 3,
        listener_bits: [0; 3],
        rsqrt_seed_bits: None,
    }
}
fn request(identity: u32, duration_bits: u32) -> Request {
    let mut counter = 10;
    let mut s = Source::new(&mut counter);
    s.allocated = true;
    s.sound = 100;
    s.original_sound = 100;
    s.flags = 0x100;
    s.identity = identity;
    s.priority_bits = 1f32.to_bits();
    s.volume_bits = 1f32.to_bits();
    s.output_volume_bits = 1f32.to_bits();
    s.pitch_bits = 1f32.to_bits();
    s.base_radius_bits = 100f32.to_bits();
    s.radii = SavedRadii::capture(&Radii::initial(100.).unwrap()).unwrap();
    s.fade = Fade::new(1.).unwrap();
    s.voice_exempt = true;
    Request {
        incoming: s,
        sound: driver::Sound {
            handle: 100,
            flags: 0,
            duration_bits,
            native_handle: 200,
        },
        engine_present: true,
        sound_valid: true,
        queued: 0,
        environment: env(),
    }
}
fn frame() -> Frame {
    Frame {
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
    }
}
fn run() -> Result<(), Box<dyn std::error::Error>> {
    let a: Vec<_> = std::env::args_os().collect();
    if a.len() != 3 {
        return Err("Usage: rc-audio-playback-probe ORIGINAL.wav REPORT.json".into());
    }
    let wave = fs::read(&a[1])?;
    let pcm = Pcm::from_wave(&wave)?;
    let duration = (pcm.bytes.len() as f64 / f64::from(pcm.info.byte_rate)) as f32;
    let mut h = Host::new(
        1,
        &[10, 11, 12],
        vec![1, 2, 3],
        Settings {
            attack: 0.3,
            release: 0.4,
            levels: [0.4, 0.5, 0.6],
        },
    )?;
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
    for id in [100, 102] {
        if h.play(&request(id, duration.to_bits()))?.node.is_none() {
            return Err("Failed initial source activation".into());
        }
    }
    let mut cold = Host::restore(&h.checkpoint()?)?;
    let mut base_events = 0;
    let mut late_events = 0;
    for _ in 0..8 {
        let base = h.update_base(0.05, &[frame(), frame()], &mut PortableCurveMath)?;
        if base != cold.update_base(0.05, &[frame(), frame()], &mut PortableCurveMath)? {
            return Err("Cold base/update mismatch".into());
        }
        base_events += base.len();
        let frames = vec![
            DriverFrame {
                environment: env(),
                delta_bits: 0.05f32.to_bits()
            };
            2
        ];
        let params = [Params {
            sound: 100,
            flags: 1,
        }];
        let late = h.late_tick(&frames, &params)?;
        if late != cold.late_tick(&frames, &params)? {
            return Err("Cold driver/census mismatch".into());
        }
        late_events += late.len();
        let saved = h.checkpoint()?;
        if saved != cold.checkpoint()? {
            return Err("Cold playback state mismatch".into());
        }
        cold = Host::restore(&saved)?;
    }
    let voices = h.status().ducking_voices;
    if voices != 2 || !h.ducking().active {
        return Err("Census did not drive ducking".into());
    }
    let mut r = request(104, duration.to_bits());
    r.incoming.priority_bits = 1000f32.to_bits();
    r.incoming.fade = Fade::new(0.)?;
    let p = h.play(&r)?;
    if p.node.is_none() || p.events != cold.play(&r)?.events {
        return Err("Cold eviction mismatch".into());
    }
    let stop = h.update_base(0.05, &[frame(), frame()], &mut PortableCurveMath)?;
    if stop != cold.update_base(0.05, &[frame(), frame()], &mut PortableCurveMath)?
        || !stop.iter().any(|e| {
            matches!(
                e,
                Event::Driver {
                    event: driver::Event::AlStop { .. },
                    ..
                }
            )
        })
    {
        return Err("Fade stop did not reach actual driver".into());
    }
    let released = h.unregister(100)?;
    if released != cold.unregister(100)?
        || h.checkpoint()? != cold.checkpoint()?
        || h.resources().counts() != (0, 0, 0)
    {
        return Err("Cold playback teardown mismatch".into());
    }
    let report = serde_json::json!({"passed":true,"pcm_bytes":pcm.bytes.len(),"slots":2,"activations":3,"rounds":8,"cold_restore_after_every_round":true,"counted_voices":voices,"ducking_active":true,"base_events":base_events,"late_events":late_events,"fade_stop_driver":true,"teardown_events":released.len(),"final_counts":h.resources().counts(),"real_audio_device":false});
    fs::write(&a[2], serde_json::to_vec_pretty(&report)?)?;
    println!("{report}");
    Ok(())
}
fn main() {
    if let Err(e) = run() {
        eprintln!("{e}");
        std::process::exit(1);
    }
}
