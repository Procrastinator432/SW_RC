use crate::{
    audio_activation as activation, audio_buffer_bank as pcm,
    audio_curve::PortableCurveMath,
    audio_driver as driver, audio_playback_host as playback,
    audio_playback_host_tests::{driver_frame, env, frame, host, settings, wave},
    audio_queue_cycle::*,
    audio_queue_order as order, audio_resource_host,
    audio_source::Init,
    audio_upload as upload,
};
use serde_json::Value;

pub(super) fn init(priority: f32) -> Init {
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
        voice_exempt: false,
    }
}
pub(super) fn sound() -> driver::Sound {
    driver::Sound {
        handle: 100,
        flags: 0,
        duration_bits: 0,
        native_handle: 200,
    }
}
pub(super) fn input(h: &Host, state: i32) -> Frame {
    let base = SourceFrame {
        base: frame(),
        delta_bits: 0.1f32.to_bits(),
    };
    Frame {
        global_delta_bits: 0.1f32.to_bits(),
        cleanup: vec![
            driver::Reply {
                source_state: state,
                unqueued: [0; 4]
            };
            h.playback().pool().slots.len()
        ],
        active: vec![base.clone(); h.playback().pool().slots.len()],
        queued: vec![
            QueuedFrame {
                source: base,
                environment: env(),
                engine_present: true,
                sound_valid: true
            };
            h.waiting().len()
        ],
        drivers_by_node: vec![driver_frame(); h.playback().pool().nodes.len()],
        params: vec![crate::audio_census::Params {
            sound: 100,
            flags: 0,
        }],
        context: 42,
    }
}
pub(super) fn queued(slots: usize, priorities: &[f32]) -> Host {
    let mut h = Host::new(host(slots), u32::MAX).unwrap();
    for &p in priorities {
        h.enqueue(&init(p), sound(), &mut PortableCurveMath)
            .unwrap();
    }
    h
}

