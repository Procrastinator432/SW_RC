use crate::{
    audio_activation::Request,
    audio_source::Source,
    audio_upload::{Environment, Event, Session},
};
use serde::{de::DeserializeOwned, Serialize};
use serde_json::{json, Value};
use std::sync::OnceLock;
fn data() -> &'static Value {
    static DATA: OnceLock<Value> = OnceLock::new();
    DATA.get_or_init(|| {
        serde_json::from_str(include_str!("../../../fixtures/audio-upload-native.json")).unwrap()
    })
}
fn read<T: DeserializeOwned>(v: &Value) -> T {
    serde_json::from_value(v.clone()).unwrap()
}
fn same<T: Serialize>(v: &T, w: &Value) {
    assert_eq!(serde_json::to_value(v).unwrap(), *w);
}
fn session(c: &Value) -> Session {
    read(&json!({"pool":c["before"],"buffers":[c["buffer"]],"eax_cache":c["cache"]}))
}
fn run(c: &Value) -> (Session, Vec<Event>) {
    let mut s = session(c);
    let p = s
        .play(
            &read::<Source>(&c["incoming"]),
            read::<Request>(&c["request"]),
            &read::<Environment>(&c["environment"]),
        )
        .unwrap();
    same(&p.prepared, &c["prepared"]);
    same(&p.prefix, &c["prefix"]);
    (s, p.events)
}
fn cases() -> &'static [Value] {
    data()["cases"].as_array().unwrap()
}
#[test]
fn native_whole_play_source_prefix_tail_and_saved_state_match() {
    assert_eq!(cases().len(), 512);
    for (i, c) in cases().iter().enumerate() {
        let (s, e) = run(c);
        assert_eq!(serde_json::to_value(&e).unwrap(), c["events"], "case {i}");
        same(&s.pool, &c["expected"]);
        same(&s.buffers[0], &c["buffer_after"]);
        same(&s.eax_cache, &c["cache_after"]);
        let bytes = s.checkpoint().unwrap();
        let mut loaded = session(c);
        loaded.load(&bytes).unwrap();
        assert_eq!(loaded.checkpoint().unwrap(), bytes);
    }
}
fn filtered_native(predicate: impl Fn(&Event) -> bool) -> usize {
    let mut count = 0;
    for c in cases() {
        let (_, events) = run(c);
        let expected: Vec<Event> = read(&c["events"]);
        let ours: Vec<_> = events.iter().filter(|e| predicate(e)).collect();
        let theirs: Vec<_> = expected.iter().filter(|e| predicate(e)).collect();
        assert_eq!(ours, theirs);
        count += ours.len();
    }
    count
}
#[test]
fn native_gain_pitch_loop_rolloff_and_category_uploads() {
    assert!(
        filtered_native(|e| matches!(
            e,
            Event::Params { .. }
                | Event::Category { .. }
                | Event::Float {
                    parameter: 0x100a | 0x1003 | 0x1021,
                    ..
                }
                | Event::Integer {
                    parameter: 0x1007,
                    ..
                }
        )) > 2000
    );
}
#[test]
fn native_positional_editor_relative_and_distance_seed_uploads() {
    assert!(
        filtered_native(|e| matches!(
            e,
            Event::Vector { .. }
                | Event::Listener { .. }
                | Event::Float {
                    parameter: 0x1020,
                    ..
                }
                | Event::Integer {
                    parameter: 0x202,
                    ..
                }
        )) > 1000
    );
    assert!(cases()
        .iter()
        .any(|c| c["environment"]["rsqrt_seed_bits"].is_u64()));
}
#[test]
fn native_eax_versions_radius_log_and_lazy_static_cache() {
    assert!(filtered_native(|e| matches!(e, Event::Eax { .. })) > 400);
    for c in cases() {
        let (mut s, _) = run(c);
        // Reusing the native cache across another call survives a cold save.
        let bytes = s.checkpoint().unwrap();
        s.load(&bytes).unwrap();
        let mut copy = s.clone();
        let input = read::<Source>(&c["incoming"]);
        let req = read::<Request>(&c["request"]);
        let env = read::<Environment>(&c["environment"]);
        let a = s.play(&input, req, &env).unwrap();
        let b = copy.play(&input, req, &env).unwrap();
        assert_eq!(a.events, b.events);
        assert_eq!(s.checkpoint().unwrap(), copy.checkpoint().unwrap());
    }
}
#[test]
fn native_empty_single_multiple_buffers_reset_progress_and_looping() {
    assert!(
        filtered_native(|e| matches!(
            e,
            Event::Queue { .. }
                | Event::Integer {
                    parameter: 0x1009 | 0x1007,
                    ..
                }
        )) > 1200
    );
    for n in 0..5 {
        assert!(cases()
            .iter()
            .any(|c| c["prepared"].is_object()
                && c["buffer"]["buffers"].as_array().unwrap().len() == n));
    }
}
#[test]
fn native_play_then_zero_lifetime_then_boolean_deferred_pause() {
    assert!(
        filtered_native(|e| matches!(
            e,
            Event::Play { .. } | Event::Pause { .. } | Event::Check(_)
        )) > 2500
    );
    for c in cases() {
        let (_, events) = run(c);
        for (i, e) in events.iter().enumerate() {
            if let Event::Pause {
                request,
                lifetime_words,
                ..
            } = e
            {
                assert_eq!(*request, 1);
                assert_eq!(*lifetime_words, [0; 2]);
                assert!(matches!(events[i - 1], Event::Play { .. }));
            }
        }
    }
}
#[test]
fn invalid_late_uploads_and_snapshots_leave_the_entire_session_unchanged() {
    let c = cases()
        .iter()
        .find(|c| c["environment"]["rsqrt_seed_bits"].is_u64())
        .unwrap();
    let mut s = session(c);
    let initial = s.checkpoint().unwrap();
    let input = read::<Source>(&c["incoming"]);
    let req = read::<Request>(&c["request"]);
    let mut env = read::<Environment>(&c["environment"]);
    env.rsqrt_seed_bits = None;
    assert!(s.play(&input, req, &env).is_err());
    assert_eq!(s.checkpoint().unwrap(), initial);
    env.rsqrt_seed_bits = Some(f32::NAN.to_bits());
    assert!(s.play(&input, req, &env).is_err());
    assert_eq!(s.checkpoint().unwrap(), initial);
    let mut corrupt: Value = serde_json::from_slice(&initial).unwrap();
    corrupt["schema"] = json!(2);
    assert!(s.load(&serde_json::to_vec(&corrupt).unwrap()).is_err());
    corrupt = serde_json::from_slice(&initial).unwrap();
    corrupt["state"]["buffers"][0]["buffers"] = json!(null);
    assert!(s.load(&serde_json::to_vec(&corrupt).unwrap()).is_err());
    corrupt = serde_json::from_slice(&initial).unwrap();
    corrupt["state"]["extra"] = json!(true);
    assert!(s.load(&serde_json::to_vec(&corrupt).unwrap()).is_err());
    assert_eq!(s.checkpoint().unwrap(), initial);
    s.buffers.clear();
    let empty = s.checkpoint().unwrap();
    assert!(s
        .play(&input, req, &read::<Environment>(&c["environment"]))
        .is_err());
    assert_eq!(s.checkpoint().unwrap(), empty);
}

#[test]
fn native_eax_integer_boundaries_agree_with_portable_logarithm() {
    let cases = data()["attenuations"].as_array().unwrap();
    assert_eq!(cases.len(), 2048);
    for c in cases {
        assert_eq!(
            crate::audio_upload::attenuation(f32::from_bits(
                c["ratio_bits"].as_u64().unwrap() as u32
            ))
            .unwrap(),
            c["value"].as_i64().unwrap() as i32
        );
    }
    assert!(crate::audio_upload::attenuation(f32::INFINITY).is_err());
    assert!(crate::audio_upload::attenuation(f32::NAN).is_err());
}
