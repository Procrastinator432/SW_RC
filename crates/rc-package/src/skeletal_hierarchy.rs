//! PostLoad hierarchy preparation and ApplyAnimation parent-matrix stage.
//! Supplied local pose; no channel blend, director, bounds or skinning stage.
use crate::{
    skeletal_mesh::StoredBone,
    skeletal_root_pose::{quaternion_translation_matrix, RootTransform},
};
use serde::Serialize;
#[derive(Debug, Serialize)]
pub struct BoneHierarchy {
    pub parents: Vec<i32>,
    pub descendants: Vec<i32>,
    pub depths: Vec<i32>,
    pub move_bone: i32,
}
pub fn prepare_hierarchy(bones: &[StoredBone]) -> Result<BoneHierarchy, String> {
    let mut parents: Vec<_> = bones.iter().map(|b| b.word_34).collect();
    let move_bone = if bones
        .first()
        .is_some_and(|b| b.name.name.eq_ignore_ascii_case("Move"))
    {
        0
    } else {
        -1
    };
    if let Some(root) = parents.first_mut() {
        *root = -1;
    }
    let mut descendants = vec![0; parents.len()];
    let mut depths = vec![0; parents.len()];
    for i in 0..parents.len() {
        let mut parent = parents[i];
        while parent >= 0 {
            let p = parent as usize;
            if p >= i {
                return Err(format!("parent {p} must precede bone {i}"));
            }
            depths[i] += 1;
            descendants[p] += 1;
            parent = parents[p];
        }
    }
    Ok(BoneHierarchy {
        parents,
        descendants,
        depths,
        move_bone,
    })
}

