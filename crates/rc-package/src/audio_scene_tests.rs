use crate::{audio_device::*, audio_device_model::Model, audio_scene::*};
use std::{
    cell::{Cell, RefCell},
    rc::Rc,
};
type Pose = ([f32; 3], [f32; 3], [f32; 3]);
struct Fake {
    model: Model,
    ops: Rc<RefCell<Vec<Op>>>,
    poses: Rc<RefCell<Vec<Pose>>>,
    frames: Rc<Cell<usize>>,
    fail: Rc<Cell<bool>>,
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
    fn execute(&mut self, op: &Op) -> Result<Reply, String> {
        self.ops.borrow_mut().push(op.clone());
        self.model.execute(op)
    }
    fn listener(&mut self, p: [f32; 3], f: [f32; 3], u: [f32; 3]) -> Result<(), String> {
        self.poses.borrow_mut().push((p, f, u));
        Ok(())
    }
    fn supports_render(&self) -> bool {
        true
    }
    fn render(&mut self, n: usize) -> Result<Rendered, String> {
        self.frames.set(self.frames.get() + n);
        if self.fail.get() {
            return Err("device lost".into());
        }
        Ok(Rendered {
            frequency: 48000,
            samples: vec![0; n * 2],
        })
    }
}
fn bank() -> Vec<u8> {
    let w = include_bytes!("../../../fixtures/audio-live-tone.wav");
    [
        b"RCSND001".as_slice(),
        &1u32.to_le_bytes(),
        &(w.len() as u32).to_le_bytes(),
        w,
    ]
    .concat()
}
fn events(sequence: u64, tick: u64, category: u32) -> Vec<u8> {
    let mut b = b"RCAUD003".to_vec();
    b.extend(1u32.to_le_bytes());
    b.extend(sequence.to_le_bytes());
    b.extend(tick.to_le_bytes());
    for v in [0u32, 42, category, 0] {
        b.extend(v.to_le_bytes());
    }
    for v in [0.5f32, 1.25, 1.] {
        b.extend(v.to_le_bytes());
    }
    b.extend(1u32.to_le_bytes());
    b
}
fn empty() -> Vec<u8> {
    [b"RCAUD003".as_slice(), &0u32.to_le_bytes()].concat()
}
fn loops(active: bool, activation: u64, yaw: f32, pitch: f32) -> Vec<u8> {
    let mut b = b"RCLOOP04".to_vec();
    b.extend(u32::from(active).to_le_bytes());
    for v in [0f32, 0., 0., yaw, pitch] {
        b.extend(v.to_le_bytes());
    }
    if active {
        b.extend(0u32.to_le_bytes());
        b.extend(42u32.to_le_bytes());
        for v in [0f32, 100., 0., 0.5, 1., 100.] {
            b.extend(v.to_le_bytes());
        }
        b.extend(1u32.to_le_bytes());
        b.extend(activation.to_le_bytes());
    }
    b
}
type Setup = (
    Scene<Fake>,
    Rc<RefCell<Vec<Op>>>,
    Rc<RefCell<Vec<Pose>>>,
    Rc<Cell<usize>>,
    Rc<Cell<bool>>,
);
fn setup() -> Setup {
    let ops = Rc::new(RefCell::new(vec![]));
    let poses = Rc::new(RefCell::new(vec![]));
    let frames = Rc::new(Cell::new(0));
    let fail = Rc::new(Cell::new(false));
    let scene = Scene::open(
        Fake {
            model: Model::default(),
            ops: ops.clone(),
            poses: poses.clone(),
            frames: frames.clone(),
            fail: fail.clone(),
        },
        &bank(),
        1,
    )
    .unwrap();
    (scene, ops, poses, frames, fail)
}
fn plays(ops: &Rc<RefCell<Vec<Op>>>) -> usize {
    ops.borrow()
        .iter()
        .filter(|v| matches!(v, Op::Play(_)))
        .count()
}
fn mix(values: [f32; 4]) -> Vec<u8> {
    [
        b"RCMIX001".to_vec(),
        values.into_iter().flat_map(f32::to_le_bytes).collect(),
    ]
    .concat()
}
fn last_gain(ops: &Rc<RefCell<Vec<Op>>>) -> f32 {
    ops.borrow()
        .iter()
        .rev()
        .find_map(|op| match op {
            Op::Float {
                parameter: 0x100a,
                value,
                ..
            } => Some(f32::from_bits(*value)),
            _ => None,
        })
        .unwrap()
}
#[test]
fn mix_validation_precedes_all_backend_effects() {
    let (mut scene, ops, _, _, _) = setup();
    let count = ops.borrow().len();
    for bad in [
        mix([f32::NAN, 1., 1., 1.]),
        mix([-0.1, 1., 1., 1.]),
        mix([1.1, 1., 1., 1.]),
        vec![0; 24],
        vec![],
    ] {
        assert!(scene.set_mix(&bad).is_err());
        assert_eq!(ops.borrow().len(), count);
    }
    scene.set_mix(&mix([1.; 4])).unwrap();
    assert_eq!(ops.borrow().len(), count);
}
#[test]
fn mix_separates_dialog_from_effect_categories() {
    for category in 0..=2 {
        let (mut scene, ops, _, _, _) = setup();
        scene
            .update(&events(1, 0, category), &loops(false, 0, 0., 0.))
            .unwrap();
        scene.render(1, 1, 0).unwrap();
        let base = last_gain(&ops);
        scene.set_mix(&mix([0.5, 0.2, 0.8, 0.4])).unwrap();
        let ratio = if category == 1 { 0.4 } else { 0.1 };
        assert!((last_gain(&ops) - base * ratio).abs() < 1e-7);
    }
}
#[test]
fn live_mix_changes_only_gains_without_clock_or_source_restart() {
    let (mut scene, ops, _, frames, _) = setup();
    scene
        .update(&events(1, 0, 1), &loops(false, 0, 0., 0.))
        .unwrap();
    scene.render(1, 1, 0).unwrap();
    let saved = scene.snapshot().unwrap();
    let count = ops.borrow().len();
    scene.set_mix(&mix([0.5, 1., 0.5, 1.])).unwrap();
    assert!(ops.borrow()[count..].iter().all(|op| matches!(
        op,
        Op::Float {
            parameter: 0x100a,
            ..
        }
    )));
    assert_eq!(frames.get(), 1);
    assert_eq!(scene.snapshot().unwrap(), saved);
    let count = ops.borrow().len();
    scene.set_mix(&mix([0.5, 1., 0.5, 1.])).unwrap();
    assert_eq!(ops.borrow().len(), count);
}
#[test]
fn ambient_mix_uses_latest_original_volume_and_separate_bus() {
    let (mut scene, ops, _, _, _) = setup();
    scene.update(&empty(), &loops(true, 1, 0., 0.)).unwrap();
    let mut changed = loops(true, 1, 0., 0.);
    changed[52..56].copy_from_slice(&0.25f32.to_le_bytes());
    scene.update(&empty(), &changed).unwrap();
    let starts = plays(&ops);
    scene.set_mix(&mix([0.5, 0., 0., 0.4])).unwrap();
    assert!((last_gain(&ops) - 0.35 * 0.25 * 0.5 * 0.4).abs() < 1e-7);
    assert_eq!(plays(&ops), starts);
}
#[test]
fn current_mix_wins_over_audio_resume_without_changing_source_metadata() {
    let (mut scene, ops, _, _, _) = setup();
    scene
        .update(&events(1, 0, 1), &loops(false, 0, 0., 0.))
        .unwrap();
    scene.render(1, 1, 0).unwrap();
    let base = last_gain(&ops);
    scene.set_mix(&mix([0.; 4])).unwrap();
    let saved = scene.snapshot().unwrap();
    let (mut next, next_ops, _, _, _) = setup();
    next.set_mix(&mix([0.5, 1., 0.5, 1.])).unwrap();
    next.restore(&saved).unwrap();
    assert!((last_gain(&next_ops) - base * 0.25).abs() < 1e-7);
    assert_eq!(next.snapshot().unwrap(), saved);
}
#[test]
fn bank_validation_precedes_io() {
    assert_eq!(super::audio_scene::bank(&bank()).unwrap().len(), 1);
    let mut b = bank();
    b[8] = 129;
    assert!(super::audio_scene::bank(&b).is_err());
    let b = bank();
    for n in [0, 8, 12, 20, b.len() - 1] {
        assert!(super::audio_scene::bank(&b[..n]).is_err());
    }
}
#[test]
fn event_and_loop_validation_are_atomic() {
    let (mut s, ops, poses, _, _) = setup();
    let before = ops.borrow().len();
    let mut bad = loops(false, 0, 0., 0.);
    bad[0] = 0;
    assert!(s.update(&events(1, 0, 1), &bad).is_err());
    assert_eq!(s.last_sequence(), 0);
    assert_eq!(ops.borrow().len(), before);
    assert!(poses.borrow().is_empty());
    s.update(&events(1, 0, 1), &loops(false, 0, 0., 0.))
        .unwrap();
    assert_eq!(s.last_sequence(), 1);
}
#[test]
fn timeline_and_original_parameters_reach_backend() {
    let (mut s, ops, _, _, _) = setup();
    s.update(&events(1, 0, 2), &loops(false, 0, 0., 0.))
        .unwrap();
    s.render(480, 1, 0).unwrap();
    s.update(&events(2, 3, 2), &loops(false, 0, 0., 0.))
        .unwrap();
    for seq in 1..5 {
        s.render(480, 1, seq).unwrap();
    }
    assert_eq!(plays(&ops), 1);
    s.render(480, 1, 5).unwrap();
    assert_eq!(plays(&ops), 2);
    assert!(ops
        .borrow()
        .iter()
        .any(|o| matches!(o,Op::Float{parameter:0x1003,value,..}if *value==1.25f32.to_bits())));
}
#[test]
fn generation_and_duplicate_packets_do_not_render() {
    let (mut s, _, _, frames, _) = setup();
    s.update(&events(99, 0, 1), &loops(false, 0, 0., 0.))
        .unwrap();
    assert!(s
        .update(&events(99, 0, 1), &loops(false, 0, 0., 0.))
        .is_err());
    for (f, e, n) in [(0, 1, 0), (481, 1, 0), (480, 2, 0), (480, 1, 1)] {
        assert!(s.render(f, e, n).is_err());
    }
    assert_eq!(frames.get(), 0);
}
#[test]
fn loop_identity_reuses_source_and_reactivation_restarts() {
    let (mut s, ops, _, _, _) = setup();
    s.update(&empty(), &loops(true, 5, 0., 0.)).unwrap();
    let p = plays(&ops);
    s.update(&empty(), &loops(true, 5, 1., 0.)).unwrap();
    assert_eq!(plays(&ops), p);
    s.update(&empty(), &loops(true, 6, 1., 0.)).unwrap();
    assert_eq!(plays(&ops), p + 1);
    s.update(&empty(), &loops(false, 0, 0., 0.)).unwrap();
    assert_eq!(s.active(), 0);
}
#[test]
fn physical_listener_has_full_orientation() {
    let (mut s, _, poses, _, _) = setup();
    s.update(&empty(), &loops(false, 0, 0.8, 0.4)).unwrap();
    let (_, f, u) = poses.borrow()[0];
    assert!((f[1] - 0.4f32.sin()).abs() < 1e-6);
    assert!((f.iter().map(|v| v * v).sum::<f32>() - 1.).abs() < 1e-5);
    assert!(f.iter().zip(u).map(|(a, b)| a * b).sum::<f32>().abs() < 1e-5);
}
#[test]
fn restored_host_rebuilds_loops_without_replaying_effects() {
    let (mut s, ops, _, _, _) = setup();
    s.update(&events(5, 0, 1), &loops(true, 7, 0., 0.)).unwrap();
    s.render(480, 1, 0).unwrap();
    let old = plays(&ops);
    drop(s);
    let (mut s, ops, _, _, _) = setup();
    s.update(&empty(), &loops(true, 7, 0., 0.)).unwrap();
    s.render(480, 1, 0).unwrap();
    assert_eq!(plays(&ops), 1);
    assert!(old > 1);
    assert_eq!(s.last_sequence(), 0);
}
#[test]
fn replaced_dialog_stops_previous_owner() {
    let (mut s, ops, _, _, _) = setup();
    s.update(&events(1, 0, 1), &loops(false, 0, 0., 0.))
        .unwrap();
    s.render(480, 1, 0).unwrap();
    s.update(&events(2, 0, 1), &loops(false, 0, 0., 0.))
        .unwrap();
    s.render(480, 1, 1).unwrap();
    assert_eq!(s.active(), 1);
    assert!(ops.borrow().iter().any(|op| matches!(op, Op::Stop(_))));
}
#[test]
fn renderer_fault_is_terminal() {
    let (mut s, _, _, frames, fail) = setup();
    fail.set(true);
    assert!(s.render(480, 1, 0).is_err());
    fail.set(false);
    assert!(s.render(480, 1, 0).is_err());
    assert!(s.update(&empty(), &loops(false, 0, 0., 0.)).is_err());
    assert_eq!(frames.get(), 480);
}
#[test]
fn listener_invalidates_program_and_rejects_bad_pose() {
    let mut d = Executor::new(Model::default(), 1).unwrap();
    let p = d.prepare(&[Op::Check("before".into())]).unwrap();
    assert!(d.listener([0.; 3], [1., 0., 0.], [1., 0., 0.]).is_err());
    d.listener([0.; 3], [1., 0., 0.], [0., 1., 0.]).unwrap();
    assert!(d.apply(&p).is_err());
}
#[cfg(windows)]
#[test]
fn actual_listener_turns_move_sound_between_ears_and_wav_ends() {
    let path = std::path::PathBuf::from(env!("CARGO_MANIFEST_DIR"))
        .join("../../tools/openal-soft/openal-soft-1.25.2-bin/bin/Win64/soft_oal.dll");
    let mut s = Scene::open(
        crate::audio_openal::OpenAl::open_loopback(&path, 48000).unwrap(),
        &bank(),
        1,
    )
    .unwrap();
    s.update(&empty(), &loops(true, 1, 0., 0.)).unwrap();
    fn energy(bytes: &[u8]) -> [f64; 2] {
        let p = crate::audio_pcm::Packet::decode(bytes).unwrap();
        let mut e = [0.; 2];
        for f in p.samples.chunks_exact(2) {
            e[0] += f64::from(f[0]).powi(2);
            e[1] += f64::from(f[1]).powi(2);
        }
        e
    }
    for seq in 0..5 {
        s.render(480, 1, seq).unwrap();
    }
    let left = energy(&s.render(480, 1, 5).unwrap());
    assert!(left[0] > left[1] * 2., "{left:?}");
    s.update(&empty(), &loops(true, 1, std::f32::consts::PI, 0.))
        .unwrap();
    for seq in 6..11 {
        s.render(480, 1, seq).unwrap();
    }
    let right = energy(&s.render(480, 1, 11).unwrap());
    assert!(right[1] > right[0] * 2., "{right:?}");
    s.update(&events(1, 0, 2), &loops(false, 0, 0., 0.))
        .unwrap();
    for seq in 12..16 {
        s.render(480, 1, seq).unwrap();
    }
    assert_eq!(s.active(), 0);
}
