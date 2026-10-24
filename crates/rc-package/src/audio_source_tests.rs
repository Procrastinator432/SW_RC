use crate::{
    audio_curve::{CurveMath, PortableCurveMath},
    audio_source::{Init, Source},
    audio_source_lookup::{self as lookup, Found, Query},
    audio_source_runtime::{Event, Frame, Runtime},
    audio_source_update::{self as base, BaseInput, UpdateHost},
};
use serde::{de::DeserializeOwned, Serialize};
use serde_json::{json, Value};
use std::sync::OnceLock;
fn data() -> &'static Value {
    static DATA: OnceLock<Value> = OnceLock::new();
    DATA.get_or_init(|| {
        serde_json::from_str(include_str!("../../../fixtures/audio-sources-native.json")).unwrap()
    })
}
fn read<T: DeserializeOwned>(v: &Value) -> T {
    serde_json::from_value(v.clone()).unwrap()
}
fn n(v: &Value) -> u32 {
    v.as_u64().unwrap() as u32
}
fn f(v: &Value) -> f32 {
    v.as_f64().unwrap() as f32
}
fn cold<T: Serialize + DeserializeOwned>(v: &T) -> T {
    serde_json::from_slice(&serde_json::to_vec(v).unwrap()).unwrap()
}
fn same(s: &Source, v: &Value) {
    s.validate().unwrap();
    assert_eq!(serde_json::to_value(s).unwrap(), *v);
}
#[test]
fn original_constructor_and_unique_counter_wrap_on_zero_storage() {
    for case in data()["constructors"].as_array().unwrap() {
        let mut counter = n(&case["seed"]);
        let s = Source::new(&mut counter);
        same(&s, &case["expected"]);
        assert_eq!(counter, n(&case["counter"]));
    }
}
#[test]
fn full_original_initialization_caps_fade_flags_and_untouched_fields() {
    for (i, case) in data()["initialization"]
        .as_array()
        .unwrap()
        .iter()
        .enumerate()
    {
        let mut s: Source = read(&case["before"]);
        let input: Init = read(&case["input"]);
        assert_eq!(
            s.initialize(&input, &mut PortableCurveMath).unwrap(),
            input.engine_present,
            "case {i}"
        );
        same(&s, &case["expected"]);
        same(&cold(&s), &case["expected"]);
    }
}
#[test]
fn original_pause_clear_and_base_stop_retain_the_same_fields() {
    for case in data()["lifecycle"].as_array().unwrap() {
        let mut s: Source = read(&case["before"]);
        for step in case["steps"].as_array().unwrap() {
            match step["op"].as_str().unwrap() {
                "pause" => assert_eq!(
                    s.pause(step["request"].as_i64().unwrap() as i32),
                    n(&step["result"]) != 0
                ),
                "stop" => assert_eq!(s.stop(), n(&step["result"]) != 0),
                "clear" => s.clear(),
                _ => panic!(),
            }
            same(&s, &step["expected"]);
            s = cold(&s);
        }
    }
}
#[derive(Default)]
struct Host {
    calls: Vec<&'static str>,
    clear: bool,
    fail_radii: bool,
}
impl UpdateHost for Host {
    fn volume(&mut self, s: &mut Source, _: f32) -> Result<(), String> {
        self.calls.push("volume");
        if self.clear {
            s.clear();
        }
        Ok(())
    }
    fn radii(&mut self, _: &mut Source, _: f32) -> Result<(), String> {
        self.calls.push("radii");
        if self.fail_radii {
            Err("driver".into())
        } else {
            Ok(())
        }
    }
    fn priority(&mut self, _: &mut Source, _: f32) -> Result<(), String> {
        self.calls.push("priority");
        Ok(())
    }
}
#[test]
fn original_update_base_gates_motion_counter_and_callback_order() {
    for (i, case) in data()["base_updates"]
        .as_array()
        .unwrap()
        .iter()
        .enumerate()
    {
        let mut s: Source = read(&case["before"]);
        let input: BaseInput = read(&case["input"]);
        let mut h = Host {
            clear: case["clear_on_volume"].as_bool().unwrap(),
            ..Host::default()
        };
        let out = base::update(&mut s, &input, f(&case["delta"]), &mut h).unwrap();
        assert_eq!(out.updated, case["updated"].as_bool().unwrap(), "{i}");
        assert_eq!(
            out.counter_incremented,
            case["counter_incremented"].as_bool().unwrap(),
            "{i}"
        );
        assert_eq!(json!(h.calls), case["calls"], "{i}");
        same(&s, &case["expected"]);
    }
}
#[test]
fn original_active_queued_lookup_auto_actor_and_exact_ids() {
    for case in data()["lookups"].as_array().unwrap() {
        let a: Vec<Source> = read(&case["active"]);
        let q: Vec<Source> = read(&case["queued"]);
        let query: Query = read(&case["query"]);
        assert_eq!(
            lookup::index(&a, query).unwrap(),
            read::<Option<usize>>(&case["index"])
        );
        assert_eq!(
            lookup::queued(&q, query.actor, query.sound).unwrap(),
            read::<Option<usize>>(&case["queued_index"])
        );
        assert_eq!(
            lookup::source(&a, &q, query).unwrap(),
            read::<Option<Found>>(&case["source"])
        );
        assert_eq!(
            lookup::by_id(&a, &q, n(&case["id"])).unwrap(),
            read::<Option<Found>>(&case["by_id"])
        );
        let m: Vec<_> = a
            .iter()
            .chain(&q)
            .map(|s| s.matches(query.actor, query.sound))
            .collect();
        assert_eq!(json!(m), case["matches"]);
        let ids: Vec<_> = a.iter().chain(&q).map(|s| s.unique_id).collect();
        assert_eq!(json!(ids), case["generated_ids"]);
    }
    let null = Query {
        actor: 0,
        sound: 0,
        auto_actor: false,
        engine_actor: 0,
    };
    assert!(lookup::index(&[], null).is_err());
    assert_eq!(lookup::queued(&[], 0, 0).unwrap(), None);
}
fn event_names(events: &[Event]) -> Vec<&'static str> {
    events
        .iter()
        .map(|e| match e {
            Event::Volume(_) => "volume",
            Event::Stop(_) => "stop",
            Event::Radii(_) => "radii",
            Event::Priority(_) => "priority",
        })
        .collect()
}
#[test]
fn full_original_source_tick_composes_real_duck_fade_radii_priority_and_clear() {
    for (i, case) in data()["composed"].as_array().unwrap().iter().enumerate() {
        let mut r = Runtime {
            sources: vec![read(&case["before"])],
            ducking: read(&case["duck_before"]),
        };
        for (tick, step) in case["steps"].as_array().unwrap().iter().enumerate() {
            if !step["begin"].is_null() {
                let b = &step["begin"];
                r.sources[0]
                    .fade
                    .begin(
                        n(&b["mode"]) as u8,
                        f(&b["duration"]),
                        n(&b["kind"]) as u8,
                        &mut PortableCurveMath,
                    )
                    .unwrap();
            }
            let frame: Frame = read(&step["frame"]);
            let result = r
                .step(
                    step["voices"].as_i64().unwrap() as i32,
                    f(&step["delta"]),
                    &[frame],
                    &mut PortableCurveMath,
                )
                .unwrap();
            assert_eq!(
                json!(event_names(&result.events)),
                step["events"],
                "{i}/{tick}"
            );
            assert_eq!(
                result.outcomes[0].updated,
                step["updated"].as_bool().unwrap(),
                "{i}/{tick}"
            );
            assert_eq!(
                result.outcomes[0].counter_incremented,
                step["counter_incremented"].as_bool().unwrap(),
                "{i}/{tick}"
            );
            same(&r.sources[0], &step["expected"]);
            assert_eq!(
                serde_json::to_value(&r.ducking).unwrap(),
                step["ducking"],
                "{i}/{tick}"
            );
            r = Runtime::restore(&r.checkpoint().unwrap()).unwrap();
            r.validate().unwrap();
        }
    }
}
#[test]
fn runtime_admission_uses_original_pool_fixtures_without_external_effects() {
    let cases: Value = serde_json::from_str(include_str!(
        "../../../fixtures/audio-controllers-native.json"
    ))
    .unwrap();
    for c in cases["pools"].as_array().unwrap() {
        let mut counter = 100;
        let sources = c["slots"]
            .as_array()
            .unwrap()
            .iter()
            .map(|p| {
                let mut s = Source::new(&mut counter);
                s.allocated = p["occupied"].as_bool().unwrap();
                s.identity = n(&p["identity"]);
                s.priority_bits = f(&p["priority"]).to_bits();
                s
            })
            .collect();
        let mut runtime = Runtime {
            sources,
            ducking: read(&data()["composed"][0]["duck_before"]),
        };
        let mut incoming = Source::new(&mut counter);
        incoming.allocated = true;
        incoming.identity = n(&c["identity"]);
        incoming.priority_bits = f(&c["priority"]).to_bits();
        let before = serde_json::to_value(&runtime).unwrap();
        let chosen = runtime.admit(incoming.clone()).unwrap();
        let expected = c["calls"]
            .as_array()
            .unwrap()
            .first()
            .map(|call| call[1].as_u64().unwrap() as usize);
        assert_eq!(chosen, expected);
        assert_eq!(chosen.is_some(), c["accepted"].as_bool().unwrap());
        if let Some(i) = chosen {
            same(
                &runtime.sources[i],
                &serde_json::to_value(incoming).unwrap(),
            );
            for j in 0..runtime.sources.len() {
                if i != j {
                    same(&runtime.sources[j], &before["sources"][j]);
                }
            }
        } else {
            assert_eq!(serde_json::to_value(runtime).unwrap(), before);
        }
    }
}
#[test]
fn staged_runtime_errors_restore_all_sources_and_ducking_but_raw_callbacks_are_ordered() {
    struct Fail;
    impl CurveMath for Fail {
        fn transform(&mut self, _: u8, _: bool, _: f32) -> Result<f32, String> {
            Err("math failure".into())
        }
    }
    let c = &data()["composed"][0];
    let mut source: Source = read(&c["before"]);
    source.flags = 0x100;
    source.identity = 0x100;
    source
        .fade
        .begin(1, 0.3, 0, &mut PortableCurveMath)
        .unwrap();
    let mut second = source.clone();
    second
        .fade
        .begin(1, 0.3, 1, &mut PortableCurveMath)
        .unwrap();
    let mut r = Runtime {
        sources: vec![source.clone(), second],
        ducking: read(&c["duck_before"]),
    };
    let before = serde_json::to_vec(&r).unwrap();
    let mut frame: Frame = read(&c["steps"][1]["frame"]);
    frame.base.paused = [false; 2];
    assert!(r
        .step(2, 0.01, &[frame.clone(), frame.clone()], &mut Fail)
        .is_err());
    assert_eq!(serde_json::to_vec(&r).unwrap(), before);
    let mut bad = frame.clone();
    bad.now = f32::NAN;
    assert!(r
        .step(2, 0.01, &[frame.clone(), bad], &mut PortableCurveMath)
        .is_err());
    assert_eq!(serde_json::to_vec(&r).unwrap(), before);
    assert!(r.step(2, 0.01, &[], &mut PortableCurveMath).is_err());
    assert_eq!(serde_json::to_vec(&r).unwrap(), before);
    let mut input: Init = read(&data()["initialization"][1]["input"]);
    input.radius = f32::NAN;
    let before = serde_json::to_vec(&source).unwrap();
    assert!(source.initialize(&input, &mut PortableCurveMath).is_err());
    assert_eq!(serde_json::to_vec(&source).unwrap(), before);
    let mut invalid = source.clone();
    invalid.radii.radius_bits[0] = f32::INFINITY.to_bits();
    assert!(invalid.validate().is_err());
    let mut host = Host {
        clear: true,
        fail_radii: true,
        ..Host::default()
    };
    assert!(base::update(&mut source, &frame.base, 0.01, &mut host).is_err());
    assert_eq!(host.calls, vec!["volume", "radii"]);
    assert!(!source.allocated);
}