#[test]
fn exact_whole_original_priority_sort_including_ties() {
    let data: Value = serde_json::from_str(include_str!(
        "../../../fixtures/audio-queue-order-native.json"
    ))
    .unwrap();
    for (i, case) in data["cases"].as_array().unwrap().iter().enumerate() {
        let words: Vec<u32> = serde_json::from_value(case["words"].clone()).unwrap();
        assert_eq!(
            serde_json::to_value(order::order(&words).unwrap()).unwrap(),
            case["order"],
            "case {i}"
        );
    }
}
#[test]
fn cutoff_truncates_toward_zero_and_checks_bounds() {
    assert_eq!(
        order::cutoff(&[2f32.to_bits(), 1.9f32.to_bits()], 1).unwrap(),
        1
    );
    assert_eq!(
        order::cutoff(&[2f32.to_bits(), (-1.9f32).to_bits()], 1).unwrap(),
        -1
    );
    assert_eq!(order::cutoff(&[], 0).unwrap(), 0);
    assert!(order::order(&[f32::NAN.to_bits()]).is_err());
    assert!(order::order(&vec![0; 4097]).is_err());
}
#[test]
fn whole_original_finished_cleanup_matches_pool_and_driver_trace() {
    let data: Value =
        serde_json::from_str(include_str!("../../../fixtures/audio-cleanup-native.json")).unwrap();
    for (index, c) in data["cases"].as_array().unwrap().iter().enumerate() {
        let pool: activation::Pool = serde_json::from_value(c["before"].clone()).unwrap();
        let buffer = c["buffer"].as_u64().unwrap() as u32;
        let sound = driver::Sound {
            handle: c["sound"].as_u64().unwrap() as u32,
            flags: 0,
            duration_bits: 0,
            native_handle: buffer as i32,
        };
        let mut resources = audio_resource_host::Host::new(1).unwrap();
        resources
            .register_pcm(
                &wave(),
                &pcm::Registration {
                    sound: sound.handle,
                    handle: buffer,
                    allocation: 300,
                    generated: 101,
                    allow_stereo: false,
                    generation_error: 0,
                    upload_error: 0,
                },
            )
            .unwrap();
        let keys = (1..=pool.nodes.len() as u32).collect::<Vec<_>>();
        for (n, node) in pool.nodes.iter().enumerate() {
            if node.source.allocated {
                resources
                    .attach_static(keys[n], node.clone(), sound.clone())
                    .unwrap();
            }
        }
        let mut h = playback::Host::from_prepared(
            resources,
            pool,
            keys,
            settings(),
            upload::EaxCache {
                initialized: false,
                value: 0,
            },
        )
        .unwrap();
        let replies = c["states"]
            .as_array()
            .unwrap()
            .iter()
            .map(|v| driver::Reply {
                source_state: v.as_i64().unwrap() as i32,
                unqueued: [0; 4],
            })
            .collect::<Vec<_>>();
        let events = h.cleanup_finished(&replies).unwrap();
        assert_eq!(
            serde_json::to_value(events).unwrap(),
            c["events"],
            "events {index}"
        );
        assert_eq!(
            serde_json::to_value(h.pool()).unwrap(),
            c["expected"],
            "pool {index}"
        );
    }
}
#[test]
fn queue_order_budget_discards_and_platform_order() {
    let mut h = queued(2, &[0.5, 3., 2.]);
    assert_eq!(h.counter(), 2);
    let events = h.cycle(&input(&h, 0x1012), &mut PortableCurveMath).unwrap();
    assert!(events.contains(&Event::Sorted(vec![
        Candidate::Queued(1),
        Candidate::Queued(2),
        Candidate::Queued(0)
    ])));
    assert!(events.contains(&Event::Cutoff(0)));
    assert_eq!(
        events
            .iter()
            .filter(|e| matches!(e, Event::Attempt { node: Some(_), .. }))
            .count(),
        2
    );
    assert_eq!(
        events
            .iter()
            .filter(|e| matches!(e, Event::Discard { .. }))
            .count(),
        3
    );
    assert_eq!(
        &events[events.len() - 2..],
        &[Event::ProcessContext(42), Event::SuspendContext(42)]
    );
    assert!(h.waiting().is_empty());
}
#[test]
fn finished_slot_is_reused_and_pcm_cache_survives() {
    let mut h = queued(1, &[1.]);
    h.cycle(&input(&h, 0x1012), &mut PortableCurveMath).unwrap();
    h.enqueue(&init(2.), sound(), &mut PortableCurveMath)
        .unwrap();
    let events = h.cycle(&input(&h, 0x1014), &mut PortableCurveMath).unwrap();
    let stopped = events
        .iter()
        .position(|e| matches!(e, Event::Playback(playback::Event::Stop(_))))
        .unwrap();
    let admitted = events
        .iter()
        .position(|e| matches!(e, Event::Attempt { node: Some(_), .. }))
        .unwrap();
    assert!(stopped < admitted);
    assert_eq!(h.playback().resources().counts().0, 1);
}
#[test]
fn queued_pause_does_not_suppress_base_but_pauses_started_driver() {
    let mut h = queued(1, &[1.]);
    h.pause(0, -7).unwrap();
    let events = h.cycle(&input(&h, 0x1012), &mut PortableCurveMath).unwrap();
    assert!(events.iter().any(|e| matches!(
        e,
        Event::QueuedBase {
            event: crate::audio_source_runtime::Event::Volume(_),
            ..
        }
    )));
    assert!(events.iter().any(|e| matches!(
        e,
        Event::Playback(playback::Event::Driver {
            event: driver::Event::AlPause { .. },
            ..
        })
    )));
}
#[test]
fn canceled_request_is_discarded_without_admission() {
    let mut h = queued(1, &[1.]);
    h.cancel(0).unwrap();
    let events = h.cycle(&input(&h, 0x1012), &mut PortableCurveMath).unwrap();
    assert!(!events.iter().any(|e| matches!(e, Event::Attempt { .. })));
    assert!(h.waiting().is_empty());
}
#[test]
fn late_failure_restores_cleanup_admission_queue_counter_and_duck() {
    let mut h = queued(1, &[1.]);
    h.cycle(&input(&h, 0x1012), &mut PortableCurveMath).unwrap();
    h.enqueue(&init(2.), sound(), &mut PortableCurveMath)
        .unwrap();
    let before = h.checkpoint().unwrap();
    let mut f = input(&h, 0x1014);
    f.params.clear();
    assert!(h.cycle(&f, &mut PortableCurveMath).is_err());
    assert_eq!(h.checkpoint().unwrap(), before);
    h.cycle(&input(&h, 0x1012), &mut PortableCurveMath).unwrap();
}
#[test]
fn pending_and_live_cycle_cold_resume_is_identical() {
    let mut h = queued(2, &[2., 1.]);
    for _ in 0..8 {
        let mut cold = Host::restore(&h.checkpoint().unwrap()).unwrap();
        let f = input(&h, 0x1012);
        assert_eq!(
            h.cycle(&f, &mut PortableCurveMath).unwrap(),
            cold.cycle(&f, &mut PortableCurveMath).unwrap()
        );
        assert_eq!(h.checkpoint().unwrap(), cold.checkpoint().unwrap());
    }
}
#[test]
fn rejected_enqueue_and_corrupt_restore_leave_state_unchanged() {
    let mut h = queued(1, &[]);
    let before = h.checkpoint().unwrap();
    let mut bad = init(1.);
    bad.flags = 0x40;
    assert!(h.enqueue(&bad, sound(), &mut PortableCurveMath).is_err());
    assert_eq!(before, h.checkpoint().unwrap());
    let mut bad: Value = serde_json::from_slice(&before).unwrap();
    bad["schema"] = 9.into();
    assert!(h.load(&serde_json::to_vec(&bad).unwrap()).is_err());
    assert_eq!(before, h.checkpoint().unwrap());
}

