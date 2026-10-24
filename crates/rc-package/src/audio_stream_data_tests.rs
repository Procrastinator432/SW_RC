use crate::audio_stream_data::*;
use serde::{de::DeserializeOwned, Serialize};
use serde_json::{json, Value};
use std::sync::OnceLock;
pub(super) fn data() -> &'static Value {
    static DATA: OnceLock<Value> = OnceLock::new();
    DATA.get_or_init(|| {
        serde_json::from_str(include_str!(
            "../../../fixtures/audio-stream-data-native.json"
        ))
        .unwrap()
    })
}
fn read<T: DeserializeOwned>(v: &Value) -> T {
    serde_json::from_value(v.clone()).unwrap()
}
fn same<T: Serialize>(a: &T, e: &Value) {
    assert_eq!(serde_json::to_value(a).unwrap(), *e);
}
fn cases() -> &'static [Value] {
    data()["cases"].as_array().unwrap()
}
fn run(c: &Value) -> Session {
    let mut s: Session = read(&c["before"]);
    same(&s.execute(&read(&c["action"])).unwrap(), &c["outcome"]);
    same(&s, &c["expected"]);
    s
}
#[test]
fn original_request_replaces_mailbox_and_query_preserves_state() {
    for c in cases()
        .iter()
        .filter(|c| c["action"] == "Query" || c["action"].get("Request").is_some())
    {
        run(c);
    }
    let c = &cases()[0];
    let mut s: Session = read(&c["before"]);
    s.execute(&Action::Request {
        chunks: 3,
        destination: 123,
    })
    .unwrap();
    s.execute(&Action::Request {
        chunks: 1,
        destination: 456,
    })
    .unwrap();
    assert_eq!((s.state.requested, s.state.cursor), (1, 456));
}
#[test]
fn original_partial_pcm_writes_and_silence_preserve_all_destination_bytes() {
    let mut zeros = false;
    let mut partial = false;
    for c in cases().iter().filter(|c| c["action"].get("Read").is_some()) {
        run(c);
        partial |= c["outcome"]["consumed"].as_u64().unwrap() > 1;
        zeros |= c["before"]["state"]["ended"] != c["expected"]["state"]["ended"];
    }
    assert!(partial && zeros);
}
#[test]
fn original_seek_widens_time_and_does_not_clear_end_state() {
    for c in cases().iter().filter(|c| c["action"].get("Seek").is_some()) {
        run(c);
        assert_eq!(c["before"], c["expected"]);
    }
}
#[test]
fn original_worker_sequences_match_after_every_cold_restore() {
    for seq in data()["sequences"].as_array().unwrap() {
        let mut s: Session = read(&seq["before"]);
        for c in seq["steps"].as_array().unwrap() {
            let saved = s.checkpoint().unwrap();
            let mut copy = s.clone();
            copy.load(&saved).unwrap();
            let action = read(&c["action"]);
            let a = s.execute(&action).unwrap();
            let b = copy.execute(&action).unwrap();
            assert_eq!(a, b);
            same(&a, &c["outcome"]);
            same(&s, &c["expected"]);
            assert_eq!(s.checkpoint().unwrap(), copy.checkpoint().unwrap());
        }
    }
}
fn pcm_session(kind: i32) -> Session {
    Session {
        state: State {
            handle: 11,
            critical: 123,
            decoder: 456,
            cursor: 1024,
            chunk_bytes: 8,
            requested: 1,
            loops: 0,
            bitstream: 0,
            ended: 0,
            kind,
        },
        heap: Heap {
            regions: vec![Region {
                base: 1024,
                bytes: vec![0xaa; 32],
            }],
        },
    }
}
#[test]
fn owned_pcm_provider_fills_tail_and_loops_actual_bytes() {
    let mut s = pcm_session(0);
    let mut pcm = Pcm {
        bytes: vec![1, 2, 3],
        cursor: 0,
        max_read: 2,
        bitstream: 7,
    };
    let result = s.service_pcm(1, &mut pcm).unwrap();
    assert_eq!(result.result, Some(0));
    assert_eq!(
        &s.heap.regions[0].bytes[..9],
        &[1, 2, 3, 0, 0, 0, 0, 0, 0xaa]
    );
    assert_eq!(s.state.ended, 1);
    assert_eq!(s.state.bitstream, 7);
    let mut s = pcm_session(1);
    let mut pcm = Pcm {
        bytes: vec![1, 2, 3],
        cursor: 0,
        max_read: 2,
        bitstream: 0,
    };
    assert_eq!(s.service_pcm(1, &mut pcm).unwrap().result, Some(1));
    assert_eq!(&s.heap.regions[0].bytes[..8], &[1, 2, 3, 1, 2, 3, 1, 2]);
    assert_eq!(s.state.loops, 2);
    assert_eq!(pcm.cursor, 2);
}
#[test]
fn eof_error_and_empty_loop_have_distinct_state_and_atomic_failure() {
    let mut s = pcm_session(0);
    s.execute(&Action::Read {
        chunks: 1,
        replies: vec![Decode::Error(-3)],
        seek_reply: 0,
    })
    .unwrap();
    assert_eq!(s.state.ended, 0);
    assert_eq!(&s.heap.regions[0].bytes[..8], &[0; 8]);
    let mut s = pcm_session(1);
    let mut pcm = Pcm {
        bytes: vec![],
        cursor: 0,
        max_read: 8,
        bitstream: 0,
    };
    let saved = s.checkpoint().unwrap();
    assert!(s.service_pcm(1, &mut pcm).is_err());
    assert_eq!(saved, s.checkpoint().unwrap());
    assert_eq!(pcm.cursor, 0);
}
#[test]
fn pcm_bounds_snapshot_and_batch_failures_preserve_owned_data() {
    let mut s = pcm_session(0);
    let saved = s.checkpoint().unwrap();
    let request = Action::Request {
        chunks: 1,
        destination: 1024,
    };
    let bad = Action::Read {
        chunks: 5,
        replies: vec![Decode::End],
        seek_reply: 0,
    };
    assert!(s.batch(&[request, bad]).is_err());
    assert_eq!(saved, s.checkpoint().unwrap());
    let mut value = serde_json::to_value(&s).unwrap();
    value["heap"]["regions"][0]["base"] = json!(0);
    assert!(s
        .load(&serde_json::to_vec(&json!({"schema":1,"state":value})).unwrap())
        .is_err());
    assert_eq!(saved, s.checkpoint().unwrap());
    let mut heap = s.heap.clone();
    heap.regions.push(heap.regions[0].clone());
    assert!(heap.validate().is_err());
    assert!(s.heap.read(u32::MAX, 2).is_err());
    assert!(s.heap.free(777).is_err());
    assert_eq!(saved, s.checkpoint().unwrap());
}
