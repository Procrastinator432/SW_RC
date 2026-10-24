//! Frozen original outcomes, synthetic stateful replays; no assets or Android.
use crate::{
    audio_curve::{Curve, CurveMath, PortableCurveMath},
    audio_duck::{Ducking, Settings, Source},
    audio_fade::{Fade, Stop, VolumeInput},
    audio_pool::{self, Incoming, PoolHost, Slot},
    audio_priority::RunningPriority,
};
use serde::{de::DeserializeOwned, Serialize};
use serde_json::{json, Value};
use std::sync::OnceLock;
fn data() -> &'static Value {
    static DATA: OnceLock<Value> = OnceLock::new();
    DATA.get_or_init(|| {
        serde_json::from_str(include_str!(
            "../../../fixtures/audio-controllers-native.json"
        ))
        .unwrap()
    })
}
fn f(v: &Value) -> f32 {
    v.as_f64().unwrap() as f32
}
fn n(v: &Value) -> u32 {
    v.as_u64().unwrap() as u32
}
fn flag(v: &Value) -> bool {
    v.as_bool().unwrap()
}
fn restored<T: Serialize + DeserializeOwned>(state: &T) -> T {
    serde_json::from_slice(&serde_json::to_vec(state).unwrap()).unwrap()
}
fn helpers(step: &Value) {
    for r in step["math"].as_array().unwrap() {
        assert_eq!(
            PortableCurveMath
                .transform(
                    n(&r["kind"]) as u8,
                    flag(&r["encode"]),
                    f32::from_bits(n(&r["input_bits"]))
                )
                .unwrap()
                .to_bits(),
            n(&r["output_bits"]),
            "{r}"
        );
    }
}
fn fade_snapshot(s: &Fade) -> Value {
    json!({"curve":s.curve,"duration_bits":s.duration_bits,"elapsed_bits":s.elapsed_bits,"mode":s.mode})
}
#[test]
fn curves_match_all_original_state_words_and_helper_probes() {
    let mut count = 0;
    for case in data()["curves"].as_array().unwrap() {
        let mut curve = Curve::new(f(&case["initial"])).unwrap();
        for step in case["steps"].as_array().unwrap() {
            helpers(step);
            let actual = if step["op"] == "set" {
                curve
                    .set(
                        f(&step["target"]),
                        f(&step["duration"]),
                        n(&step["kind"]) as u8,
                        &mut PortableCurveMath,
                    )
                    .unwrap();
                curve.value(&mut PortableCurveMath).unwrap()
            } else {
                curve
                    .step(f(&step["delta"]), &mut PortableCurveMath)
                    .unwrap()
            };
            assert_eq!(actual.to_bits(), n(&step["value_bits"]), "{step}");
            assert_eq!(
                serde_json::to_value(&curve).unwrap(),
                step["expected"],
                "{step}"
            );
            let before = curve.clone();
            assert_eq!(
                curve.value(&mut PortableCurveMath).unwrap().to_bits(),
                actual.to_bits()
            );
            assert_eq!(curve, before);
            curve = restored(&curve);
            curve.validate().unwrap();
            count += 1;
        }
    }
    assert_eq!(count, 1728);
}
#[test]
fn fades_and_streamed_stop_dispatch_match_original_states() {
    let mut count = 0;
    for case in data()["fades"].as_array().unwrap() {
        let mut fade = Fade::new(f(&case["initial"])).unwrap();
        for step in case["steps"].as_array().unwrap() {
            helpers(step);
            if step["op"] == "begin" {
                fade.begin(
                    n(&step["mode"]) as u8,
                    f(&step["duration"]),
                    n(&step["kind"]) as u8,
                    &mut PortableCurveMath,
                )
                .unwrap();
            } else {
                let result = fade
                    .step(f(&step["delta"]), &mut PortableCurveMath)
                    .unwrap();
                assert_eq!(result.to_bits(), n(&step["value_bits"]), "{step}");
            }
            assert_eq!(fade_snapshot(&fade), step["expected"], "{step}");
            fade = restored(&fade);
            fade.validate().unwrap();
            count += 1;
        }
    }
    assert_eq!(count, 1536);
    for case in data()["stops"].as_array().unwrap() {
        helpers(case);
        let mut fade = Fade::new(f(&case["initial"])).unwrap();
        let stop = fade
            .stop(
                f(&case["duration"]),
                flag(&case["streamed"]),
                n(&case["kind"]) as u8,
                &mut PortableCurveMath,
            )
            .unwrap();
        assert_eq!(stop == Stop::Immediate, flag(&case["stopped"]));
        assert_eq!(fade_snapshot(&fade), case["expected"]);
    }
    assert_eq!(data()["stops"].as_array().unwrap().len(), 72);
}
#[test]
fn duck_controller_and_source_exemptions_match_original() {
    let mut count = 0;
    for case in data()["ducking"].as_array().unwrap() {
        let levels = std::array::from_fn(|i| f(&case["levels"][i]));
        let mut duck = Ducking::new(Settings {
            attack: f(&case["attack"]),
            release: f(&case["release"]),
            levels,
        })
        .unwrap();
        for step in case["steps"].as_array().unwrap() {
            duck.step(step["voices"].as_i64().unwrap() as i32, f(&step["delta"]))
                .unwrap();
            assert_eq!(json!(duck.words), step["words"], "{case}");
            assert_eq!(duck.active, flag(&step["active"]));
            assert_eq!(duck.direction_bits, n(&step["direction_bits"]));
            assert_eq!(duck.remaining_bits, n(&step["remaining_bits"]));
            duck = restored(&duck);
            duck.validate().unwrap();
            count += 1;
        }
    }
    assert_eq!(count, 1152);
    let mut duck = Ducking::new(Settings {
        attack: 0.1,
        release: 0.3,
        levels: [0.2, 0.4, 0.8],
    })
    .unwrap();
    for (i, v) in [0.2f32, 0.4, 0.8].iter().enumerate() {
        duck.words[i * 3] = v.to_bits();
    }
    for case in data()["duck_factors"].as_array().unwrap() {
        let source = Source {
            category: n(&case["category"]) as u8,
            slot: n(&case["slot"]) as u8,
            voice_exempt: flag(&case["voice_exempt"]),
            special_level: flag(&case["special_level"]),
            ducker: flag(&case["ducker"]),
        };
        assert_eq!(
            duck.factor(source).unwrap().to_bits(),
            n(&case["expected_bits"])
        );
    }
}
#[test]
fn real_fade_and_duck_controllers_compose_with_original_update_volume() {
    let mut count = 0;
    for case in data()["volumes"].as_array().unwrap() {
        let mut fade = Fade::new(f(&case["initial"])).unwrap();
        let mut duck = Ducking::new(Settings {
            attack: 0.1,
            release: 0.3,
            levels: [0.2, 0.4, 0.8],
        })
        .unwrap();
        let source = Source {
            category: n(&case["category"]) as u8,
            slot: n(&case["slot"]) as u8,
            voice_exempt: flag(&case["voice_exempt"]),
            special_level: false,
            ducker: false,
        };
        for step in case["steps"].as_array().unwrap() {
            helpers(step);
            if let Some(begin) = step["begin"].as_object() {
                fade.begin(
                    n(&begin["mode"]) as u8,
                    f(&begin["duration"]),
                    n(&begin["kind"]) as u8,
                    &mut PortableCurveMath,
                )
                .unwrap();
            }
            duck.step(step["voices"].as_i64().unwrap() as i32, f(&step["delta"]))
                .unwrap();
            let out = fade
                .update_volume(
                    VolumeInput {
                        base: f(&step["base"]),
                        duck: duck.factor(source).unwrap(),
                        flags: n(&step["flags"]),
                        queued: flag(&step["queued"]),
                        delta: f(&step["delta"]),
                    },
                    &mut PortableCurveMath,
                )
                .unwrap();
            assert_eq!(out.volume.to_bits(), n(&step["volume_bits"]), "{step}");
            assert_eq!(out.stop, flag(&step["stopped"]));
            assert_eq!(out.held_fade.to_bits(), n(&step["held_bits"]));
            assert_eq!(fade_snapshot(&fade), step["expected"]);
            fade = restored(&fade);
            duck = restored(&duck);
            count += 1;
        }
    }
    assert_eq!(count, 768);
}
#[test]
fn running_priority_matches_original_and_keeps_admission_separate() {
    for case in data()["priorities"].as_array().unwrap() {
        let mut input = case.clone();
        for k in ["previous", "seed_bits", "expected_bits"] {
            input.as_object_mut().unwrap().remove(k);
        }
        let priority: RunningPriority = serde_json::from_value(input).unwrap();
        let seed = case["seed_bits"].as_u64().map(|v| f32::from_bits(v as u32));
        assert_eq!(
            priority
                .update(f(&case["previous"]), seed)
                .unwrap()
                .to_bits(),
            n(&case["expected_bits"]),
            "{case}"
        );
    }
    let p = RunningPriority {
        location: [0.; 3],
        listener: [50., 0., 0.],
        volume: 1.,
        radius: 100.,
        radius_multiplier: 1.,
        flags: 0,
        slot: 0,
    };
    let running = p.update_portable(0.).unwrap();
    let admission = crate::world_audio::Priority {
        location: p.location,
        listener: p.listener,
        volume: p.volume,
        radius: p.radius,
        radius_multiplier: p.radius_multiplier,
        flags: p.flags,
        slot: p.slot,
    }
    .admission()
    .unwrap();
    assert_ne!(running, admission);
}
#[derive(Default)]
struct Host {
    calls: Vec<Value>,
    fail_stop: bool,
    fail_attach: bool,
}
impl PoolHost for Host {
    fn stop(&mut self, index: usize) -> Result<(), String> {
        self.calls.push(json!(["stop", index]));
        if self.fail_stop {
            Err("stop".into())
        } else {
            Ok(())
        }
    }
    fn attach(&mut self, index: usize, _: Incoming) -> Result<(), String> {
        self.calls.push(json!(["attach", index]));
        if self.fail_attach {
            Err("attach".into())
        } else {
            Ok(())
        }
    }
}
#[test]
fn source_pool_selection_and_callback_order_match_whole_original_function() {
    for case in data()["pools"].as_array().unwrap() {
        let slots: Vec<Slot> = serde_json::from_value(case["slots"].clone()).unwrap();
        let incoming = Incoming {
            identity: n(&case["identity"]),
            priority: f(&case["priority"]),
        };
        let before = serde_json::to_vec(&slots).unwrap();
        let mut host = Host::default();
        let result = audio_pool::apply(incoming, &slots, &mut host).unwrap();
        assert_eq!(result.is_some(), flag(&case["accepted"]), "{case}");
        assert_eq!(json!(host.calls), case["calls"], "{case}");
        assert_eq!(before, serde_json::to_vec(&slots).unwrap());
    }
    let slots = vec![Slot {
        occupied: false,
        identity: 2,
        priority: 0.,
    }];
    let incoming = Incoming {
        identity: 2,
        priority: 1.,
    };
    let mut host = Host {
        fail_stop: true,
        ..Default::default()
    };
    assert!(audio_pool::apply(incoming, &slots, &mut host).is_err());
    assert_eq!(host.calls, vec![json!(["stop", 0])]);
    let mut host = Host {
        fail_attach: true,
        ..Default::default()
    };
    assert!(audio_pool::apply(incoming, &slots, &mut host).is_err());
    assert_eq!(host.calls, vec![json!(["stop", 0]), json!(["attach", 0])]);
}
#[test]
fn invalid_controller_inputs_are_atomic_and_large_fade_delta_is_original_bypass() {
    struct Fail;
    impl CurveMath for Fail {
        fn transform(&mut self, _: u8, _: bool, _: f32) -> Result<f32, String> {
            Err("math".into())
        }
    }
    let mut curve = Curve::new(0.5).unwrap();
    let before = curve.clone();
    assert!(curve.set(0.8, 0.3, 1, &mut Fail).is_err());
    assert_eq!(curve, before);
    for (value, duration, kind) in [(f32::NAN, 0.3, 0), (0.5, -1., 0), (0.5, 0.3, 4)] {
        assert!(curve
            .set(value, duration, kind, &mut PortableCurveMath)
            .is_err());
        assert_eq!(curve, before);
    }
    let mut corrupt = curve.clone();
    corrupt.words[3] = f32::INFINITY.to_bits();
    assert!(corrupt.validate().is_err());
    let mut fade = Fade::new(1.).unwrap();
    fade.begin(2, 0.3, 1, &mut PortableCurveMath).unwrap();
    let before = fade.clone();
    assert_eq!(fade.step(1., &mut PortableCurveMath).unwrap(), 1.);
    assert_eq!(fade, before);
    assert!(fade.step(-0.1, &mut PortableCurveMath).is_err());
    assert_eq!(fade, before);
    assert!(fade
        .update_volume(
            VolumeInput {
                base: f32::NAN,
                duck: 1.,
                flags: 0,
                queued: false,
                delta: 0.1
            },
            &mut PortableCurveMath
        )
        .is_err());
    assert_eq!(fade, before);
    let mut duck = Ducking::new(Settings {
        attack: 0.1,
        release: 0.3,
        levels: [0.2, 0.4, 0.8],
    })
    .unwrap();
    let before = duck.clone();
    assert!(duck.step(1, f32::INFINITY).is_err());
    assert_eq!(duck, before);
    assert!(Ducking::new(Settings {
        attack: 0.,
        release: 0.3,
        levels: [0.2, 0.4, 0.8]
    })
    .is_err());
    let mut corrupt = duck.clone();
    corrupt.words[0] = f32::NAN.to_bits();
    assert!(corrupt.validate().is_err());
    let p = RunningPriority {
        location: [0.; 3],
        listener: [1., 0., 0.],
        volume: 1.,
        radius: 10.,
        radius_multiplier: 100.,
        flags: 0,
        slot: 0,
    };
    assert!(p.update(0., None).is_err());
    assert!(p.update(0., Some(-1.)).is_err());
    assert!(audio_pool::select(
        Incoming {
            identity: 0,
            priority: f32::NAN
        },
        &[]
    )
    .is_err());
}
