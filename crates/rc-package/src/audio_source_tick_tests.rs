use crate::{audio_driver as driver, audio_source_tick::*};
use serde::{de::DeserializeOwned, Serialize};
use serde_json::{json, Value};
use std::sync::OnceLock;
fn data() -> &'static Value {
    static DATA: OnceLock<Value> = OnceLock::new();
    DATA.get_or_init(|| {
        serde_json::from_str(include_str!(
            "../../../fixtures/audio-source-tick-native.json"
        ))
        .unwrap()
    })
}
fn read<T: DeserializeOwned>(v: &Value) -> T {
    serde_json::from_value(v.clone()).unwrap()
}
fn same<T: Serialize>(actual: &T, v: &Value) {
    assert_eq!(serde_json::to_value(actual).unwrap(), *v);
}
fn step(c: &Value) -> Step {
    Step {
        environment: read(&c["environment"]),
        reply: read(&c["reply"]),
        delta_bits: c["delta_bits"].as_u64().unwrap() as u32,
    }
}
fn run(c: &Value) -> Session {
    let mut s: Session = read(&c["before"]);
    same(&s.execute(&step(c)).unwrap(), &c["outcome"]);
    same(&s, &c["expected"]);
    s
}
fn cases() -> &'static [Value] {
    data()["cases"].as_array().unwrap()
}
fn event(c: &Value, name: &str) -> bool {
    c["outcome"]["events"]
        .as_array()
        .unwrap()
        .iter()
        .any(|e| e.get(name).is_some())
}
#[test]
fn original_tick_inactive_and_paused_have_no_callbacks_or_clock_progress() {
    let mut count = 0;
    for c in cases().iter().filter(|c| c["outcome"]["result"] == 0) {
        run(c);
        assert_eq!(c["before"], c["expected"]);
        count += 1;
    }
    assert!(count > 20);
}
#[test]
fn original_tick_gain_position_reference_and_eax_upload_order() {
    let mut versions = std::collections::BTreeSet::new();
    let mut distance_eax = false;
    for c in cases().iter().filter(|c| c["outcome"]["result"] == 1) {
        run(c);
        let e = c["outcome"]["events"].as_array().unwrap();
        assert!(e[0].get("Params").is_some());
        assert!(e[1].get("Category").is_some());
        assert_eq!(e[2]["Float"]["parameter"], 0x100a);
        for v in e {
            if let Some(eax) = v.get("Eax") {
                versions.insert(eax["version"].as_u64().unwrap());
                distance_eax |= c["before"]["driver"]["node"]["source"]["flags"]
                    .as_u64()
                    .unwrap()
                    & 0x1000
                    != 0;
            }
        }
    }
    assert_eq!(versions.into_iter().collect::<Vec<_>>(), vec![2, 3]);
    assert!(distance_eax);
}
#[test]
fn original_refill_one_buffer_handles_alias_and_returned_unqueue_id() {
    let mut alias = false;
    let mut many = false;
    for c in cases().iter().filter(|c| event(c, "Data")) {
        run(c);
        let e = c["outcome"]["events"].as_array().unwrap();
        let i = e.iter().position(|v| v.get("Unqueue").is_some()).unwrap();
        assert_eq!(e[i]["Unqueue"]["buffers"].as_array().unwrap().len(), 1);
        assert!(e[i + 1].get("Data").is_some());
        assert!(e[i + 2].get("Queue").is_some());
        assert!(e[i + 3].get("RequestChunks").is_some());
        assert_eq!(e[i + 3]["RequestChunks"]["count"], 1);
        if c["before"]["progress"][0]["indices"][0] == c["before"]["progress"][0]["indices"][1] {
            alias = true;
            assert_eq!(e[i + 1]["Data"]["buffer"], c["reply"]["unqueued"]);
        }
        many |= c["reply"]["processed"] == 5;
    }
    assert!(alias && many);
}
#[test]
fn original_stream_restart_uses_actual_play_without_resetting_lifetime() {
    let mut refill = false;
    let mut waiting = false;
    for c in cases().iter().filter(|c| event(c, "AlPlay")) {
        let s = run(c);
        assert_eq!(c["reply"]["state"], 0x1014);
        assert_eq!(s.driver.node.source.cursor, 1);
        assert_eq!(s.driver.node.source.requested_pause, 0);
        refill |= event(c, "Data");
        waiting |= !event(c, "Data");
    }
    assert!(refill && waiting);
}
#[test]
fn original_stream_end_destroy_free_handle_order_and_later_completion() {
    for c in cases().iter().filter(|c| event(c, "DestroyStream")) {
        let mut s = run(c);
        let e = c["outcome"]["events"].as_array().unwrap();
        let i = e
            .iter()
            .position(|v| v.get("DestroyStream").is_some())
            .unwrap();
        assert!(e[i + 1].get("Free").is_some());
        assert!(e[i + 2].get("SetHandle").is_some());
        assert_eq!(s.driver.buffers[0].stream_token, 0);
        assert_eq!(s.progress[0].staging, 0);
        assert_eq!(s.driver.sounds[0].native_handle, 0);
        let outcome = s.execute(&step(c)).unwrap();
        assert!(!outcome
            .events
            .iter()
            .any(|e| matches!(e, Event::Stream(StreamEvent::QueryStream { .. }))));
        let _ = s
            .driver
            .execute(
                &driver::Command::IsFinished,
                &driver::Reply {
                    source_state: 0x1012,
                    unqueued: [0; 4],
                },
            )
            .unwrap();
    }
}
#[test]
fn original_tick_sequences_match_after_every_cold_restore() {
    for sequence in data()["sequences"].as_array().unwrap() {
        let mut s: Session = read(&sequence["before"]);
        for c in sequence["steps"].as_array().unwrap() {
            let saved = s.checkpoint().unwrap();
            let mut restored = s.clone();
            restored.load(&saved).unwrap();
            let a = s.execute(&step(c)).unwrap();
            let b = restored.execute(&step(c)).unwrap();
            assert_eq!(a, b);
            same(&a, &c["outcome"]);
            same(&s, &c["expected"]);
            assert_eq!(s.checkpoint().unwrap(), restored.checkpoint().unwrap());
        }
    }
}
#[test]
fn tick_failures_restore_entire_batch_and_reject_bad_snapshot() {
    let c = cases().iter().find(|c| event(c, "Data")).unwrap();
    let mut s: Session = read(&c["before"]);
    let saved = s.checkpoint().unwrap();
    let valid = step(c);
    let mut bad = valid.clone();
    bad.delta_bits = f32::NAN.to_bits();
    assert!(s.batch(&[valid.clone(), bad]).is_err());
    assert_eq!(saved, s.checkpoint().unwrap());
    for value in [
        json!({"schema":2,"state":c["before"]}),
        json!({"schema":1,"state":c["before"],"extra":0}),
    ] {
        assert!(s.load(&serde_json::to_vec(&value).unwrap()).is_err());
        assert_eq!(saved, s.checkpoint().unwrap());
    }
    let mut bad = read::<Value>(&json!({"schema":1,"state":c["before"]}));
    bad["state"]["progress"][0]["indices"][0] = json!(9999);
    assert!(s.load(&serde_json::to_vec(&bad).unwrap()).is_err());
    assert_eq!(saved, s.checkpoint().unwrap());
    let mut missing = s.clone();
    missing.streams.clear();
    let before = missing.checkpoint().unwrap();
    assert!(missing.execute(&valid).is_err());
    assert_eq!(before, missing.checkpoint().unwrap());
}
#[test]
fn play_resumes_paused_tick_and_unknown_driver_replies_remain_raw() {
    let c = cases()
        .iter()
        .find(|c| {
            c["before"]["driver"]["node"]["source"]["allocated"] == true
                && c["before"]["driver"]["node"]["source"]["requested_pause"] != 0
                && c["before"]["driver"]["node"]["source"]["flags"] == 0x100
        })
        .unwrap();
    let mut s: Session = read(&c["before"]);
    assert_eq!(s.execute(&step(c)).unwrap().result, 0);
    s.driver
        .execute(
            &driver::Command::Play,
            &driver::Reply {
                source_state: 0,
                unqueued: [0; 4],
            },
        )
        .unwrap();
    assert_eq!(s.execute(&step(c)).unwrap().result, 1);
    assert!(cases()
        .iter()
        .any(|c| c["reply"]["processed"] == -1 && event(c, "Data")));
}
