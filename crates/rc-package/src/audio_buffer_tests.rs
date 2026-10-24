use crate::{audio_buffer::*, audio_buffer_bank as bank, audio_wave as wave};
use serde::{de::DeserializeOwned, Serialize};
use serde_json::Value;
fn fixture() -> Value {
    serde_json::from_str(include_str!("../../../fixtures/audio-buffer-native.json")).unwrap()
}
fn registrations() -> Value {
    serde_json::from_str(include_str!(
        "../../../fixtures/audio-wave-registration-native.json"
    ))
    .unwrap()
}
fn read<T: DeserializeOwned>(v: &Value) -> T {
    serde_json::from_value(v.clone()).unwrap()
}
fn same<T: Serialize>(a: &T, b: &Value) {
    assert_eq!(serde_json::to_value(a).unwrap(), *b);
}
#[test]
fn native_wave_scans_preserve_chunk_order_padding_first_match_and_sampler() {
    for c in fixture()["waves"].as_array().unwrap() {
        let b: Vec<u8> = read(&c["bytes"]);
        same(&wave::scan(&b).unwrap(), &c["expected"]);
    }
}
#[test]
fn wave_admission_rejects_truncation_bad_pcm_and_container_lengths() {
    let f = registrations();
    let bytes: Vec<u8> = read(&f["cases"][1]["wave"]);
    wave::Pcm::from_wave(&bytes).unwrap();
    for i in 0..bytes.len() {
        assert!(wave::Pcm::from_wave(&bytes[..i]).is_err());
    }
    let mut bad = bytes.clone();
    bad[0] = 0;
    assert!(wave::scan(&bad).is_ok());
    assert!(wave::Pcm::from_wave(&bad).is_err());
    for at in [20, 22, 24, 28, 32, 34] {
        let mut bad = bytes.clone();
        bad[at..at + 2].fill(0);
        assert!(wave::Pcm::from_wave(&bad).is_err());
    }
    for channels in [0, 1, 2, 3, 65535] {
        for bits in [0, 8, 16, 24, 65535] {
            assert_eq!(
                wave::registration_format(channels, bits),
                0x1100 + u32::from(channels != 1) * 2 + u32::from(bits != 8)
            );
        }
    }
}
#[test]
fn native_constructor_has_no_stale_resource_fields() {
    for c in fixture()["constructors"].as_array().unwrap() {
        let b = Buffer::new(
            c["handle"].as_u64().unwrap() as u32,
            c["owner"].as_u64().unwrap() as u32,
        );
        b.validate().unwrap();
        same(
            &[
                b.owner,
                b.stream,
                b.staging,
                b.sound,
                b.allocation,
                b.count_word,
                b.indices[0],
                b.indices[1],
            ],
            &c["words"],
        );
    }
}
#[test]
fn whole_native_release_and_destructor_follow_stop_replies_and_array_flags() {
    for c in fixture()["releases"].as_array().unwrap() {
        let mut s: Session = read(&c["before"]);
        let events = s
            .release(&read(&c["reply"]), c["destructor"].as_bool().unwrap())
            .unwrap();
        same(&events, &c["events"]);
        same(&s, &c["expected"]);
    }
}
#[test]
fn whole_native_unregister_removes_every_registry_match_before_cleanup() {
    for c in fixture()["unregister"].as_array().unwrap() {
        let mut s: Session = read(&c["before"]);
        let mut registry = read(&c["registry"]);
        let events = s
            .unregister(
                &mut registry,
                c["sound"].as_u64().unwrap() as u32,
                c["reported"].as_u64().unwrap() as u32,
                &read(&c["reply"]),
            )
            .unwrap();
        same(&events, &c["events"]);
        same(&s, &c["expected"]);
        same(&registry, &c["registry_after"]);
    }
}
#[test]
fn whole_native_wav_precache_matches_pcm_upload_and_handle_admission() {
    let mut uploads = 0;
    let mut failures = 0;
    for c in registrations()["cases"].as_array().unwrap() {
        let mut b = bank::Bank::new(c["owner"].as_u64().unwrap() as u32);
        let r: bank::Registration = read(&c["registration"]);
        let bytes: Vec<u8> = read(&c["wave"]);
        if c["reported"].as_u64().unwrap() != 0 {
            let mut seed = r.clone();
            seed.allow_stereo = true;
            seed.generation_error = 0;
            seed.upload_error = 0;
            b.register(&bytes, &seed).unwrap();
            let out = b.register(&[], &r).unwrap();
            assert!(out.cached);
            assert_eq!(out.handle, c["result"].as_u64().unwrap() as u32);
            assert_eq!(out.events.len(), 1);
            continue;
        }
        let p = wave::Pcm::from_wave(&bytes).unwrap();
        if p.info.channels != 1 && !r.allow_stereo {
            assert!(b.register(&bytes, &r).is_err());
            assert_eq!(c["result"], 0);
            continue;
        }
        let out = b.register(&bytes, &r).unwrap();
        assert_eq!(out.handle, c["result"].as_u64().unwrap() as u32);
        let native = c["events"]
            .as_array()
            .unwrap()
            .iter()
            .find_map(|e| e.get("Upload"))
            .unwrap();
        let upload = out
            .events
            .iter()
            .find_map(|e| {
                if let bank::Event::Upload(u) = e {
                    Some(u)
                } else {
                    None
                }
            })
            .unwrap();
        same(upload, native);
        uploads += 1;
        if r.upload_error != 0 {
            assert!(b.entries.is_empty());
            assert!(out.events.contains(&bank::Event::Cleanup(Event::Delete {
                buffer: r.generated
            })));
            assert!(!c["events"]
                .as_array()
                .unwrap()
                .iter()
                .any(|e| e.get("Delete").is_some()));
            failures += 1;
        } else {
            assert_eq!(b.entries.len(), 1);
            assert_eq!(b.entries[0].buffer.storage, [r.generated]);
            assert_eq!(c["registry_count"], 1);
        }
    }
    assert!(uploads > 70 && failures > 5);
}
#[test]
fn pcm_bank_cold_reuse_unregister_and_reregister_keep_exact_bytes() {
    let f = registrations();
    let c = &f["cases"][2];
    let bytes: Vec<u8> = read(&c["wave"]);
    let mut r: bank::Registration = read(&c["registration"]);
    r.allow_stereo = true;
    r.upload_error = 0;
    let mut b = bank::Bank::new(c["owner"].as_u64().unwrap() as u32);
    b.register(&bytes, &r).unwrap();
    b.sources = vec![
        Source {
            handle: 1,
            buffer: r.handle,
        },
        Source {
            handle: 2,
            buffer: 900,
        },
    ];
    let before = b.checkpoint().unwrap();
    let mut cold = bank::Bank::new(1);
    cold.load(&before).unwrap();
    assert_eq!(
        b.register(&[], &r).unwrap(),
        cold.register(&[], &r).unwrap()
    );
    let reply = Reply {
        stops: vec![StopReply {
            detached: true,
            storage: None,
        }],
        reallocated: 0,
    };
    let a = b.unregister(r.sound, &reply).unwrap();
    let z = cold.unregister(r.sound, &reply).unwrap();
    assert_eq!(a, z);
    assert_eq!(b.checkpoint().unwrap(), cold.checkpoint().unwrap());
    assert!(b.entries.is_empty());
    assert_eq!(b.sources[0].buffer, 0);
    assert_eq!(b.sources[1].buffer, 900);
    let saved = b.checkpoint().unwrap();
    cold.load(&saved).unwrap();
    r.generated += 1;
    r.handle += 4;
    r.allocation += 4;
    assert_eq!(
        b.register(&bytes, &r).unwrap(),
        cold.register(&bytes, &r).unwrap()
    );
    assert_eq!(b.checkpoint().unwrap(), cold.checkpoint().unwrap());
}
#[test]
fn missing_stop_duplicate_resources_and_corrupt_snapshot_do_not_partially_free() {
    let f = fixture();
    let c = f["releases"]
        .as_array()
        .unwrap()
        .iter()
        .find(|c| !c["reply"]["stops"].as_array().unwrap().is_empty())
        .unwrap();
    let mut s: Session = read(&c["before"]);
    let before = serde_json::to_value(&s).unwrap();
    let bad = Reply {
        stops: vec![],
        reallocated: 0,
    };
    assert!(s.release(&bad, true).is_err());
    same(&s, &before);
    let f = registrations();
    let c = &f["cases"][2];
    let bytes: Vec<u8> = read(&c["wave"]);
    let r: bank::Registration = read(&c["registration"]);
    let mut b = bank::Bank::new(c["owner"].as_u64().unwrap() as u32);
    b.register(&bytes, &r).unwrap();
    let saved = b.checkpoint().unwrap();
    let mut duplicate = r.clone();
    duplicate.sound += 1;
    assert!(b.register(&bytes, &duplicate).is_err());
    assert_eq!(b.checkpoint().unwrap(), saved);
    let mut corrupt: Value = serde_json::from_slice(&saved).unwrap();
    corrupt["bank"]["entries"][0]["pcm"]["info"]["byte_rate"] = 0.into();
    assert!(b.load(&serde_json::to_vec(&corrupt).unwrap()).is_err());
    assert_eq!(b.checkpoint().unwrap(), saved);
    b.sources.push(Source {
        handle: 123,
        buffer: r.handle,
    });
    let bound = b.checkpoint().unwrap();
    for stop in [
        StopReply {
            detached: false,
            storage: None,
        },
        StopReply {
            detached: true,
            storage: Some(vec![999]),
        },
    ] {
        assert!(b
            .unregister(
                r.sound,
                &Reply {
                    stops: vec![stop],
                    reallocated: 0
                }
            )
            .is_err());
        assert_eq!(b.checkpoint().unwrap(), bound);
    }
}
