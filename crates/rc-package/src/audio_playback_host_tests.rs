use crate::{
    audio_activation as activation, audio_buffer_bank as pcm, audio_census as census,
    audio_curve::PortableCurveMath, audio_driver as driver, audio_duck::Settings,
    audio_playback_host::*, audio_resource_host, audio_source::Source,
    audio_source_runtime as runtime, audio_source_update::BaseInput, audio_upload as upload,
};
use serde_json::{json, Value};
use std::sync::OnceLock;
fn native() -> &'static Value {
    static DATA: OnceLock<Value> = OnceLock::new();
    DATA.get_or_init(|| {
        serde_json::from_str(include_str!("../../../fixtures/audio-playback-native.json")).unwrap()
    })
}
pub(super) fn wave() -> Vec<u8> {
    let mut b=b"RIFF\x26\0\0\0WAVEfmt \x10\0\0\0\x01\0\x01\0\x40\x1f\0\0\x80\x3e\0\0\x02\0\x10\0data\x02\0\0\0".to_vec();
    b.extend([1, 2]);
    b
}
pub(super) fn settings() -> Settings {
    Settings {
        attack: 0.2,
        release: 0.3,
        levels: [0.2, 0.4, 0.6],
    }
}
fn registration() -> pcm::Registration {
    pcm::Registration {
        sound: 100,
        handle: 200,
        allocation: 300,
        generated: 400,
        allow_stereo: false,
        generation_error: 0,
        upload_error: 0,
    }
}
pub(super) fn env() -> upload::Environment {
    upload::Environment {
        category: 0,
        category_gain_bits: 1f32.to_bits(),
        editor: false,
        eax_enabled: false,
        eax_version: 3,
        listener_bits: [0; 3],
        rsqrt_seed_bits: None,
    }
}
fn request(identity: u32) -> Request {
    let t: Value = serde_json::from_str(include_str!(
        "../../../fixtures/audio-vorbis/host-tick.json"
    ))
    .unwrap();
    let mut s: Source = serde_json::from_value(t["driver"]["node"]["source"].clone()).unwrap();
    s.sound = 100;
    s.original_sound = 100;
    s.identity = identity;
    s.flags = 0x100;
    s.priority_bits = 1f32.to_bits();
    s.volume_bits = 0.75f32.to_bits();
    s.output_volume_bits = 0.75f32.to_bits();
    s.requested_pause = 0;
    s.cursor = 0;
    Request {
        incoming: s,
        sound: driver::Sound {
            handle: 100,
            flags: 0,
            duration_bits: 0,
            native_handle: 200,
        },
        engine_present: true,
        sound_valid: true,
        queued: 0,
        environment: env(),
    }
}
pub(super) fn host(slots: usize) -> Host {
    let mut h = Host::new(
        1,
        &(0..slots + 1).map(|i| 10 + i as u32).collect::<Vec<_>>(),
        (0..slots + 1).map(|i| 1 + i as u32).collect(),
        settings(),
    )
    .unwrap();
    h.register_pcm(&wave(), &registration()).unwrap();
    h
}
pub(super) fn frame() -> runtime::Frame {
    runtime::Frame {
        base: BaseInput {
            paused: [false; 2],
            actor_valid: true,
            engine_present: true,
            listener_velocity: [0.; 3],
            actor_location: [0.; 3],
            actor_velocity: [0.; 3],
        },
        now: 1.,
        occlusion: None,
        listener_location: [0.; 3],
        radius_multiplier: 1.,
        rsqrt_seed_bits: None,
        category: 0,
        special_level: false,
        ducker: false,
        queued: false,
    }
}
pub(super) fn driver_frame() -> DriverFrame {
    DriverFrame {
        environment: env(),
        delta_bits: 0.1f32.to_bits(),
    }
}
#[test]
fn whole_original_play_source_matches_actual_driver_composition() {
    for (index, c) in native()["cases"].as_array().unwrap().iter().enumerate() {
        let pool: activation::Pool = serde_json::from_value(c["before"].clone()).unwrap();
        let incoming: Source = serde_json::from_value(c["incoming"].clone()).unwrap();
        let handle = c["request"]["buffer"].as_u64().unwrap() as u32;
        let sound = driver::Sound {
            handle: incoming.sound,
            flags: 0,
            duration_bits: 0,
            native_handle: handle as i32,
        };
        let mut resources = audio_resource_host::Host::new(1).unwrap();
        resources
            .register_pcm(
                &wave(),
                &pcm::Registration {
                    sound: incoming.sound,
                    handle,
                    allocation: 300,
                    generated: 101,
                    allow_stereo: false,
                    generation_error: 0,
                    upload_error: 0,
                },
            )
            .unwrap();
        let keys = (1..=pool.nodes.len() as u32).collect::<Vec<_>>();
        for (i, n) in pool.nodes.iter().enumerate() {
            if n.source.allocated {
                resources
                    .attach_static(keys[i], n.clone(), sound.clone())
                    .unwrap();
            }
        }
        let mut h = Host::from_prepared(
            resources,
            pool,
            keys,
            settings(),
            serde_json::from_value(c["cache"].clone()).unwrap(),
        )
        .unwrap();
        let r = Request {
            incoming,
            sound,
            engine_present: c["request"]["engine_present"].as_bool().unwrap(),
            sound_valid: c["request"]["sound_valid"].as_bool().unwrap(),
            queued: c["request"]["queued"].as_i64().unwrap() as i32,
            environment: serde_json::from_value(c["environment"].clone()).unwrap(),
        };
        let out = h.play(&r).unwrap_or_else(|e| panic!("case {index}: {e}"));
        assert_eq!(
            serde_json::to_value(out.events).unwrap(),
            c["events"],
            "events {index}"
        );
        assert_eq!(
            serde_json::to_value(h.pool()).unwrap(),
            c["expected"],
            "pool {index}"
        );
        let saved: Value = serde_json::from_slice(&h.checkpoint().unwrap()).unwrap();
        assert_eq!(saved["upload"]["eax_cache"], c["cache_after"]);
    }
}
#[test]
fn transient_swap_eviction_and_protected_refusal_keep_one_real_driver() {
    let mut h = host(1);
    let a = h.play(&request(101)).unwrap();
    assert_eq!(a.node, Some(1));
    let saved = h.checkpoint().unwrap();
    let refused = h.play(&request(100)).unwrap();
    assert!(refused.node.is_none());
    assert_eq!(h.pool().slots, [Some(1)]);
    assert_eq!(h.resources().counts(), (1, 1, 0));
    let mut restored = Host::restore(&saved).unwrap();
    assert_eq!(refused.events, restored.play(&request(100)).unwrap().events);
    h.unregister(100).unwrap();
    h.register_pcm(&wave(), &registration()).unwrap();
    h.play(&request(100)).unwrap();
    assert!(h.play(&request(102)).unwrap().node.is_none());
    let mut higher = request(102);
    higher.incoming.priority_bits = 2f32.to_bits();
    let out = h.play(&higher).unwrap();
    assert!(out.node.is_some());
    assert_eq!(h.resources().counts(), (1, 1, 0));
    assert_eq!(
        out.events
            .iter()
            .filter(|e| matches!(
                e,
                Event::Driver {
                    event: driver::Event::AlStop { .. },
                    ..
                }
            ))
            .count(),
        1
    );
}
#[test]
fn parameter_upload_drives_play_lifetime_reset_and_queued_pause() {
    let mut h = host(1);
    let mut r = request(100);
    r.queued = 1;
    r.incoming.requested_pause = -7;
    r.incoming.lifetime_words = [0, 3.5f64.to_bits().wrapping_shr(32) as u32];
    let out = h.play(&r).unwrap();
    let node = &h.pool().nodes[out.node.unwrap()];
    assert_eq!(node.source.cursor, 1);
    assert_eq!(node.source.requested_pause, 1);
    assert_eq!(node.source.lifetime_words, [0; 2]);
    assert!(out.events.iter().any(|e| matches!(
        e,
        Event::Driver {
            event: driver::Event::AlPlay {
                pause_before: -7,
                ..
            },
            ..
        }
    )));
    assert!(out.events.iter().any(|e| matches!(
        e,
        Event::Driver {
            event: driver::Event::AlPause { .. },
            ..
        }
    )));
}
#[test]
fn base_uses_owned_pause_and_actor_motion_then_syncs_pool_and_driver() {
    let mut h = host(1);
    let mut r = request(100);
    r.incoming.flags = 0;
    r.incoming.actor = 77;
    r.queued = 1;
    r.incoming.requested_pause = 1;
    let node = h.play(&r).unwrap().node.unwrap();
    let before = h.pool().nodes[node].source.location_bits;
    let mut f = frame();
    f.base.actor_location = [5., 6., 7.];
    f.base.actor_velocity = [8., 9., 10.];
    f.base.listener_velocity = [1., 2., 3.];
    f.listener_location = [5., 6., 7.];
    assert!(h
        .update_base(0.1, &[f.clone()], &mut PortableCurveMath)
        .unwrap()
        .is_empty());
    assert_eq!(h.pool().nodes[node].source.location_bits, before);
    r.queued = 0;
    r.incoming.requested_pause = 0;
    r.incoming.identity = 102;
    r.incoming.priority_bits = 2f32.to_bits();
    let node = h.play(&r).unwrap().node.unwrap();
    h.update_base(0.1, &[f], &mut PortableCurveMath).unwrap();
    assert_eq!(
        h.pool().nodes[node].source.location_bits,
        [5f32, 6., 7.].map(f32::to_bits)
    );
    assert_eq!(
        h.pool().nodes[node].source.velocity_bits,
        [7f32; 3].map(f32::to_bits)
    );
    h.validate().unwrap();
}
#[test]
fn volume_stop_executes_real_driver_before_radii_and_priority() {
    let mut h = host(1);
    let mut r = request(100);
    r.incoming.fade = crate::audio_fade::Fade::new(0.).unwrap();
    let node = h.play(&r).unwrap().node.unwrap();
    let out = h
        .update_base(0.1, &[frame()], &mut PortableCurveMath)
        .unwrap();
    let stop = out
        .iter()
        .position(|e| {
            matches!(
                e,
                Event::Driver {
                    event: driver::Event::AlStop { .. },
                    ..
                }
            )
        })
        .unwrap();
    let radii = out
        .iter()
        .position(|e| matches!(e, Event::Base(runtime::Event::Radii(_))))
        .unwrap();
    assert!(stop < radii);
    assert!(!h.pool().nodes[node].source.allocated);
    assert_eq!(h.pool().nodes[node].buffer, 0);
    assert_eq!(h.resources().counts().0, 1);
    h.late_tick(&[driver_frame()], &[]).unwrap();
    assert!(!h.status().any_playing);
}
#[test]
fn real_driver_tick_supplies_census_clock_and_next_phase_ducking() {
    let mut h = host(1);
    let node = h.play(&request(100)).unwrap().node.unwrap();
    h.set_pending(16);
    h.refresh_cues().unwrap();
    assert_eq!(h.status().current, 17);
    let out = h
        .late_tick(
            &[driver_frame()],
            &[census::Params {
                sound: 100,
                flags: 1,
            }],
        )
        .unwrap();
    assert!(out.iter().any(|e| matches!(e, Event::Tick { .. })));
    assert_eq!(h.status().ducking_voices, 1);
    assert!(h.status().any_playing);
    let life = h.pool().nodes[node].source.lifetime_words;
    assert_eq!(
        u64::from(life[0]) | (u64::from(life[1]) << 32),
        f64::from(0.1f32).to_bits()
    );
    h.update_base(0.05, &[frame()], &mut PortableCurveMath)
        .unwrap();
    assert!(h.ducking().active);
    assert!(f32::from_bits(h.pool().nodes[node].source.output_volume_bits) < 0.75);
}
#[test]
fn late_missing_params_and_late_base_error_roll_back_earlier_driver_work() {
    let mut h = host(2);
    let mut a = request(100);
    a.incoming.fade = crate::audio_fade::Fade::new(0.).unwrap();
    h.play(&a).unwrap();
    let mut b = request(102);
    b.incoming.volume_bits = 32f32.to_bits();
    h.play(&b).unwrap();
    let before = h.checkpoint().unwrap();
    assert!(h
        .update_base(0.1, &[frame(), frame()], &mut PortableCurveMath)
        .is_err());
    assert_eq!(h.checkpoint().unwrap(), before);
    assert!(h.late_tick(&[driver_frame(), driver_frame()], &[]).is_err());
    assert_eq!(h.checkpoint().unwrap(), before);
}
#[test]
fn cold_restore_keeps_pool_upload_ducking_census_and_teardown_identical() {
    let mut h = host(2);
    h.play(&request(100)).unwrap();
    h.play(&request(102)).unwrap();
    let mut cold = Host::restore(&h.checkpoint().unwrap()).unwrap();
    for _ in 0..8 {
        assert_eq!(
            h.update_base(0.05, &[frame(), frame()], &mut PortableCurveMath)
                .unwrap(),
            cold.update_base(0.05, &[frame(), frame()], &mut PortableCurveMath)
                .unwrap()
        );
        let params = [census::Params {
            sound: 100,
            flags: 1,
        }];
        assert_eq!(
            h.late_tick(&[driver_frame(), driver_frame()], &params)
                .unwrap(),
            cold.late_tick(&[driver_frame(), driver_frame()], &params)
                .unwrap()
        );
        let saved = h.checkpoint().unwrap();
        assert_eq!(saved, cold.checkpoint().unwrap());
        cold = Host::restore(&saved).unwrap();
    }
    assert_eq!(h.unregister(100).unwrap(), cold.unregister(100).unwrap());
    assert_eq!(h.checkpoint().unwrap(), cold.checkpoint().unwrap());
    assert_eq!(h.resources().counts(), (0, 0, 0));
}
#[test]
fn corrupt_alias_or_driver_state_snapshot_preserves_live_host() {
    let mut h = host(1);
    h.play(&request(100)).unwrap();
    let saved = h.checkpoint().unwrap();
    let v: Value = serde_json::from_slice(&saved).unwrap();
    for i in 0..4 {
        let mut bad = v.clone();
        match i {
            0 => bad["keys"][1] = bad["keys"][0].clone(),
            1 => bad["upload"]["pool"]["nodes"][1]["source"]["cursor"] = json!(99),
            2 => bad["upload"]["buffers"][0]["buffers"][0] = json!(999),
            _ => bad["schema"] = json!(2),
        }
        assert!(h.load(&serde_json::to_vec(&bad).unwrap()).is_err());
        assert_eq!(h.checkpoint().unwrap(), saved);
    }
}
