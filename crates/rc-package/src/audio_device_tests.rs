use crate::{
    audio_device::*, audio_device_events as lower, audio_device_model::Model,
    audio_device_session::Session, audio_mixed_cycle as cycle, audio_mixed_cycle_tests as owner,
    audio_mixed_host as mixed, audio_mixed_host_tests as mixed_helpers,
    audio_playback_host_tests::env,
};
use serde_json::{json, Value};
pub(super) fn executor<B: Backend>(backend: B) -> Executor<B> {
    let mut e = Executor::new(backend, 1).unwrap();
    for id in [10, 11, 12] {
        e.create(Kind::Source, id).unwrap();
    }
    for id in [400, 7004, 7005, 7006, 7007] {
        e.create(Kind::Buffer, id).unwrap();
    }
    e.create(Kind::Context, 42).unwrap();
    e
}
pub(super) fn setup<B: Backend>(backend: B) -> Session<B> {
    let mut h = owner::host(2);
    owner::add_static(&mut h, 6, 1.);
    owner::add_stream(&mut h, 1, 2.);
    let mut s = Session::new(h, executor(backend)).unwrap();
    let manifest = s.manifest().unwrap();
    s.restart(&manifest, &vec![env(); 3]).unwrap();
    s
}
#[test]
fn namespace_mapping_is_bijective_and_independent() {
    let mut e = executor(Model::default());
    assert_eq!(
        e.physical(Kind::Source, 10).unwrap(),
        e.physical(Kind::Buffer, 400).unwrap()
    );
    assert!(e.create(Kind::Source, 10).is_err());
    assert!(e.fault().is_none());
    let p = e
        .prepare(&[
            Op::Upload {
                buffer: 400,
                format: 0x1101,
                frequency: 8000,
                bytes: vec![1, 2],
            },
            Op::Integer {
                source: 10,
                parameter: 0x1009,
                value: 400,
            },
            Op::Play(10),
        ])
        .unwrap();
    e.apply(&p).unwrap();
    assert_eq!(e.observe(10, 0x1010).unwrap(), 0x1012);
}
#[test]
fn bad_late_handle_or_capability_is_rejected_before_any_io() {
    let e = executor(Model::new(Capabilities {
        eax: false,
        context: false,
        music: false,
    }));
    assert!(e
        .prepare(&[
            Op::Check("first".into()),
            Op::Queue {
                source: 10,
                buffers: vec![999]
            }
        ])
        .is_err());
    for op in [
        Op::Eax {
            source: 10,
            version: 3,
            property: 11,
            value: 0,
        },
        Op::Process(42),
        Op::MusicStop,
    ] {
        assert!(e.prepare(&[op]).is_err());
    }
    assert!(e.backend().calls().is_empty());
}
#[test]
fn pcm_and_parameter_preflight_reject_nonfinite_misaligned_and_unknown_values() {
    let e = executor(Model::default());
    for op in [
        Op::Upload {
            buffer: 400,
            format: 0x1101,
            frequency: 8000,
            bytes: vec![1],
        },
        Op::Upload {
            buffer: 400,
            format: 0x1101,
            frequency: 0,
            bytes: vec![],
        },
        Op::Float {
            source: 10,
            parameter: 0x100a,
            value: f32::NAN.to_bits(),
        },
        Op::Integer {
            source: 10,
            parameter: 0xdead,
            value: 0,
        },
        Op::Queue {
            source: 10,
            buffers: vec![400, 400],
        },
    ] {
        assert!(e.prepare(&[op]).is_err());
    }
    assert!(e.backend().calls().is_empty());
}
#[test]
fn whole_original_upload_events_lower_to_exactly_one_play_pause_and_binding() {
    let f: Value = serde_json::from_str(include_str!(
        "../../../fixtures/audio-mixed-upload-native.json"
    ))
    .unwrap();
    for c in f["cases"].as_array().unwrap() {
        let count = c["before"]["nodes"].as_array().unwrap().len();
        let mut e = Executor::new(Model::default(), 1).unwrap();
        for n in 0..count {
            e.create(Kind::Source, 1000 + n as u32).unwrap();
        }
        for id in 101..=104 {
            e.create(Kind::Buffer, id).unwrap();
        }
        let events: Vec<cycle::Event> = c["events"]
            .as_array()
            .unwrap()
            .iter()
            .map(|v| cycle::Event::Mixed(serde_json::from_value(v.clone()).unwrap()))
            .collect();
        let lowered = lower::cycle(&events, &[]).unwrap();
        assert_eq!(
            lowered
                .ops
                .iter()
                .filter(|o| matches!(o, Op::Play(_)))
                .count(),
            1
        );
        let native_pause = c["events"]
            .as_array()
            .unwrap()
            .iter()
            .filter(|v| v["Driver"]["event"]["AlPause"].is_object())
            .count();
        assert_eq!(
            lowered
                .ops
                .iter()
                .filter(|o| matches!(o, Op::Pause(_)))
                .count(),
            native_pause
        );
        assert_eq!(
            lowered
                .ops
                .iter()
                .filter(|o| matches!(o, Op::Queue { .. }))
                .count(),
            usize::from(c["streaming"].as_bool().unwrap())
        );
        let mut ops = (101..=104)
            .map(|buffer| Op::Upload {
                buffer,
                format: 0x1101,
                frequency: 8000,
                bytes: vec![0; 4],
            })
            .collect::<Vec<_>>();
        ops.extend(lowered.ops);
        let p = e.prepare(&ops).unwrap();
        e.apply(&p).unwrap();
        let node = c["expected"]["slots"][0].as_u64().unwrap() as usize;
        let hardware = c["expected"]["nodes"][node]["hardware"].as_u64().unwrap() as u32;
        assert_eq!(
            e.observe(hardware, 0x1010).unwrap(),
            if native_pause == 0 { 0x1012 } else { 0x1013 }
        );
    }
}
#[test]
fn cycle_data_binding_and_clear_markers_are_not_executed_twice() {
    let mut s = setup(Model::default());
    let f = owner::frame(s.owner());
    s.cycle(&f, &mut crate::audio_curve::PortableCurveMath)
        .unwrap();
    let calls = s.device().backend().calls();
    assert_eq!(calls.iter().filter(|o| matches!(o, Op::Play(_))).count(), 2);
    assert_eq!(
        calls
            .iter()
            .filter(|o| matches!(o, Op::Queue { .. }))
            .count(),
        1
    );
    assert_eq!(
        calls
            .iter()
            .filter(|o| matches!(o, Op::Upload { .. }))
            .count(),
        5
    );
    assert_eq!(s.owner().mixed().resources().counts(), (1, 1, 1));
}
#[test]
fn missing_or_wrong_read_expectation_cannot_reach_backend() {
    let event = cycle::Event::Mixed(mixed::Event::Driver {
        node: 0,
        event: crate::audio_driver::Event::GetState {
            hardware: 10,
            parameter: 0x1010,
        },
    });
    assert!(lower::cycle(std::slice::from_ref(&event), &[]).is_err());
    assert!(lower::cycle(
        &[event],
        &[lower::Read {
            source: 11,
            parameter: 0x1010,
            value: 0x1012
        }]
    )
    .is_err());
}
#[test]
fn actual_query_mismatch_quarantines_without_accepting_predicted_reply() {
    let mut e = executor(Model::default());
    assert_eq!(e.observe(10, 0x1010).unwrap(), 0x1011);
    let p = e
        .prepare(&[Op::Query {
            source: 10,
            parameter: 0x1010,
            expected: 0x1012,
        }])
        .unwrap();
    let f = e.apply(&p).unwrap_err();
    assert_eq!(f.completed, 0);
    assert!(e.fault().is_some());
    assert!(e.prepare(&[]).is_err());
}
#[test]
fn unqueue_returns_logical_ids_and_checks_set_vs_exact_order() {
    let mut e = executor(Model::default());
    let ids = [7004, 7005, 7006, 7007];
    let mut ops = ids
        .iter()
        .map(|&buffer| Op::Upload {
            buffer,
            format: 0x1101,
            frequency: 8000,
            bytes: vec![0; 4],
        })
        .collect::<Vec<_>>();
    ops.extend([
        Op::Queue {
            source: 10,
            buffers: ids.to_vec(),
        },
        Op::Play(10),
        Op::Stop(10),
        Op::Unqueue {
            source: 10,
            expected: vec![7004],
            exact_order: true,
        },
        Op::Queue {
            source: 10,
            buffers: vec![7004],
        },
        Op::Stop(10),
        Op::Unqueue {
            source: 10,
            expected: ids.to_vec(),
            exact_order: false,
        },
    ]);
    let p = e.prepare(&ops).unwrap();
    let out = e.apply(&p).unwrap();
    assert_eq!(
        out.replies.last(),
        Some(&Reply::Buffers(vec![7005, 7006, 7007, 7004]))
    );
}
#[test]
fn partial_failure_records_prefix_and_prevents_replay_until_reset() {
    let mut model = Model::default();
    model.fail_on_attempt(1);
    let mut e = executor(model);
    let p = e
        .prepare(&[
            Op::Check("one".into()),
            Op::Check("two".into()),
            Op::Check("three".into()),
        ])
        .unwrap();
    let f = e.apply(&p).unwrap_err();
    assert_eq!((f.failed_index, f.completed), (1, 1));
    let calls = e.backend().calls().len();
    assert!(e.apply(&p).is_err());
    assert_eq!(e.backend().calls().len(), calls);
    let manifest = e.checkpoint_manifest().unwrap();
    e.restart(&manifest).unwrap();
    assert!(e.fault().is_none());
    assert!(e.apply(&p).is_err());
}
#[test]
fn stale_foreign_or_already_used_program_never_reaches_backend() {
    let mut a = executor(Model::default());
    let mut b = executor(Model::default());
    let p = a.prepare(&[Op::Check("once".into())]).unwrap();
    assert!(b.apply(&p).is_err());
    assert!(b.backend().calls().is_empty());
    a.apply(&p).unwrap();
    let calls = a.backend().calls().len();
    assert!(a.apply(&p).is_err());
    assert_eq!(a.backend().calls().len(), calls);
}
#[test]
fn manifest_contains_only_logical_requirements_and_bad_manifest_preserves_session() {
    let mut e = executor(Model::default());
    let bytes = e.checkpoint_manifest().unwrap();
    let mut v: Value = serde_json::from_slice(&bytes).unwrap();
    assert_eq!(v.as_object().unwrap().len(), 3);
    let old = e.physical(Kind::Source, 10).unwrap();
    v["objects"] = json!([["Source", 10], ["Source", 10]]);
    assert!(e.restart(&serde_json::to_vec(&v).unwrap()).is_err());
    assert_eq!(e.epoch(), 1);
    assert!(e.fault().is_none());
    e.restart(&bytes).unwrap();
    assert_eq!(e.epoch(), 2);
    assert_ne!(e.physical(Kind::Source, 10).unwrap(), old);
}
struct FailAfterPlay {
    model: Model,
    once: bool,
}
impl Backend for FailAfterPlay {
    fn capabilities(&self) -> Capabilities {
        self.model.capabilities()
    }
    fn reset(&mut self) -> Result<(), String> {
        self.model.reset()
    }
    fn create(&mut self, k: Kind) -> Result<u32, String> {
        self.model.create(k)
    }
    fn execute(&mut self, op: &Op) -> Result<Reply, String> {
        let result = self.model.execute(op)?;
        if self.once && matches!(op, Op::Play(_)) {
            self.once = false;
            return Err("Injected error after physical Play".into());
        }
        Ok(result)
    }
}
#[test]
fn owner_rollback_and_epoch_rebuild_recover_after_an_actual_side_effect() {
    let mut s = setup(FailAfterPlay {
        model: Model::default(),
        once: true,
    });
    let before = s.owner().checkpoint().unwrap();
    let manifest = s.manifest().unwrap();
    let f = owner::frame(s.owner());
    assert!(s
        .cycle(&f, &mut crate::audio_curve::PortableCurveMath)
        .is_err());
    assert_eq!(s.owner().checkpoint().unwrap(), before);
    assert!(s.device().fault().is_some());
    assert!(s
        .cycle(&f, &mut crate::audio_curve::PortableCurveMath)
        .is_err());
    s.restart(&manifest, &vec![env(); 3]).unwrap();
    s.cycle(
        &owner::frame(s.owner()),
        &mut crate::audio_curve::PortableCurveMath,
    )
    .unwrap();
    assert_eq!(s.owner().mixed().resources().counts(), (1, 1, 1));
}
#[test]
fn rebuilding_active_paused_sources_preserves_owner_and_reuploads_real_bytes() {
    let mut s = setup(Model::default());
    s.cycle(
        &owner::frame(s.owner()),
        &mut crate::audio_curve::PortableCurveMath,
    )
    .unwrap();
    s.dispatch(
        &cycle::Command::PauseAll {
            request: 1,
            exclude: 0,
        },
        &owner::bank(),
        &mut crate::audio_curve::PortableCurveMath,
        &mixed_helpers::replies(s.owner().mixed()),
    )
    .unwrap();
    let before = s.owner().checkpoint().unwrap();
    let manifest = s.manifest().unwrap();
    s.restart(&manifest, &vec![env(); 3]).unwrap();
    assert_eq!(s.owner().checkpoint().unwrap(), before);
    for n in s
        .owner()
        .mixed()
        .pool()
        .nodes
        .iter()
        .filter(|n| n.source.allocated)
        .map(|n| n.hardware)
        .collect::<Vec<_>>()
    {
        assert_eq!(s.observe(n, 0x1010).unwrap(), 0x1013);
    }
    assert!(s.device().backend().uploaded_bytes() > 2);
}
#[test]
fn referenced_buffer_deletion_is_an_acknowledged_backend_error_not_fake_rollback() {
    let mut e = executor(Model::default());
    let p = e
        .prepare(&[
            Op::Upload {
                buffer: 400,
                format: 0x1101,
                frequency: 8000,
                bytes: vec![0; 4],
            },
            Op::Integer {
                source: 10,
                parameter: 0x1009,
                value: 400,
            },
            Op::DeleteBuffer(400),
        ])
        .unwrap();
    let f = e.apply(&p).unwrap_err();
    assert_eq!(f.completed, 2);
    assert!(e.fault().is_some());
    assert_eq!(e.backend().buffer_count(), 5);
}

