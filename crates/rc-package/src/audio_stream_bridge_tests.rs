use crate::audio_stream_data_tests::data as fixture;
use crate::{audio_stream_bridge::*, audio_stream_data as data};
use serde::{de::DeserializeOwned, Serialize};
use serde_json::Value;
fn read<T: DeserializeOwned>(v: &Value) -> T {
    serde_json::from_value(v.clone()).unwrap()
}
fn same<T: Serialize>(a: &T, e: &Value) {
    assert_eq!(serde_json::to_value(a).unwrap(), *e);
}
#[test]
fn composed_original_pcm_driver_follows_every_byte_and_control_transition() {
    let mut uploads = 0;
    let mut closed = 0;
    for sequence in fixture()["composed"].as_array().unwrap() {
        let mut state: Session = read(&sequence["before"]);
        state.validate().unwrap();
        for c in sequence["steps"].as_array().unwrap() {
            let out = state.execute(&read(&c["command"])).unwrap();
            same(&out, &c["outcome"]);
            same(&state, &c["expected"]);
            uploads += out.uploads.len();
        }
        if state.worker.is_none() {
            closed += 1;
            assert!(state.pcm.is_none());
            assert!(state.heap.regions.is_empty());
            assert!(!state.uploads.is_empty());
        }
    }
    assert!(uploads > 50 && closed > 5);
}
#[test]
fn composed_pcm_and_buffer_bytes_survive_every_cold_resume() {
    for sequence in fixture()["composed"].as_array().unwrap() {
        let mut state: Session = read(&sequence["before"]);
        for c in sequence["steps"].as_array().unwrap() {
            let saved = state.checkpoint().unwrap();
            let mut restored = state.clone();
            restored.load(&saved).unwrap();
            let command = read(&c["command"]);
            let a = state.execute(&command).unwrap();
            let b = restored.execute(&command).unwrap();
            assert_eq!(a, b);
            assert_eq!(state.checkpoint().unwrap(), restored.checkpoint().unwrap());
            same(&state, &c["expected"]);
        }
    }
}
#[test]
fn busy_worker_defers_upload_and_zero_buffer_reply_rolls_back_owned_bytes() {
    let seq = &fixture()["composed"][0];
    let mut state: Session = read(&seq["before"]);
    let command: Command = read(&seq["steps"][1]["command"]);
    let out = state.execute(&command).unwrap();
    assert!(out.uploads.is_empty());
    assert!(state.worker.as_ref().unwrap().requested > 0);
    let mut state: Session = read(&seq["before"]);
    state.tick.progress[0].indices = [0, 0];
    state.execute(&Command::Service).unwrap();
    let saved = state.checkpoint().unwrap();
    let mut bad = command.clone();
    if let Command::Tick(ref mut tick) = bad {
        tick.al.unqueued = 0;
    }
    assert!(state.execute(&bad).is_err());
    assert_eq!(state.checkpoint().unwrap(), saved);
    let mut interior: Value = serde_json::from_slice(&saved).unwrap();
    interior["state"]["tick"]["progress"][0]["staging"] =
        Value::from(state.tick.progress[0].staging + 1);
    assert!(state.load(&serde_json::to_vec(&interior).unwrap()).is_err());
    assert_eq!(state.checkpoint().unwrap(), saved);
    let mut corrupt: Value = serde_json::from_slice(&saved).unwrap();
    corrupt["state"]["pcm"]["cursor"] = Value::from(data::MAX_BYTES + 1);
    assert!(state.load(&serde_json::to_vec(&corrupt).unwrap()).is_err());
    assert_eq!(state.checkpoint().unwrap(), saved);
}
