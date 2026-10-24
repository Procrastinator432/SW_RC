use crate::{
    audio_buffer_bank::Registration,
    audio_curve::PortableCurveMath,
    audio_device::*,
    audio_device_session::Session,
    audio_mixed_cycle as cycle, audio_mixed_cycle_tests as owner, audio_mixed_host as mixed,
    audio_openal::OpenAl,
    audio_playback_host_tests::{env, settings},
    audio_source::Init,
};
use std::path::PathBuf;
fn library() -> PathBuf {
    if let Some(path) = std::env::var_os("RC_OPENAL_SOFT_DLL") {
        return PathBuf::from(path);
    }
    PathBuf::from(env!("CARGO_MANIFEST_DIR"))
        .join("../../tools/openal-soft/openal-soft-1.25.2-bin/bin/Win64/soft_oal.dll")
}
fn executor() -> Executor<OpenAl> {
    let mut e = Executor::new(OpenAl::open_loopback(&library(), 48000).unwrap(), 1).unwrap();
    e.create(Kind::Source, 10).unwrap();
    for id in 100..104 {
        e.create(Kind::Buffer, id).unwrap();
    }
    e.create(Kind::Context, 42).unwrap();
    e
}
// Encode S16 explicitly, rather than native f64 bytes.
fn pcm(frames: usize) -> Vec<u8> {
    (0..frames)
        .flat_map(|i| {
            (((i as f64 * 440. * std::f64::consts::TAU / 48000.).sin() * 12000.) as i16)
                .to_le_bytes()
        })
        .collect()
}
fn upload(buffer: u32, frames: usize) -> Op {
    Op::Upload {
        buffer,
        format: 0x1101,
        frequency: 48000,
        bytes: pcm(frames),
    }
}
fn play(e: &mut Executor<OpenAl>, frames: usize, looping: bool) {
    let p = e
        .prepare(&[
            upload(100, frames),
            Op::Integer {
                source: 10,
                parameter: 0x1009,
                value: 100,
            },
            Op::Integer {
                source: 10,
                parameter: 0x1007,
                value: u32::from(looping),
            },
            Op::Play(10),
        ])
        .unwrap();
    e.apply(&p).unwrap();
}
fn rms(samples: &[i16]) -> f64 {
    (samples.iter().map(|&s| f64::from(s).powi(2)).sum::<f64>() / samples.len() as f64).sqrt()
}
fn crossings(samples: &[i16]) -> usize {
    samples
        .chunks_exact(2)
        .map(|s| s[0])
        .collect::<Vec<_>>()
        .windows(2)
        .filter(|w| w[0] <= 0 && w[1] > 0)
        .count()
}
#[test]
fn explicit_loader_and_rate_failures_do_not_open_contexts() {
    assert!(OpenAl::open_loopback(&library(), 0).is_err());
    assert!(OpenAl::open_loopback(&library().with_file_name("missing-rc.dll"), 48000).is_err());
    assert!(OpenAl::open_loopback(
        std::path::Path::new("C:/Windows/System32/kernel32.dll"),
        48000
    )
    .is_err());
}
#[test]
fn actual_mixer_produces_pcm_and_finite_source_reaches_stopped() {
    let mut e = executor();
    play(&mut e, 4800, false);
    let stale = e.prepare(&[Op::Check("stale".into())]).unwrap();
    let rendered = e.render(9600).unwrap();
    assert!(rms(&rendered.samples[..9600]) > 500.);
    assert_eq!(e.observe(10, 0x1010).unwrap(), 0x1014);
    assert!(e.apply(&stale).is_err());
    assert!(e.fault().is_none());
}
#[test]
fn actual_pause_and_stop_render_silence_after_settling() {
    let mut e = executor();
    play(&mut e, 48000, true);
    assert!(rms(&e.render(4096).unwrap().samples) > 500.);
    e.apply(&e.prepare(&[Op::Pause(10)]).unwrap()).unwrap();
    e.render(512).unwrap();
    assert!(rms(&e.render(4096).unwrap().samples) < 2.);
    assert_eq!(e.observe(10, 0x1010).unwrap(), 0x1013);
    e.apply(&e.prepare(&[Op::Play(10)]).unwrap()).unwrap();
    assert!(rms(&e.render(4096).unwrap().samples) > 500.);
    e.apply(&e.prepare(&[Op::Stop(10)]).unwrap()).unwrap();
    e.render(512).unwrap();
    assert!(rms(&e.render(4096).unwrap().samples) < 2.);
}
#[test]
fn four_real_buffers_are_consumed_unqueued_and_deleted() {
    let mut e = executor();
    let mut ops = (100..104).map(|id| upload(id, 512)).collect::<Vec<_>>();
    ops.extend([
        Op::Queue {
            source: 10,
            buffers: (100..104).collect(),
        },
        Op::Play(10),
    ]);
    e.apply(&e.prepare(&ops).unwrap()).unwrap();
    e.render(4096).unwrap();
    assert_eq!(e.observe(10, 0x1016).unwrap(), 4);
    let mut ops = vec![Op::Unqueue {
        source: 10,
        expected: (100..104).collect(),
        exact_order: true,
    }];
    ops.extend((100..104).map(Op::DeleteBuffer));
    let out = e.apply(&e.prepare(&ops).unwrap()).unwrap();
    assert_eq!(out.replies[0], Reply::Buffers((100..104).collect()));
    assert_eq!(e.backend().live_objects().1, 0);
}
#[test]
fn real_partial_refill_returns_owned_id_before_new_bytes_are_queued() {
    let mut e = executor();
    let mut ops = (100..104).map(|id| upload(id, 512)).collect::<Vec<_>>();
    ops.extend([
        Op::Queue {
            source: 10,
            buffers: (100..104).collect(),
        },
        Op::Play(10),
    ]);
    e.apply(&e.prepare(&ops).unwrap()).unwrap();
    e.render(600).unwrap();
    let processed = e.observe(10, 0x1016).unwrap();
    assert!((1..4).contains(&processed));
    let out = e
        .apply(
            &e.prepare(&[
                Op::Unqueue {
                    source: 10,
                    expected: vec![100],
                    exact_order: true,
                },
                upload(100, 512),
                Op::Queue {
                    source: 10,
                    buffers: vec![100],
                },
            ])
            .unwrap(),
        )
        .unwrap();
    assert_eq!(out.replies[0], Reply::Buffers(vec![100]));
    assert_eq!(e.observe(10, 0x1015).unwrap(), 4);
}
#[test]
fn actual_gain_and_pitch_change_the_rendered_signal() {
    let mut e = executor();
    play(&mut e, 48000, true);
    e.render(1024).unwrap();
    let loud = rms(&e.render(8192).unwrap().samples);
    e.apply(
        &e.prepare(&[Op::Float {
            source: 10,
            parameter: 0x100a,
            value: 0.25f32.to_bits(),
        }])
        .unwrap(),
    )
    .unwrap();
    e.render(1024).unwrap();
    let quiet = rms(&e.render(8192).unwrap().samples);
    assert!((0.22..0.28).contains(&(quiet / loud)));
    e.apply(
        &e.prepare(&[Op::Float {
            source: 10,
            parameter: 0x1003,
            value: 0.7f32.to_bits(),
        }])
        .unwrap(),
    )
    .unwrap();
    e.render(1024).unwrap();
    let low = crossings(&e.render(24000).unwrap().samples);
    e.apply(
        &e.prepare(&[Op::Float {
            source: 10,
            parameter: 0x1003,
            value: 1.2f32.to_bits(),
        }])
        .unwrap(),
    )
    .unwrap();
    e.render(1024).unwrap();
    let high = crossings(&e.render(24000).unwrap().samples);
    assert!((1.6..1.85).contains(&(high as f64 / low as f64)));
}
#[test]
fn real_al_error_quarantines_then_new_epoch_rebuilds() {
    let mut e = executor();
    let p = e
        .prepare(&[
            upload(100, 4800),
            Op::Integer {
                source: 10,
                parameter: 0x1009,
                value: 100,
            },
            Op::Play(10),
            Op::DeleteBuffer(100),
        ])
        .unwrap();
    let f = e.apply(&p).unwrap_err();
    assert_eq!(f.completed, 3);
    assert!(f.reason.contains("0xa004"));
    let manifest = e.checkpoint_manifest().unwrap();
    e.restart(&manifest).unwrap();
    assert!(e.apply(&p).is_err());
    play(&mut e, 4800, false);
    assert!(rms(&e.render(2048).unwrap().samples) > 500.);
}
fn wav() -> Vec<u8> {
    let data = pcm(48000);
    let mut b = crate::audio_playback_host_tests::wave();
    b.resize(44, 0);
    b[4..8].copy_from_slice(&(36 + data.len() as u32).to_le_bytes());
    b[24..28].copy_from_slice(&48000u32.to_le_bytes());
    b[28..32].copy_from_slice(&96000u32.to_le_bytes());
    b[40..44].copy_from_slice(&(data.len() as u32).to_le_bytes());
    b.extend(data);
    b
}
#[test]
fn actual_session_rebuilds_a_paused_source_without_mutating_owner() {
    let mixed = mixed::Host::new(1, &[10, 11], vec![1, 2], settings()).unwrap();
    let mut h = cycle::Host::new(mixed, 100).unwrap();
    h.register_pcm(
        &wav(),
        &Registration {
            sound: 100,
            handle: 200,
            allocation: 300,
            generated: 400,
            allow_stereo: false,
            generation_error: 0,
            upload_error: 0,
        },
    )
    .unwrap();
    let i = Init {
        engine_present: true,
        sound: 100,
        original_sound: 100,
        actor: 42,
        flags: 0x100,
        identity: 6,
        engine_index: -1,
        location: [0.; 3],
        volume: 1.,
        pitch: 1.,
        initial_output: 1.,
        fade_seconds: 0.,
        fade_curve: 0,
        radius: 40.,
        ambient_priority: 1.,
        radius_caps: [0.; 3],
        voice_exempt: true,
    };
    h.enqueue_static(
        &i,
        crate::audio_driver::Sound {
            handle: 100,
            flags: 0,
            duration_bits: 0,
            native_handle: 200,
        },
        &mut PortableCurveMath,
    )
    .unwrap();
    let mut d = Executor::new(OpenAl::open_loopback(&library(), 48000).unwrap(), 1).unwrap();
    for id in [10, 11] {
        d.create(Kind::Source, id).unwrap();
    }
    d.create(Kind::Buffer, 400).unwrap();
    d.create(Kind::Context, 42).unwrap();
    let mut s = Session::new(h, d).unwrap();
    let manifest = s.manifest().unwrap();
    s.restart(&manifest, &vec![env(); 2]).unwrap();
    s.cycle(&owner::frame(s.owner()), &mut PortableCurveMath)
        .unwrap();
    assert!(rms(&s.render(4096).unwrap().samples) > 500.);
    s.dispatch(
        &cycle::Command::PauseSound {
            actor: 42,
            sound: 100,
            request: 1,
        },
        &owner::bank(),
        &mut PortableCurveMath,
        &crate::audio_mixed_host_tests::replies(s.owner().mixed()),
    )
    .unwrap();
    let before = s.owner().checkpoint().unwrap();
    let manifest = s.manifest().unwrap();
    s.restart(&manifest, &vec![env(); 2]).unwrap();
    assert_eq!(s.owner().checkpoint().unwrap(), before);
    assert!(rms(&s.render(4096).unwrap().samples) < 2.);
}
#[test]
fn independent_backends_restore_each_others_current_context() {
    let mut a = executor();
    let mut b = executor();
    play(&mut a, 48000, true);
    play(&mut b, 48000, true);
    b.apply(
        &b.prepare(&[Op::Float {
            source: 10,
            parameter: 0x1003,
            value: 1.2f32.to_bits(),
        }])
        .unwrap(),
    )
    .unwrap();
    a.render(1024).unwrap();
    b.render(1024).unwrap();
    let low = crossings(&a.render(24000).unwrap().samples);
    let high = crossings(&b.render(24000).unwrap().samples);
    assert!((1.15..1.25).contains(&(high as f64 / low as f64)));
}
#[test]
fn repeated_real_reset_and_drop_release_source_and_buffer_objects() {
    for _ in 0..4 {
        let mut e = executor();
        play(&mut e, 1024, false);
        let manifest = e.checkpoint_manifest().unwrap();
        e.restart(&manifest).unwrap();
        assert_eq!(e.backend().live_objects(), (1, 4));
        assert!(!e.backend().version().is_empty());
    }
}