#[test]
fn core_checkpoint_rejects_corrupt_sources_unknown_fields_and_oversized_pools_atomically() {
    let c = &data()["composed"][0];
    let mut r = Runtime {
        sources: vec![read(&c["before"])],
        ducking: read(&c["duck_before"]),
    };
    let before = r.checkpoint().unwrap();
    let mut broken: Value = serde_json::from_slice(&before).unwrap();
    broken["state"]["sources"][0]["fade"]["held_bits"] = json!(f32::NAN.to_bits());
    assert!(r.load(&serde_json::to_vec(&broken).unwrap()).is_err());
    assert_eq!(r.checkpoint().unwrap(), before);
    let mut unknown: Value = serde_json::from_slice(&before).unwrap();
    unknown["future_driver"] = json!(1);
    assert!(r.load(&serde_json::to_vec(&unknown).unwrap()).is_err());
    assert_eq!(r.checkpoint().unwrap(), before);
    let mut oversized = r.clone();
    oversized.sources = vec![r.sources[0].clone(); 4097];
    assert!(r
        .load(&serde_json::to_vec(&json!({"schema":1,"state":oversized})).unwrap())
        .is_err());
    assert_eq!(r.checkpoint().unwrap(), before);
    assert!(r.load(b"{bad").is_err());
    assert_eq!(r.checkpoint().unwrap(), before);
    let mut future: Value = serde_json::from_slice(&before).unwrap();
    future["schema"] = json!(2);
    assert!(r.load(&serde_json::to_vec(&future).unwrap()).is_err());
    assert_eq!(r.checkpoint().unwrap(), before);
    let restored = Runtime::restore(&before).unwrap();
    assert_eq!(restored.checkpoint().unwrap(), before);
}