/// Native 1050a7c5..1050ab60: output-specific SSE summation order.
pub fn compose_bone_matrix(local: [u32; 16], parent: [u32; 16]) -> [u32; 16] {
    const ORDER: [[usize; 4]; 16] = [
        [2, 1, 0, 3],
        [3, 0, 1, 2],
        [2, 0, 3, 1],
        [1, 0, 3, 2],
        [0, 2, 3, 1],
        [3, 1, 2, 0],
        [3, 1, 2, 0],
        [3, 2, 1, 0],
        [0, 2, 3, 1],
        [3, 1, 2, 0],
        [3, 1, 2, 0],
        [3, 2, 1, 0],
        [0, 2, 3, 1],
        [1, 3, 2, 0],
        [3, 1, 2, 0],
        [3, 2, 1, 0],
    ];
    let a = local.map(f32::from_bits);
    let b = parent.map(f32::from_bits);
    std::array::from_fn(|index| {
        let row = index / 4;
        let col = index % 4;
        let [i, j, k, l] = ORDER[index];
        (((a[row * 4 + i] * b[i * 4 + col] + a[row * 4 + j] * b[j * 4 + col])
            + a[row * 4 + k] * b[k * 4 + col])
            + a[row * 4 + l] * b[l * 4 + col])
            .to_bits()
    })
}
/// Writes each local matrix before its parent lookup, preserving prior output on error.
/// Parent-forward/cyclic snapshots are rejected instead of reading stale native buffers.
pub fn build_pose_matrices(
    local: &[RootTransform],
    hierarchy: &BoneHierarchy,
    editor: bool,
    output: &mut Vec<[u32; 16]>,
) -> Result<(), String> {
    build_pose_matrices_with_hook(local, hierarchy, editor, output, |_, _| Ok(()))
}
/// Hook runs after each parent composition, before the next bone uses that matrix.
/// Native directors can also modify earlier matrices; callers must collect bounds
/// in this hook if their backend does so, rather than recomputing them afterwards.
pub fn build_pose_matrices_with_hook(
    local: &[RootTransform],
    hierarchy: &BoneHierarchy,
    editor: bool,
    output: &mut Vec<[u32; 16]>,
    mut hook: impl FnMut(usize, &mut [[u32; 16]]) -> Result<(), String>,
) -> Result<(), String> {
    if local.len() != hierarchy.parents.len() {
        return Err("local pose/hierarchy length mismatch".into());
    }
    output.resize(local.len(), [0; 16]);
    for (i, transform) in local.iter().enumerate() {
        output[i] = quaternion_translation_matrix(*transform);
        let p = hierarchy.parents[i];
        if i > 0 && (p != hierarchy.move_bone || editor) {
            let index = usize::try_from(p).map_err(|_| format!("negative parent at bone {i}"))?;
            if index >= i {
                return Err(format!("parent {index} must precede bone {i}"));
            }
            output[i] = compose_bone_matrix(output[i], output[index]);
        }
        hook(i, output)?;
    }
    Ok(())
}
#[cfg(test)]
mod tests {
    use super::*;
    fn bone(name: &str, parent: i32) -> StoredBone {
        StoredBone {
            name: crate::skeletal_animation::AnimationName {
                index: 0,
                name: name.into(),
            },
            flags: 0,
            rotation: [0, 0, 0, 1f32.to_bits()],
            position: [0; 3],
            word_24: 0,
            word_28_first: 0,
            word_28: 0,
            word_30: 0,
            word_38: 99,
            word_34: parent,
        }
    }
    fn transform(x: f32) -> RootTransform {
        RootTransform {
            rotation: [0, 0, 0, 1f32.to_bits()],
            position: [x.to_bits(), 0, 0],
        }
    }
    #[test]
    fn postload_root_move_depth_and_descendants() {
        let h = prepare_hierarchy(&[bone("mOvE", 42), bone("A", 0), bone("B", 1), bone("C", 0)])
            .unwrap();
        assert_eq!(h.parents, [-1, 0, 1, 0]);
        assert_eq!(h.depths, [0, 1, 2, 1]);
        assert_eq!(h.descendants, [3, 1, 0, 0]);
        assert_eq!(h.move_bone, 0);
        assert_eq!(prepare_hierarchy(&[bone("Root", 0)]).unwrap().move_bone, -1);
    }
    #[test]
    fn invalid_parent_and_cycles_are_explicit() {
        assert!(prepare_hierarchy(&[bone("Root", 0), bone("A", 2), bone("B", 1)]).is_err());
        assert!(prepare_hierarchy(&[bone("Root", 0), bone("A", 1)]).is_err());
        assert!(prepare_hierarchy(&[]).unwrap().parents.is_empty());
    }
    #[test]
    fn move_children_detach_only_outside_editor() {
        let h = prepare_hierarchy(&[bone("Move", 0), bone("A", 0), bone("B", 1)]).unwrap();
        let local = [transform(10.), transform(2.), transform(3.)];
        let mut m = vec![];
        build_pose_matrices(&local, &h, false, &mut m).unwrap();
        assert_eq!(
            m.iter().map(|a| f32::from_bits(a[12])).collect::<Vec<_>>(),
            [10., 2., 5.]
        );
        build_pose_matrices(&local, &h, true, &mut m).unwrap();
        assert_eq!(
            m.iter().map(|a| f32::from_bits(a[12])).collect::<Vec<_>>(),
            [10., 12., 15.]
        );
    }
    #[test]
    fn ordinary_root_chain_and_partial_output_error() {
        let mut h = prepare_hierarchy(&[bone("Root", 0), bone("A", 0), bone("B", 1)]).unwrap();
        let local = [transform(10.), transform(2.), transform(3.)];
        let mut out = vec![];
        build_pose_matrices(&local, &h, false, &mut out).unwrap();
        assert_eq!(f32::from_bits(out[2][12]), 15.);
        h.parents[1] = 9;
        out.fill([7; 16]);
        assert!(build_pose_matrices(&local, &h, false, &mut out).is_err());
        assert_eq!(out[0], quaternion_translation_matrix(local[0]));
        assert_eq!(out[1], quaternion_translation_matrix(local[1]));
        assert_eq!(out[2], [7; 16]);
    }
    #[test]
    fn composition_order_preserves_original_cancellation() {
        let mut a = [0f32; 16];
        let mut b = [0f32; 16];
        a[0] = 1.;
        a[1] = 1.;
        a[2] = 1.;
        a[3] = 1.;
        b[0] = 1e20;
        b[4] = -1e20;
        b[8] = 1.;
        b[12] = 2.;
        // Native output0 accumulates terms2,1,0,3; ordinary 0,1,2,3 would yield3.
        assert_eq!(
            f32::from_bits(compose_bone_matrix(a.map(f32::to_bits), b.map(f32::to_bits))[0]),
            2.
        );
    }
}
