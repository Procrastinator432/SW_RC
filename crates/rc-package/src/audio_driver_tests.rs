use crate::audio_driver::{Command, Event, Outcome, Reply, Session, Step};
use serde::{de::DeserializeOwned, Serialize};
use serde_json::{json, Value};
use std::sync::OnceLock;
fn data() -> &'static Value {
    static DATA: OnceLock<Value> = OnceLock::new();
    DATA.get_or_init(|| {
        serde_json::from_str(include_str!("../../../fixtures/audio-driver-native.json")).unwrap()
    })
}
fn read<T: DeserializeOwned>(v: &Value) -> T {
    serde_json::from_value(v.clone()).unwrap()
}
fn same<T: Serialize>(v: &T, expected: &Value) {
    assert_eq!(serde_json::to_value(v).unwrap(), *expected);
}
fn cases() -> &'static [Value] {
    data()["cases"].as_array().unwrap()
}
fn run(c: &Value) -> (Session, Outcome) {
    let mut session: Session = read(&c["before"]);
    let outcome = session
        .execute(&read::<Command>(&c["command"]), &read::<Reply>(&c["reply"]))
        .unwrap();
    same(&outcome, &c["outcome"]);
    same(&session, &c["expected"]);
    (session, outcome)
}
#[test]
fn native_driver_play_and_raw_pause_resume_preserve_callback_write_order() {
    for c in cases()
        .iter()
        .filter(|c| c["command"] == "Play" || c["command"].is_object())
    {
        run(c);
    }
    assert!(cases()
        .iter()
        .any(|c| c["command"] == json!({"Pause":-7}) && c["outcome"]["result"] == 1));
    assert!(cases()
        .iter()
        .any(|c| c["command"] == "Play" && c["outcome"]["result"] == 0));
}
#[test]
fn native_driver_stop_unqueues_exactly_four_and_observes_returned_ids_before_clear() {
    let mut single = 0;
    let mut queue = 0;
    let mut loads = 0;
    for c in cases().iter().filter(|c| c["command"] == "Stop") {
        let (_, o) = run(c);
        for e in o.events {
            match e {
                Event::Integer {
                    parameter: 0x1009,
                    value: 0,
                    ..
                } => single += 1,
                Event::Unqueue { count: 4, .. } => queue += 1,
                Event::SetHandle {
                    request: 0,
                    buffer_before,
                    allocated_before: true,
                    ..
                } => {
                    assert_ne!(buffer_before, 0);
                    loads += 1;
                }
                _ => {}
            }
        }
    }
    assert!(single > 0 && queue > 0 && loads > 0);
}
#[test]
fn native_driver_status_separates_local_markers_from_actual_al_stopped_reply() {
    for c in cases().iter().filter(|c| {
        matches!(
            c["command"].as_str(),
            Some("IsPlaying" | "IsPaused" | "IsQueued" | "IsStopped")
        )
    }) {
        run(c);
    }
    for c in cases().iter().filter(|c| c["command"] == "IsQueued") {
        assert_eq!(c["outcome"]["result"], 0);
    }
    assert!(cases()
        .iter()
        .any(|c| c["command"] == "IsPlaying" && c["outcome"]["result"] == u32::MAX));
}
#[test]
fn native_finished_uses_buffer_token_sound_object_flags_fade_and_duration_gates() {
    let mut duration = 0;
    let mut state = 0;
    let mut early = 0;
    for c in cases().iter().filter(|c| c["command"] == "IsFinished") {
        let (_, o) = run(c);
        duration += o
            .events
            .iter()
            .filter(|e| matches!(e, Event::Duration { .. }))
            .count();
        state += o
            .events
            .iter()
            .filter(|e| matches!(e, Event::GetState { .. }))
            .count();
        early += usize::from(o.events.is_empty());
    }
    assert!(duration > 0 && state > 0 && early > 0);
}
#[test]
fn all_whole_native_driver_calls_and_cold_snapshots_match() {
    assert_eq!(cases().len(), 800);
    for c in cases() {
        let (session, _) = run(c);
        let bytes = session.checkpoint().unwrap();
        let mut saved: Session = read(&c["before"]);
        saved.load(&bytes).unwrap();
        assert_eq!(saved.checkpoint().unwrap(), bytes);
        same(&saved, &c["expected"]);
    }
}
#[test]
fn native_driver_sequences_continue_after_every_cold_restore_and_match_atomic_batch() {
    let sequences = data()["sequences"].as_array().unwrap();
    assert_eq!(sequences.len(), 96);
    for seq in sequences {
        let mut session: Session = read(&seq["before"]);
        let mut steps = vec![];
        let mut expected = vec![];
        for step in seq["steps"].as_array().unwrap() {
            let bytes = session.checkpoint().unwrap();
            session.load(&bytes).unwrap();
            let command = read::<Command>(&step["command"]);
            let reply = read::<Reply>(&step["reply"]);
            let outcome = session.execute(&command, &reply).unwrap();
            same(&outcome, &step["outcome"]);
            same(&session, &step["expected"]);
            steps.push(Step { command, reply });
            expected.push(outcome);
        }
        let mut batch: Session = read(&seq["before"]);
        assert_eq!(batch.batch(&steps).unwrap(), expected);
        assert_eq!(batch.checkpoint().unwrap(), session.checkpoint().unwrap());
    }
}
#[test]
fn late_driver_resource_errors_and_invalid_snapshots_are_atomic() {
    let c = cases()
        .iter()
        .find(|c| c["command"] == "Stop" && c["before"]["node"]["source"]["allocated"] == true)
        .unwrap();
    let mut s: Session = read(&c["before"]);
    s.buffers.clear();
    let bytes = s.checkpoint().unwrap();
    let reply: Reply = read(&c["reply"]);
    assert!(s
        .batch(&[
            Step {
                command: Command::Play,
                reply: reply.clone()
            },
            Step {
                command: Command::Pause(-7),
                reply: reply.clone()
            },
            Step {
                command: Command::Stop,
                reply: reply.clone()
            }
        ])
        .is_err());
    assert_eq!(s.checkpoint().unwrap(), bytes);
    for (field, value) in [("schema", json!(2)), ("unknown", json!(true))] {
        let mut bad: Value = serde_json::from_slice(&bytes).unwrap();
        bad[field] = value;
        assert!(s.load(&serde_json::to_vec(&bad).unwrap()).is_err());
        assert_eq!(s.checkpoint().unwrap(), bytes);
    }
    let mut bad: Value = serde_json::from_slice(&bytes).unwrap();
    for duration_bits in [f32::NAN.to_bits(), 0x30800000u32] {
        bad["state"]["sounds"][0]["duration_bits"] = json!(duration_bits);
        assert!(s.load(&serde_json::to_vec(&bad).unwrap()).is_err());
        assert_eq!(s.checkpoint().unwrap(), bytes);
    }
    s.sounds.clear();
    let bytes = s.checkpoint().unwrap();
    assert!(s.execute(&Command::Stop, &reply).is_err());
    assert_eq!(s.checkpoint().unwrap(), bytes);
}
#[test]
fn native_driver_vtable_offsets_and_backing_storage_contract_are_explicit() {
    let v = data()["vtable"].as_array().unwrap();
    assert!(data()["sound_vtable"]
        .as_array()
        .unwrap()
        .iter()
        .any(|e| e["offset"] == 0x94 && e["address"] == 0x103d06c0u32));
    for (offset, address) in [
        (8, 0x10004f90u32),
        (12, 0x10005040),
        (16, 0x10006760),
        (20, 0x10004f40),
        (24, 0x100051c0),
        (28, 0x10004f60),
        (32, 0x10004f50),
    ] {
        assert!(v
            .iter()
            .any(|e| e["offset"] == offset && e["address"] == address));
    }
    let c = cases()
        .iter()
        .find(|c| {
            c["command"] == "Stop"
                && c["before"]["node"]["source"]["allocated"] == true
                && c["before"]["buffers"][0]["count_word"].as_u64().unwrap() & 0x1fffffff == 0
        })
        .unwrap();
    let mut s: Session = read(&c["before"]);
    s.buffers[0].storage.clear();
    let bytes = s.checkpoint().unwrap();
    assert!(s
        .execute(&Command::Stop, &read::<Reply>(&c["reply"]))
        .is_err());
    assert_eq!(s.checkpoint().unwrap(), bytes);
}
