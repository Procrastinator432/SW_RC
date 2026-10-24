use crate::{
    audio_buffer_bank::Registration,
    audio_commands as commands,
    audio_curve::PortableCurveMath,
    audio_driver as driver,
    audio_mixed_cycle::*,
    audio_mixed_host as mixed, audio_mixed_host_tests as helpers,
    audio_playback_host_tests::{env, frame as base, settings, wave},
    audio_queue_cycle_tests::init,
    audio_reservation::{Config, NamedSound},
    audio_source::Source,
    audio_source_lookup::Query,
};
use serde_json::{json, Value};
const OGG: &[u8] = include_bytes!("../../../fixtures/audio-vorbis/stereo.ogg");
pub(super) fn host(slots: usize) -> Host {
    Host::new(helpers::host(slots), 100).unwrap()
}
pub(super) fn add_static(h: &mut Host, id: u32, p: f32) {
    let mut i = init(p);
    i.identity = id;
    i.actor = 42;
    i.voice_exempt = true;
    h.enqueue_static(
        &i,
        driver::Sound {
            handle: 100,
            flags: 0,
            duration_bits: 0,
            native_handle: 200,
        },
        &mut PortableCurveMath,
    )
    .unwrap();
}
pub(super) fn add_stream(h: &mut Host, n: u32, p: f32) {
    let mut i = init(p);
    i.identity = 6 + 16 * n;
    i.sound = 1000 + n;
    i.original_sound = i.sound;
    i.flags = 0x104;
    i.actor = 42;
    i.voice_exempt = true;
    h.enqueue_stream(
        &i,
        OGG.to_vec(),
        helpers::template(n, p),
        helpers::registration(h.mixed(), n),
        &mut PortableCurveMath,
    )
    .unwrap();
}
pub(super) fn frame(h: &Host) -> Frame {
    Frame {
        global_delta_bits: 0.1f32.to_bits(),
        nodes: helpers::frames(h.mixed()),
        queued: h
            .waiting()
            .iter()
            .map(|_| QueuedFrame {
                base: base(),
                delta_bits: 0.1f32.to_bits(),
                environment: env(),
            })
            .collect(),
        params: helpers::params(),
        context: 42,
    }
}
pub(super) fn bank() -> Vec<commands::Sound> {
    vec![
        commands::Sound {
            handle: 100,
            default_fade: 0.2,
            streamed: false,
            curve: 0,
            category: 0,
        },
        commands::Sound {
            handle: 1001,
            default_fade: 0.2,
            streamed: true,
            curve: 1,
            category: 1,
        },
        commands::Sound {
            handle: 1002,
            default_fade: 0.2,
            streamed: true,
            curve: 0,
            category: 1,
        },
    ]
}
#[test]
fn whole_original_mixed_upload_matches_all_events_pool_and_eax_cache() {
    let data: Value = serde_json::from_str(include_str!(
        "../../../fixtures/audio-mixed-upload-native.json"
    ))
    .unwrap();
    for (index, c) in data["cases"].as_array().unwrap().iter().enumerate() {
        let count = c["before"]["slots"].as_array().unwrap().len();
        let h = mixed::Host::new(
            1,
            &(0..count + 1).map(|n| 1000 + n as u32).collect::<Vec<_>>(),
            (1..=count as u32 + 1).collect(),
            settings(),
        )
        .unwrap();
        let mut saved: Value = serde_json::from_slice(&h.checkpoint().unwrap()).unwrap();
        saved["pool"] = c["before"].clone();
        saved["eax"] = c["cache"].clone();
        let mut h = mixed::Host::restore(&serde_json::to_vec(&saved).unwrap()).unwrap();
        let incoming: Source = serde_json::from_value(c["incoming"].clone()).unwrap();
        let environment = serde_json::from_value(c["environment"].clone()).unwrap();
        let buffer = c["request"]["buffer"].as_u64().unwrap() as u32;
        let out = if c["streaming"].as_bool().unwrap() {
            let mut t = helpers::template(1, 1.);
            t.driver.node.source = incoming.clone();
            t.driver.node.buffer = buffer;
            t.driver.buffers[0].handle = buffer;
            t.driver.sounds[0].handle = incoming.sound;
            let mut r = helpers::registration(&h, 1);
            r.tokens.buffers = [101, 102, 103, 104];
            h.play_stream(OGG, &t, &r, &helpers::replies(&h), &environment)
                .unwrap()
        } else {
            h.register_pcm(
                &wave(),
                &Registration {
                    sound: incoming.sound,
                    handle: buffer,
                    allocation: 300,
                    generated: 101,
                    allow_stereo: false,
                    generation_error: 0,
                    upload_error: 0,
                },
            )
            .unwrap();
            h.play_static(
                &incoming,
                &driver::Sound {
                    handle: incoming.sound,
                    flags: 0,
                    duration_bits: 0,
                    native_handle: buffer as i32,
                },
                &helpers::replies(&h),
                &environment,
            )
            .unwrap()
        };
        assert_eq!(
            serde_json::to_value(out.events).unwrap(),
            c["events"],
            "events {index}"
        );
        assert_eq!(
            serde_json::to_value(h.pool()).unwrap(),
            c["expected"],
            "pool {index}"
        );
        let saved: Value = serde_json::from_slice(&h.checkpoint().unwrap()).unwrap();
        assert_eq!(saved["eax"], c["cache_after"], "cache {index}");
    }
}
#[test]
fn mixed_queue_sorts_once_uploads_four_buffers_once_and_uses_one_owner() {
    let mut h = host(2);
    add_static(&mut h, 6, 1.);
    add_stream(&mut h, 1, 2.);
    let out = h.cycle(&frame(&h), &mut PortableCurveMath).unwrap();
    assert!(out.contains(&Event::Sorted(vec![
        Candidate::Queued(1),
        Candidate::Queued(0)
    ])));
    assert_eq!(
        out.iter().filter(|e| matches!(e, Event::Upload(_))).count(),
        4
    );
    assert_eq!(
        out.iter()
            .filter(|e| matches!(
                e,
                Event::Mixed(mixed::Event::Parameter(
                    crate::audio_upload::Event::Queue { .. }
                ))
            ))
            .count(),
        1
    );
    assert_eq!(h.mixed().resources().counts(), (1, 1, 1));
    assert_eq!(h.mixed().status().ducking_voices, 2);
    assert!(h.waiting().is_empty());
}
#[test]
fn queued_start_pause_normalizes_to_original_one_after_base_update() {
    let mut h = host(1);
    add_stream(&mut h, 1, 2.);
    h.pause(0, -7).unwrap();
    let out = h.cycle(&frame(&h), &mut PortableCurveMath).unwrap();
    assert!(out.iter().any(|e| matches!(
        e,
        Event::QueuedBase {
            event: crate::audio_source_runtime::Event::Volume(_),
            ..
        }
    )));
    assert_eq!(
        h.mixed()
            .pool()
            .nodes
            .iter()
            .find(|n| n.source.allocated)
            .unwrap()
            .source
            .requested_pause,
        1
    );
}
#[test]
fn matching_protected_id_refuses_stream_and_does_not_backfill() {
    let mut h = host(1);
    add_static(&mut h, 7, 1.);
    h.cycle(&frame(&h), &mut PortableCurveMath).unwrap();
    let mut i = init(2.);
    i.identity = 6;
    i.sound = 1001;
    i.original_sound = 1001;
    i.flags = 0x104;
    h.enqueue_stream(
        &i,
        OGG.to_vec(),
        helpers::template(1, 2.),
        helpers::registration(h.mixed(), 1),
        &mut PortableCurveMath,
    )
    .unwrap();
    add_static(&mut h, 22, 0.5);
    let out = h.cycle(&frame(&h), &mut PortableCurveMath).unwrap();
    assert!(out.contains(&Event::Attempt {
        index: 0,
        node: None
    }));
    assert_eq!(
        out.iter()
            .filter(|e| matches!(e, Event::Attempt { .. }))
            .count(),
        1
    );
    assert_eq!(h.mixed().resources().counts(), (1, 1, 0));
    assert!(h.waiting().is_empty());
}
#[test]
fn reservation_can_drop_queued_stream_and_promote_static_before_decode() {
    let mut h = host(1);
    h.configure_reservation(Config {
        limit: 1,
        names: vec![NamedSound {
            sound: 1001,
            name: "imp_rock".into(),
        }],
    })
    .unwrap();
    let mut i = init(1.);
    i.sound = 1001;
    i.original_sound = 1001;
    i.flags = 0x44;
    h.enqueue_stream(
        &i,
        OGG.to_vec(),
        helpers::template(1, 1.),
        helpers::registration(h.mixed(), 1),
        &mut PortableCurveMath,
    )
    .unwrap();
    add_static(&mut h, 6, 0.5);
    let out = h.cycle(&frame(&h), &mut PortableCurveMath).unwrap();
    assert!(out.contains(&Event::QueuedStop(0)));
    assert!(out.contains(&Event::Attempt {
        index: 1,
        node: Some(1)
    }));
    assert_eq!(h.mixed().resources().counts(), (1, 1, 0));
}
#[test]
fn pause_commands_and_status_reach_both_owned_driver_types() {
    let mut h = host(2);
    add_static(&mut h, 6, 1.);
    add_stream(&mut h, 1, 2.);
    h.cycle(&frame(&h), &mut PortableCurveMath).unwrap();
    h.dispatch(
        &Command::PauseAll {
            request: -7,
            exclude: 0,
        },
        &bank(),
        &mut PortableCurveMath,
        &helpers::replies(h.mixed()),
    )
    .unwrap();
    for sound in [100, 1001] {
        assert_eq!(
            h.is_sound_paused(Query {
                actor: 42,
                sound,
                auto_actor: false,
                engine_actor: 0
            })
            .unwrap(),
            -7
        );
    }
    h.dispatch(
        &Command::PauseAll {
            request: 0,
            exclude: u32::MAX,
        },
        &bank(),
        &mut PortableCurveMath,
        &helpers::replies(h.mixed()),
    )
    .unwrap();
    assert!(h
        .mixed()
        .pool()
        .nodes
        .iter()
        .all(|n| n.source.requested_pause == 0));
}
#[test]
fn fade_commands_stop_and_dispose_both_kinds_in_controller_phase() {
    let mut h = host(2);
    add_static(&mut h, 6, 1.);
    add_stream(&mut h, 1, 2.);
    h.cycle(&frame(&h), &mut PortableCurveMath).unwrap();
    for sound in [100, 1001] {
        h.dispatch(
            &Command::StopSound {
                actor: 0,
                sound,
                fade_bits: (-1f32).to_bits(),
            },
            &bank(),
            &mut PortableCurveMath,
            &helpers::replies(h.mixed()),
        )
        .unwrap();
    }
    let mut f = frame(&h);
    for n in &mut f.nodes {
        n.base_delta_bits = 0.3f32.to_bits();
    }
    h.cycle(&f, &mut PortableCurveMath).unwrap();
    assert_eq!(h.mixed().resources().counts(), (1, 0, 0));
}
#[test]
fn cue_identity_can_preserve_stream_while_stop_all_removes_static() {
    let mut h = host(2);
    add_static(&mut h, 6, 1.);
    add_stream(&mut h, 1, 2.);
    let id = h.waiting()[1].source.unique_id;
    h.set_cue_id(id);
    h.set_pending_cues(8);
    h.cycle(&frame(&h), &mut PortableCurveMath).unwrap();
    h.dispatch(
        &Command::StopAll { exclude: 0x80 },
        &bank(),
        &mut PortableCurveMath,
        &helpers::replies(h.mixed()),
    )
    .unwrap();
    h.refresh_cues().unwrap();
    assert_eq!(h.mixed().resources().counts(), (1, 0, 1));
    assert_eq!(h.mixed().status().current, 9);
}
#[test]
fn unregister_clears_pending_encoded_bytes_and_live_references() {
    let mut h = host(2);
    add_static(&mut h, 6, 1.);
    h.cycle(&frame(&h), &mut PortableCurveMath).unwrap();
    add_stream(&mut h, 1, 2.);
    h.unregister(1001, &helpers::replies(h.mixed())).unwrap();
    h.unregister(100, &helpers::replies(h.mixed())).unwrap();
    assert!(h.waiting().is_empty());
    assert_eq!(h.mixed().resources().counts(), (0, 0, 0));
}
#[test]
fn pending_and_live_queue_resume_with_identical_worker_cycles_and_commands() {
    let mut h = host(2);
    add_static(&mut h, 6, 1.);
    add_stream(&mut h, 1, 2.);
    h.set_cue_id(102);
    for _ in 0..8 {
        let mut cold = Host::restore(&h.checkpoint().unwrap()).unwrap();
        assert_eq!(h.service_round(1).unwrap(), cold.service_round(1).unwrap());
        let f = frame(&h);
        assert_eq!(
            h.cycle(&f, &mut PortableCurveMath).unwrap(),
            cold.cycle(&f, &mut PortableCurveMath).unwrap()
        );
        assert_eq!(h.checkpoint().unwrap(), cold.checkpoint().unwrap());
    }
}
#[test]
fn decoder_failure_after_budget_stop_restores_live_pool_and_pending_queue() {
    let mut h = host(1);
    add_static(&mut h, 6, 1.);
    h.cycle(&frame(&h), &mut PortableCurveMath).unwrap();
    let mut i = init(2.);
    i.sound = 1001;
    i.original_sound = 1001;
    i.flags = 0x104;
    i.identity = 22;
    h.enqueue_stream(
        &i,
        b"OggS invalid decoder".to_vec(),
        helpers::template(1, 2.),
        helpers::registration(h.mixed(), 1),
        &mut PortableCurveMath,
    )
    .unwrap();
    let before = h.checkpoint().unwrap();
    assert!(h.cycle(&frame(&h), &mut PortableCurveMath).is_err());
    assert_eq!(h.checkpoint().unwrap(), before);
}
#[test]
fn late_failure_rolls_back_decoding_upload_cache_and_all_prior_phases() {
    let mut h = host(2);
    add_static(&mut h, 6, 1.);
    add_stream(&mut h, 1, 2.);
    let before = h.checkpoint().unwrap();
    let mut f = frame(&h);
    f.params.clear();
    assert!(h.cycle(&f, &mut PortableCurveMath).is_err());
    assert_eq!(h.checkpoint().unwrap(), before);
}
#[test]
fn malformed_stream_snapshot_and_old_mixed_cache_default_are_checked() {
    let mut h = host(1);
    add_stream(&mut h, 1, 1.);
    let before = h.checkpoint().unwrap();
    let mut v: Value = serde_json::from_slice(&before).unwrap();
    v["waiting"][0]["content"]["Stream"]["template"]["driver"]["sounds"] = json!([]);
    assert!(h.load(&serde_json::to_vec(&v).unwrap()).is_err());
    assert_eq!(h.checkpoint().unwrap(), before);
    let mut old: Value = serde_json::from_slice(&h.mixed().checkpoint().unwrap()).unwrap();
    old.as_object_mut().unwrap().remove("eax");
    assert!(mixed::Host::restore(&serde_json::to_vec(&old).unwrap()).is_ok());
}
#[test]
fn duplicate_stream_owner_and_wrong_command_profile_are_atomic() {
    let mut h = host(2);
    add_stream(&mut h, 1, 1.);
    let before = h.checkpoint().unwrap();
    let mut i = init(2.);
    i.sound = 1001;
    i.original_sound = 1001;
    i.flags = 0x104;
    assert!(h
        .enqueue_stream(
            &i,
            OGG.to_vec(),
            helpers::template(1, 2.),
            helpers::registration(h.mixed(), 1),
            &mut PortableCurveMath
        )
        .is_err());
    assert_eq!(h.checkpoint().unwrap(), before);
    let mut bank = bank();
    bank[1].streamed = false;
    assert!(h
        .dispatch(
            &Command::PauseAll {
                request: 1,
                exclude: 0
            },
            &bank,
            &mut PortableCurveMath,
            &helpers::replies(h.mixed())
        )
        .is_err());
    assert_eq!(h.checkpoint().unwrap(), before);
}

#[test]
fn pcm_registration_cannot_exceed_combined_pending_and_live_byte_budget() {
    let mut h = host(1);
    let mut i = init(1.);
    i.sound = 1001;
    i.original_sound = 1001;
    i.flags = 0x104;
    let mut encoded = vec![0; crate::audio_stream_data::MAX_BYTES - 16];
    encoded[..4].copy_from_slice(b"OggS");
    h.enqueue_stream(
        &i,
        encoded,
        helpers::template(1, 1.),
        helpers::registration(h.mixed(), 1),
        &mut PortableCurveMath,
    )
    .unwrap();
    let mut large = wave();
    large.resize(44 + 128, 0);
    large[4..8].copy_from_slice(&(36u32 + 128).to_le_bytes());
    large[40..44].copy_from_slice(&128u32.to_le_bytes());
    assert!(h
        .register_pcm(
            &large,
            &Registration {
                sound: 200,
                handle: 201,
                allocation: 301,
                generated: 401,
                allow_stereo: false,
                generation_error: 0,
                upload_error: 0
            }
        )
        .is_err());
    assert_eq!(h.mixed().resources().counts(), (1, 0, 0));
    assert_eq!(h.waiting().len(), 1);
    h.validate().unwrap();
}
