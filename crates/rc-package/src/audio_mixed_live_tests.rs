use crate::{
    audio_device::*, audio_device_model::Model, audio_mixed_cycle_tests as helper,
    audio_mixed_live::*,
};
use serde_json::json;
use std::{cell::Cell, rc::Rc};
struct Fake {
    model: Model,
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
    fn execute(&mut self, o: &Op) -> Result<Reply, String> {
        self.model.execute(o)
    }
    fn supports_render(&self) -> bool {
        true
    }
    fn render(&mut self, frames: usize) -> Result<Rendered, String> {
        self.frames.set(self.frames.get() + frames);
        if self.fail.get() {
            return Err("lost renderer".into());
        }
        Ok(Rendered {
            frequency: 48000,
            samples: vec![0; frames * 2],
        })
    }
}
fn bundle() -> Bundle {
    let mut owner = helper::host(2);
    helper::add_static(&mut owner, 6, 1.);
    helper::add_stream(&mut owner, 1, 2.);
    let f = helper::frame(&owner);
    let device = crate::audio_device_tests::executor(Model::default());
    Bundle {
        schema: 1,
        owner: serde_json::from_slice(&owner.checkpoint().unwrap()).unwrap(),
        manifest: serde_json::from_slice(&device.checkpoint_manifest().unwrap()).unwrap(),
        base: f.nodes[0].base.clone(),
        environment: f.nodes[0].late.environment.clone(),
        params: f.params,
        bank: helper::bank(),
        context: 42,
        limit_frames: 48000,
        rendered_frames: 0,
        next_control: 0,
        pending: vec![],
        actors: vec![],
        services: 0,
        cycles: 0,
    }
}
fn live() -> (Mixed<Fake>, Rc<Cell<usize>>, Rc<Cell<bool>>) {
    let frames = Rc::new(Cell::new(0));
    let fail = Rc::new(Cell::new(false));
    let b = Fake {
        model: Model::default(),
        frames: frames.clone(),
        fail: fail.clone(),
    };
    (Mixed::open(b, bundle(), 1).unwrap(), frames, fail)
}
fn submit<B: Backend>(m: &mut Mixed<B>, sequence: u64, action: Action) {
    m.submit(&serde_json::to_vec(&Control { sequence, action }).unwrap())
        .unwrap();
}
#[test]
fn strict_bundle_roundtrip_and_unknown_fields() {
    let b = bundle();
    let bytes = b.encode().unwrap();
    assert_eq!(Bundle::parse(&bytes).unwrap().encode().unwrap(), bytes);
    let mut v = serde_json::to_value(b).unwrap();
    v["unexpected"] = json!(1);
    assert!(Bundle::parse(&serde_json::to_vec(&v).unwrap()).is_err());
    assert!(Bundle::parse(&bytes[..bytes.len() - 1]).is_err());
}
#[test]
fn bundle_limits_metadata_and_counter_validation() {
    for mutate in [0, 1, 2, 3, 4, 5] {
        let mut b = bundle();
        match mutate {
            0 => b.schema = 2,
            1 => b.limit_frames = 480001,
            2 => b.bank.clear(),
            3 => b.environment.eax_enabled = true,
            4 => b.services = u64::MAX,
            _ => b.cycles = u64::MAX,
        }
        assert!(b.encode().is_err());
    }
    assert!(Bundle::parse(&vec![b' '; MAX_BUNDLE + 1]).is_err());
}
#[test]
fn bad_render_never_consumes_controls_or_clock() {
    let (mut m, frames, _) = live();
    submit(
        &mut m,
        0,
        Action::Listener {
            location: [1., 2., 3.],
            velocity: [0.; 3],
        },
    );
    let before = m.checkpoint().unwrap();
    for (f, e, s) in [(0, 1, 0), (481, 1, 0), (480, 2, 0), (480, 1, 1)] {
        assert!(m.render(f, e, s).is_err());
    }
    assert_eq!(frames.get(), 0);
    assert_eq!(before, m.checkpoint().unwrap());
}
#[test]
fn controls_are_bounded_sequenced_and_do_not_advance_time() {
    let (mut m, frames, _) = live();
    for sequence in 0..32 {
        submit(
            &mut m,
            sequence,
            Action::Listener {
                location: [0.; 3],
                velocity: [0.; 3],
            },
        );
    }
    let c = serde_json::to_vec(&Control {
        sequence: 32,
        action: Action::StopAll,
    })
    .unwrap();
    assert!(m.submit(&c).is_err());
    assert!(m.submit(&vec![0; MAX_CONTROL + 1]).is_err());
    assert_eq!(m.next_control(), 32);
    assert_eq!(frames.get(), 0);
    m.render(480, 1, 0).unwrap();
    assert_eq!(m.next_control(), 32);
    assert!(Bundle::parse(&m.checkpoint().unwrap())
        .unwrap()
        .pending
        .is_empty());
}
#[test]
fn duplicate_and_bad_control_do_not_consume_sequence() {
    let (mut m, _, _) = live();
    assert!(m.submit(br#"{"sequence":1,"action":"StopAll"}"#).is_err());
    assert!(m.submit(br#"{"sequence":0,"action":{"Actor":{"id":0,"location":[0,0,0],"velocity":[0,0,0],"valid":true}}}"#).is_err());
    submit(
        &mut m,
        0,
        Action::Listener {
            location: [0.; 3],
            velocity: [0.; 3],
        },
    );
    assert!(m.submit(br#"{"sequence":0,"action":"StopAll"}"#).is_err());
    assert_eq!(m.next_control(), 1);
}
#[test]
fn actor_and_listener_survive_cold_restore_with_pending_controls() {
    let (mut m, _, _) = live();
    submit(
        &mut m,
        0,
        Action::Actor(Actor {
            id: 42,
            location: [10., 0., 0.],
            velocity: [0.; 3],
            valid: true,
        }),
    );
    submit(
        &mut m,
        1,
        Action::Listener {
            location: [1., 0., 0.],
            velocity: [0.; 3],
        },
    );
    let saved = Bundle::parse(&m.checkpoint().unwrap()).unwrap();
    assert_eq!(saved.pending.len(), 2);
    let frames = Rc::new(Cell::new(0));
    let mut restored = Mixed::open(
        Fake {
            model: Model::default(),
            frames,
            fail: Rc::new(Cell::new(false)),
        },
        saved,
        7,
    )
    .unwrap();
    let p = crate::audio_pcm::Packet::decode(&restored.render(480, 7, 0).unwrap()).unwrap();
    assert_eq!((p.epoch, p.sequence), (7, 0));
    let b = Bundle::parse(&restored.checkpoint().unwrap()).unwrap();
    assert!(b.pending.is_empty());
    assert_eq!(b.actors[0].location, [10., 0., 0.]);
    assert_eq!(b.base.listener_location, [1., 0., 0.]);
    restored.render(480, 7, 1).unwrap();
    assert_eq!(restored.next_control(), 2);
}
#[test]
fn remaining_budget_and_transport_sequence_are_independent() {
    let mut b = bundle();
    b.rendered_frames = 47900;
    let frames = Rc::new(Cell::new(0));
    let mut m = Mixed::open(
        Fake {
            model: Model::default(),
            frames: frames.clone(),
            fail: Rc::new(Cell::new(false)),
        },
        b,
        9,
    )
    .unwrap();
    assert_eq!(m.remaining(), 100);
    m.render(100, 9, 0).unwrap();
    assert_eq!(m.remaining(), 0);
    assert!(m.render(1, 9, 1).is_err());
    assert_eq!(frames.get(), 100);
}
#[test]
fn renderer_fault_quarantines_session_and_checkpoint() {
    let (mut m, frames, fail) = live();
    fail.set(true);
    assert!(m.render(480, 1, 0).is_err());
    assert!(m.checkpoint().is_err());
    fail.set(false);
    assert!(m.render(480, 1, 0).is_err());
    assert_eq!(frames.get(), 480);
    assert!(m.submit(br#"{"sequence":0,"action":"StopAll"}"#).is_err());
}
#[test]
fn invalid_manifest_and_bank_are_rejected() {
    let mut b = bundle();
    b.manifest = json!({"bad":true});
    let frames = Rc::new(Cell::new(0));
    assert!(Mixed::open(
        Fake {
            model: Model::default(),
            frames,
            fail: Rc::new(Cell::new(false))
        },
        b,
        1
    )
    .is_err());
    let mut b = bundle();
    b.bank.retain(|b| b.handle != 100);
    assert!(b.encode().is_err());
}
#[test]
fn pending_actor_union_and_sequence_are_checked_on_import() {
    let mut b = bundle();
    b.actors = (1..=64)
        .map(|id| Actor {
            id,
            location: [0.; 3],
            velocity: [0.; 3],
            valid: true,
        })
        .collect();
    b.next_control = 1;
    b.pending.push(Control {
        sequence: 0,
        action: Action::Actor(Actor {
            id: 65,
            location: [0.; 3],
            velocity: [0.; 3],
            valid: true,
        }),
    });
    assert!(b.encode().is_err());
    let mut b = bundle();
    b.pending.push(Control {
        sequence: 0,
        action: Action::StopAll,
    });
    assert!(b.encode().is_err());
}
#[cfg(windows)]
#[test]
fn actual_mixed_renderer_refills_streams_and_rebuilds_pause() {
    let path = std::path::PathBuf::from(env!("CARGO_MANIFEST_DIR"))
        .join("../../tools/openal-soft/openal-soft-1.25.2-bin/bin/Win64/soft_oal.dll");
    let mut m = Mixed::open(
        crate::audio_openal::OpenAl::open_loopback(&path, 48000).unwrap(),
        bundle(),
        1,
    )
    .unwrap();
    let mut energy = 0.;
    for seq in 0..50 {
        let p = crate::audio_pcm::Packet::decode(&m.render(480, 1, seq).unwrap()).unwrap();
        energy += p.samples.iter().map(|&s| f64::from(s).powi(2)).sum::<f64>();
    }
    assert!(energy > 1e6);
    assert!(m.services() > 0);
    assert!(!m
        .owner()
        .mixed()
        .pool()
        .nodes
        .iter()
        .any(|n| n.source.allocated && n.source.sound == 100));
    submit(
        &mut m,
        0,
        Action::Pause {
            actor: 42,
            sound: 1001,
            paused: true,
        },
    );
    let saved = Bundle::parse(&m.checkpoint().unwrap()).unwrap();
    drop(m);
    let mut m = Mixed::open(
        crate::audio_openal::OpenAl::open_loopback(&path, 48000).unwrap(),
        saved,
        2,
    )
    .unwrap();
    m.render(480, 2, 0).unwrap();
    let p = crate::audio_pcm::Packet::decode(&m.render(480, 2, 1).unwrap()).unwrap();
    assert!(p.samples.iter().all(|s| s.abs() < 3));
}
