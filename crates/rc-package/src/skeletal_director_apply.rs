//! ApplyDirector with supplied MeshToWorld/actor scale and explicit quaternion policy.
use crate::{
    skeletal_director::{director_to_local, selected_director, BoneDirector},
    skeletal_director_products::{compose_relative_director, correct_ancestor_rows},
    skeletal_director_rotation::{prepare_director_rotation, DirectorRotationHost},
    skeletal_hierarchy::{build_pose_matrices_with_hook, BoneHierarchy},
    skeletal_matrix_inverse::inverse_matrix,
    skeletal_root_pose::{quaternion_translation_matrix, RootTransform},
};

/// Applies rotation, relative composition, previous-index correction and final
/// translation/component scale. Completed history/matrix writes survive host errors.
pub fn apply_director_prepared(
    director: &mut BoneDirector,
    bone: usize,
    matrices: &mut [[u32; 16]],
    mesh_to_world: [u32; 16],
    actor_scale: [u32; 3],
    host: &mut impl DirectorRotationHost,
) -> Result<(), String> {
    let current = *matrices
        .get(bone)
        .ok_or("director bone matrix unavailable")?;
    let supplied: [u32; 16] = director.words[2..18].try_into().unwrap();
    let mut working = if director.words[19] & 1 != 0 {
        supplied
    } else {
        director_to_local(supplied, inverse_matrix(mesh_to_world))
    };
    if director.words[18] & 0xff00 != 0 {
        let prepared = prepare_director_rotation(director, current, working, actor_scale, host)?;
        working = prepared.matrix;
        let relative = director.words[19] & 2 != 0;
        if relative {
            working = compose_relative_director(current, working);
        }
        for (i, v) in matrices[bone][..12].iter_mut().enumerate() {
            let scale = if i % 4 == 3 {
                0.0f32
            } else {
                f32::from_bits(prepared.scale[i % 4])
            };
            // FPlane::operator* loads the supplied scale plane before current plane.
            *v = (scale * f32::from_bits(working[i])).to_bits();
        }
        let first = ((director.words[1] << 1) as i32) >> 1;
        let bone_i32 =
            i32::try_from(bone).map_err(|_| "director bone index exceeds signed range")?;
        if relative && first < bone_i32 {
            let denominator = (i64::from(bone_i32) - i64::from(first) + 1) as f32;
            for previous in (first..bone_i32).rev() {
                let index = usize::try_from(previous)
                    .map_err(|_| "director correction references negative bone")?;
                if index >= matrices.len() {
                    return Err("director correction bone unavailable".into());
                }
                let numerator = (i64::from(previous) - i64::from(first) + 1) as f32;
                let rotation = host.power(prepared.quaternion, numerator / denominator)?;
                let correction = quaternion_translation_matrix(RootTransform {
                    rotation,
                    position: [0; 3],
                });
                matrices[index] = correct_ancestor_rows(matrices[index], correction);
            }
        }
    }
    if director.words[18] & 0xff != 0 {
        matrices[bone][12..16].copy_from_slice(&working[12..16]);
    }
    if director.words[18] & 0xff0000 != 0 {
        for (i, v) in matrices[bone][..12].iter_mut().enumerate() {
            *v = (f32::from_bits(working[i]) * f32::from_bits(*v)).to_bits();
        }
    }
    Ok(())
}

/// The selected mutable director retains history for subsequent pose evaluations.
#[allow(clippy::too_many_arguments)] // Explicit pose, scene snapshots, math policy and output.
pub fn build_mutable_directed_pose(
    local: &[RootTransform],
    hierarchy: &BoneHierarchy,
    editor: bool,
    directors: &mut [BoneDirector],
    mesh_to_world: [u32; 16],
    actor_scale: [u32; 3],
    host: &mut impl DirectorRotationHost,
    output: &mut Vec<[u32; 16]>,
) -> Result<usize, String> {
    let mut applied = 0;
    build_pose_matrices_with_hook(local, hierarchy, editor, output, |bone, matrices| {
        if let Some(index) = selected_director(directors, bone as i32) {
            apply_director_prepared(
                &mut directors[index],
                bone,
                matrices,
                mesh_to_world,
                actor_scale,
                host,
            )?;
            applied += 1;
        }
        Ok(())
    })?;
    Ok(applied)
}

#[cfg(test)]
#[path = "skeletal_director_apply_tests.rs"]
mod tests;
