use super::*;
use crate::{
    quaternion_animation::PortableQuaternionMath, skeletal_animation::AnimationName,
    skeletal_channel::PortableChannelAngularMath, skeletal_track::AnimationTrack,
};
fn sequence() -> StoredSequence {
    StoredSequence {
        payload_offset: 0,
        end_offset: 0,
        word_2c: 0,
        name: AnimationName {
            index: 0,
            name: "Idle".into(),
        },
        groups: vec![],
        first_frame: 0,
        frames: 10,
        notifies: vec![],
        rate_bits: 1f32.to_bits(),
        word_18: 0,
        minimum_blend_bits: 0,
        randomize_start: 0,
        word_30: 0,
        word_50: 0,
        flags: 0,
        word_58: 0,
        tracks: vec![AnimationTrack {
            rotations: vec![[0; 3]],
            rotation_count_word: 1,
            positions: vec![[32767, 0, 0]],
            position_count_word: 1,
            position_scale_bits: 1f32.to_bits(),
            durations: vec![10],
            duration_count_word: 1,
        }],
    }
}
fn pose() -> RootTransform {
    RootTransform {
        rotation: [0, 0, 0, 1f32.to_bits()],
        position: [0; 3],
    }
}
fn data<'a>(
    catalog: &'a [ResolvedSequence<'a>],
    names: &'a [(u32, u32)],
) -> RuntimeAnimationBindings<'a> {
    RuntimeAnimationBindings {
        sequences: catalog,
        names,
        linkups: Rc::new(RefCell::new(vec![ResolvedLinkup {
            key: 7,
            animation_names: Some(vec![11]),
            mapping: vec![0],
        }])),
        events: Rc::new(RefCell::new(vec![])),
    }
}
fn host<'a>(
    data: RuntimeAnimationBindings<'a>,
    reference: &'a [RootTransform],
) -> RuntimePoseHost<'a, PortableQuaternionMath, PortableChannelAngularMath> {
    RuntimePoseHost {
        data,
        reference,
        move_bone: -1,
        editor: false,
        math: PortableQuaternionMath,
        angular: PortableChannelAngularMath,
    }
}
fn channel() -> AnimationChannel {
    let mut c = AnimationChannel::default();
    c.words[0] = 11;
    c.words[17] = 1;
    c.words[16] = 1;
    c.words[11] = 1f32.to_bits();
    c.words[7] = 0.5f32.to_bits();
    c
}
#[test]
fn cached_identity_selects_loaded_sequence_without_name_lookup() {
    let s = sequence();
    let catalog = [ResolvedSequence {
        identity: 1,
        linkup_key: 7,
        sequence: &s,
    }];
    let reference = [pose()];
    let d = data(&catalog, &[]);
    let mut h = host(d.clone(), &reference);
    let mut cs = [channel()];
    let selected = h.sequence(0, &mut cs).unwrap().unwrap();
    assert_eq!(selected.token, 1);
    assert_eq!(h.root_track(selected).unwrap(), 0);
    assert!(!d
        .events
        .borrow()
        .iter()
        .any(|e| matches!(e, RuntimeAnimationEvent::Lookup { .. })));
}
#[test]
fn editor_name_lookup_overwrites_invalid_cached_identity() {
    let s = sequence();
    let catalog = [ResolvedSequence {
        identity: 1,
        linkup_key: 7,
        sequence: &s,
    }];
    let reference = [pose()];
    let names = [(11, 1), (11, 99)];
    let d = data(&catalog, &names);
    let mut h = host(d.clone(), &reference);
    h.editor = true;
    let mut cs = [channel()];
    cs[0].words[17] = 99;
    assert_eq!(h.sequence(0, &mut cs).unwrap().unwrap().token, 1);
    assert_eq!(cs[0].words[17], 1);
    assert!(d.events.borrow().iter().any(|e| matches!(
        e,
        RuntimeAnimationEvent::Lookup {
            name: 11,
            load: false,
            result: 1
        }
    )));
}
#[test]
fn unknown_nonnull_identity_errors_before_channel_history_or_scratch() {
    let s = sequence();
    let catalog = [ResolvedSequence {
        identity: 1,
        linkup_key: 7,
        sequence: &s,
    }];
    let reference = [pose()];
    let mut h = host(data(&catalog, &[]), &reference);
    let mut cs = [channel()];
    cs[0].words[17] = 99;
    cs[0].words[12] = 123;
    let mut out = [RootTransform {
        rotation: [9; 4],
        position: [9; 3],
    }];
    assert!(h.apply(0, &mut cs, &reference, &mut out).is_err());
    assert_eq!(cs[0].words[12], 123);
    assert_eq!(out[0].position, [9; 3]);
}
#[test]
fn null_sequence_keeps_scratch_but_updates_channel_history() {
    let reference = [pose()];
    let mut h = host(data(&[], &[]), &reference);
    let mut cs = [channel()];
    cs[0].words[17] = 0;
    let mut out = [RootTransform {
        rotation: [9; 4],
        position: [9; 3],
    }];
    assert!(!h.apply(0, &mut cs, &reference, &mut out).unwrap());
    assert_eq!(cs[0].words[12], cs[0].words[7]);
    assert_eq!(cs[0].words[13], cs[0].words[11]);
    assert_eq!(out[0].position, [9; 3]);
}
#[test]
fn preparation_rebuild_is_seen_by_pose_host_and_duplicate_names_use_first() {
    let s = sequence();
    let catalog = [ResolvedSequence {
        identity: 1,
        linkup_key: 7,
        sequence: &s,
    }];
    let reference = [pose(), pose()];
    let d = data(&catalog, &[]);
    d.linkups.borrow_mut()[0] = ResolvedLinkup {
        key: 7,
        animation_names: Some(vec![22, 11, 11]),
        mapping: vec![],
    };
    let mut p = RuntimePreparationHost {
        data: d.clone(),
        mesh_names: &[11, 22],
        mesh_to_world: [0; 16],
    };
    p.refresh_linkup(0).unwrap();
    assert_eq!(d.linkups.borrow()[0].mapping, [1, 0]);
    let mut h = host(d.clone(), &reference);
    let mut cs = [channel()];
    let selected = h.sequence(0, &mut cs).unwrap().unwrap();
    assert_eq!(h.root_track(selected).unwrap(), 1);
    assert!(matches!(
        &d.events.borrow()[0],
        RuntimeAnimationEvent::Refresh {
            result: LinkupRefresh::Rebuilt {
                matched: 2,
                warning: false
            },
            ..
        }
    ));
}
#[test]
fn same_length_cache_survives_missing_animation_and_short_missing_mapping_stays() {
    let d = data(&[], &[]);
    d.linkups.borrow_mut()[0] = ResolvedLinkup {
        key: 7,
        animation_names: None,
        mapping: vec![99],
    };
    let mut p = RuntimePreparationHost {
        data: d.clone(),
        mesh_names: &[11],
        mesh_to_world: [0; 16],
    };
    p.refresh_linkup(0).unwrap();
    assert_eq!(d.linkups.borrow()[0].mapping, [99]);
    p.mesh_names = &[11, 22];
    p.refresh_linkup(0).unwrap();
    assert_eq!(d.linkups.borrow()[0].mapping, [99]);
    let e = d.events.borrow();
    assert!(matches!(
        &e[0],
        RuntimeAnimationEvent::Refresh {
            result: LinkupRefresh::SameLength,
            ..
        }
    ));
    assert!(matches!(
        &e[1],
        RuntimeAnimationEvent::Refresh {
            result: LinkupRefresh::MissingAnimation,
            ..
        }
    ));
}
#[test]
fn absent_linkup_errors_and_first_duplicate_key_wins() {
    let s = sequence();
    let catalog = [ResolvedSequence {
        identity: 1,
        linkup_key: 7,
        sequence: &s,
    }];
    let reference = [pose()];
    let d = data(&catalog, &[]);
    d.linkups.borrow_mut().push(ResolvedLinkup {
        key: 7,
        animation_names: None,
        mapping: vec![99],
    });
    let mut h = host(d.clone(), &reference);
    let mut cs = [channel()];
    let selected = h.sequence(0, &mut cs).unwrap().unwrap();
    assert_eq!(h.root_track(selected).unwrap(), 0);
    d.linkups.borrow_mut().clear();
    assert!(h.root_track(selected).is_err());
    assert!(h.apply(0, &mut cs, &reference, &mut [pose()]).is_err());
}
#[test]
fn root_track_failure_keeps_committed_quaternion_before_position_error() {
    let mut s = sequence();
    s.tracks[0].positions.clear();
    let catalog = [ResolvedSequence {
        identity: 1,
        linkup_key: 7,
        sequence: &s,
    }];
    let reference = [pose()];
    let mut h = host(data(&catalog, &[]), &reference);
    let mut out = RootTransform {
        rotation: [9; 4],
        position: [9; 3],
    };
    assert!(h
        .sample(
            RootSampleRequest {
                channel: 0,
                sequence_token: 1,
                track: 0,
                frames: 10,
                normalized_frame: 0.5
            },
            &mut out
        )
        .is_err());
    assert_eq!(out.rotation, pose().rotation);
    assert_eq!(out.position, [9; 3]);
}