#[test]
fn refill_consumes_owned_pcm_at_data_boundary_after_unqueue_before_queue() {
    use crate::{audio_source_tick as tick, audio_stream_bridge as bridge, audio_upload as upload};
    let mut e = executor(Model::default());
    let mut initial = [7004, 7005, 7006, 7007]
        .iter()
        .map(|&buffer| Op::Upload {
            buffer,
            format: 0x1101,
            frequency: 8000,
            bytes: vec![0; 4],
        })
        .collect::<Vec<_>>();
    initial.push(Op::Queue {
        source: 10,
        buffers: vec![7004, 7005, 7006, 7007],
    });
    initial.push(Op::Play(10));
    e.apply(&e.prepare(&initial).unwrap()).unwrap();
    let mut outcome = bridge::Outcome {
        driver: Some(tick::Outcome {
            result: 1,
            events: vec![
                tick::Event::Driver(crate::audio_driver::Event::AlStop {
                    hardware: 10,
                    playing_before: 1,
                    pause_before: 0,
                }),
                tick::Event::Stream(tick::StreamEvent::Unqueue {
                    hardware: 10,
                    buffers: vec![7004],
                }),
                tick::Event::Stream(tick::StreamEvent::Data {
                    buffer: 7004,
                    format: 0x1101,
                    staging: 123,
                    bytes: 4,
                    frequency: 8000,
                }),
                tick::Event::Parameter(upload::Event::Queue {
                    hardware: 10,
                    buffers: vec![7004],
                }),
                tick::Event::Driver(crate::audio_driver::Event::AlPlay {
                    hardware: 10,
                    playing_before: 1,
                    pause_before: 0,
                }),
            ],
        }),
        uploads: vec![bridge::Upload {
            buffer: 7004,
            format: 0x1101,
            frequency: 8000,
            bytes: vec![1, 2, 3, 4],
        }],
        worker: vec![],
    };
    let events = vec![cycle::Event::Mixed(mixed::Event::StreamTick {
        node: 0,
        outcome: outcome.clone(),
    })];
    let plan = lower::cycle(&events, &[]).unwrap();
    assert!(matches!(plan.ops[1], Op::Unqueue { .. }));
    assert!(matches!(plan.ops[2], Op::Upload { .. }));
    assert!(matches!(plan.ops[3], Op::Queue { .. }));
    e.apply(&e.prepare(&plan.ops).unwrap()).unwrap();
    outcome.uploads[0].bytes.pop();
    assert!(lower::cycle(
        &[cycle::Event::Mixed(mixed::Event::StreamTick {
            node: 0,
            outcome
        })],
        &[]
    )
    .is_err());
}

#[test]
fn unsupported_or_oversized_render_does_not_issue_backend_work() {
    let mut e = executor(Model::default());
    assert!(e.render(1).is_err());
    assert!(e.render(usize::MAX).is_err());
    assert!(e.fault().is_none());
    assert!(e.backend().calls().is_empty());
}
struct BadRender(Model);
impl Backend for BadRender {
    fn capabilities(&self) -> Capabilities {
        self.0.capabilities()
    }
    fn reset(&mut self) -> Result<(), String> {
        self.0.reset()
    }
    fn create(&mut self, k: Kind) -> Result<u32, String> {
        self.0.create(k)
    }
    fn execute(&mut self, o: &Op) -> Result<Reply, String> {
        self.0.execute(o)
    }
    fn supports_render(&self) -> bool {
        true
    }
    fn render(&mut self, _frames: usize) -> Result<Rendered, String> {
        Ok(Rendered {
            frequency: 48000,
            samples: vec![0],
        })
    }
}
#[test]
fn malformed_render_receipt_quarantines_the_device_session() {
    let mut e = executor(BadRender(Model::default()));
    assert!(e.render(64).is_err());
    assert!(e.fault().is_some());
    assert!(e.prepare(&[]).is_err());
}
