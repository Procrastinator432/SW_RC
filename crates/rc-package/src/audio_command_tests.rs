use crate::{
    audio_commands::{Event, Sound, Sources},
    audio_curve::{CurveMath, PortableCurveMath},
    audio_source::Init,
    audio_source_lookup::Query,
};
use serde::de::DeserializeOwned;
use serde_json::{json, Value};
use std::sync::OnceLock;
fn data() -> &'static Value {
    static DATA: OnceLock<Value> = OnceLock::new();
    DATA.get_or_init(|| {
        serde_json::from_str(include_str!("../../../fixtures/audio-commands-native.json")).unwrap()
    })
}
fn read<T: DeserializeOwned>(v: &Value) -> T {
    serde_json::from_value(v.clone()).unwrap()
}
fn n(v: &Value) -> u32 {
    v.as_u64().unwrap() as u32
}
fn execute(s: &mut Sources, c: &Value, bank: &[Sound], replies: &Value) -> (Value, Vec<Event>) {
    let op = c["op"].as_str().unwrap();
    let mut result = Value::Null;
    let events = match op {
        "pause_sound" => s
            .pause_sound(
                n(&c["actor"]),
                n(&c["sound"]),
                c["request"].as_i64().unwrap() as i32,
            )
            .unwrap(),
        "pause_all" => s
            .pause_all(c["request"].as_i64().unwrap() as i32, n(&c["exclude"]))
            .unwrap(),
        "paused" => {
            result = json!(s
                .is_sound_paused(
                    read::<Query>(&c["query"]),
                    &read::<Vec<i32>>(&replies["active"]),
                    &read::<Vec<i32>>(&replies["waiting"])
                )
                .unwrap());
            vec![]
        }
        "stop_sound" => {
            let (found, events) = s
                .stop_sound(
                    n(&c["actor"]),
                    n(&c["sound"]),
                    c["fade"].as_f64().unwrap() as f32,
                    bank,
                    &mut PortableCurveMath,
                )
                .unwrap();
            result = json!(found);
            events
        }
        "stop_all" => s.stop_all(n(&c["exclude"])).unwrap(),
        "stop_voices" => s.stop_voices(n(&c["actor"]), bank).unwrap(),
        "empty" => s.empty_waiting().unwrap(),
        "enqueue" => {
            result = json!(s
                .enqueue(&read::<Init>(&c["input"]), &mut PortableCurveMath)
                .unwrap());
            vec![]
        }
        _ => panic!("{op}"),
    };
    (result, events)
}
fn original(ops: &[&str]) -> usize {
    let mut count = 0;
    for (i, c) in data()["cases"].as_array().unwrap().iter().enumerate() {
        if !ops.contains(&c["command"]["op"].as_str().unwrap()) {
            continue;
        }
        count += 1;
        let mut s: Sources = read(&c["before"]);
        s.validate().unwrap();
        // The production snapshot API is the cold entry point to each command.
        let bytes = s.checkpoint().unwrap();
        s.load(&bytes).unwrap();
        let (result, events) = execute(
            &mut s,
            &c["command"],
            &read::<Vec<Sound>>(&c["bank"]),
            &c["replies"],
        );
        assert_eq!(result, c["result"], "case {i}");
        assert_eq!(json!(events), c["events"], "case {i}");
        assert_eq!(serde_json::to_value(&s).unwrap(), c["expected"], "case {i}");
        s.validate().unwrap();
        let bytes = s.checkpoint().unwrap();
        s.load(&bytes).unwrap();
        assert_eq!(serde_json::to_value(s).unwrap(), c["expected"]);
    }
    count
}
#[test]
fn original_individual_pause_and_driver_pause_query() {
    assert_eq!(original(&["pause_sound", "paused"]), 128);
}
#[test]
fn original_global_pause_masks_resume_and_music_order() {
    assert_eq!(original(&["pause_all"]), 64);
}
#[test]
fn whole_original_stop_sound_defaults_streamed_curves_and_queue() {
    assert_eq!(original(&["stop_sound"]), 64);
}
#[test]
fn original_stop_all_cue_preservation_and_voice_filter() {
    assert_eq!(original(&["stop_all", "stop_voices"]), 128);
}
#[test]
fn original_queue_allocation_counter_and_empty_destructor_order() {
    assert_eq!(original(&["enqueue", "empty"]), 128);
}
#[test]
fn invalid_metadata_math_and_snapshots_do_not_partially_change_sources_or_counter() {
    struct Fail;
    impl CurveMath for Fail {
        fn transform(&mut self, _: u8, _: bool, _: f32) -> Result<f32, String> {
            Err("math".into())
        }
    }
    let c = data()["cases"]
        .as_array()
        .unwrap()
        .iter()
        .find(|c| c["command"]["op"] == "enqueue")
        .unwrap();
    let mut s: Sources = read(&c["before"]);
    let before = s.checkpoint().unwrap();
    let mut input: Init = read(&c["command"]["input"]);
    input.fade_curve = 1;
    assert!(s.enqueue(&input, &mut Fail).is_err());
    assert_eq!(s.checkpoint().unwrap(), before);
    input.engine_present = false;
    assert!(s.enqueue(&input, &mut PortableCurveMath).is_err());
    assert_eq!(s.checkpoint().unwrap(), before);
    assert!(s
        .stop_sound(1, 1, f32::NAN, &[], &mut PortableCurveMath)
        .is_err());
    assert_eq!(s.checkpoint().unwrap(), before);
    // A matching early source can be modified before a later metadata failure.
    if !s.active.is_empty() {
        let actor = 123;
        s.active[0].actor = actor;
        s.active[0].sound = 1;
        let mut extra = s.active[0].clone();
        extra.sound = 2;
        s.active.push(extra);
        let before = s.checkpoint().unwrap();
        let bank = [Sound {
            handle: 1,
            default_fade: 0.,
            streamed: false,
            curve: 0,
            category: 1,
        }];
        assert!(s.stop_voices(actor, &bank).is_err());
        assert_eq!(s.checkpoint().unwrap(), before);
    }
    let before = s.checkpoint().unwrap();
    let mut corrupt: Value = serde_json::from_slice(&before).unwrap();
    corrupt["state"]["counter"] = json!(-1);
    assert!(s.load(&serde_json::to_vec(&corrupt).unwrap()).is_err());
    assert_eq!(s.checkpoint().unwrap(), before);
    corrupt = serde_json::from_slice(&before).unwrap();
    corrupt["schema"] = json!(2);
    assert!(s.load(&serde_json::to_vec(&corrupt).unwrap()).is_err());
    assert_eq!(s.checkpoint().unwrap(), before);
    corrupt = serde_json::from_slice(&before).unwrap();
    corrupt["state"]["unknown"] = json!(true);
    assert!(s.load(&serde_json::to_vec(&corrupt).unwrap()).is_err());
    assert_eq!(s.checkpoint().unwrap(), before);
}
#[test]
fn pause_and_queued_queries_have_distinct_original_vtable_slots() {
    assert_eq!(
        data()["vtable"],
        json!([{"offset":28,"address":0x10358f40u32},{"offset":32,"address":0x10358f50u32}])
    );
    let source: crate::audio_source::Source = read(&data()["cases"][1]["before"]["active"][0]);
    let mut s = Sources {
        active: vec![source],
        waiting: vec![],
        counter: 7,
        cue_id: 0,
    };
    s.active[0].identity = 2;
    s.active[0].actor = 1;
    s.active[0].sound = 2;
    s.pause_sound(1, 2, 7).unwrap();
    assert_eq!(
        s.is_sound_paused(
            Query {
                actor: 1,
                sound: 2,
                auto_actor: false,
                engine_actor: 0
            },
            &[-3],
            &[]
        )
        .unwrap(),
        -3
    );
    let mut runtime = crate::audio_source_runtime::Runtime {
        sources: s.active,
        ducking: read(
            &serde_json::from_str::<Value>(include_str!(
                "../../../fixtures/audio-sources-native.json"
            ))
            .unwrap()["composed"][0]["duck_before"],
        ),
    };
    let mut frame: crate::audio_source_runtime::Frame = read(
        &serde_json::from_str::<Value>(include_str!("../../../fixtures/audio-sources-native.json"))
            .unwrap()["composed"][0]["steps"][1]["frame"],
    );
    frame.base.paused = [true, false];
    frame.queued = false;
    let tick = runtime
        .step(0, 0.1, &[frame], &mut PortableCurveMath)
        .unwrap();
    assert!(!tick.outcomes[0].updated);
    assert!(tick.events.is_empty());
    // Queued sources bypass the zero-fade stop; pause is a different gate.
    let v = crate::world_audio::Volume {
        base_volume: 1.,
        duck: 1.,
        fade: 0.,
        held_fade: 0.,
        mode_before: 2,
        mode_after: 0,
        flags: 0,
        queued: true,
    };
    assert!(!v.update().unwrap().stop);
    assert!(
        crate::world_audio::Volume { queued: false, ..v }
            .update()
            .unwrap()
            .stop
    );
}

