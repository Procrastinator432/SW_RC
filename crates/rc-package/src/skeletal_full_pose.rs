//! Prepared full ApplyAnimation branch: channel commit through bounds completion.
use crate::{
    mesh_animation::AnimationChannel,
    skeletal_bounds::{BoundsPadding, PoseBounds, PoseBoundsHost},
    skeletal_channel_stack::{
        apply_channels_to_buffers, ChannelStackResult, PreparedChannelBuffers,
        PreparedChannelStackHost,
    },
    skeletal_directed_bounds::{evaluate_directed_pose, DirectedPoseInput},
    skeletal_director::BoneDirector,
    skeletal_director_rotation::DirectorRotationHost,
    skeletal_hierarchy::BoneHierarchy,
    skeletal_root_pose::RootTransform,
};
use serde::Serialize;
pub struct PreparedFullPose {
    pub local: Vec<RootTransform>,
    pub scratch: Vec<RootTransform>,
    pub matrices: Vec<[u32; 16]>,
    /// Owns the single cache byte shared by channel selection and completion.
    pub bounds: PoseBounds,
}
pub struct FullPoseBuffers<'a> {
    pub local: &'a mut [RootTransform],
    pub scratch: &'a mut [RootTransform],
    pub matrices: &'a mut Vec<[u32; 16]>,
    pub bounds: &'a mut PoseBounds,
}
pub struct FullPoseInput<'a> {
    pub reference: &'a [RootTransform],
    pub hierarchy: &'a BoneHierarchy,
    pub editor: bool,
    pub word_11c: u32,
    pub mesh_to_world: [u32; 16],
    pub actor_scale: [u32; 3],
    pub padding: &'a BoundsPadding,
}
#[derive(Debug, Serialize, PartialEq, Eq)]
pub struct FullPoseResult {
    pub channels: ChannelStackResult,
    pub directors: usize,
}
pub struct FullPoseHosts<'a, C, R, P> {
    pub channels: &'a mut C,
    pub rotation: &'a mut R,
    pub publication: &'a mut P,
}
/// Caller has already prepared instance buffers, transform, inverse cache and linkups.
/// No root-only branch, allocation policy, runtime ticking or skinning is supplied here.
pub fn apply_full_pose_prepared<
    C: PreparedChannelStackHost,
    R: DirectorRotationHost,
    P: PoseBoundsHost,
>(
    state: &mut PreparedFullPose,
    input: FullPoseInput<'_>,
    channels: &mut [AnimationChannel],
    directors: &mut [BoneDirector],
    hosts: FullPoseHosts<'_, C, R, P>,
) -> Result<FullPoseResult, String> {
    apply_full_pose_to_buffers(
        FullPoseBuffers {
            local: &mut state.local,
            scratch: &mut state.scratch,
            matrices: &mut state.matrices,
            bounds: &mut state.bounds,
        },
        input,
        channels,
        directors,
        hosts,
    )
}
pub fn apply_full_pose_to_buffers<
    C: PreparedChannelStackHost,
    R: DirectorRotationHost,
    P: PoseBoundsHost,
>(
    state: FullPoseBuffers<'_>,
    input: FullPoseInput<'_>,
    channels: &mut [AnimationChannel],
    directors: &mut [BoneDirector],
    hosts: FullPoseHosts<'_, C, R, P>,
) -> Result<FullPoseResult, String> {
    let result = apply_channels_to_buffers(
        PreparedChannelBuffers {
            local: state.local,
            scratch: state.scratch,
            byte_61: state.bounds.byte_61,
        },
        input.reference,
        channels,
        input.editor,
        input.word_11c,
        hosts.channels,
    )?;
    if result == ChannelStackResult::Cached {
        return Ok(FullPoseResult {
            channels: result,
            directors: 0,
        });
    }
    let applied = evaluate_directed_pose(
        DirectedPoseInput {
            local: state.local,
            hierarchy: input.hierarchy,
            editor: input.editor,
            mesh_to_world: input.mesh_to_world,
            actor_scale: input.actor_scale,
            padding: input.padding,
        },
        directors,
        state.matrices,
        state.bounds,
        hosts.rotation,
        hosts.publication,
    )?;
    Ok(FullPoseResult {
        channels: result,
        directors: applied,
    })
}
#[cfg(test)]
#[path = "skeletal_full_pose_tests.rs"]
mod tests;
