//! Native director selection and local translation/plane-scale subset of ApplyDirector.
use crate::{
    skeletal_hierarchy::{build_pose_matrices_with_hook, BoneHierarchy},
    skeletal_root_pose::RootTransform,
};
use serde::{Deserialize, Serialize};
#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct BoneDirector {
    pub words: [u32; 28],
}
impl BoneDirector {
    pub fn bone(&self) -> i32 {
        ((self.words[0] << 1) as i32) >> 1
    }
    pub fn enabled(&self) -> bool {
        self.words[18] & 0x00ffffff != 0
    }
}
pub fn selected_director(directors: &[BoneDirector], bone: i32) -> Option<usize> {
    directors
        .iter()
        .position(|d| d.bone() == bone && d.enabled())
}
/// ApplyDirector 10501c85..1050209d: D * inverse(MeshToWorld), terms 3,2,1,0.
pub fn director_to_local(d: [u32; 16], inverse: [u32; 16]) -> [u32; 16] {
    let a = d.map(f32::from_bits);
    let b = inverse.map(f32::from_bits);
    std::array::from_fn(|index| {
        let row = index / 4;
        let col = index % 4;
        let product = |k: usize| {
            if row == 0 {
                b[k * 4 + col] * a[row * 4 + k]
            } else {
                a[row * 4 + k] * b[k * 4 + col]
            }
        };
        (((product(3) + product(2)) + product(1)) + product(0)).to_bits()
    })
}
/// Includes native inverse and world-to-local director conversion, still without rotation.
pub fn apply_director_with_transform(
    d: &BoneDirector,
    mesh_to_world: [u32; 16],
    matrix: &mut [u32; 16],
) -> Result<(), String> {
    if d.words[19] & 1 != 0 {
        return apply_local_director(d, matrix);
    }
    let inverse = crate::skeletal_matrix_inverse::inverse_matrix(mesh_to_world);
    let supplied: [u32; 16] = d.words[2..18].try_into().unwrap();
    let converted = director_to_local(supplied, inverse);
    let mut local = d.clone();
    local.words[2..18].copy_from_slice(&converted);
    local.words[19] |= 1;
    apply_local_director(&local, matrix)
}
pub fn build_directed_pose_with_transform(
    local: &[RootTransform],
    hierarchy: &BoneHierarchy,
    editor: bool,
    directors: &[BoneDirector],
    mesh_to_world: [u32; 16],
    output: &mut Vec<[u32; 16]>,
) -> Result<usize, String> {
    let mut applied = 0;
    build_pose_matrices_with_hook(local, hierarchy, editor, output, |bone, matrices| {
        if let Some(index) = selected_director(directors, bone as i32) {
            apply_director_with_transform(&directors[index], mesh_to_world, &mut matrices[bone])?;
            applied += 1;
        }
        Ok(())
    })?;
    Ok(applied)
}
/// Requires director +4c bit0 and disabled rotation byte +49.
/// Unsupported paths fail explicitly, without changing the selected matrix.
pub fn apply_local_director(d: &BoneDirector, matrix: &mut [u32; 16]) -> Result<(), String> {
    if d.words[19] & 1 == 0 {
        return Err("world-space director requires inverse MeshToWorld".into());
    }
    if d.words[18] & 0x0000ff00 != 0 {
        return Err("director rotation/ancestor correction not implemented".into());
    }
    if d.words[18] & 0xff != 0 {
        matrix[12..16].copy_from_slice(&d.words[14..18]);
    }
    if d.words[18] & 0x00ff0000 != 0 {
        for (i, v) in matrix[..12].iter_mut().enumerate() {
            // FPlane::operator*= loads supplied plane first, then multiplies by current.
            *v = (f32::from_bits(d.words[2 + i]) * f32::from_bits(*v)).to_bits();
        }
    }
    Ok(())
}
pub fn build_local_directed_pose(
    local: &[RootTransform],
    hierarchy: &BoneHierarchy,
    editor: bool,
    directors: &[BoneDirector],
    output: &mut Vec<[u32; 16]>,
) -> Result<usize, String> {
    let mut applied = 0;
    build_pose_matrices_with_hook(local, hierarchy, editor, output, |bone, matrices| {
        if let Some(index) = selected_director(directors, bone as i32) {
            apply_local_director(&directors[index], &mut matrices[bone])?;
            applied += 1;
        }
        Ok(())
    })?;
    Ok(applied)
}
#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn world_translation_and_component_scale_use_inverse_mesh_to_world() {
        let mut mesh = [0; 16];
        for i in [0, 5, 10, 15] {
            mesh[i] = 1f32.to_bits();
        }
        mesh[0] = 2f32.to_bits();
        mesh[5] = 4f32.to_bits();
        mesh[10] = 0.5f32.to_bits();
        mesh[12..15].copy_from_slice(&[10f32, 20., 30.].map(f32::to_bits));
        let mut d = director(0, 0x10001);
        d.words[19] = 0;
        for i in [0, 5, 10, 15] {
            d.words[2 + i] = 1f32.to_bits();
        }
        d.words[14..17].copy_from_slice(&[14f32, 28., 31.].map(f32::to_bits));
        let mut m = [1f32.to_bits(); 16];
        apply_director_with_transform(&d, mesh, &mut m).unwrap();
        assert_eq!(m[12..16], [2f32, 2., 2., 1.].map(f32::to_bits));
        assert_eq!([m[0], m[5], m[10]], [0.5f32, 0.25, 2.].map(f32::to_bits));
        let before = d.words;
        apply_director_with_transform(&d, [0; 16], &mut m).unwrap();
        assert_eq!(d.words, before);
        assert_eq!(m[12..16], d.words[14..18]);
    }
    #[test]
    fn conversion_keeps_reverse_term_cancellation() {
        let mut a = [0; 16];
        a[..4].copy_from_slice(&[1f32; 4].map(f32::to_bits));
        let mut b = [0; 16];
        for (i, v) in [(0, 1e20f32), (4, -1e20), (8, 1.), (12, 2.)] {
            b[i] = v.to_bits();
        }
        assert_eq!(director_to_local(a, b)[0], 0);
    }
    fn director(bone: u32, flags: u32) -> BoneDirector {
        let mut words = [0; 28];
        words[0] = bone;
        words[18] = flags;
        words[19] = 1;
        BoneDirector { words }
    }
    fn pose(x: f32) -> RootTransform {
        RootTransform {
            rotation: [0, 0, 0, 1f32.to_bits()],
            position: [x.to_bits(), 0, 0],
        }
    }
    fn hierarchy() -> BoneHierarchy {
        BoneHierarchy {
            parents: vec![-1, 0, 1],
            descendants: vec![2, 1, 0],
            depths: vec![0, 1, 2],
            move_bone: -1,
        }
    }
    #[test]
    fn selection_skips_disabled_and_uses_first_active_with_signed_31bit_index() {
        let ds = [
            director(1, 0xff000000),
            director(0x80000001, 2),
            director(1, 0x10000),
            director(0x40000001, 1),
        ];
        assert_eq!(selected_director(&ds, 1), Some(1));
        assert_eq!(ds[3].bone(), -1073741823);
        assert!(selected_director(&ds, 0).is_none());
    }
    #[test]
    fn translation_replaces_fourth_row_before_child_composition() {
        let mut d = director(1, 1);
        d.words[14..18].copy_from_slice(&[10f32.to_bits(), 0, 0, 1f32.to_bits()]);
        let mut output = vec![];
        assert_eq!(
            build_local_directed_pose(
                &[pose(1.), pose(2.), pose(3.)],
                &hierarchy(),
                false,
                &[d],
                &mut output
            )
            .unwrap(),
            1
        );
        assert_eq!(output[1][12], 10f32.to_bits());
        assert_eq!(output[2][12], 13f32.to_bits());
    }
    #[test]
    fn scaling_is_componentwise_on_first_three_rows_including_w() {
        let mut d = director(0, 0x10001);
        for i in 0..12 {
            d.words[2 + i] = (i as f32 + 2.).to_bits();
        }
        d.words[14..18].copy_from_slice(&[7, 8, 9, 10]);
        let mut m = [1f32.to_bits(); 16];
        apply_local_director(&d, &mut m).unwrap();
        assert_eq!(m[..12], d.words[2..14]);
        assert_eq!(m[12..16], [7, 8, 9, 10]);
    }
    #[test]
    fn unsupported_selected_director_stops_before_children_without_fallback() {
        let ds = [director(1, 0x100), director(1, 1)];
        let mut output = vec![[99; 16]; 3];
        assert!(build_local_directed_pose(
            &[pose(1.), pose(2.), pose(3.)],
            &hierarchy(),
            false,
            &ds,
            &mut output
        )
        .is_err());
        assert_eq!(output[1][12], 3f32.to_bits());
        assert_eq!(output[2], [99; 16]);
        let mut d = director(0, 1);
        d.words[19] = 0;
        let mut m = [99; 16];
        assert!(apply_local_director(&d, &mut m).is_err());
        assert_eq!(m, [99; 16]);
    }
    #[test]
    fn move_children_use_corrected_root_only_in_editor() {
        let mut h = hierarchy();
        h.move_bone = 0;
        let mut d = director(0, 1);
        d.words[14] = 10f32.to_bits();
        d.words[17] = 1f32.to_bits();
        let mut out = vec![];
        for (editor, x) in [(false, 2f32), (true, 12.)] {
            build_local_directed_pose(
                &[pose(1.), pose(2.), pose(3.)],
                &h,
                editor,
                std::slice::from_ref(&d),
                &mut out,
            )
            .unwrap();
            assert_eq!(out[1][12], x.to_bits());
        }
    }
}
