use super::audio_live::*;
use crate::audio_device::*;
use std::{cell::RefCell, rc::Rc};
#[derive(Default)]
struct Calls {
    creates: usize,
    renders: usize,
    frames: usize,
    fail: bool,
    wrong_rate: bool,
}
struct Fake(Rc<RefCell<Calls>>);
impl Backend for Fake {
    fn capabilities(&self) -> Capabilities {
        Capabilities {
            eax: false,
            context: true,
            music: false,
        }
    }
    fn reset(&mut self) -> Result<(), String> {
        Ok(())
    }
    fn create(&mut self, _kind: Kind) -> Result<u32, String> {
        let mut c = self.0.borrow_mut();
        c.creates += 1;
        Ok(c.creates as u32)
    }
    fn execute(&mut self, _op: &Op) -> Result<Reply, String> {
        Ok(Reply::Unit)
    }
    fn supports_render(&self) -> bool {
        true
    }
    fn render(&mut self, frames: usize) -> Result<Rendered, String> {
        let mut c = self.0.borrow_mut();
        c.renders += 1;
        c.frames += frames;
        if c.fail {
            return Err("lost device".into());
        }
        Ok(Rendered {
            frequency: if c.wrong_rate { 44100 } else { 48000 },
            samples: vec![7; frames * 2],
        })
    }
}
pub(crate) fn wave() -> Vec<u8> {
    let samples: Vec<i16> = (0..480)
        .map(|i| ((i as f32 * 440. * std::f32::consts::TAU / 48000.).sin() * 12000.) as i16)
        .collect();
    let mut bytes = Vec::new();
    bytes.extend(b"RIFF");
    bytes.extend((36 + samples.len() as u32 * 2).to_le_bytes());
    bytes.extend(b"WAVEfmt ");
    bytes.extend(16u32.to_le_bytes());
    bytes.extend(1u16.to_le_bytes());
    bytes.extend(1u16.to_le_bytes());
    bytes.extend(48000u32.to_le_bytes());
    bytes.extend(96000u32.to_le_bytes());
    bytes.extend(2u16.to_le_bytes());
    bytes.extend(16u16.to_le_bytes());
    bytes.extend(b"data");
    bytes.extend((samples.len() as u32 * 2).to_le_bytes());
    for s in samples {
        bytes.extend(s.to_le_bytes());
    }
    bytes
}
fn live() -> (Live<Fake>, Rc<RefCell<Calls>>) {
    let c = Rc::new(RefCell::new(Calls::default()));
    (Live::open(Fake(c.clone()), &wave(), 48000, 3).unwrap(), c)
}
#[test]
fn pcm_packets_are_sequenced_and_exact() {
    let (mut l, c) = live();
    let p = crate::audio_pcm::Packet::decode(&l.render(480, 3, 0).unwrap()).unwrap();
    assert_eq!(
        (p.rate, p.epoch, p.sequence, p.samples.len()),
        (48000, 3, 0, 960)
    );
    assert!(p.samples.iter().all(|&s| s == 7));
    assert_eq!(
        (l.next_sequence(), l.rendered_frames(), c.borrow().frames),
        (1, 480, 480)
    );
}
#[test]
fn invalid_requests_never_advance_backend_clock() {
    let (mut l, c) = live();
    for (f, e, s) in [(0, 3, 0), (4097, 3, 0), (480, 2, 0), (480, 3, 1)] {
        assert!(l.render(f, e, s).is_err());
    }
    assert_eq!(c.borrow().renders, 0);
    l.render(480, 3, 0).unwrap();
    assert!(l.render(480, 3, 0).is_err());
    assert_eq!(c.borrow().renders, 1);
}
#[test]
fn finite_budget_accepts_exact_end() {
    let (mut l, c) = live();
    for seq in 0..60 {
        l.render(4000, 3, seq).unwrap();
    }
    assert_eq!(c.borrow().frames, 240000);
    assert!(l.render(1, 3, 60).is_err());
    assert_eq!(c.borrow().frames, 240000);
}
#[test]
fn malformed_input_precedes_physical_allocations() {
    let c = Rc::new(RefCell::new(Calls::default()));
    for (epoch, data) in [(0, wave()), (1, vec![0; 44]), (1, vec![0; MAX_WAVE + 1])] {
        assert!(Live::open(Fake(c.clone()), &data, 48000, epoch).is_err());
    }
    assert_eq!(c.borrow().creates, 0);
}
#[test]
fn render_failure_is_terminal() {
    let (mut l, c) = live();
    c.borrow_mut().fail = true;
    assert!(l.render(480, 3, 0).is_err());
    c.borrow_mut().fail = false;
    assert!(l.render(480, 3, 0).is_err());
    assert_eq!(c.borrow().renders, 1);
}
#[test]
fn wrong_rate_is_terminal() {
    let (mut l, c) = live();
    c.borrow_mut().wrong_rate = true;
    assert!(l.render(480, 3, 0).is_err());
    assert!(l.render(480, 3, 0).is_err());
    assert_eq!(c.borrow().renders, 1);
}
#[cfg(windows)]
#[test]
fn concrete_loopback_renders_live_packets() {
    let library = std::path::PathBuf::from(env!("CARGO_MANIFEST_DIR"))
        .join("../../tools/openal-soft/openal-soft-1.25.2-bin/bin/Win64/soft_oal.dll");
    let mut live = Live::open(
        crate::audio_openal::OpenAl::open_loopback(&library, 48000).unwrap(),
        &wave(),
        48000,
        1,
    )
    .unwrap();
    let packet = crate::audio_pcm::Packet::decode(&live.render(4096, 1, 0).unwrap()).unwrap();
    let rms = (packet
        .samples
        .iter()
        .map(|&s| f64::from(s).powi(2))
        .sum::<f64>()
        / packet.samples.len() as f64)
        .sqrt();
    assert!(rms > 500.);
}
