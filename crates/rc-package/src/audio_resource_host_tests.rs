use crate::{
    audio_buffer_bank as pcm, audio_driver as driver, audio_resource_host::*,
    audio_source_tick as tick, audio_stream_bridge as bridge, audio_upload::Environment,
    audio_vorbis_host::Tokens,
};
use serde_json::{json, Value};
fn wave() -> Vec<u8> {
    let f: Value = serde_json::from_str(include_str!(
        "../../../fixtures/audio-wave-registration-native.json"
    ))
    .unwrap();
    serde_json::from_value(f["cases"][2]["wave"].clone()).unwrap()
}
fn registration() -> pcm::Registration {
    pcm::Registration {
        sound: 100,
        handle: 200,
        allocation: 300,
        generated: 400,
        allow_stereo: false,
        generation_error: 0,
        upload_error: 0,
    }
}
fn prepared(sound: u32, buffer: u32, hardware: u32) -> tick::Session {
    let mut t: tick::Session = serde_json::from_str(include_str!(
        "../../../fixtures/audio-vorbis/host-tick.json"
    ))
    .unwrap();
    t.driver.node.source.sound = sound;
    t.driver.node.source.original_sound = sound;
    t.driver.node.buffer = buffer;
    t.driver.node.hardware = hardware;
    t.driver.buffers[0].handle = buffer;
    t.driver.sounds[0].handle = sound;
    t.driver.sounds[0].flags = 3;
    t
}
fn add_pcm(h: &mut Host) {
    h.register_pcm(&wave(), &registration()).unwrap();
}
fn attach(h: &mut Host, key: u32, hardware: u32) {
    let mut t = prepared(100, 200, hardware);
    t.driver.node.source.flags = 0x100;
    t.driver.sounds[0].flags = 0;
    h.attach_static(key, t.driver.node, t.driver.sounds.remove(0))
        .unwrap();
}
fn add_stream(h: &mut Host, n: u32, bytes: &[u8], looping: bool) -> Registered {
    let mut t = prepared(1000 + n, 2000 + n, 3000 + n);
    t.driver.sounds[0].flags = 2 | u8::from(looping);
    h.register_stream(
        bytes,
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
            looping,
        },
    )
    .unwrap()
}
const OGG: &[u8] = include_bytes!("../../../fixtures/audio-vorbis/stereo.ogg");
fn reply(h: &Host, key: u32) -> driver::Reply {
    let s = h.stream(key).unwrap();
    driver::Reply {
        source_state: 0x1012,
        unqueued: s.tick.driver.buffers[0].storage[..4].try_into().unwrap(),
    }
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
#[test]
fn mixed_registry_caches_pcm_and_stream_without_duplicate_uploads() {
    let mut h = Host::new(1).unwrap();
    add_pcm(&mut h);
    let before = h.checkpoint().unwrap();
    assert!(h.register_pcm(&[], &registration()).unwrap().cached);
    assert_eq!(h.checkpoint().unwrap(), before);
    add_stream(&mut h, 1, OGG, true);
    let t = prepared(1001, 9999, 9998);
    let out = h
        .register_stream(
            &[],
            t,
            &StreamRegistration {
                key: 999,
                allocation: 0,
                tokens: Tokens {
                    stream: 0,
                    critical: 0,
                    decoder: 0,
                    staging: 0,
                    buffers: [0; 4],
                },
                looping: false,
            },
        )
        .unwrap();
    assert!(out.cached);
    assert_eq!(out.source, Some(11));
    assert!(out.uploads.is_empty());
    assert_eq!(h.counts(), (1, 0, 1));
}
#[test]
fn static_driver_commands_match_original_cases_inside_the_shared_host() {
    let f: Value =
        serde_json::from_str(include_str!("../../../fixtures/audio-driver-native.json")).unwrap();
    let mut checked = 0;
    for c in f["cases"].as_array().unwrap() {
        let before: driver::Session = serde_json::from_value(c["before"].clone()).unwrap();
        if !before.node.source.allocated
            || before.node.buffer == 0
            || before.node.source.flags & 4 != 0
            || before.sounds[0].flags & 2 != 0
            || before.buffers[0].count_word & 0x1fffffff != 1
        {
            continue;
        }
        let mut h = Host::new(1).unwrap();
        let mut r = registration();
        r.sound = before.node.source.sound;
        r.handle = before.node.buffer;
        r.generated = before.buffers[0].storage[0];
        h.register_pcm(&wave(), &r).unwrap();
        h.attach_static(1, before.node.clone(), before.sounds[0].clone())
            .unwrap();
        let out = h
            .control_static(
                1,
                &serde_json::from_value(c["command"].clone()).unwrap(),
                &serde_json::from_value(c["reply"].clone()).unwrap(),
            )
            .unwrap();
        assert_eq!(serde_json::to_value(out).unwrap(), c["outcome"]);
        assert_eq!(
            serde_json::to_value(&h.voice(1).unwrap().driver.node).unwrap(),
            c["expected"]["node"]
        );
        checked += 1;
    }
    assert!(checked > 10);
}
#[test]
fn shared_static_sound_stops_all_voices_before_a_single_delete() {
    let mut h = Host::new(1).unwrap();
    add_pcm(&mut h);
    attach(&mut h, 1, 1);
    attach(&mut h, 2, 2);
    add_stream(&mut h, 1, OGG, true);
    let stream_before = serde_json::to_value(h.stream(11).unwrap()).unwrap();
    let out = h.unregister(100, None).unwrap();
    let delete = out
        .iter()
        .position(|e| {
            matches!(
                e,
                Event::Pcm(pcm::Event::Cleanup(
                    crate::audio_buffer::Event::Delete { .. }
                ))
            )
        })
        .unwrap();
    let stops = out
        .iter()
        .enumerate()
        .filter(|(_, e)| {
            matches!(
                e,
                Event::Driver {
                    event: driver::Event::AlStop { .. },
                    ..
                }
            )
        })
        .map(|(i, _)| i)
        .collect::<Vec<_>>();
    assert_eq!(stops.len(), 2);
    assert!(stops.iter().all(|&i| i < delete));
    assert_eq!(h.counts(), (0, 0, 1));
    assert_eq!(
        serde_json::to_value(h.stream(11).unwrap()).unwrap(),
        stream_before
    );
}
#[test]
fn detached_voice_reuses_hardware_without_dropping_cached_pcm() {
    let mut h = Host::new(1).unwrap();
    add_pcm(&mut h);
    attach(&mut h, 1, 7);
    h.detach_static(1).unwrap();
    assert_eq!(h.counts(), (1, 0, 0));
    attach(&mut h, 2, 7);
    let r = driver::Reply {
        source_state: 0x1012,
        unqueued: [0; 4],
    };
    h.control_static(2, &driver::Command::Play, &r).unwrap();
    h.control_static(2, &driver::Command::Pause(1), &r).unwrap();
    assert_eq!(h.voice(2).unwrap().driver.node.source.requested_pause, 1);
}
#[test]
fn round_robin_budget_cursor_and_skipping_idle_streams_survive_cold_load() {
    let mut h = Host::new(1).unwrap();
    for n in 1..=3 {
        add_stream(&mut h, n, OGG, true);
    }
    let mut cold = Host::restore(&h.checkpoint().unwrap()).unwrap();
    for expected in [11, 12, 13, 11, 12, 13] {
        let a = h.service_round(1).unwrap();
        let b = cold.service_round(1).unwrap();
        assert_eq!(a, b);
        assert_eq!(a[0].source, expected);
        let s = step(&h, expected);
        assert_eq!(
            h.tick_stream(expected, &s).unwrap(),
            cold.tick_stream(expected, &s).unwrap()
        );
        assert_eq!(h.checkpoint().unwrap(), cold.checkpoint().unwrap());
    }
    let a = h.service_round(2).unwrap();
    assert_eq!(a.iter().map(|r| r.source).collect::<Vec<_>>(), [11, 12]);
    assert_eq!(
        h.service_round(64)
            .unwrap()
            .iter()
            .map(|r| r.source)
            .collect::<Vec<_>>(),
        [13]
    );
    assert!(h.service_round(64).unwrap().is_empty());
    let saved = h.checkpoint().unwrap();
    assert!(h.service_round(65).is_err());
    assert_eq!(saved, h.checkpoint().unwrap());
}
#[test]
fn mixed_resources_reject_cross_type_id_hardware_and_memory_collisions_atomically() {
    let mut h = Host::new(1).unwrap();
    add_pcm(&mut h);
    attach(&mut h, 1, 3001);
    let saved = h.checkpoint().unwrap();
    let t = prepared(1001, 2001, 3001);
    let r = StreamRegistration {
        key: 11,
        allocation: 4001,
        tokens: Tokens {
            stream: 5001,
            critical: 1,
            decoder: 6001,
            staging: 0x20000,
            buffers: [7004, 7005, 7006, 7007],
        },
        looping: true,
    };
    assert!(h.register_stream(OGG, t, &r).is_err());
    assert_eq!(saved, h.checkpoint().unwrap());
    h.detach_static(1).unwrap();
    add_stream(&mut h, 1, OGG, true);
    let saved = h.checkpoint().unwrap();
    let mut r = registration();
    r.sound = 101;
    r.generated = 7004;
    r.upload_error = 9;
    assert!(h.register_pcm(&wave(), &r).is_err());
    assert_eq!(saved, h.checkpoint().unwrap());
    let mut r = registration();
    r.sound = 1001;
    assert!(h.register_pcm(&[], &r).is_err());
    let mut large=Vec::from(b"RIFF\0\0\0\0WAVEfmt \x10\0\0\0\x01\0\x01\0\x40\x1f\0\0\x40\x1f\0\0\x01\0\x08\0data\0\0\0\0".as_slice());
    let n = crate::audio_stream_data::MAX_BYTES - 100_000;
    large[40..44].copy_from_slice(&(n as u32).to_le_bytes());
    large.resize(44 + n, 0);
    let size = large.len() as u32 - 8;
    large[4..8].copy_from_slice(&size.to_le_bytes());
    r.sound = 102;
    r.handle = 202;
    r.allocation = 302;
    r.generated = 402;
    assert!(h.register_pcm(&large, &r).is_err());
    assert_eq!(saved, h.checkpoint().unwrap());
    // Teardown must not depend on the JSON save budget of a valid live bank.
    let mut large_host = Host::new(1).unwrap();
    large_host.register_pcm(&large, &r).unwrap();
    assert!(large_host.checkpoint().is_err());
    large_host.shutdown(&[]).unwrap();
    assert!(large_host.is_closed());
}
#[test]
fn stream_pause_tick_and_natural_end_are_isolated_from_static_resources() {
    let mut h = Host::new(1).unwrap();
    add_pcm(&mut h);
    attach(&mut h, 1, 1);
    add_stream(
        &mut h,
        1,
        include_bytes!("../../../fixtures/audio-vorbis/mono.ogg"),
        false,
    );
    let bank = serde_json::to_value(h.bank()).unwrap();
    let r = reply(&h, 11);
    h.control_stream(11, &driver::Command::Pause(1), &r)
        .unwrap();
    let mut s = step(&h, 11);
    s.al.processed = 0;
    assert!(h
        .tick_stream(11, &s)
        .unwrap()
        .driver
        .unwrap()
        .events
        .is_empty());
    h.control_stream(11, &driver::Command::Pause(0), &r)
        .unwrap();
    h.tick_stream(11, &s).unwrap();
    assert!(h.stream(11).unwrap().worker.is_none());
    assert_eq!(serde_json::to_value(h.bank()).unwrap(), bank);
    let before = h.checkpoint().unwrap();
    let t = prepared(1001, 2001, 3001);
    assert!(h
        .register_stream(
            OGG,
            t,
            &StreamRegistration {
                key: 11,
                allocation: 0,
                tokens: Tokens {
                    stream: 0,
                    critical: 0,
                    decoder: 0,
                    staging: 0,
                    buffers: [0; 4]
                },
                looping: false
            }
        )
        .is_err());
    assert_eq!(before, h.checkpoint().unwrap());
    h.unregister(1001, Some(&r)).unwrap();
    assert_eq!(h.counts(), (1, 1, 0));
}
#[test]
fn shutdown_is_atomic_when_a_late_stream_reply_loses_owned_ids() {
    let mut h = Host::new(1).unwrap();
    add_pcm(&mut h);
    attach(&mut h, 1, 1);
    for n in 1..=2 {
        add_stream(&mut h, n, OGG, true);
    }
    let saved = h.checkpoint().unwrap();
    let mut replies = vec![
        StreamReply {
            source: 11,
            reply: reply(&h, 11),
        },
        StreamReply {
            source: 12,
            reply: reply(&h, 12),
        },
    ];
    replies[1].reply.unqueued = [0; 4];
    assert!(h.shutdown(&replies).is_err());
    assert_eq!(saved, h.checkpoint().unwrap());
    replies[1].reply = reply(&h, 12);
    let mut cold = Host::restore(&saved).unwrap();
    assert_eq!(
        h.shutdown(&replies).unwrap(),
        cold.shutdown(&replies).unwrap()
    );
    assert_eq!(h.checkpoint().unwrap(), cold.checkpoint().unwrap());
    assert!(h.is_closed());
    assert_eq!(h.counts(), (0, 0, 0));
    assert!(h.shutdown(&[]).unwrap().is_empty());
    assert!(h.register_pcm(&wave(), &registration()).is_err());
    assert!(h.service_round(1).is_err());
}
#[test]
fn corrupt_combined_snapshot_does_not_replace_live_resources() {
    let mut h = Host::new(1).unwrap();
    add_pcm(&mut h);
    attach(&mut h, 1, 1);
    add_stream(&mut h, 1, OGG, true);
    let saved = h.checkpoint().unwrap();
    let v: Value = serde_json::from_slice(&saved).unwrap();
    for which in 0..4 {
        let mut bad = v.clone();
        match which {
            0 => bad["cursor"] = json!(99),
            1 => {
                bad["streams"][0]["host"]["bridge"]["tick"]["driver"]["node"]["hardware"] = json!(1)
            }
            2 => bad["bank"]["sources"][0]["buffer"] = json!(0),
            _ => bad["schema"] = json!(2),
        };
        assert!(h.load(&serde_json::to_vec(&bad).unwrap()).is_err());
        assert_eq!(h.checkpoint().unwrap(), saved);
    }
}
fn corrupt_last_audio_packet(mut b: Vec<u8>) -> Vec<u8> {
    let mut at = 0;
    let mut last = 0;
    while at < b.len() {
        last = at;
        let n = b[at + 26] as usize;
        at += 27
            + n
            + b[at + 27..at + 27 + n]
                .iter()
                .map(|&x| x as usize)
                .sum::<usize>();
    }
    let n = b[last + 26] as usize;
    let start = last + 27 + n;
    let mut packet = start;
    let mut next = start;
    for &lace in &b[last + 27..start] {
        next += lace as usize;
        if lace != 255 && next < b.len() {
            packet = next;
        }
    }
    b[packet] |= 1;
    b[last + 22..last + 26].fill(0);
    let mut crc = 0u32;
    for &v in &b[last..] {
        crc ^= u32::from(v) << 24;
        for _ in 0..8 {
            crc = (crc << 1) ^ if crc & 0x80000000 != 0 { 0x04c11db7 } else { 0 };
        }
    }
    b[last + 22..last + 26].copy_from_slice(&crc.to_le_bytes());
    b
}
#[test]
fn failed_second_decoder_rolls_back_the_whole_service_round() {
    let mut h = Host::new(1).unwrap();
    add_stream(&mut h, 1, OGG, true);
    add_stream(&mut h, 2, &corrupt_last_audio_packet(OGG.to_vec()), true);
    h.service_round(2).unwrap();
    for key in [11, 12] {
        h.tick_stream(key, &step(&h, key)).unwrap();
    }
    let saved = h.checkpoint().unwrap();
    assert!(h.service_round(2).is_err());
    assert_eq!(h.checkpoint().unwrap(), saved);
}