#[test]
fn tiny_positional_cull_has_no_backfill_beyond_channel_budget() {
    let mut h = queued(1, &[]);
    let mut small = init(3.);
    small.flags = 0;
    small.radius = 0.0005;
    let mut long = sound();
    long.duration_bits = 3f32.to_bits();
    h.enqueue(&small, long, &mut PortableCurveMath).unwrap();
    h.enqueue(&init(0.5), sound(), &mut PortableCurveMath)
        .unwrap();
    let events = h.cycle(&input(&h, 0x1012), &mut PortableCurveMath).unwrap();
    assert!(events.contains(&Event::QueuedStop(0)));
    assert!(!events.iter().any(|e| matches!(e, Event::Attempt { .. })));
    assert!(h
        .playback()
        .pool()
        .nodes
        .iter()
        .all(|n| !n.source.allocated));
    assert!(h.waiting().is_empty());
}

#[test]
fn protected_live_voice_survives_cutoff_and_refuses_replacement() {
    let mut h = queued(1, &[]);
    let mut protected = init(1.);
    protected.identity = 7;
    h.enqueue(&protected, sound(), &mut PortableCurveMath)
        .unwrap();
    h.cycle(&input(&h, 0x1012), &mut PortableCurveMath).unwrap();
    h.enqueue(&init(2.), sound(), &mut PortableCurveMath)
        .unwrap();
    let events = h.cycle(&input(&h, 0x1012), &mut PortableCurveMath).unwrap();
    assert!(events.contains(&Event::Attempt {
        queue: 0,
        node: None
    }));
    assert!(h
        .playback()
        .pool()
        .nodes
        .iter()
        .any(|n| n.source.allocated && n.source.identity == 7));
}

#[test]
fn ducking_advances_once_per_cycle_regardless_of_source_count() {
    let mut h = queued(3, &[3., 2., 1.]);
    let mut expected = h.playback().ducking().clone();
    expected.step(0, 1.).unwrap();
    let mut f = input(&h, 0x1012);
    f.global_delta_bits = 4f32.to_bits();
    for q in &mut f.queued {
        q.source.delta_bits = 0.2f32.to_bits();
    }
    h.cycle(&f, &mut PortableCurveMath).unwrap();
    assert_eq!(
        serde_json::to_value(h.playback().ducking()).unwrap(),
        serde_json::to_value(expected).unwrap()
    );
}

#[test]
fn tiny_cull_uses_strict_duration_and_radius_boundaries() {
    for (duration, radius, culled) in [
        (2f32, 0.0005, false),
        (3., f32::from_bits(0x3a83126f), false),
        (3., 0.0005, true),
    ] {
        let mut h = queued(1, &[]);
        let mut i = init(1.);
        i.flags = 0;
        i.radius = radius;
        let mut s = sound();
        s.duration_bits = duration.to_bits();
        h.enqueue(&i, s, &mut PortableCurveMath).unwrap();
        let events = h.cycle(&input(&h, 0x1012), &mut PortableCurveMath).unwrap();
        assert_eq!(events.contains(&Event::QueuedStop(0)), culled);
    }
}
