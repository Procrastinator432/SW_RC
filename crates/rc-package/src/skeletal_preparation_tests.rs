use super::*;
fn buffers(q: usize, p: usize, m: usize) -> InstanceAnimationBuffers {
    InstanceAnimationBuffers {
        rotations: vec![[0x7fc12345, 0x80000000, 33, 44]; q],
        positions: vec![[11, 22, 33]; p],
        matrices: vec![[55; 16]; m],
        mesh_to_world: [66; 16],
    }
}
#[test]
fn matching_counts_preserve_bits_and_cache() {
    let mut s = buffers(2, 2, 2);
    let mut b = 7;
    assert_eq!(
        prepare_instance_buffers(&mut s, 2, &mut b),
        BufferPreparation {
            local_resized: false,
            matrices_resized: false
        }
    );
    assert_eq!(b, 7);
    assert_eq!(s.rotations[0], [0x7fc12345, 0x80000000, 33, 44]);
    assert_eq!(s.mesh_to_world, [66; 16]);
}
#[test]
fn unequal_local_counts_resize_both_without_resetting_existing_values() {
    let mut s = buffers(2, 1, 3);
    let mut b = 7;
    assert_eq!(
        prepare_instance_buffers(&mut s, 3, &mut b),
        BufferPreparation {
            local_resized: true,
            matrices_resized: false
        }
    );
    assert_eq!(s.rotations[1], [0x7fc12345, 0x80000000, 33, 44]);
    assert_eq!(s.rotations[2], [0; 4]);
    assert_eq!(s.positions, vec![[11, 22, 33], [0; 3], [0; 3]]);
    assert_eq!(s.matrices[2], [55; 16]);
    assert_eq!(b, 0);
}
#[test]
fn matrix_mismatch_alone_invalidates_and_zero_extends() {
    let mut s = buffers(3, 3, 1);
    let mut b = 7;
    assert_eq!(
        prepare_instance_buffers(&mut s, 3, &mut b),
        BufferPreparation {
            local_resized: false,
            matrices_resized: true
        }
    );
    assert_eq!(s.matrices, vec![[55; 16], [0; 16], [0; 16]]);
    assert_eq!(b, 0);
}
#[test]
fn shrinking_and_empty_mesh_preserve_prefix_then_remove_contents() {
    let mut s = buffers(4, 3, 5);
    let mut b = 7;
    prepare_instance_buffers(&mut s, 1, &mut b);
    assert_eq!(s.positions, vec![[11, 22, 33]]);
    assert_eq!(s.matrices, vec![[55; 16]]);
    prepare_instance_buffers(&mut s, 0, &mut b);
    assert!(s.rotations.is_empty() && s.positions.is_empty() && s.matrices.is_empty());
    assert_eq!(b, 0);
}
#[test]
fn scratch_gate_does_not_repair_short_positions_or_shrink_large_scratch() {
    let mut s = ChannelScratchBuffers {
        rotations: vec![[11; 4]; 4],
        positions: vec![],
    };
    assert!(!prepare_channel_scratch(&mut s, 3));
    assert!(s.positions.is_empty());
    assert_eq!(s.rotations.len(), 4);
    assert!(!prepare_channel_scratch(&mut s, 0));
}
#[test]
fn scratch_growth_preserves_rotations_and_can_shrink_positions() {
    let mut s = ChannelScratchBuffers {
        rotations: vec![[11; 4]],
        positions: vec![[22; 3]; 5],
    };
    assert!(prepare_channel_scratch(&mut s, 3));
    assert_eq!(s.rotations, vec![[11; 4], [0; 4], [0; 4]]);
    assert_eq!(s.positions, vec![[22; 3]; 3]);
}
struct Host {
    events: Vec<String>,
    transform_error: bool,
    link_error: Option<usize>,
}
impl AnimationPreparationHost for Host {
    fn mesh_to_world(&mut self) -> Result<[u32; 16], String> {
        self.events.push("transform".into());
        if self.transform_error {
            Err("transform boundary".into())
        } else {
            Ok([77; 16])
        }
    }
    fn refresh_linkup(&mut self, i: usize) -> Result<(), String> {
        self.events.push(format!("linkup {i}"));
        if self.link_error == Some(i) {
            Err("linkup boundary".into())
        } else {
            Ok(())
        }
    }
}
fn bone(parent: i32) -> StoredBone {
    StoredBone {
        name: crate::skeletal_animation::AnimationName {
            index: 0,
            name: "Root".into(),
        },
        flags: 0,
        rotation: [0, 0, 0, 1f32.to_bits()],
        position: [0; 3],
        word_24: 0,
        word_28_first: 0,
        word_28: 0,
        word_30: 0,
        word_38: 0,
        word_34: parent,
    }
}
#[test]
fn preparation_runs_transform_and_linkups_even_when_inverse_and_pose_cached() {
    let mut s = buffers(1, 1, 1);
    let mut b = 7;
    let mut inv = vec![[88; 16]];
    let mut h = Host {
        events: vec![],
        transform_error: false,
        link_error: None,
    };
    let r = prepare_animation_instance(&mut s, &[bone(-1)], &mut inv, &mut b, 2, &mut h).unwrap();
    assert!(!r.inverse_built);
    assert_eq!(b, 7);
    assert_eq!(inv, vec![[88; 16]]);
    assert_eq!(h.events, ["transform", "linkup 0", "linkup 1"]);
    assert_eq!(s.mesh_to_world, [77; 16]);
}
#[test]
fn transform_error_retains_buffer_changes_but_old_transform_and_inverse() {
    let mut s = buffers(0, 0, 0);
    let mut b = 7;
    let mut inv = vec![];
    let mut h = Host {
        events: vec![],
        transform_error: true,
        link_error: None,
    };
    assert!(prepare_animation_instance(&mut s, &[bone(-1)], &mut inv, &mut b, 2, &mut h).is_err());
    assert_eq!(s.rotations, vec![[0; 4]]);
    assert_eq!(s.mesh_to_world, [66; 16]);
    assert!(inv.is_empty());
    assert_eq!(b, 0);
    assert_eq!(h.events, ["transform"]);
}
#[test]
fn inverse_and_linkup_failures_keep_their_ordered_partial_writes() {
    let mut s = buffers(0, 0, 0);
    let mut b = 7;
    let mut inv = vec![];
    let mut h = Host {
        events: vec![],
        transform_error: false,
        link_error: Some(1),
    };
    assert!(
        prepare_animation_instance(&mut s, &[bone(-1), bone(2)], &mut inv, &mut b, 3, &mut h)
            .is_err()
    );
    assert_eq!(h.events, ["transform"]);
    assert_eq!(inv.len(), 2);
    assert_eq!(inv[1], [0; 16]);
    assert_eq!(s.mesh_to_world, [77; 16]);
    inv.clear();
    h.events.clear();
    assert!(prepare_animation_instance(&mut s, &[bone(-1)], &mut inv, &mut b, 3, &mut h).is_err());
    assert_eq!(h.events, ["transform", "linkup 0", "linkup 1"]);
    assert_eq!(inv.len(), 1);
    assert_eq!(inv[0][0], 1f32.to_bits());
    assert_eq!(b, 0);
}
