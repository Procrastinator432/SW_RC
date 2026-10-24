use super::*;

fn speech(sequence: u64, tick: u64, owner: u32, protected: bool) -> Vec<u8> {
    let mut bytes = event(sequence, tick, 1);
    bytes[32..36].copy_from_slice(&owner.to_le_bytes());
    bytes[40..44].copy_from_slice(&u32::from(protected).to_le_bytes());
    bytes
}
fn first(protected: bool) -> Scene<Fake> {
    let mut scene = scene();
    scene
        .update(&speech(1, 0, 42, protected), &loops(false, 0))
        .unwrap();
    scene.render(480, 1, 0).unwrap();
    scene
}
#[test]
fn existing_owner_protection_controls_all_four_flag_combinations() {
    for old in [false, true] {
        for incoming in [false, true] {
            let mut scene = first(old);
            scene
                .update(&speech(2, 0, 42, incoming), &loops(false, 0))
                .unwrap();
            scene.render(480, 1, 1).unwrap();
            let voices: Vec<_> = scene.voices.iter().flatten().collect();
            assert_eq!(voices.len(), 1);
            assert_eq!(voices[0].age, if old { 1 } else { 2 });
            assert_eq!(voices[0].no_override, if old { true } else { incoming });
            assert_eq!(scene.last_sequence(), 2);
            assert!(scene.pending.is_empty());
        }
    }
}
#[test]
fn different_owners_can_speak_while_one_is_protected() {
    let mut scene = first(true);
    scene
        .update(&speech(2, 0, 43, false), &loops(false, 0))
        .unwrap();
    scene.render(480, 1, 1).unwrap();
    assert_eq!(scene.active(), 2);
    assert!(scene
        .voices
        .iter()
        .flatten()
        .any(|v| v.owner == 42 && v.age == 1 && v.no_override));
}
#[test]
fn effect_categories_do_not_use_speech_owner_replacement() {
    for category in [0u32, 2] {
        let mut scene = first(true);
        let mut request = speech(2, 0, 42, true);
        request[36..40].copy_from_slice(&category.to_le_bytes());
        scene.update(&request, &loops(false, 0)).unwrap();
        scene.render(480, 1, 1).unwrap();
        assert_eq!(scene.active(), 2);
        assert!(scene
            .voices
            .iter()
            .flatten()
            .any(|v| v.category == 1 && v.age == 1));
    }
}
#[test]
fn completed_protected_source_does_not_block_new_speech() {
    let mut scene = first(true);
    scene.apply(&[Op::Stop(1)]).unwrap(); // Controlled physical AL_STOPPED observation.
    scene
        .update(&speech(2, 0, 42, false), &loops(false, 0))
        .unwrap();
    scene.render(480, 1, 1).unwrap();
    assert_eq!(scene.active(), 1);
    assert_eq!(scene.voices.iter().flatten().next().unwrap().age, 2);
}
#[test]
fn rejected_speech_is_consumed_once_and_preserves_pending_tick_spacing() {
    let mut scene = first(true);
    scene
        .update(&speech(2, 1, 42, false), &loops(false, 0))
        .unwrap();
    scene
        .update(&speech(3, 2, 43, false), &loops(false, 0))
        .unwrap();
    scene.render(320, 1, 1).unwrap();
    assert_eq!(scene.pending.len(), 2);
    scene.render(480, 1, 2).unwrap();
    assert_eq!(scene.pending.len(), 1);
    assert_eq!(scene.active(), 1);
    assert_eq!(scene.last_played, Some((1, 800)));
    scene.render(320, 1, 3).unwrap();
    assert_eq!(scene.active(), 1);
    scene.render(1, 1, 4).unwrap();
    assert_eq!(scene.active(), 2);
    assert!(scene.pending.is_empty());
    assert_eq!(scene.last_sequence(), 3);
}
#[test]
fn active_owner_protection_survives_context_restore() {
    let mut old = first(true);
    let saved = old.snapshot().unwrap();
    assert_eq!(unpack(&saved)["dialogs"][0][0]["no_override"], true);
    let mut next = scene();
    next.restore(&saved).unwrap();
    next.update(&speech(2, 0, 42, false), &loops(false, 0))
        .unwrap();
    next.render(480, 1, 0).unwrap();
    assert_eq!(next.active(), 1);
    assert_eq!(next.voices.iter().flatten().next().unwrap().age, 1);
    assert_eq!(next.last_sequence(), 2);
}
#[test]
fn pending_protection_survives_restore_before_first_play() {
    let mut old = scene();
    old.update(&speech(1, 0, 42, true), &loops(false, 0))
        .unwrap();
    let saved = old.snapshot().unwrap();
    assert_eq!(unpack(&saved)["pending"][0]["no_override"], true);
    let mut next = scene();
    next.restore(&saved).unwrap();
    next.render(480, 1, 0).unwrap();
    assert!(next.voices.iter().flatten().next().unwrap().no_override);
}
#[test]
fn legacy_audio_snapshots_without_protection_field_remain_readable() {
    let mut old = first(true);
    old.update(&speech(2, 10, 43, true), &loops(false, 0))
        .unwrap();
    let mut value = unpack(&old.snapshot().unwrap());
    value["dialogs"][0][0]
        .as_object_mut()
        .unwrap()
        .remove("no_override");
    value["pending"][0]
        .as_object_mut()
        .unwrap()
        .remove("no_override");
    let mut next = scene();
    next.restore(&encode(value)).unwrap();
    assert!(!next.voices.iter().flatten().next().unwrap().no_override);
    assert!(!next.pending[0].no_override);
}
#[test]
fn non_boolean_saved_protection_is_rejected_before_backend_io() {
    let mut old = first(true);
    let mut value = unpack(&old.snapshot().unwrap());
    value["dialogs"][0][0]["no_override"] = serde_json::json!(2);
    let mut next = scene();
    let count = next.device.backend().model.calls().len();
    assert!(next.restore(&encode(value)).is_err());
    assert_eq!(next.device.backend().model.calls().len(), count);
}