#[test]
fn original_mixed_command_sequences_resume_after_every_step_and_keep_counter_after_empty() {
    let mut steps = 0;
    let mut wraps = 0;
    for case in data()["sequences"].as_array().unwrap() {
        let mut s: Sources = read(&case["before"]);
        let bank: Vec<Sound> = read(&case["bank"]);
        for c in case["steps"].as_array().unwrap() {
            let before_counter = s.counter;
            let (result, events) = execute(&mut s, &c["command"], &bank, &c["replies"]);
            assert_eq!(result, c["result"]);
            assert_eq!(json!(events), c["events"]);
            assert_eq!(serde_json::to_value(&s).unwrap(), c["expected"]);
            if c["command"]["op"] == "empty" {
                assert_eq!(s.counter, before_counter);
            }
            if c["command"]["op"] == "enqueue" && s.counter == 0 {
                wraps += 1;
            }
            let bytes = s.checkpoint().unwrap();
            s.load(&bytes).unwrap();
            assert_eq!(serde_json::to_value(&s).unwrap(), c["expected"]);
            steps += 1;
        }
    }
    assert_eq!(steps, 576);
    // Independent original enqueue fixtures include the UINT_MAX -> 0 boundary.
    assert!(data()["cases"]
        .as_array()
        .unwrap()
        .iter()
        .any(|c| c["command"]["op"] == "enqueue"
            && c["before"]["counter"] == u32::MAX
            && c["expected"]["counter"] == 0));
    assert!(wraps > 0);
}
