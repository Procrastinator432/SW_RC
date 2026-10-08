use super::*;
use crate::{
    skeletal_animation::{AnimationName, AnimationNotify, StoredSequence},
    skeletal_runtime_hosts::ResolvedSequence,
};
use std::{cell::RefCell, rc::Rc};
fn sequence() -> StoredSequence {
    StoredSequence {
        payload_offset: 0,
        end_offset: 0,
        word_2c: 0,
        name: AnimationName {
            index: 1,
            name: "Idle".into(),
        },
        groups: vec![],
        first_frame: 0,
        frames: 30,
        notifies: vec![AnimationNotify {
            time_bits: (-0.0f32).to_bits(),
            name: AnimationName {
                index: 2,
                name: "Event".into(),
            },
            object_index: -7,
        }],
        rate_bits: 24f32.to_bits(),
        word_18: 0.5f32.to_bits(),
        minimum_blend_bits: 0,
        randomize_start: 0,
        word_30: 0,
        word_50: 0,
        flags: 0,
        word_58: 0,
        tracks: vec![],
    }
}
fn host<'a>(
    catalog: &'a [ResolvedSequence<'a>],
    bindings: &'a [NotifyObjectBinding],
) -> RuntimeTickHost<'a, ()> {
    RuntimeTickHost {
        data: RuntimeAnimationBindings {
            sequences: catalog,
            names: &[(10, 1)],
            linkups: Rc::new(RefCell::new(vec![])),
            events: Rc::new(RefCell::new(vec![])),
        },
        notify_objects: bindings,
        environment: (),
    }
}
#[test]
fn metadata_and_notify_words_are_copied_from_loaded_sequence() {
    let s = sequence();
    let catalog = [ResolvedSequence {
        identity: 1,
        linkup_key: 7,
        sequence: &s,
    }];
    let bindings = [NotifyObjectBinding {
        sequence_identity: 1,
        object_index: -7,
        identity: 99,
    }];
    let loaded = host(&catalog, &bindings).loaded_sequence(1).unwrap();
    assert_eq!(loaded.frames, 30);
    assert_eq!(loaded.rate.to_bits(), s.rate_bits);
    assert_eq!(loaded.jitter_amplitude.to_bits(), s.word_18);
    assert_eq!(loaded.notifies[0].time_bits, (-0.0f32).to_bits());
    assert_eq!(loaded.notifies[0].object, 99);
}
#[test]
fn null_archive_notify_requires_no_binding_and_ignores_spurious_binding() {
    let mut s = sequence();
    s.notifies[0].object_index = 0;
    let catalog = [ResolvedSequence {
        identity: 1,
        linkup_key: 7,
        sequence: &s,
    }];
    let bindings = [NotifyObjectBinding {
        sequence_identity: 1,
        object_index: 0,
        identity: 99,
    }];
    assert_eq!(
        host(&catalog, &bindings)
            .loaded_sequence(1)
            .unwrap()
            .notifies[0]
            .object,
        0
    );
}
#[test]
fn unresolved_nonnull_and_null_resolutions_fail_explicitly() {
    let s = sequence();
    let catalog = [ResolvedSequence {
        identity: 1,
        linkup_key: 7,
        sequence: &s,
    }];
    assert!(host(&catalog, &[]).loaded_sequence(1).is_err());
    let bindings = [NotifyObjectBinding {
        sequence_identity: 1,
        object_index: -7,
        identity: 0,
    }];
    assert!(host(&catalog, &bindings).loaded_sequence(1).is_err());
}
#[test]
fn notify_bindings_are_scoped_to_sequence_and_first_match_wins() {
    let s = sequence();
    let catalog = [ResolvedSequence {
        identity: 1,
        linkup_key: 7,
        sequence: &s,
    }];
    let bindings = [
        NotifyObjectBinding {
            sequence_identity: 2,
            object_index: -7,
            identity: 55,
        },
        NotifyObjectBinding {
            sequence_identity: 1,
            object_index: -7,
            identity: 77,
        },
        NotifyObjectBinding {
            sequence_identity: 1,
            object_index: -7,
            identity: 88,
        },
    ];
    assert!(host(&catalog, &bindings[..1]).loaded_sequence(1).is_err());
    assert_eq!(
        host(&catalog, &bindings)
            .loaded_sequence(1)
            .unwrap()
            .notifies[0]
            .object,
        77
    );
}
#[test]
fn sequence_lookup_uses_same_name_bindings_as_pose_host() {
    let mut h = host(&[], &[]);
    assert_eq!(h.find_sequence(10, false).unwrap(), 1);
    assert_eq!(h.find_sequence(11, false).unwrap(), 0);
    assert!(h.loaded_sequence(0).is_err());
    assert!(h.loaded_sequence(99).is_err());
}
#[test]
fn null_sequence_catalog_entry_does_not_override_null_semantics() {
    let s = sequence();
    let catalog = [ResolvedSequence {
        identity: 0,
        linkup_key: 7,
        sequence: &s,
    }];
    assert!(host(&catalog, &[]).loaded_sequence(0).is_err());
}
