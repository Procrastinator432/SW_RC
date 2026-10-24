//! Real static WAV queue/cycle exercise; AL responses are supplied, no device.
use rc_package::{
    audio_buffer_bank::Registration,
    audio_census::Params,
    audio_curve::PortableCurveMath,
    audio_driver::{Reply, Sound},
    audio_duck::Settings,
    audio_playback_host as playback,
    audio_queue_cycle::*,
    audio_source::Init,
    audio_source_runtime as runtime,
    audio_source_update::BaseInput,
    audio_upload::Environment,
    audio_wave::Pcm,
};
use std::fs;
fn environment() -> Environment {
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
fn init(priority: f32) -> Init {
    Init {
        engine_present: true,
        sound: 100,
        original_sound: 100,
        actor: 0,
        flags: 0x100,
        identity: 6,
        engine_index: -1,
        location: [0.; 3],
        volume: 0.75,
        pitch: 1.,
        initial_output: 0.75,
        fade_seconds: 0.,
        fade_curve: 0,
        radius: 40.,
        ambient_priority: priority,
        radius_caps: [0.; 3],
        voice_exempt: true,
    }
}
fn frame(h: &Host, state: i32) -> Frame {
    let base = SourceFrame {
        delta_bits: 0.05f32.to_bits(),
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
    };
    Frame {
        global_delta_bits: 0.05f32.to_bits(),
        cleanup: vec![
            Reply {
                source_state: state,
                unqueued: [0; 4]
            };
            2
        ],
        active: vec![base.clone(); 2],
        queued: vec![
            QueuedFrame {
                source: base,
                environment: environment(),
                engine_present: true,
                sound_valid: true
            };
            h.waiting().len()
        ],
        drivers_by_node: vec![
            playback::DriverFrame {
                environment: environment(),
                delta_bits: 0.05f32.to_bits()
            };
            3
        ],
        params: vec![Params {
            sound: 100,
            flags: 1,
        }],
        context: 42,
    }
}
fn run() -> Result<(), Box<dyn std::error::Error>> {
    let args: Vec<_> = std::env::args_os().collect();
    if args.len() != 3 {
        return Err("Usage: rc-audio-cycle-probe ORIGINAL.wav REPORT.json".into());
    }
    let wave = fs::read(&args[1])?;
    let pcm = Pcm::from_wave(&wave)?;
    let sound = Sound {
        handle: 100,
        flags: 0,
        duration_bits: ((pcm.bytes.len() as f64 / f64::from(pcm.info.byte_rate)) as f32).to_bits(),
        native_handle: 200,
    };
    let mut playback = playback::Host::new(
        1,
        &[10, 11, 12],
        vec![1, 2, 3],
        Settings {
            attack: 0.3,
            release: 0.4,
            levels: [0.4, 0.5, 0.6],
        },
    )?;
    playback.register_pcm(
        &wave,
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
    let mut h = Host::new(playback, 0)?;
    for p in [0.5, 2., 3.] {
        h.enqueue(&init(p), sound.clone(), &mut PortableCurveMath)?;
    }
    let mut events = Vec::new();
    for round in 0..8 {
        if round == 3 {
            h.enqueue(&init(4.), sound.clone(), &mut PortableCurveMath)?;
        }
        let f = frame(&h, if round == 3 { 0x1014 } else { 0x1012 });
        let mut cold = Host::restore(&h.checkpoint()?)?;
        let out = h.cycle(&f, &mut PortableCurveMath)?;
        if out != cold.cycle(&f, &mut PortableCurveMath)? || h.checkpoint()? != cold.checkpoint()? {
            return Err("Cold cycle diverged".into());
        }
        events.extend(out);
    }
    h.enqueue(&init(5.), sound, &mut PortableCurveMath)?;
    let before = h.checkpoint()?;
    let mut bad = frame(&h, 0x1014);
    bad.params.clear();
    if h.cycle(&bad, &mut PortableCurveMath).is_ok() || h.checkpoint()? != before {
        return Err("Late failure did not preserve full cycle".into());
    }
    events.extend(h.cycle(&frame(&h, 0x1014), &mut PortableCurveMath)?);
    events.extend(h.cycle(&frame(&h, 0x1014), &mut PortableCurveMath)?);
    if h.playback().pool().nodes.iter().any(|n| n.source.allocated) || !h.waiting().is_empty() {
        return Err("Completion did not retire every static source".into());
    }
    let admissions = events
        .iter()
        .filter(|e| matches!(e, Event::Attempt { node: Some(_), .. }))
        .count();
    let discarded = events
        .iter()
        .filter(|e| matches!(e, Event::Discard { .. }))
        .count();
    if admissions != 4 || discarded != 5 {
        return Err("Unexpected admission/discard count".into());
    }
    let report = serde_json::json!({"passed":true,"pcm_bytes":pcm.bytes.len(),"slots":2,"rounds":8,"total_cycles":10,"successful_admissions":admissions,"discarded_requests":discarded,"events":events.len(),"cold_restore_every_round":true,"late_error_rollback":true,"final_live_sources":0,"cached_pcm_entries":h.playback().resources().counts().0,"real_audio_device":false});
    fs::write(&args[2], serde_json::to_vec_pretty(&report)?)?;
    println!("{report}");
    Ok(())
}
fn main() {
    if let Err(e) = run() {
        eprintln!("{e}");
        std::process::exit(1);
    }
}
