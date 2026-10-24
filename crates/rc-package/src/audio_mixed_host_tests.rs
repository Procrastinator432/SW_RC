use crate::{
    audio_buffer_bank::Registration,
    audio_census::Params,
    audio_curve::PortableCurveMath,
    audio_driver as driver,
    audio_mixed_host::*,
    audio_playback_host_tests::{env, frame, settings, wave},
    audio_queue_cycle_tests::init,
    audio_resource_host::{Event as ResourceEvent, StreamRegistration},
    audio_source::Source,
    audio_source_tick as tick, audio_stream_bridge as bridge,
    audio_vorbis_host::Tokens,
};
use serde_json::{json, Value};
const OGG: &[u8] = include_bytes!("../../../fixtures/audio-vorbis/stereo.ogg");
pub(super) fn host(slots: usize) -> Host {
    let mut h = Host::new(
        1,
        &(0..slots + 1).map(|n| 10 + n as u32).collect::<Vec<_>>(),
        (1..=slots as u32 + 1).collect(),
        settings(),
    )
    .unwrap();
    h.register_pcm(
        &wave(),
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
    h
}
pub(super) fn source(id: u32, priority: f32) -> Source {
    let mut input = init(priority);
    input.identity = id;
    input.voice_exempt = true;
    let mut counter = 100;
    let mut s = Source::new(&mut counter);
    s.initialize(&input, &mut PortableCurveMath).unwrap();
    s
}
pub(super) fn replies(h: &Host) -> Vec<driver::Reply> {
    (0..h.pool().nodes.len())
        .map(|n| driver::Reply {
            source_state: 0x1012,
            unqueued: h.resources().stream(h.key(n).unwrap()).map_or([0; 4], |s| {
                s.tick.driver.buffers[0].storage[..4].try_into().unwrap()
            }),
        })
        .collect()
}
pub(super) fn add_static(h: &mut Host, id: u32, priority: f32) -> Admission {
    h.admit_static(
        &source(id, priority),
        &driver::Sound {
            handle: 100,
            flags: 0,
            duration_bits: 0,
            native_handle: 200,
        },
        &replies(h),
    )
    .unwrap()
}
pub(super) fn template(n: u32, priority: f32) -> tick::Session {
    let mut t: tick::Session = serde_json::from_str(include_str!(
        "../../../fixtures/audio-vorbis/host-tick.json"
    ))
    .unwrap();
    t.driver.node.source = source(22 + 16 * n, priority);
    t.driver.node.source.flags = 0x104;
    t.driver.node.source.sound = 1000 + n;
    t.driver.node.source.original_sound = 1000 + n;
    t.driver.node.buffer = 2000 + n;
    t.driver.buffers[0].handle = 2000 + n;
    t.driver.sounds[0] = driver::Sound {
        handle: 1000 + n,
        flags: 3,
        duration_bits: 0,
        native_handle: 0,
    };
    t
}
pub(super) fn registration(h: &Host, n: u32) -> StreamRegistration {
    StreamRegistration {
        key: h.key(h.pool().transient.unwrap()).unwrap(),
        allocation: 4000 + n,
        tokens: Tokens {
            stream: 5000 + n,
            critical: 1,
            decoder: 6000 + n,
            staging: 0x10000 + n * 0x10000,
            buffers: std::array::from_fn(|i| 7000 + n * 4 + i as u32),
        },
        looping: true,
    }
}
pub(super) fn add_stream(h: &mut Host, n: u32, priority: f32) -> Admission {
    h.admit_stream(
        OGG,
        &template(n, priority),
        &registration(h, n),
        &replies(h),
    )
    .unwrap()
}
pub(super) fn frames(h: &Host) -> Vec<Frame> {
    let replies = replies(h);
    (0..h.pool().nodes.len())
        .map(|n| {
            let s = h.resources().stream(h.key(n).unwrap());
            Frame {
                base: frame(),
                base_delta_bits: 0.1f32.to_bits(),
                cleanup: replies[n].clone(),
                late: bridge::Tick {
                    environment: env(),
                    delta_bits: 0.1f32.to_bits(),
                    al: bridge::AlReply {
                        state: 0x1012,
                        processed: i32::from(s.is_some()),
                        queued: 4,
                        unqueued: s.map_or(0, |s| {
                            s.tick.driver.buffers[0].storage[s.tick.progress[0].indices[1] as usize]
                        }),
                    },
                },
            }
        })
        .collect()
}
pub(super) fn params() -> Vec<Params> {
    (0..4)
        .map(|n| Params {
            sound: if n == 0 { 100 } else { 1000 + n },
            flags: 1,
        })
        .collect()
}
#[test]
fn original_mixed_prefix_sequences_match_admission_and_transient_swaps() {
    let data: Value = serde_json::from_str(include_str!(
        "../../../fixtures/audio-mixed-pool-native.json"
    ))
    .unwrap();
    for (case, sequence) in data["sequences"].as_array().unwrap().iter().enumerate() {
        let count = sequence["channels"].as_u64().unwrap() as usize;
        let static_sound = sequence["static_sound"].as_u64().unwrap() as u32;
        let mut h = Host::new(
            1,
            &(0..count + 1).map(|n| 1000 + n as u32).collect::<Vec<_>>(),
            (1..=count as u32 + 1).collect(),
            settings(),
        )
        .unwrap();
        h.register_pcm(
            &wave(),
            &Registration {
                sound: static_sound,
                handle: 200,
                allocation: 300,
                generated: 400,
                allow_stereo: false,
                generation_error: 0,
                upload_error: 0,
            },
        )
        .unwrap();
        for (index, step) in sequence["steps"].as_array().unwrap().iter().enumerate() {
            let s: Source = serde_json::from_value(step["source"].clone()).unwrap();
            let out = if step["streaming"].as_bool().unwrap() {
                let mut t = template(index as u32, 1.);
                t.driver.node.source = s.clone();
                t.driver.node.buffer = step["buffer"].as_u64().unwrap() as u32;
                t.driver.buffers[0].handle = t.driver.node.buffer;
                t.driver.sounds[0].handle = s.sound;
                h.admit_stream(OGG, &t, &registration(&h, index as u32), &replies(&h))
                    .unwrap()
            } else {
                h.admit_static(
                    &s,
                    &driver::Sound {
                        handle: static_sound,
                        flags: 0,
                        duration_bits: 0,
                        native_handle: 200,
                    },
                    &replies(&h),
                )
                .unwrap()
            };
            let prefix = out
                .events
                .iter()
                .filter_map(|e| {
                    if let Event::Activation(e) = e {
                        Some(e)
                    } else {
                        None
                    }
                })
                .collect::<Vec<_>>();
            assert_eq!(
                serde_json::to_value(prefix).unwrap(),
                step["events"],
                "prefix {case}/{index}"
            );
            assert_eq!(
                out.node,
                step["prepared"]["node"].as_u64().map(|n| n as usize),
                "chosen {case}/{index}"
            );
            assert_eq!(
                serde_json::to_value(&h.pool().slots).unwrap(),
                step["slots"],
                "slots {case}/{index}"
            );
            assert_eq!(
                serde_json::to_value(h.pool().transient).unwrap(),
                step["transient"],
                "transient {case}/{index}"
            );
        }
    }
}
#[test]
fn shared_pool_admits_static_and_stream_on_distinct_physical_channels() {
    let mut h = host(2);
    assert_eq!(add_static(&mut h, 6, 1.).node, Some(2));
    assert_eq!(add_stream(&mut h, 1, 2.).node, Some(0));
    assert_eq!(h.pool().slots, vec![Some(2), Some(0)]);
    assert_eq!(h.pool().transient, Some(1));
    assert_eq!(h.resources().counts(), (1, 1, 1));
    h.validate().unwrap();
}
#[test]
fn stream_eviction_disposes_decoder_and_four_buffers_before_static_reuse() {
    let mut h = host(1);
    add_stream(&mut h, 1, 1.);
    let out = add_static(&mut h, 6, 3.);
    assert_eq!(out.node, Some(0));
    assert_eq!(h.resources().counts(), (1, 1, 0));
    assert!(out
        .events
        .iter()
        .any(|e| matches!(e, Event::Resource(ResourceEvent::StreamDispose { .. }))));
    let disposal = out
        .events
        .iter()
        .position(|e| matches!(e, Event::Resource(ResourceEvent::StreamDispose { .. })))
        .unwrap();
    let play = out
        .events
        .iter()
        .position(|e| {
            matches!(
                e,
                Event::Driver {
                    event: driver::Event::AlPlay { .. },
                    ..
                }
            )
        })
        .unwrap();
    assert!(disposal < play);
}
#[test]
fn static_eviction_keeps_pcm_cached_and_registers_stream_only_once() {
    let mut h = host(1);
    add_static(&mut h, 6, 1.);
    add_stream(&mut h, 1, 3.);
    assert_eq!(h.resources().counts(), (1, 0, 1));
    let before = h.checkpoint().unwrap();
    assert!(h
        .admit_stream(OGG, &template(1, 4.), &registration(&h, 1), &replies(&h))
        .is_err());
    assert_eq!(h.checkpoint().unwrap(), before);
}
#[test]
fn protected_hardware_refuses_stream_without_decoding_or_uploading() {
    let mut h = host(1);
    add_static(&mut h, 7, 2.);
    let counts = h.resources().counts();
    let mut incoming = template(1, 3.);
    incoming.driver.node.source.identity = 6;
    let out = h
        .admit_stream(b"invalid", &incoming, &registration(&h, 1), &replies(&h))
        .unwrap();
    assert!(out.node.is_none());
    assert!(out.uploads.is_empty());
    assert_eq!(h.resources().counts(), counts);
    h.validate().unwrap();
}
#[test]
fn failed_registration_after_eviction_restores_all_owners_and_slot_order() {
    let mut h = host(1);
    add_static(&mut h, 6, 1.);
    let before = h.checkpoint().unwrap();
    assert!(h
        .admit_stream(
            b"invalid",
            &template(1, 3.),
            &registration(&h, 1),
            &replies(&h)
        )
        .is_err());
    assert_eq!(h.checkpoint().unwrap(), before);
    let mut r = registration(&h, 1);
    r.key = 999;
    assert!(h
        .admit_stream(OGG, &template(1, 3.), &r, &replies(&h))
        .is_err());
    assert_eq!(h.checkpoint().unwrap(), before);
}
#[test]
fn mixed_pause_and_resume_use_actual_driver_cursor_and_start_pause() {
    let mut h = host(2);
    let a = add_static(&mut h, 6, 1.).node.unwrap();
    let mut t = template(1, 2.);
    t.driver.node.source.requested_pause = -7;
    let b = h
        .admit_stream(OGG, &t, &registration(&h, 1), &replies(&h))
        .unwrap()
        .node
        .unwrap();
    assert_eq!(h.pool().nodes[b].source.requested_pause, -7);
    for n in [a, b] {
        h.control(n, &driver::Command::Pause(1), &replies(&h)[n])
            .unwrap();
        h.control(n, &driver::Command::Pause(0), &replies(&h)[n])
            .unwrap();
        assert_eq!(h.pool().nodes[n].source.requested_pause, 0);
    }
}
#[test]
fn common_census_counts_static_and_stream_and_drives_next_duck_phase() {
    let mut h = host(2);
    add_static(&mut h, 6, 1.);
    add_stream(&mut h, 1, 2.);
    h.set_pending(0x80);
    h.service_round(1).unwrap();
    h.update(0.1, &frames(&h), &params(), &mut PortableCurveMath)
        .unwrap();
    assert_eq!(h.status().ducking_voices, 2);
    assert_eq!(h.status().current, 0x81);
    assert!(h.status().any_playing);
    h.service_round(1).unwrap();
    h.update(0.1, &frames(&h), &params(), &mut PortableCurveMath)
        .unwrap();
    assert!(h.ducking().active);
}
#[test]
fn late_census_error_rolls_back_stream_ring_static_clock_and_controllers() {
    let mut h = host(2);
    add_static(&mut h, 6, 1.);
    add_stream(&mut h, 1, 2.);
    h.service_round(1).unwrap();
    let before = h.checkpoint().unwrap();
    assert!(h
        .update(0.1, &frames(&h), &[], &mut PortableCurveMath)
        .is_err());
    assert_eq!(h.checkpoint().unwrap(), before);
}
#[test]
fn end_cleanup_releases_nonlooping_stream_and_static_without_losing_pcm() {
    let mut h = host(2);
    add_static(&mut h, 6, 1.);
    let mut t = template(1, 2.);
    t.driver.sounds[0].flags = 2;
    let mut r = registration(&h, 1);
    r.looping = false;
    h.admit_stream(OGG, &t, &r, &replies(&h)).unwrap();
    let mut f = frames(&h);
    for frame in &mut f {
        frame.cleanup.source_state = 0x1014;
    }
    h.update(0.1, &f, &[], &mut PortableCurveMath).unwrap();
    assert_eq!(h.resources().counts(), (1, 0, 0));
    assert!(!h.status().any_playing);
    h.validate().unwrap();
}
#[test]
fn worker_budget_and_mixed_rounds_are_identical_after_every_cold_restore() {
    let mut h = host(3);
    add_static(&mut h, 6, 1.);
    add_stream(&mut h, 1, 2.);
    add_stream(&mut h, 2, 3.);
    let mut serviced = std::collections::BTreeSet::new();
    for _ in 0..8 {
        let mut cold = Host::restore(&h.checkpoint().unwrap()).unwrap();
        let served = h.service_round(1).unwrap();
        assert_eq!(served, cold.service_round(1).unwrap());
        assert!(served.len() <= 1);
        serviced.extend(served.into_iter().map(|s| s.source));
        let f = frames(&h);
        assert_eq!(
            h.update(0.1, &f, &params(), &mut PortableCurveMath)
                .unwrap(),
            cold.update(0.1, &f, &params(), &mut PortableCurveMath)
                .unwrap()
        );
        assert_eq!(h.checkpoint().unwrap(), cold.checkpoint().unwrap());
    }
    assert_eq!(serviced.len(), 2);
}
#[test]
fn bad_stream_unqueue_and_bad_snapshot_do_not_replace_live_resources() {
    let mut h = host(1);
    let n = add_stream(&mut h, 1, 2.).node.unwrap();
    let before = h.checkpoint().unwrap();
    let bad = driver::Reply {
        source_state: 0x1014,
        unqueued: [0; 4],
    };
    assert!(h.control(n, &driver::Command::Stop, &bad).is_err());
    assert_eq!(h.checkpoint().unwrap(), before);
    let mut saved: Value = serde_json::from_slice(&before).unwrap();
    saved["pool"]["slots"] = json!([saved["pool"]["transient"]]);
    assert!(h.load(&serde_json::to_vec(&saved).unwrap()).is_err());
    assert_eq!(h.checkpoint().unwrap(), before);
}
#[test]
fn unregister_removes_mixed_streams_then_shared_pcm_and_leaves_reusable_pool() {
    let mut h = host(2);
    add_static(&mut h, 6, 1.);
    add_stream(&mut h, 1, 2.);
    h.unregister(1001, &replies(&h)).unwrap();
    h.unregister(100, &replies(&h)).unwrap();
    assert_eq!(h.resources().counts(), (0, 0, 0));
    h.validate().unwrap();
    assert_eq!(add_stream(&mut h, 2, 1.).uploads.len(), 4);
}
