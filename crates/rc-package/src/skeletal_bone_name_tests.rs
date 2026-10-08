use crate::{
    mesh_animation::{BoneAlias, ReferenceBone, SkeletonSnapshot},
    skeletal_set_bone_place::{
        set_bone_place, BonePlaceDefaults, BonePlaceNames, SnapshotBonePlaceNames,
    },
};
fn skeleton() -> SkeletonSnapshot {
    SkeletonSnapshot {
        bones: vec![
            ReferenceBone {
                name_handle: 20,
                word_38: 0,
            },
            ReferenceBone {
                name_handle: 30,
                word_38: 0,
            },
            ReferenceBone {
                name_handle: 20,
                word_38: 0,
            },
        ],
        aliases: vec![
            BoneAlias {
                name_handle: 40,
                target_handle: 30,
            },
            BoneAlias {
                name_handle: 40,
                target_handle: 20,
            },
            BoneAlias {
                name_handle: 50,
                target_handle: 40,
            },
        ],
    }
}
#[test]
fn identity_alias_first_match_and_chain_are_shared_with_existing_channel_search() {
    let s = skeleton();
    let mut names = SnapshotBonePlaceNames {
        global_names: &[0, 20, 40, 50, 20],
        skeleton: &s,
    };
    assert_eq!(names.match_name(1).unwrap(), 0);
    assert_eq!(names.match_name(2).unwrap(), 1);
    assert_eq!(names.match_name(3).unwrap(), -1);
    assert_eq!(names.match_name(4).unwrap(), 0);
}
#[test]
fn matched_alias_matrix_is_copied_even_when_one_pass_target_has_no_bone() {
    let s = skeleton();
    let mut out = [99; 16];
    let matrices = [[1; 16], [2; 16], [3; 16]];
    assert_eq!(
        s.match_ref_bone_with_matrix(40, &matrices, Some(&mut out))
            .unwrap(),
        Some(1)
    );
    assert_eq!(out, [1; 16]);
    assert_eq!(
        s.match_ref_bone_with_matrix(50, &matrices, Some(&mut out))
            .unwrap(),
        None
    );
    assert_eq!(out, [3; 16]);
}
#[test]
fn none_and_unaliased_names_leave_matrix_untouched() {
    let mut s = skeleton();
    s.aliases.push(BoneAlias {
        name_handle: 0,
        target_handle: 20,
    });
    let mut out = [99; 16];
    assert_eq!(
        s.match_ref_bone_with_matrix(0, &[], Some(&mut out))
            .unwrap(),
        None
    );
    assert_eq!(out, [99; 16]);
    assert_eq!(
        s.match_ref_bone_with_matrix(20, &[], Some(&mut out))
            .unwrap(),
        Some(0)
    );
    assert_eq!(out, [99; 16]);
}
#[test]
fn missing_matrix_fails_only_when_a_matching_alias_output_was_requested() {
    let s = skeleton();
    assert_eq!(
        s.match_ref_bone_with_matrix(40, &[], None).unwrap(),
        Some(1)
    );
    let mut out = [99; 16];
    assert!(s
        .match_ref_bone_with_matrix(40, &[], Some(&mut out))
        .is_err());
    assert_eq!(out, [99; 16]);
}
fn defaults() -> BonePlaceDefaults {
    BonePlaceDefaults {
        identity_quaternion: [0, 0, 0, 1f32.to_bits()],
        append_padding: [0; 3],
    }
}
#[test]
fn global_table_failure_preserves_cache_before_target_but_after_start_invalidates() {
    let s = skeleton();
    let mut names = SnapshotBonePlaceNames {
        global_names: &[0, 20],
        skeleton: &s,
    };
    for start in [false, true] {
        let mut input = [0; 22];
        input[0] = if start { 0x80000001 } else { 0x80000002 };
        input[1] = 0x80000002;
        let mut cache = 7;
        let mut tracks = vec![];
        assert!(
            set_bone_place(input, 3, &mut cache, &mut tracks, &defaults(), &mut names).is_err()
        );
        assert_eq!(cache, if start { 0 } else { 7 });
        assert!(tracks.is_empty());
    }
    assert!(names.match_name(-1).is_err());
}
#[test]
fn named_alias_update_reuses_first_director_and_keeps_history() {
    let s = skeleton();
    let mut names = SnapshotBonePlaceNames {
        global_names: &[0, 40, 999],
        skeleton: &s,
    };
    let mut input = [0; 22];
    input[0] = 0x80000001;
    input[1] = 0x80000002;
    let mut tracks = vec![];
    let mut cache = 7;
    assert!(set_bone_place(input, 3, &mut cache, &mut tracks, &defaults(), &mut names).unwrap());
    assert_eq!(tracks[0].words[0], 1);
    assert_eq!(tracks[0].words[1], 1);
    tracks[0].words[22..28].copy_from_slice(&[101, 102, 103, 104, 105, 106]);
    input[18] = 0x101;
    assert!(set_bone_place(input, 3, &mut cache, &mut tracks, &defaults(), &mut names).unwrap());
    assert_eq!(tracks.len(), 1);
    assert_eq!(tracks[0].words[18], 0x101);
    assert_eq!(&tracks[0].words[22..], &[101, 102, 103, 104, 105, 106]);
}
