use super::*;
#[path = "audio_scene_owner_tests.rs"]
mod owner_tests;
#[path = "audio_scene_pool_tests.rs"]
mod pool_tests;
use crate::audio_device::{Capabilities, Rendered, Reply};
use crate::audio_device_model::Model;
struct Fake {
    model: Model,
    fail_seek: bool,
}
impl Backend for Fake {
    fn capabilities(&self) -> Capabilities {
        self.model.capabilities()
    }
    fn reset(&mut self) -> Result<(), String> {
        self.model.reset()
    }
    fn create(&mut self, k: Kind) -> Result<u32, String> {
        self.model.create(k)
    }
    fn execute(&mut self, o: &Op) -> Result<Reply, String> {
        if self.fail_seek
            && matches!(
                o,
                Op::Integer {
                    parameter: 0x1025,
                    ..
                }
            )
        {
            return Err("seek failed".into());
        }
        self.model.execute(o)
    }
    fn listener(&mut self, _p: [f32; 3], _f: [f32; 3], _u: [f32; 3]) -> Result<(), String> {
        Ok(())
    }
    fn supports_render(&self) -> bool {
        true
    }
    fn render(&mut self, n: usize) -> Result<Rendered, String> {
        Ok(Rendered {
            frequency: 48000,
            samples: vec![0; n * 2],
        })
    }
}
fn bank() -> Vec<u8> {
    let tone = include_bytes!("../../../fixtures/audio-live-tone.wav");
    let data = tone[44..].repeat(200);
    let mut w = tone[..44].to_vec();
    w[4..8].copy_from_slice(&(36 + data.len() as u32).to_le_bytes());
    w[40..44].copy_from_slice(&(data.len() as u32).to_le_bytes());
    w.extend(data);
    [
        b"RCSND001".as_slice(),
        &1u32.to_le_bytes(),
        &(w.len() as u32).to_le_bytes(),
        &w,
    ]
    .concat()
}
fn scene() -> Scene<Fake> {
    Scene::open(
        Fake {
            model: Model::default(),
            fail_seek: false,
        },
        &bank(),
        1,
    )
    .unwrap()
}
fn empty() -> Vec<u8> {
    [b"RCAUD003".as_slice(), &0u32.to_le_bytes()].concat()
}
fn event(sequence: u64, tick: u64, category: u32) -> Vec<u8> {
    let mut b = b"RCAUD003".to_vec();
    b.extend(1u32.to_le_bytes());
    b.extend(sequence.to_le_bytes());
    b.extend(tick.to_le_bytes());
    for v in [0, 42, category, 0] {
        b.extend(v.to_le_bytes());
    }
    for v in [0.5f32, 1.25, 1.] {
        b.extend(v.to_le_bytes());
    }
    b.extend(1u32.to_le_bytes());
    b
}
fn loops(active: bool, activation: u64) -> Vec<u8> {
    let mut b = b"RCLOOP04".to_vec();
    b.extend(u32::from(active).to_le_bytes());
    b.extend([0u8; 20]);
    if active {
        b.extend(0u32.to_le_bytes());
        b.extend(42u32.to_le_bytes());
        for v in [0f32, 10., 0., 0.5, 1., 100.] {
            b.extend(v.to_le_bytes());
        }
        b.extend(1u32.to_le_bytes());
        b.extend(activation.to_le_bytes());
    }
    b
}
fn unpack(bytes: &[u8]) -> serde_json::Value {
    serde_json::from_slice(&bytes[12..]).unwrap()
}
fn encode(v: serde_json::Value) -> Vec<u8> {
    let b = serde_json::to_vec(&v).unwrap();
    [b"RCRES001".as_slice(), &(b.len() as u32).to_le_bytes(), &b].concat()
}
fn dialog() -> Scene<Fake> {
    let mut s = scene();
    s.update(&event(1, 0, 1), &loops(false, 0)).unwrap();
    s.render(480, 1, 0).unwrap();
    s.apply(&[Op::Integer {
        source: 1,
        parameter: 0x1025,
        value: 12345,
    }])
    .unwrap();
    s
}
#[test]
fn dialog_offset_timeline_and_parameters_are_restored() {
    let mut old = dialog();
    let bytes = old.snapshot().unwrap();
    let mut next = scene();
    next.restore(&bytes).unwrap();
    assert_eq!(next.active(), 1);
    assert_eq!(next.frames, 480);
    assert_eq!(next.last_sequence(), 1);
    assert_eq!(next.device.observe(1, 0x1025).unwrap(), 12345);
    assert_eq!(
        unpack(&next.snapshot().unwrap())["dialogs"],
        unpack(&bytes)["dialogs"]
    );
    next.render(480, 1, 0).unwrap();
}
#[test]
fn snapshots_are_readonly_and_only_keep_dialogs() {
    let mut s = dialog();
    s.update(&event(2, 0, 2), &loops(false, 0)).unwrap();
    s.render(480, 1, 1).unwrap();
    assert_eq!(s.active(), 2);
    let a = s.snapshot().unwrap();
    let b = s.snapshot().unwrap();
    assert_eq!(a, b);
    let v = unpack(&a);
    assert_eq!(v["dialogs"].as_array().unwrap().len(), 1);
    assert_eq!(v["last"], 2);
}
#[test]
fn completed_dialogs_are_not_replayed() {
    let mut s = dialog();
    s.apply(&[Op::Stop(1)]).unwrap();
    let bytes = s.snapshot().unwrap();
    assert!(unpack(&bytes)["dialogs"].as_array().unwrap().is_empty());
    let mut next = scene();
    next.restore(&bytes).unwrap();
    assert_eq!(next.active(), 0);
}
#[test]
fn loops_wait_for_authoritative_state_and_preserve_offsets() {
    let mut old = scene();
    old.update(&empty(), &loops(true, 8)).unwrap();
    old.apply(&[Op::Integer {
        source: 1,
        parameter: 0x1025,
        value: 400,
    }])
    .unwrap();
    let bytes = old.snapshot().unwrap();
    let mut next = scene();
    next.restore(&bytes).unwrap();
    assert_eq!(next.active(), 0);
    assert_eq!(
        unpack(&next.snapshot().unwrap())["loops"],
        unpack(&bytes)["loops"]
    );
    next.update(&empty(), &loops(true, 8)).unwrap();
    assert_eq!(next.active(), 1);
    assert_eq!(next.device.observe(1, 0x1025).unwrap(), 400);
}
#[test]
fn changed_or_removed_loop_activation_does_not_seek() {
    let mut old = scene();
    old.update(&empty(), &loops(true, 8)).unwrap();
    old.apply(&[Op::Integer {
        source: 1,
        parameter: 0x1025,
        value: 400,
    }])
    .unwrap();
    let bytes = old.snapshot().unwrap();
    let mut next = scene();
    next.restore(&bytes).unwrap();
    next.update(&empty(), &loops(true, 9)).unwrap();
    assert_eq!(next.device.observe(1, 0x1025).unwrap(), 0);
    let mut removed = scene();
    removed.restore(&bytes).unwrap();
    removed.update(&empty(), &loops(false, 0)).unwrap();
    assert_eq!(removed.active(), 0);
    assert!(unpack(&removed.snapshot().unwrap())["loops"]
        .as_array()
        .unwrap()
        .is_empty());
}
#[test]
fn pending_dialogs_keep_order_but_transient_effects_are_dropped() {
    let mut s = dialog();
    s.update(&event(2, 60, 2), &loops(false, 0)).unwrap();
    s.update(&event(3, 61, 1), &loops(false, 0)).unwrap();
    let v = unpack(&s.snapshot().unwrap());
    let pending = v["pending"].as_array().unwrap();
    assert_eq!(pending.len(), 1);
    assert_eq!(pending[0]["sequence"], 3);
}
#[test]
fn corrupt_resume_is_rejected_before_any_physical_effect() {
    let mut old = dialog();
    let original = old.snapshot().unwrap();
    for mode in 0..7 {
        let mut v = unpack(&original);
        match mode {
            0 => v["bank"] = serde_json::json!(0),
            1 => v["dialogs"][0][1] = serde_json::json!(999999),
            2 => v["dialogs"][0][0]["sound"] = serde_json::json!(128),
            3 => v["dialogs"][0][0]["category"] = serde_json::json!(2),
            4 => v["pose"] = serde_json::json!([[0, 0, 0], [0, 0, 0], [0, 1, 0]]),
            5 => v["unexpected"] = serde_json::json!(1),
            _ => v["pending"] = serde_json::json!([{"bad":true}]),
        };
        let mut next = scene();
        let before = next.device.backend().model.calls().len();
        assert!(next.restore(&encode(v)).is_err(), "mode {mode}");
        assert_eq!(next.device.backend().model.calls().len(), before);
        assert_eq!(next.active(), 0);
        next.render(480, 1, 0).unwrap();
    }
}
#[test]
fn only_fresh_targets_accept_one_resume() {
    let mut old = dialog();
    let bytes = old.snapshot().unwrap();
    assert!(old.restore(&bytes).is_err());
    let mut next = scene();
    next.restore(&bytes).unwrap();
    assert!(next.restore(&bytes).is_err());
    let mut rendered = scene();
    rendered.render(1, 1, 0).unwrap();
    assert!(rendered.restore(&bytes).is_err());
}
#[test]
fn partial_seek_failure_quarantines_the_new_target() {
    let mut old = dialog();
    let bytes = old.snapshot().unwrap();
    let mut next = Scene::open(
        Fake {
            model: Model::default(),
            fail_seek: true,
        },
        &bank(),
        1,
    )
    .unwrap();
    assert!(next.restore(&bytes).is_err());
    assert!(next.render(480, 1, 0).is_err());
    assert!(next.snapshot().is_err());
}
#[test]
fn malformed_size_and_truncation_never_restore() {
    let mut old = dialog();
    let bytes = old.snapshot().unwrap();
    for len in [0, 8, 12, bytes.len() - 1] {
        assert!(scene().restore(&bytes[..len]).is_err());
    }
    assert!(scene().restore(&vec![0; MAX_RESUME + 1]).is_err());
}
#[cfg(windows)]
#[test]
fn actual_openal_dialog_continues_from_source_offset_and_ends_at_remaining_duration() {
    let path = std::path::PathBuf::from(env!("CARGO_MANIFEST_DIR"))
        .join("../../tools/openal-soft/openal-soft-1.25.2-bin/bin/Win64/soft_oal.dll");
    let mut old = Scene::open(
        crate::audio_openal::OpenAl::open_loopback(&path, 48000).unwrap(),
        &bank(),
        1,
    )
    .unwrap();
    old.update(&event(1, 0, 1), &loops(false, 0)).unwrap();
    for seq in 0..10 {
        old.render(480, 1, seq).unwrap();
    }
    let bytes = old.snapshot().unwrap();
    let offset = unpack(&bytes)["dialogs"][0][1].as_u64().unwrap();
    assert!((5900..6100).contains(&offset), "{offset}");
    drop(old);
    let mut next = Scene::open(
        crate::audio_openal::OpenAl::open_loopback(&path, 48000).unwrap(),
        &bank(),
        2,
    )
    .unwrap();
    next.restore(&bytes).unwrap();
    assert_eq!(next.device.observe(1, 0x1025).unwrap() as u64, offset);
    next.update(&empty(), &loops(false, 0)).unwrap();
    for seq in 0..152 {
        next.render(480, 2, seq).unwrap();
    }
    assert_eq!(next.active(), 0, "dialog restarted instead of continuing");
}
