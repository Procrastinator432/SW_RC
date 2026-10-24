//! Real static WAV control/reservation/cue lifetime exercise, without a device.
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
fn command(h: &mut Host, c: &Command) -> Result<usize, String> {
    let bank = [rc_package::audio_commands::Sound {
        handle: 100,
        default_fade: 0.2,
        streamed: false,
        curve: 0,
        category: 1,
    }];
    let mut cold = Host::restore(&h.checkpoint()?)?;
    let out = h.dispatch(c, &bank, &mut PortableCurveMath)?;
    if out.events != cold.dispatch(c, &bank, &mut PortableCurveMath)?.events
        || h.checkpoint()? != cold.checkpoint()?
    {
        return Err("Cold command diverged".into());
    }
    Ok(out.events.len())
}
fn cycle(h: &mut Host, delta: f32) -> Result<Vec<Event>, String> {
    let mut f = frame(h, 0x1012);
    for a in &mut f.active {
        a.delta_bits = delta.to_bits();
    }
    let mut cold = Host::restore(&h.checkpoint()?)?;
    let events = h.cycle(&f, &mut PortableCurveMath)?;
    if events != cold.cycle(&f, &mut PortableCurveMath)? || h.checkpoint()? != cold.checkpoint()? {
        return Err("Cold cycle diverged".into());
    }
    Ok(events)
}
fn run() -> Result<(), Box<dyn std::error::Error>> {
    let args: Vec<_> = std::env::args_os().collect();
    if args.len() != 3 {
        return Err("Usage: rc-audio-control-probe ORIGINAL.wav REPORT.json".into());
    }
    let wave = fs::read(&args[1])?;
    let pcm = Pcm::from_wave(&wave)?;
    let sound = Sound {
        handle: 100,
        flags: 0,
        duration_bits: ((pcm.bytes.len() as f64 / f64::from(pcm.info.byte_rate)) as f32).to_bits(),
        native_handle: 200,
    };
    let playback = playback::Host::new(
        1,
        &[10, 11, 12],
        vec![1, 2, 3],
        Settings {
            attack: 0.3,
            release: 0.4,
            levels: [0.4, 0.5, 0.6],
        },
    )?;
    let mut h = Host::new(playback, 100)?;
    h.register_pcm(
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
    h.configure_reservation(rc_package::audio_reservation::Config {
        limit: 2,
        names: vec![rc_package::audio_reservation::NamedSound {
            sound: 100,
            name: "imp_sample".into(),
        }],
    })?;
    for (index, (p, spatial)) in [(1., true), (0.5, false), (2., false)]
        .into_iter()
        .enumerate()
    {
        let mut i = init(p);
        i.identity = 6 + 16 * index as u32;
        i.actor = 42;
        if spatial {
            i.flags = 0x40;
        }
        h.enqueue(&i, sound.clone(), &mut PortableCurveMath)?;
    }
    let first = cycle(&mut h, 0.05)?;
    if !first.contains(&Event::QueuedStop(0))
        || first
            .iter()
            .filter(|e| matches!(e, Event::Attempt { node: Some(_), .. }))
            .count()
            != 2
    {
        return Err("Reservation promotion failed".into());
    }
    let mut command_events = command(
        &mut h,
        &Command::PauseAll {
            request: 1,
            exclude: 0,
        },
    )?;
    if h.is_sound_paused(rc_package::audio_source_lookup::Query {
        actor: 42,
        sound: 100,
        auto_actor: false,
        engine_actor: 0,
    })? != 1
    {
        return Err("Actual pause query failed".into());
    }
    command_events += command(
        &mut h,
        &Command::PauseSound {
            actor: 42,
            sound: 100,
            request: 0,
        },
    )?;
    h.enqueue(&init(3.), sound.clone(), &mut PortableCurveMath)?;
    command_events += command(
        &mut h,
        &Command::StopSound {
            actor: 0,
            sound: 100,
            fade_bits: (-1f32).to_bits(),
        },
    )?;
    if h.waiting()[0].source.allocated {
        return Err("Pending stop was incorrectly faded".into());
    }
    let faded = cycle(&mut h, 0.3)?;
    let fade_stops = faded
        .iter()
        .filter(|e| {
            matches!(
                e,
                Event::ActiveBase {
                    event: playback::Event::Driver {
                        event: rc_package::audio_driver::Event::AlStop { .. },
                        ..
                    },
                    ..
                }
            )
        })
        .count();
    if fade_stops != 2 {
        return Err("Both fading drivers must stop".into());
    }
    let mut spatial = init(2.);
    spatial.flags = 0x40;
    spatial.identity = 70;
    h.enqueue(&spatial, sound.clone(), &mut PortableCurveMath)?;
    let cue = h.waiting()[0].source.unique_id;
    h.set_cue_id(cue);
    let mut second = init(0.5);
    second.identity = 86;
    h.enqueue(&second, sound, &mut PortableCurveMath)?;
    cycle(&mut h, 0.05)?;
    command_events += command(&mut h, &Command::StopAll { exclude: 0x80 })?;
    let live = h
        .playback()
        .pool()
        .nodes
        .iter()
        .filter(|n| n.source.allocated)
        .count();
    if live != 1 {
        return Err("Cue protection did not preserve exactly one driver".into());
    }
    h.set_pending_cues(8);
    h.refresh_cues()?;
    if h.playback().status().current != 9 {
        return Err("Cue refresh did not use actual live cursor".into());
    }
    let before = h.checkpoint()?;
    let mut corrupt: serde_json::Value = serde_json::from_slice(&before)?;
    corrupt["reservation"]["names"] = serde_json::json!([]);
    if h.load(&serde_json::to_vec(&corrupt)?).is_ok() || h.checkpoint()? != before {
        return Err("Invalid name restore changed host".into());
    }
    let release = h.unregister(100)?;
    if h.playback().resources().counts() != (0, 0, 0) || !h.waiting().is_empty() {
        return Err("Resources were not fully released".into());
    }
    let report = serde_json::json!({"passed":true,"pcm_bytes":pcm.bytes.len(),"slots":2,"successful_admissions":4,"reservation_promoted":true,"commands":4,"command_events":command_events,"fade_driver_stops":fade_stops,"cue_protected_live":live,"cue_mask":9,"cold_command_and_cycle_restore":true,"invalid_name_restore_atomic":true,"release_events":release.len(),"final_counts":h.playback().resources().counts(),"real_audio_device":false});
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
