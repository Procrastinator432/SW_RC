use super::*;
use std::{cell::Cell, rc::Rc};
fn request(sequence: u64, category: u32) -> Vec<u8> {
    let mut bytes = event(sequence, 0, category);
    bytes[32..36].copy_from_slice(&(sequence as u32).to_le_bytes());
    bytes
}
fn fill<B: Backend>(scene: &mut Scene<B>) {
    for sequence in 1..=16 {
        scene
            .update(&request(sequence, 1), &loops(false, 0))
            .unwrap();
        scene.render(1, 1, sequence - 1).unwrap();
    }
}
fn desired(count: u32) -> Vec<u8> {
    let template = loops(true, 1);
    let mut bytes = template[..32].to_vec();
    bytes[8..12].copy_from_slice(&count.to_le_bytes());
    for owner in 1..=count {
        let mut record = template[32..].to_vec();
        record[4..8].copy_from_slice(&owner.to_le_bytes());
        bytes.extend(record);
    }
    bytes
}
struct FaultBackend {
    base: Fake,
    failure: Rc<Cell<u8>>,
}
impl Backend for FaultBackend {
    fn capabilities(&self) -> Capabilities {
        self.base.capabilities()
    }
    fn reset(&mut self) -> Result<(), String> {
        self.base.reset()
    }
    fn create(&mut self, k: Kind) -> Result<u32, String> {
        self.base.create(k)
    }
    fn execute(&mut self, op: &Op) -> Result<Reply, String> {
        let fail = match self.failure.get() {
            1 => matches!(op, Op::Stop(_)),
            2 => matches!(
                op,
                Op::Integer {
                    parameter: 0x1009,
                    value: 0,
                    ..
                }
            ),
            3 => matches!(op, Op::Query { .. }),
            _ => false,
        };
        if fail {
            return Err("controlled pool IO failure".into());
        }
        self.base.execute(op)
    }
    fn listener(&mut self, p: [f32; 3], f: [f32; 3], u: [f32; 3]) -> Result<(), String> {
        self.base.listener(p, f, u)
    }
    fn supports_render(&self) -> bool {
        true
    }
    fn render(&mut self, n: usize) -> Result<Rendered, String> {
        self.base.render(n)
    }
}
fn faulty() -> (Scene<FaultBackend>, Rc<Cell<u8>>) {
    let failure = Rc::new(Cell::new(0));
    let backend = FaultBackend {
        base: Fake {
            model: Model::default(),
            fail_seek: false,
        },
        failure: failure.clone(),
    };
    (Scene::open(backend, &bank(), 1).unwrap(), failure)
}
#[test]
fn reaps_completed_slot_before_loop_admission_without_preempting_live_oldest() {
    let mut scene = scene();
    fill(&mut scene);
    scene.apply(&[Op::Stop(16)]).unwrap();
    scene.update(&empty(), &desired(1)).unwrap();
    assert_eq!(scene.voices[0].as_ref().unwrap().age, 1);
    assert!(scene.voices[15].as_ref().unwrap().activation.is_some());
    assert_eq!(scene.stats.completed, 1);
    assert_eq!(scene.stats.preempted, 0);
    assert_eq!(scene.active(), 16);
}
#[test]
fn completion_is_released_and_counted_exactly_once() {
    let mut scene = scene();
    scene.update(&request(1, 0), &loops(false, 0)).unwrap();
    scene.render(1, 1, 0).unwrap();
    scene.apply(&[Op::Stop(1)]).unwrap();
    scene.update(&empty(), &loops(false, 0)).unwrap();
    scene.render(1, 1, 1).unwrap();
    scene.update(&empty(), &loops(false, 0)).unwrap();
    assert_eq!(scene.stats.completed, 1);
    assert_eq!(scene.active(), 0);
}
#[test]
fn full_loop_pool_is_nonfatal_and_consumes_one_rejected_request() {
    let mut scene = scene();
    scene.update(&request(1, 0), &desired(16)).unwrap();
    scene.render(1, 1, 0).unwrap();
    assert_eq!(scene.stats.capacity_rejected, 1);
    assert_eq!(scene.stats.admitted, 0);
    assert_eq!(scene.active(), 16);
    assert!(scene.pending.is_empty());
    scene.render(1, 1, 1).unwrap();
    assert_eq!(scene.stats.capacity_rejected, 1);
    assert!(!scene.fault);
}
#[test]
fn equal_priority_speech_pool_replaces_oldest_unprotected_dialogue() {
    let mut scene = scene();
    fill(&mut scene);
    scene.update(&request(17, 1), &loops(false, 0)).unwrap();
    scene.render(1, 1, 16).unwrap();
    assert_eq!(scene.stats.preempted, 1);
    assert_eq!(scene.stats.admitted, 17);
    assert_eq!(scene.stats.capacity_rejected, 0);
    assert_eq!(scene.voices[0].as_ref().unwrap().age, 17);
    assert_eq!(scene.voices[1].as_ref().unwrap().age, 2);
}
fn assert_eviction_failure(mode: u8) {
    let (mut scene, failure) = faulty();
    fill(&mut scene);
    scene.update(&request(17, 1), &loops(false, 0)).unwrap();
    failure.set(mode);
    assert!(scene.render(1, 1, 16).is_err());
    assert!(scene.fault);
    assert_eq!(scene.stats.capacity_rejected, 0);
    assert_eq!(scene.stats.preempted, 0);
    assert_eq!(scene.stats.admitted, 16);
    assert!(scene.snapshot().is_err());
    assert!(scene.render(1, 1, 16).is_err());
    assert!(scene.update(&empty(), &loops(false, 0)).is_err());
    assert_eq!(
        u32::from_le_bytes(scene.status()[28..32].try_into().unwrap()),
        1
    );
}
#[test]
fn effect_pressure_never_evicts_dialogue() {
    let mut s = scene();
    fill(&mut s);
    s.update(&request(17, 0), &loops(false, 0)).unwrap();
    s.render(1, 1, 16).unwrap();
    assert_eq!(s.stats.capacity_rejected, 1);
    assert_eq!(s.stats.preempted, 0);
    assert_eq!(s.stats.admitted, 16);
    assert!(s.voices.iter().flatten().all(|v| v.category == 1));
}
#[test]
fn failed_snapshot_quarantines_receipts_and_later_calls() {
    let (mut s, failure) = faulty();
    s.update(&request(1, 1), &loops(false, 0)).unwrap();
    s.render(1, 1, 0).unwrap();
    failure.set(3);
    assert!(s.snapshot().is_err());
    assert!(s.fault);
    assert!(s.consume_through(2).is_err());
    assert_eq!(
        u32::from_le_bytes(s.status()[28..32].try_into().unwrap()),
        1
    );
}
#[test]
fn new_speech_prefers_an_effect_over_older_dialogue() {
    let mut s = scene();
    fill(&mut s);
    s.voices[15].as_mut().unwrap().category = 0;
    s.update(&request(17, 1), &loops(false, 0)).unwrap();
    s.render(1, 1, 16).unwrap();
    assert_eq!(s.voices[0].as_ref().unwrap().age, 1);
    assert_eq!(s.voices[15].as_ref().unwrap().age, 17);
}
#[test]
fn speech_duck_restores_and_recovers_without_restarting_sources() {
    let mut s = scene();
    s.update(&request(1, 1), &loops(false, 0)).unwrap();
    for i in 0..100 {
        s.render(480, 1, i).unwrap();
    }
    assert_eq!(s.effects_duck, 0.25);
    let saved = s.snapshot().unwrap();
    let mut next = scene();
    next.restore(&saved).unwrap();
    assert_eq!(next.effects_duck, 0.25);
    next.apply(&[Op::Stop(1)]).unwrap();
    for i in 0..100 {
        next.render(480, 1, i).unwrap();
    }
    assert_eq!(next.effects_duck, 1.);
}
#[test]
fn stop_failure_cannot_be_swallowed_as_pool_capacity() {
    assert_eviction_failure(1);
}
#[test]
fn partial_unbind_failure_quarantines_scene_and_exposes_fault_status() {
    assert_eviction_failure(2);
}
#[test]
fn reap_query_failure_during_update_is_terminal() {
    let (mut scene, failure) = faulty();
    scene.update(&request(1, 0), &loops(false, 0)).unwrap();
    scene.render(1, 1, 0).unwrap();
    failure.set(3);
    assert!(scene.update(&empty(), &loops(false, 0)).is_err());
    assert!(scene.fault);
    assert!(scene.snapshot().is_err());
    assert_eq!(scene.stats.completed, 0);
}
#[test]
fn status_is_fixed_readonly_and_tracks_pending_separately() {
    let mut scene = scene();
    scene.update(&request(1, 1), &loops(false, 0)).unwrap();
    let calls = scene.device.backend().model.calls().len();
    let first = scene.status();
    assert_eq!(first.len(), 88);
    assert_eq!(&first[..8], b"RCPOOL01");
    assert_eq!(scene.status(), first);
    assert_eq!(scene.device.backend().model.calls().len(), calls);
    assert_eq!(u32::from_le_bytes(first[24..28].try_into().unwrap()), 1);
    assert_eq!(u64::from_le_bytes(first[48..56].try_into().unwrap()), 0);
    scene.render(1, 1, 0).unwrap();
    let next = scene.status();
    assert_eq!(u32::from_le_bytes(next[12..16].try_into().unwrap()), 1);
    assert_eq!(u64::from_le_bytes(next[48..56].try_into().unwrap()), 1);
}
#[test]
fn stats_survive_restore_and_older_snapshots_default_to_zero() {
    let mut old = scene();
    old.update(&request(1, 1), &loops(false, 0)).unwrap();
    old.render(1, 1, 0).unwrap();
    let saved = old.snapshot().unwrap();
    let mut next = scene();
    next.restore(&saved).unwrap();
    assert_eq!(next.stats.admitted, 1);
    let mut value = unpack(&saved);
    value.as_object_mut().unwrap().remove("stats");
    let mut legacy = scene();
    legacy.restore(&encode(value)).unwrap();
    assert_eq!(legacy.stats.admitted, 0);
    assert_eq!(legacy.active(), 1);
}
#[test]
fn oversized_saved_counters_are_rejected_before_backend_io() {
    let mut old = scene();
    let mut value = unpack(&old.snapshot().unwrap());
    value["stats"]["admitted"] = serde_json::json!(u64::MAX);
    let mut next = scene();
    let calls = next.device.backend().model.calls().len();
    assert!(next.restore(&encode(value)).is_err());
    assert_eq!(next.device.backend().model.calls().len(), calls);
}
#[test]
fn protected_rejection_has_its_own_counter() {
    let mut scene = scene();
    let mut protected = request(1, 1);
    protected[40..44].copy_from_slice(&1u32.to_le_bytes());
    scene.update(&protected, &loops(false, 0)).unwrap();
    scene.render(1, 1, 0).unwrap();
    let mut second = request(2, 1);
    second[32..36].copy_from_slice(&1u32.to_le_bytes());
    scene.update(&second, &loops(false, 0)).unwrap();
    scene.render(1, 1, 1).unwrap();
    assert_eq!(scene.stats.protected_rejected, 1);
    assert_eq!(scene.stats.capacity_rejected, 0);
    assert_eq!(scene.stats.admitted, 1);
}
#[test]
fn counters_saturate_at_java_signed_limit() {
    let mut scene = scene();
    scene.stats.admitted = i64::MAX as u64;
    scene.update(&request(1, 0), &loops(false, 0)).unwrap();
    scene.render(1, 1, 0).unwrap();
    assert_eq!(scene.stats.admitted, i64::MAX as u64);
}
#[test]
fn consumption_receipt_is_readonly_monotonic_and_idempotent() {
    let mut scene = scene();
    scene.update(&request(1, 1), &loops(false, 0)).unwrap();
    scene.render(1, 1, 0).unwrap();
    let calls = scene.device.backend().model.calls().len();
    let frames = scene.frames;
    let timeline = scene.last_played;
    let admitted = scene.stats.admitted;
    scene.consume_through(3).unwrap();
    let saved = scene.snapshot().unwrap();
    let after = scene.device.backend().model.calls().len();
    scene.consume_through(3).unwrap();
    assert_eq!(scene.device.backend().model.calls().len(), after);
    assert!(after >= calls);
    assert_eq!(scene.frames, frames);
    assert_eq!(scene.last_played, timeline);
    assert_eq!(scene.stats.admitted, admitted);
    assert_eq!(scene.last_sequence(), 3);
    assert_eq!(unpack(&saved)["last"], 3);
}
#[test]
fn invalid_or_faulted_receipt_does_not_mutate_cursor() {
    let mut scene = scene();
    scene.consume_through(4).unwrap();
    let before = scene.status();
    assert!(scene.consume_through(3).is_err());
    assert!(scene.consume_through(u64::MAX).is_err());
    assert_eq!(scene.status(), before);
    scene.fault = true;
    assert!(scene.consume_through(5).is_err());
    assert_eq!(scene.last_sequence(), 4);
}
#[test]
fn receipt_preserves_pending_speech_and_survives_restore() {
    let mut old = scene();
    old.update(&request(1, 1), &loops(false, 0)).unwrap();
    old.render(1, 1, 0).unwrap();
    let mut queued = request(2, 1);
    queued[20..28].copy_from_slice(&10u64.to_le_bytes());
    old.update(&queued, &loops(false, 0)).unwrap();
    let timeline = old.last_played;
    old.consume_through(3).unwrap();
    assert_eq!(old.pending.len(), 1);
    assert_eq!(old.last_played, timeline);
    let saved = old.snapshot().unwrap();
    let mut next = scene();
    next.restore(&saved).unwrap();
    assert_eq!(next.last_sequence(), 3);
    assert_eq!(next.pending[0].sequence, 2);
    assert_eq!(next.last_played, timeline);
}
#[test]
fn acknowledged_effect_is_not_replayed_and_repeated_input_is_rejected() {
    let mut scene = scene();
    scene.consume_through(3).unwrap();
    assert!(scene.pending.is_empty());
    assert_eq!(scene.active(), 0);
    assert!(scene.update(&request(3, 0), &loops(false, 0)).is_err());
    scene.update(&request(4, 1), &loops(false, 0)).unwrap();
    scene.render(1, 1, 0).unwrap();
    assert_eq!(scene.stats.admitted, 1);
    assert_eq!(scene.last_sequence(), 4);
    assert!(scene.pending.is_empty());
    assert_eq!(scene.active(), 1);
}
