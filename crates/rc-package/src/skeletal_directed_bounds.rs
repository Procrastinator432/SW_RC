//! Prepared local pose -> directed hierarchy -> local/world bounds -> completion flags.
use crate::{
    skeletal_bounds::{
        accumulate_pose_point, finish_accumulated_bounds, BoundsPadding, PoseBounds, PoseBoundsHost,
    },
    skeletal_director::{selected_director, BoneDirector},
    skeletal_director_apply::apply_director_prepared,
    skeletal_director_rotation::DirectorRotationHost,
    skeletal_hierarchy::{build_pose_matrices_with_hook, BoneHierarchy},
    skeletal_root_pose::RootTransform,
};
pub struct DirectedPoseInput<'a> {
    pub local: &'a [RootTransform],
    pub hierarchy: &'a BoneHierarchy,
    pub editor: bool,
    pub mesh_to_world: [u32; 16],
    pub actor_scale: [u32; 3],
    pub padding: &'a BoundsPadding,
}
/// No cache gate or root-only branch: caller has already selected full evaluation.
/// Failed directors retain prior point bounds and writes but skip current point and completion.
pub fn evaluate_directed_pose(
    input: DirectedPoseInput<'_>,
    directors: &mut [BoneDirector],
    matrices: &mut Vec<[u32; 16]>,
    bounds: &mut PoseBounds,
    rotation: &mut impl DirectorRotationHost,
    publication: &mut impl PoseBoundsHost,
) -> Result<usize, String> {
    let mut applied = 0;
    build_pose_matrices_with_hook(
        input.local,
        input.hierarchy,
        input.editor,
        matrices,
        |bone, output| {
            if let Some(index) = selected_director(directors, bone as i32) {
                apply_director_prepared(
                    &mut directors[index],
                    bone,
                    output,
                    input.mesh_to_world,
                    input.actor_scale,
                    rotation,
                )?;
                applied += 1;
            }
            let m = output[bone];
            accumulate_pose_point(
                bounds,
                bone,
                input.hierarchy.move_bone,
                [m[12], m[13], m[14]],
            );
            Ok(())
        },
    )?;
    finish_accumulated_bounds(bounds, input.padding, publication)?;
    Ok(applied)
}

#[cfg(test)]
#[path = "skeletal_directed_bounds_tests.rs"]
mod tests;
