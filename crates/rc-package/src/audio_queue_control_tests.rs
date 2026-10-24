use crate::{
    audio_activation::Pool,
    audio_buffer_bank::Registration,
    audio_commands as commands,
    audio_curve::PortableCurveMath,
    audio_driver as driver, audio_playback_host as playback,
    audio_playback_host_tests::{settings, wave},
    audio_queue_cycle::*,
    audio_queue_cycle_tests::{init, input, queued, sound},
    audio_reservation as reserve, audio_resource_host,
    audio_source::Source,
    audio_source_lookup::Query,
    audio_upload::EaxCache,
};
use serde_json::{json, Value};
fn config(name: &str) -> reserve::Config {
    reserve::Config {
        limit: 2,
        names: vec![reserve::NamedSound {
            sound: 100,
            name: name.into(),
        }],
    }
}
fn profile() -> Vec<commands::Sound> {
    vec![commands::Sound {
        handle: 100,
        default_fade: 0.2,
        streamed: false,
        curve: 0,
        category: 1,
    }]
}
#[test]
fn original_reservation_slice_matches_reordering_stops_and_full_source_states() {
    let data: Value = serde_json::from_str(include_str!(
        "../../../fixtures/audio-reservation-native.json"
    ))
    .unwrap();
    for (i, c) in data["cases"].as_array().unwrap().iter().enumerate() {
        let entries: Vec<reserve::Entry> = serde_json::from_value(c["entries"].clone()).unwrap();
        let plan = reserve::plan(
            &entries,
            c["channels"].as_u64().unwrap() as usize,
            c["remaining"].as_i64().unwrap() as i32,
            c["limit"].as_i64().unwrap() as i32,
        )
        .unwrap();
        assert_eq!(
            serde_json::to_value(&plan).unwrap(),
            c["expected"],
            "plan {i}"
        );
        let mut sources: Vec<Source> = serde_json::from_value(c["before"].clone()).unwrap();
        for n in plan.stopped {
            sources[n].stop();
        }
        assert_eq!(
            serde_json::to_value(sources).unwrap(),
            c["after"],
            "source {i}"
        );
    }
}
#[test]
fn whole_original_static_commands_match_actual_driver_actions_and_queue() {
    let data: Value = serde_json::from_str(include_str!(
        "../../../fixtures/audio-queue-control-native.json"
    ))
    .unwrap();
    for (index, c) in data["cases"].as_array().unwrap().iter().enumerate() {
        let pool: Pool = serde_json::from_value(c["before"].clone()).unwrap();
        let buffer = c["buffer"].as_u64().unwrap() as u32;
        let sound = driver::Sound {
            handle: c["sound"].as_u64().unwrap() as u32,
            flags: 0,
            duration_bits: 0,
            native_handle: buffer as i32,
        };
        let mut resources = audio_resource_host::Host::new(1).unwrap();
        resources
            .register_pcm(
                &wave(),
                &Registration {
                    sound: sound.handle,
                    handle: buffer,
                    allocation: 300,
                    generated: 101,
                    allow_stereo: false,
                    generation_error: 0,
                    upload_error: 0,
                },
            )
            .unwrap();
        let keys = (1..=pool.nodes.len() as u32).collect::<Vec<_>>();
        for (n, node) in pool.nodes.iter().enumerate() {
            if node.source.allocated {
                resources
                    .attach_static(keys[n], node.clone(), sound.clone())
                    .unwrap();
            }
        }
        let playback = playback::Host::from_prepared(
            resources,
            pool,
            keys,
            settings(),
            EaxCache {
                initialized: false,
                value: 0,
            },
        )
        .unwrap();
        let h = Host::new(playback, 1).unwrap();
        let mut saved: Value = serde_json::from_slice(&h.checkpoint().unwrap()).unwrap();
        saved["waiting"] = json!([{ "source": c["waiting"], "sound": sound }]);
        let mut h = Host::restore(&serde_json::to_vec(&saved).unwrap()).unwrap();
        h.set_cue_id(c["cue_id"].as_u64().unwrap() as u32);
        let command: Command = serde_json::from_value(c["command"].clone()).unwrap();
        let bank = vec![commands::Sound {
            handle: sound.handle,
            default_fade: 0.,
            streamed: false,
            curve: 0,
            category: c["category"].as_u64().unwrap() as u8,
        }];
        let out = h
            .dispatch(&command, &bank, &mut PortableCurveMath)
            .unwrap_or_else(|e| panic!("case {index}: {e}"));
        assert_eq!(
            serde_json::to_value(out.matched).unwrap(),
            c["matched"],
            "matched {index}"
        );
        assert_eq!(
            serde_json::to_value(out.events).unwrap(),
            c["events"],
            "events {index}"
        );
        assert_eq!(
            serde_json::to_value(h.playback().pool()).unwrap(),
            c["after"],
            "pool {index}"
        );
        assert_eq!(
            serde_json::to_value(&h.waiting()[0].source).unwrap(),
            c["waiting_after"],
            "queue {index}"
        );
    }
}
#[test]
fn reservation_cycle_replaces_low_spatial_candidate_with_next_candidate() {
    let mut h = queued(1, &[]);
    h.configure_reservation(config("imp_rock")).unwrap();
    let mut i = init(1.);
    i.flags = 0x40;
    h.enqueue(&i, sound(), &mut PortableCurveMath).unwrap();
    h.enqueue(&init(0.5), sound(), &mut PortableCurveMath)
        .unwrap();
    let events = h.cycle(&input(&h, 0x1012), &mut PortableCurveMath).unwrap();
    assert!(events.contains(&Event::QueuedStop(0)));
    assert!(events.contains(&Event::Attempt {
        queue: 1,
        node: Some(1)
    }));
}
#[test]
fn reservation_names_are_case_sensitive_and_required_only_for_spatial_flag() {
    for (name, stopped) in [
        ("imp_x", true),
        ("IMP_x", false),
        ("exp_GEN_proxMine_01", true),
        ("exp_gen_proxMine_01", false),
    ] {
        let mut h = queued(1, &[]);
        h.configure_reservation(config(name)).unwrap();
        let mut i = init(1.);
        i.flags = 0x40;
        h.enqueue(&i, sound(), &mut PortableCurveMath).unwrap();
        let events = h.cycle(&input(&h, 0x1012), &mut PortableCurveMath).unwrap();
        assert_eq!(events.contains(&Event::QueuedStop(0)), stopped);
    }
    let mut h = queued(1, &[]);
    let before = h.checkpoint().unwrap();
    let mut bad = config("a");
    bad.names.push(bad.names[0].clone());
    assert!(h.configure_reservation(bad).is_err());
    assert_eq!(h.checkpoint().unwrap(), before);
}
#[test]
fn queued_pause_status_uses_base_vtable_and_live_resume_reaches_driver() {
    let mut h = queued(1, &[1.]);
    let query = Query {
        actor: 0,
        sound: 100,
        auto_actor: false,
        engine_actor: 0,
    };
    h.dispatch(
        &Command::PauseAll {
            request: -7,
            exclude: 0,
        },
        &[],
        &mut PortableCurveMath,
    )
    .unwrap();
    assert_eq!(h.waiting()[0].source.requested_pause, -7);
    assert!(h
        .playback()
        .pool()
        .nodes
        .iter()
        .all(|n| n.source.requested_pause == 0));
    assert_eq!(h.is_sound_paused(query).unwrap(), 0);
    h.cycle(&input(&h, 0x1012), &mut PortableCurveMath).unwrap();
    let out = h
        .dispatch(
            &Command::PauseAll {
                request: 0,
                exclude: u32::MAX,
            },
            &[],
            &mut PortableCurveMath,
        )
        .unwrap();
    assert!(out.events.iter().any(|e| matches!(
        e,
        Event::Playback(playback::Event::Driver {
            event: driver::Event::AlPlay { .. },
            ..
        })
    )));
    assert!(h
        .playback()
        .pool()
        .nodes
        .iter()
        .filter(|n| n.source.allocated)
        .all(|n| n.source.requested_pause == 0));
}
#[test]
fn fade_changes_live_controller_then_cycle_executes_real_stop() {
    let mut h = queued(1, &[1.]);
    h.cycle(&input(&h, 0x1012), &mut PortableCurveMath).unwrap();
    h.enqueue(&init(2.), sound(), &mut PortableCurveMath)
        .unwrap();
    let result = h
        .dispatch(
            &Command::StopSound {
                actor: 0,
                sound: 100,
                fade_bits: (-1f32).to_bits(),
            },
            &profile(),
            &mut PortableCurveMath,
        )
        .unwrap();
    assert_eq!(result.matched, Some(true));
    assert!(!h.waiting()[0].source.allocated);
    assert!(!result.events.iter().any(|e| matches!(
        e,
        Event::Playback(playback::Event::Driver {
            event: driver::Event::AlStop { .. },
            ..
        })
    )));
    let mut f = input(&h, 0x1012);
    for a in &mut f.active {
        a.delta_bits = 0.3f32.to_bits();
    }
    let events = h.cycle(&f, &mut PortableCurveMath).unwrap();
    assert!(events.iter().any(|e| matches!(
        e,
        Event::ActiveBase {
            event: playback::Event::Driver {
                event: driver::Event::AlStop { .. },
                ..
            },
            ..
        }
    )));
}
#[test]
fn global_stop_preserves_queue_until_explicit_empty() {
    let mut h = queued(1, &[1.]);
    h.dispatch(
        &Command::StopSound {
            actor: 0,
            sound: 0,
            fade_bits: 0,
        },
        &[],
        &mut PortableCurveMath,
    )
    .unwrap();
    assert!(h.waiting()[0].source.allocated);
    let deleted = h.empty_waiting().unwrap();
    assert_eq!(deleted.len(), 1);
    assert!(h.waiting().is_empty());
}
#[test]
fn cue_identity_preserves_waiting_source_and_refresh_uses_actual_live_cursor() {
    let mut h = Host::new(crate::audio_playback_host_tests::host(1), 100).unwrap();
    h.enqueue(&init(1.), sound(), &mut PortableCurveMath)
        .unwrap();
    let id = h.waiting()[0].source.unique_id;
    h.set_cue_id(id);
    h.set_pending_cues(0x80000000);
    h.dispatch(
        &Command::StopAll { exclude: 0x80 },
        &[],
        &mut PortableCurveMath,
    )
    .unwrap();
    assert!(h.waiting()[0].source.allocated);
    h.refresh_cues().unwrap();
    assert_eq!(h.playback().status().current, 0x80000000);
    h.cycle(&input(&h, 0x1012), &mut PortableCurveMath).unwrap();
    h.dispatch(
        &Command::StopAll { exclude: 0 },
        &[],
        &mut PortableCurveMath,
    )
    .unwrap();
    h.refresh_cues().unwrap();
    assert!(!h.playback().pool().nodes.iter().any(|n| n.source.allocated));
}
#[test]
fn unregister_deletes_pending_and_live_references_before_releasing_pcm() {
    let mut h = queued(1, &[1.]);
    h.configure_reservation(config("imp_x")).unwrap();
    h.cycle(&input(&h, 0x1012), &mut PortableCurveMath).unwrap();
    h.enqueue(&init(2.), sound(), &mut PortableCurveMath)
        .unwrap();
    let events = h.unregister(100).unwrap();
    assert!(matches!(
        events[0],
        Event::Command(commands::Event::Delete { .. })
    ));
    assert!(h.waiting().is_empty());
    assert_eq!(h.playback().resources().counts(), (0, 0, 0));
    h.validate().unwrap();
    h.register_pcm(
        &wave(),
        &Registration {
            sound: 100,
            handle: 200,
            allocation: 300,
            generated: 400,
            allow_stereo: false,
            generation_error: 0,
            upload_error: 0,
        },
    )
    .unwrap();
    assert_eq!(h.playback().resources().counts().0, 1);
}
#[test]
fn commands_config_and_cue_cold_resume_and_old_snapshot_defaults() {
    let mut h = queued(2, &[2., 1.]);
    h.configure_reservation(config("imp_x")).unwrap();
    h.set_cue_id(7);
    let mut cold = Host::restore(&h.checkpoint().unwrap()).unwrap();
    assert_eq!(cold.cue_id(), 7);
    for command in [
        Command::PauseAll {
            request: 1,
            exclude: 0,
        },
        Command::PauseAll {
            request: 0,
            exclude: 0,
        },
        Command::StopAll { exclude: 0x80 },
    ] {
        assert_eq!(
            h.dispatch(&command, &[], &mut PortableCurveMath)
                .unwrap()
                .events,
            cold.dispatch(&command, &[], &mut PortableCurveMath)
                .unwrap()
                .events
        );
        assert_eq!(h.checkpoint().unwrap(), cold.checkpoint().unwrap());
    }
    let mut old: Value = serde_json::from_slice(&h.checkpoint().unwrap()).unwrap();
    old.as_object_mut().unwrap().remove("cue_id");
    old.as_object_mut().unwrap().remove("reservation");
    assert_eq!(
        Host::restore(&serde_json::to_vec(&old).unwrap())
            .unwrap()
            .cue_id(),
        0
    );
}
#[test]
fn invalid_command_or_name_relationship_preserves_full_host() {
    let mut h = queued(1, &[1.]);
    let before = h.checkpoint().unwrap();
    let mut bank = profile();
    bank[0].default_fade = f32::NAN;
    assert!(h
        .dispatch(
            &Command::StopSound {
                actor: 0,
                sound: 100,
                fade_bits: 0
            },
            &bank,
            &mut PortableCurveMath
        )
        .is_err());
    assert_eq!(h.checkpoint().unwrap(), before);
    h.configure_reservation(config("imp_x")).unwrap();
    let mut spatial = init(2.);
    spatial.flags = 0x40;
    h.enqueue(&spatial, sound(), &mut PortableCurveMath)
        .unwrap();
    let before = h.checkpoint().unwrap();
    let mut bad: Value = serde_json::from_slice(&before).unwrap();
    bad["reservation"]["names"] = json!([]);
    assert!(h.load(&serde_json::to_vec(&bad).unwrap()).is_err());
    assert_eq!(h.checkpoint().unwrap(), before);
}

#[test]
fn cue_lookup_preserves_first_active_id_even_when_wrapping_queue_id_collides() {
    let mut h = queued(1, &[0., 1.]);
    let id = h.waiting()[1].source.unique_id;
    assert!(h
        .playback()
        .pool()
        .nodes
        .iter()
        .any(|n| n.source.unique_id == id));
    h.set_cue_id(id);
    h.dispatch(
        &Command::StopAll { exclude: 0x80 },
        &[],
        &mut PortableCurveMath,
    )
    .unwrap();
    assert!(!h.waiting()[1].source.allocated);
}
