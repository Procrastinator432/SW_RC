use crate::{
    audio_activation::{self, Event as ActivationEvent, Pool, Prepared, Request},
    audio_census::{Frame, Params, Status},
    audio_handoff::Handoff,
    audio_source::Source,
};
use serde::{de::DeserializeOwned, Serialize};
use serde_json::{json, Value};
use std::sync::OnceLock;
fn data() -> &'static Value {
    static DATA: OnceLock<Value> = OnceLock::new();
    DATA.get_or_init(|| {
        serde_json::from_str(include_str!(
            "../../../fixtures/audio-activation-native.json"
        ))
        .unwrap()
    })
}
fn read<T: DeserializeOwned>(v: &Value) -> T {
    serde_json::from_value(v.clone()).unwrap()
}
fn n(v: &Value) -> usize {
    v.as_u64().unwrap() as usize
}
fn cold<T: Serialize + DeserializeOwned>(v: &T) -> T {
    serde_json::from_slice(&serde_json::to_vec(v).unwrap()).unwrap()
}
fn same<T: Serialize>(v: &T, expected: &Value) {
    assert_eq!(serde_json::to_value(v).unwrap(), *expected);
}
#[test]
fn native_base_assignment_and_copy_constructor_preserve_physical_resources_and_counter() {
    assert_eq!(data()["copies"].as_array().unwrap().len(), 128);
    for c in data()["copies"].as_array().unwrap() {
        let mut pool: Pool = read(&c["before"]);
        let input = pool.nodes[n(&c["source"])].source.clone();
        audio_activation::copy_source(&mut pool.nodes[n(&c["destination"])].source, &input)
            .unwrap();
        same(&pool, &c["expected"]);
        assert_eq!(c["counter_before"], c["counter_after"]);
        pool = cold(&pool);
        pool.validate().unwrap();
        same(&pool, &c["expected"]);
    }
}
#[test]
fn native_transient_swaps_first_match_then_stops_supplied_node_even_outside_pool() {
    assert_eq!(data()["swaps"].as_array().unwrap().len(), 128);
    for c in data()["swaps"].as_array().unwrap() {
        let mut pool: Pool = read(&c["before"]);
        let events = pool.set_transient(n(&c["node"])).unwrap();
        same(&events, &c["events"]);
        same(&pool, &c["expected"]);
        same(&pool.transient, &c["getter"]);
    }
}
#[test]
fn native_channel_availability_uses_finished_reply_not_allocation_or_priority() {
    for c in data()["available"].as_array().unwrap() {
        let pool: Pool = read(&c["before"]);
        let bytes = serde_json::to_vec(&pool).unwrap();
        same(
            &pool.available(&read::<Vec<i32>>(&c["finished"])).unwrap(),
            &c["expected"],
        );
        assert_eq!(serde_json::to_vec(&pool).unwrap(), bytes);
    }
}
#[test]
fn native_play_preparation_gates_copy_buffer_admission_double_stop_and_clear() {
    let mut double_stops = 0;
    let mut ready = 0;
    let mut failures = 0;
    for c in data()["preparations"].as_array().unwrap() {
        let mut pool: Pool = read(&c["before"]);
        let (prepared, events) = pool
            .prepare(
                &read::<Source>(&c["incoming"]),
                read::<Request>(&c["request"]),
            )
            .unwrap();
        same(&prepared, &c["prepared"]);
        same(&events, &c["events"]);
        same(&pool, &c["expected"]);
        double_stops+=events.windows(2).filter(|w|matches!((&w[0],&w[1]),(ActivationEvent::Stop(a),ActivationEvent::Stop(b)) if a==b)).count();
        ready += usize::from(prepared.is_some());
        failures += usize::from(
            events
                .iter()
                .any(|e| matches!(e, ActivationEvent::Clear(_))),
        );
    }
    assert!(double_stops > 0 && ready > 0 && failures > 0);
}
#[test]
fn native_standalone_cue_refresh_keeps_pending_bits_and_two_playing_replies() {
    for c in data()["refresh"].as_array().unwrap() {
        let pool: Pool = read(&c["before"]);
        let sources: Vec<_> = pool
            .slots
            .iter()
            .map(|i| pool.nodes[i.unwrap()].source.clone())
            .collect();
        let mut s = Status {
            pending: c["pending"].as_u64().unwrap() as u32,
            current: 17,
            ducking_voices: 7,
            any_playing: true,
        };
        let events = s
            .refresh(&sources, &read::<Vec<[i32; 2]>>(&c["replies"]))
            .unwrap();
        same(&events, &c["events"]);
        same(&s.current, &c["current"]);
        assert_eq!(s.ducking_voices, 7);
        assert!(s.any_playing);
    }
}
#[test]
fn native_update_census_reads_post_update_cursor_flag_and_sound_params() {
    for c in data()["censuses"].as_array().unwrap() {
        let mut pool: Pool = read(&c["before"]);
        let mut sources: Vec<_> = pool
            .slots
            .iter()
            .map(|i| pool.nodes[i.unwrap()].source.clone())
            .collect();
        let mut s = Status {
            pending: c["status"]["pending"].as_u64().unwrap() as u32,
            current: 99,
            ducking_voices: 99,
            any_playing: false,
        };
        let events = s
            .recount(
                &mut sources,
                &read::<Vec<Frame>>(&c["frames"]),
                &read::<Vec<Params>>(&c["params"]),
            )
            .unwrap();
        for (slot, source) in pool.slots.iter().zip(sources) {
            pool.nodes[slot.unwrap()].source = source;
        }
        same(&events, &c["events"]);
        same(&s, &c["status"]);
        same(&pool, &c["expected"]);
    }
}
#[test]
fn native_composed_handoff_cycles_persist_graph_and_delete_queue_after_census() {
    for c in data()["cycles"].as_array().unwrap() {
        let mut h: Handoff = read(&c["before"]);
        let before = h.checkpoint().unwrap();
        h.load(&before).unwrap();
        let result = h
            .cycle(
                &read::<Vec<usize>>(&c["order"]),
                &read::<Vec<Request>>(&c["requests"]),
                &read::<Vec<Frame>>(&c["frames"]),
                &read::<Vec<Params>>(&c["params"]),
            )
            .unwrap();
        let expected: Vec<Option<Prepared>> = read(&c["prepared"]);
        assert_eq!(result.prepared, expected);
        same(&result.events, &c["events"]);
        same(&h, &c["expected"]);
        let bytes = h.checkpoint().unwrap();
        h.load(&bytes).unwrap();
        assert_eq!(h.checkpoint().unwrap(), bytes);
        same(&h, &c["expected"]);
    }
}
#[test]
fn graph_snapshot_and_late_census_errors_are_atomic() {
    let c = data()["cycles"]
        .as_array()
        .unwrap()
        .iter()
        .find(|c| c["before"]["pool"]["slots"].as_array().unwrap().len() > 1)
        .unwrap();
    let mut h: Handoff = read(&c["before"]);
    let before = h.checkpoint().unwrap();
    let order: Vec<usize> = read(&c["order"]);
    let requests: Vec<Request> = read(&c["requests"]);
    let mut frames: Vec<Frame> = read(&c["frames"]);
    frames.last_mut().unwrap().playing = 1;
    frames.last_mut().unwrap().after.sound = 0xffffffff;
    assert!(h
        .cycle(
            &order,
            &requests,
            &frames,
            &read::<Vec<Params>>(&c["params"])
        )
        .is_err());
    assert_eq!(h.checkpoint().unwrap(), before);
    let wrong = vec![0; order.len()];
    assert!(h.cycle(&wrong, &requests, &frames, &[]).is_err());
    assert_eq!(h.checkpoint().unwrap(), before);
    let mut broken: Value = serde_json::from_slice(&before).unwrap();
    broken["state"]["pool"]["transient"] = json!(99999);
    assert!(h.load(&serde_json::to_vec(&broken).unwrap()).is_err());
    assert_eq!(h.checkpoint().unwrap(), before);
    broken = serde_json::from_slice(&before).unwrap();
    broken["schema"] = json!(2);
    assert!(h.load(&serde_json::to_vec(&broken).unwrap()).is_err());
    assert_eq!(h.checkpoint().unwrap(), before);
    broken = serde_json::from_slice(&before).unwrap();
    broken["state"]["unrecognized_driver"] = json!(true);
    assert!(h.load(&serde_json::to_vec(&broken).unwrap()).is_err());
    assert_eq!(h.checkpoint().unwrap(), before);

    let mut alias: Value = serde_json::from_slice(&before).unwrap();
    alias["state"]["pool"]["transient"] = alias["state"]["pool"]["slots"][0].clone();
    assert!(h.load(&serde_json::to_vec(&alias).unwrap()).is_err());
    assert_eq!(h.checkpoint().unwrap(), before);
    let mut pool = h.pool.clone();
    pool.slots[0] = None;
    let old = serde_json::to_vec(&pool).unwrap();
    let source = h.waiting[0].clone();
    let request = Request {
        engine_present: true,
        sound_valid: true,
        buffer: 1,
        queued: 0,
    };
    assert!(pool.prepare(&source, request).is_err());
    assert_eq!(serde_json::to_vec(&pool).unwrap(), old);
}
