use crate::{
    audio_source_tick as tick, audio_stream_bridge as bridge,
    audio_upload::Environment,
    audio_vorbis::*,
    audio_vorbis_host::{Session, Tokens},
};
const MONO: &[u8] = include_bytes!("../../../fixtures/audio-vorbis/mono.ogg");
const STEREO: &[u8] = include_bytes!("../../../fixtures/audio-vorbis/stereo.ogg");
#[test]
fn original_init_confirms_format_selection_and_chunk_size() {
    let f: serde_json::Value = serde_json::from_str(include_str!(
        "../../../fixtures/audio-vorbis/init-native.json"
    ))
    .unwrap();
    let mut checked = 0;
    for c in f["cases"].as_array().unwrap() {
        let q = &c["input"];
        let e = &c["expected"];
        if q["file"].as_u64().unwrap() != 0 && q["open_reply"].as_i64().unwrap() >= 0 {
            let fmt = al_format(q["channels"].as_i64().unwrap() as i32);
            assert_eq!(fmt.unwrap_or(0), e["format"].as_u64().unwrap() as u32);
            assert_eq!(fmt.is_some(), e["result"].as_u64().unwrap() != 0);
            assert_eq!(CHUNK_BYTES, e["chunk_bytes"].as_u64().unwrap() as usize);
            checked += 1;
        }
    }
    assert!(checked > 90);
}
fn decode(bytes: &[u8], max: usize) -> Vec<u8> {
    let mut d = Provider::new(bytes.to_vec()).unwrap();
    let mut result = vec![];
    loop {
        let b = d.read_bytes(max).unwrap();
        if b.is_empty() {
            break;
        }
        result.extend(b);
    }
    result
}
fn host(bytes: &[u8], looping: bool) -> Session {
    let tick: tick::Session = serde_json::from_str(include_str!(
        "../../../fixtures/audio-vorbis/host-tick.json"
    ))
    .unwrap();
    Session::register(
        tick,
        Provider::new(bytes.to_vec()).unwrap(),
        Tokens {
            stream: 11,
            critical: 12,
            decoder: 13,
            staging: 0x10000,
            buffers: [101, 102, 103, 104],
        },
        looping,
    )
    .unwrap()
}
fn tick_command(s: &Session, processed: i32) -> bridge::Command {
    let state = s.state();
    let unqueued = state.tick.driver.buffers[0].storage[state.tick.progress[0].indices[1] as usize];
    bridge::Command::Tick(bridge::Tick {
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
            processed,
            queued: 4,
            unqueued,
        },
        delta_bits: (1f32 / 60.).to_bits(),
    })
}
#[test]
fn real_vorbis_matches_independent_libsndfile_frame_counts_and_samples() {
    for (ogg, golden, channels, hz) in [
        (
            MONO,
            include_bytes!("../../../fixtures/audio-vorbis/mono.s16le").as_slice(),
            1,
            22050,
        ),
        (
            STEREO,
            include_bytes!("../../../fixtures/audio-vorbis/stereo.s16le").as_slice(),
            2,
            44100,
        ),
        (
            include_bytes!("../../../fixtures/audio-vorbis/silence.ogg").as_slice(),
            include_bytes!("../../../fixtures/audio-vorbis/silence.s16le").as_slice(),
            1,
            22050,
        ),
        (
            include_bytes!("../../../fixtures/audio-vorbis/clipped.ogg").as_slice(),
            include_bytes!("../../../fixtures/audio-vorbis/clipped.s16le").as_slice(),
            1,
            22050,
        ),
    ] {
        let d = Provider::new(ogg.to_vec()).unwrap();
        assert_eq!(d.metadata().channels, channels);
        assert_eq!(d.metadata().frequency, hz);
        assert_eq!(
            d.metadata().frames as usize * usize::from(channels) * 2,
            golden.len()
        );
        let out = decode(ogg, 65536);
        assert_eq!(out.len(), golden.len());
        // Different decoder floating point/rounding: at most one int16 unit.
        for (a, b) in out.chunks_exact(2).zip(golden.chunks_exact(2)) {
            assert!(
                (i32::from(i16::from_le_bytes(a.try_into().unwrap()))
                    - i32::from(i16::from_le_bytes(b.try_into().unwrap())))
                .abs()
                    <= 1
            );
        }
        assert_eq!(out, decode(ogg, 7));
    }
}
#[test]
fn odd_byte_reads_and_mid_packet_cold_resume_are_exact() {
    let expected = decode(STEREO, 4096);
    for at in [0, 1, 173, 4097, 65535, expected.len() - 1, expected.len()] {
        let mut d = Provider::new(STEREO.to_vec()).unwrap();
        d.seek_byte(at as u64).unwrap();
        let mut cold = Provider::restore(&d.checkpoint().unwrap()).unwrap();
        let mut rest = vec![];
        loop {
            let a = d.read_bytes(173).unwrap();
            let b = cold.read_bytes(173).unwrap();
            assert_eq!(a, b);
            if a.is_empty() {
                break;
            }
            rest.extend(a);
        }
        assert_eq!(rest, expected[at..]);
    }
}
#[test]
fn damaged_truncated_chained_and_multichannel_files_fail() {
    for cut in [0, 1, 26, 27, 100, MONO.len() - 1] {
        assert!(Provider::new(MONO[..cut].to_vec()).is_err());
    }
    let mut bad = MONO.to_vec();
    let last = bad.len() - 1;
    bad[last] ^= 1;
    assert!(Provider::new(bad).is_err());
    let mut chain = MONO.to_vec();
    chain.extend(MONO);
    assert!(Provider::new(chain).is_err());
    assert!(Provider::new(
        include_bytes!("../../../fixtures/audio-vorbis/three-channel.ogg").to_vec()
    )
    .is_err());
    assert!(Provider::new(vec![0; crate::audio_stream_data::MAX_BYTES + 1]).is_err());
}
#[test]
fn seek_and_snapshot_failures_preserve_the_provider() {
    let mut d = Provider::new(MONO.to_vec()).unwrap();
    d.seek_byte(17).unwrap();
    let before = d.checkpoint().unwrap();
    assert!(d.seek_byte(10_000).is_err());
    assert_eq!(before, d.checkpoint().unwrap());
    assert!(d.read_bytes(0).is_err());
    assert_eq!(before, d.checkpoint().unwrap());
    let mut saved: serde_json::Value = serde_json::from_slice(&before).unwrap();
    saved["cursor"] = serde_json::json!(10_000);
    assert!(Provider::restore(&serde_json::to_vec(&saved).unwrap()).is_err());
    saved["schema"] = serde_json::json!(2);
    assert!(Provider::restore(&serde_json::to_vec(&saved).unwrap()).is_err());
}
#[test]
fn registration_prefills_four_buffers_and_preserves_short_stream_eof() {
    for ogg in [MONO, STEREO] {
        let h = host(ogg, false);
        let pcm = decode(ogg, 65536);
        let state = h.state();
        let joined: Vec<u8> = state.uploads.iter().flat_map(|u| u.bytes.clone()).collect();
        assert_eq!(joined.len(), 4 * CHUNK_BYTES);
        let n = pcm.len().min(joined.len());
        assert_eq!(joined[..n], pcm[..n]);
        assert!(joined[n..].iter().all(|&v| v == 0));
        assert_eq!(state.worker.as_ref().unwrap().requested, 1);
        assert_eq!(state.worker.as_ref().unwrap().cursor, 0x10000);
        assert_eq!(state.heap.regions[0].bytes.len(), CHUNK_BYTES);
        assert_eq!(
            state.worker.as_ref().unwrap().ended,
            i32::from(pcm.len() < 4 * CHUNK_BYTES)
        );
        assert_eq!(state.tick.driver.buffers[0].storage, [101, 102, 103, 104]);
    }
}
#[test]
fn real_decoder_loop_crosses_into_uploads_and_cold_host_continues() {
    let pcm = decode(STEREO, 65536);
    let mut h = host(STEREO, true);
    let mut offset = 4 * CHUNK_BYTES;
    for _ in 0..8 {
        let mut cold = Session::restore(&h.checkpoint().unwrap()).unwrap();
        let a = h.execute(&bridge::Command::Service).unwrap();
        let b = cold.execute(&bridge::Command::Service).unwrap();
        assert_eq!(a, b);
        let command = tick_command(&h, 1);
        let a = h.execute(&command).unwrap();
        let b = cold.execute(&command).unwrap();
        assert_eq!(a, b);
        assert_eq!(h.checkpoint().unwrap(), cold.checkpoint().unwrap());
        assert_eq!(a.uploads.len(), 1);
        let expected: Vec<u8> = (offset..offset + CHUNK_BYTES)
            .map(|i| pcm[i % pcm.len()])
            .collect();
        assert_eq!(a.uploads[0].bytes, expected);
        offset += CHUNK_BYTES;
    }
    assert!(h.state().worker.as_ref().unwrap().loops > 0);
    assert_eq!(h.state().worker.as_ref().unwrap().ended, 0);
}
#[test]
fn nonlooping_end_frees_decoder_and_staging_then_restores_closed() {
    let mut h = host(MONO, false);
    let command = tick_command(&h, 0);
    h.execute(&command).unwrap();
    assert!(h.metadata().is_none());
    assert!(h.state().worker.is_none());
    assert!(h.state().heap.regions.is_empty());
    assert_eq!(h.state().uploads.len(), 4);
    let saved = h.checkpoint().unwrap();
    let restored = Session::restore(&saved).unwrap();
    assert_eq!(saved, restored.checkpoint().unwrap());
    assert!(h.execute(&bridge::Command::Service).is_err());
    assert_eq!(saved, h.checkpoint().unwrap());
}
#[test]
fn busy_tick_and_bad_buffer_do_not_advance_decoder_or_publish_partial_state() {
    let mut h = host(STEREO, false);
    let cursor = h.decoded_cursor();
    let busy = h.execute(&tick_command(&h, 1)).unwrap();
    assert!(busy.uploads.is_empty());
    assert_eq!(h.decoded_cursor(), cursor);
    h.execute(&bridge::Command::Service).unwrap();
    let before = h.checkpoint().unwrap();
    let mut command = tick_command(&h, 1);
    if let bridge::Command::Tick(t) = &mut command {
        t.al.unqueued = 0;
    }
    assert!(h.execute(&command).is_err());
    assert_eq!(before, h.checkpoint().unwrap());
    let mut damaged: serde_json::Value = serde_json::from_slice(&before).unwrap();
    damaged["bridge"]["tick"]["streams"][0]["format"] = serde_json::json!(0x1101);
    assert!(Session::restore(&serde_json::to_vec(&damaged).unwrap()).is_err());
}
#[test]
fn live_vorbis_disposal_stops_before_delete_then_frees_decoder_and_all_bytes() {
    use crate::{
        audio_buffer::Event as B,
        audio_driver::{Event as D, Reply},
        audio_vorbis_host::{DisposalEvent as E, DisposalTokens},
    };
    let mut h = host(STEREO, true);
    h.execute(&bridge::Command::Service).unwrap();
    let mut cold = Session::restore(&h.checkpoint().unwrap()).unwrap();
    let tokens = DisposalTokens {
        owner: 1,
        allocation: 2,
        source: 3,
    };
    let reply = Reply {
        source_state: 0x1012,
        unqueued: [104, 102, 101, 103],
    };
    let events = h.dispose(&tokens, &reply).unwrap();
    assert_eq!(events, cold.dispose(&tokens, &reply).unwrap());
    assert_eq!(
        events
            .iter()
            .filter_map(|e| if let E::Buffer(B::Delete { buffer }) = e {
                Some(*buffer)
            } else {
                None
            })
            .collect::<Vec<_>>(),
        reply.unqueued
    );
    let stop = events
        .iter()
        .position(|e| matches!(e, E::Driver(D::AlStop { .. })))
        .unwrap();
    let delete = events
        .iter()
        .position(|e| matches!(e, E::Buffer(B::Delete { .. })))
        .unwrap();
    let destroy = events
        .iter()
        .position(|e| matches!(e, E::Buffer(B::DestroyStream { .. })))
        .unwrap();
    assert!(stop < delete && delete < destroy);
    assert!(h.metadata().is_none());
    assert!(h.state().uploads.is_empty());
    assert!(h.state().heap.regions.is_empty());
    assert!(h.state().tick.driver.buffers.is_empty());
    assert_eq!(h.checkpoint().unwrap(), cold.checkpoint().unwrap());
    let saved = h.checkpoint().unwrap();
    assert!(h.dispose(&tokens, &reply).unwrap().is_empty());
    assert_eq!(h.checkpoint().unwrap(), saved);
}
#[test]
fn disposing_an_ended_stream_does_not_destroy_or_free_the_stream_twice() {
    use crate::{
        audio_buffer::Event as B,
        audio_driver::Reply,
        audio_vorbis_host::{DisposalEvent as E, DisposalTokens},
    };
    let mut h = host(MONO, false);
    h.execute(&tick_command(&h, 0)).unwrap();
    let mut cold = Session::restore(&h.checkpoint().unwrap()).unwrap();
    let tokens = DisposalTokens {
        owner: 1,
        allocation: 2,
        source: 3,
    };
    let reply = Reply {
        source_state: 0x1014,
        unqueued: [101, 102, 103, 104],
    };
    let events = h.dispose(&tokens, &reply).unwrap();
    assert_eq!(events, cold.dispose(&tokens, &reply).unwrap());
    assert!(!events
        .iter()
        .any(|e| matches!(e, E::Buffer(B::DestroyStream { .. }))));
    assert!(!events
        .iter()
        .any(|e| matches!(e,E::Buffer(B::Free{allocation}) if *allocation==0x10000)));
    assert_eq!(
        events
            .iter()
            .filter(|e| matches!(e, E::Buffer(B::Delete { .. })))
            .count(),
        4
    );
}
#[test]
fn bad_disposal_tokens_or_unqueue_cannot_orphan_owned_buffers() {
    use crate::{audio_driver::Reply, audio_vorbis_host::DisposalTokens};
    let mut h = host(STEREO, true);
    let before = h.checkpoint().unwrap();
    let tokens = DisposalTokens {
        owner: 1,
        allocation: 2,
        source: 3,
    };
    let mut reply = Reply {
        source_state: 0x1012,
        unqueued: [0; 4],
    };
    assert!(h.dispose(&tokens, &reply).is_err());
    assert_eq!(h.checkpoint().unwrap(), before);
    reply.unqueued = [101, 102, 103, 104];
    assert!(h
        .dispose(&DisposalTokens { owner: 0, ..tokens }, &reply)
        .is_err());
    assert_eq!(h.checkpoint().unwrap(), before);
}
