//! Root-only and full ApplyAnimation entries over native-shaped arrays and supplied runtime hosts.
use crate::{
    mesh_animation::AnimationChannel,
    skeletal_bounds::{BoundsPadding, PoseBounds, PoseBoundsHost},
    skeletal_channel_stack::{ChannelStackResult, PreparedChannelStackHost},
    skeletal_director::BoneDirector,
    skeletal_director_rotation::DirectorRotationHost,
    skeletal_full_pose::{
        apply_full_pose_to_buffers, FullPoseBuffers, FullPoseHosts, FullPoseInput, FullPoseResult,
    },
    skeletal_hierarchy::BoneHierarchy,
    skeletal_mesh::StoredBone,
    skeletal_preparation::{
        prepare_animation_instance, prepare_channel_scratch, AnimationPreparationHost,
        AnimationPreparationResult, ChannelScratchBuffers, InstanceAnimationBuffers,
    },
    skeletal_root_pose::{apply_root_prepared, PreparedRootPose, RootPoseHost, RootTransform},
};
use serde::Serialize;
pub struct AnimationInstance {
    pub buffers: InstanceAnimationBuffers,
    pub bounds: PoseBounds,
}
pub struct AnimationFullInput<'a> {
    pub bones: &'a [StoredBone],
    pub hierarchy: &'a BoneHierarchy,
    pub linkup_count: usize,
    pub editor: bool,
    pub word_11c: u32,
    pub actor_scale: [u32; 3],
    pub padding: &'a BoundsPadding,
}
pub struct AnimationHosts<'a, H, C, R, P> {
    pub preparation: &'a mut H,
    pub pose: FullPoseHosts<'a, C, R, P>,
}
#[derive(Debug, Serialize, PartialEq, Eq)]
pub struct AnimationFullResult {
    pub preparation: AnimationPreparationResult,
    pub scratch_resized: bool,
    pub pose: FullPoseResult,
}
pub struct AnimationRootInput<'a> {
    pub bones: &'a [StoredBone],
    pub linkup_count: usize,
    pub word_11c: u32,
}
#[derive(Debug, Serialize, PartialEq, Eq)]
pub struct AnimationRootResult {
    pub preparation: AnimationPreparationResult,
    pub sampled: usize,
}
pub struct AnimationFrameInput<'a> {
    pub frame_type: i32,
    pub animation: AnimationFullInput<'a>,
}
pub struct AnimationFrameHosts<'a, H, C, R, P, T> {
    pub preparation: &'a mut H,
    pub root: &'a mut T,
    pub pose: FullPoseHosts<'a, C, R, P>,
}
#[derive(Debug, Serialize, PartialEq, Eq)]
pub enum AnimationFrameResult {
    Root(AnimationRootResult),
    Full(AnimationFullResult),
}
/// GetFrame 1050b330 passes (frame_type == 3) to ApplyAnimation. Preparation occurs once.
pub fn evaluate_animation_frame<
    H: AnimationPreparationHost,
    C: PreparedChannelStackHost,
    R: DirectorRotationHost,
    P: PoseBoundsHost,
    T: RootPoseHost,
>(
    state: &mut AnimationInstance,
    input: AnimationFrameInput<'_>,
    inverse: &mut Vec<[u32; 16]>,
    scratch: &mut ChannelScratchBuffers,
    channels: &mut [AnimationChannel],
    directors: &mut [BoneDirector],
    hosts: AnimationFrameHosts<'_, H, C, R, P, T>,
) -> Result<AnimationFrameResult, String> {
    if input.frame_type == 3 {
        apply_animation_root(
            state,
            AnimationRootInput {
                bones: input.animation.bones,
                linkup_count: input.animation.linkup_count,
                word_11c: input.animation.word_11c,
            },
            inverse,
            channels,
            hosts.preparation,
            hosts.root,
        )
        .map(AnimationFrameResult::Root)
    } else {
        apply_animation_full(
            state,
            input.animation,
            inverse,
            scratch,
            channels,
            directors,
            AnimationHosts {
                preparation: hosts.preparation,
                pose: hosts.pose,
            },
        )
        .map(AnimationFrameResult::Full)
    }
}
/// Root-only branches before the full-pose cache gate and never accesses global scratch.
pub fn apply_animation_root(
    state: &mut AnimationInstance,
    input: AnimationRootInput<'_>,
    inverse: &mut Vec<[u32; 16]>,
    channels: &mut [AnimationChannel],
    preparation_host: &mut impl AnimationPreparationHost,
    root_host: &mut impl RootPoseHost,
) -> Result<AnimationRootResult, String> {
    let preparation = prepare_animation_instance(
        &mut state.buffers,
        input.bones,
        inverse,
        &mut state.bounds.byte_61,
        input.linkup_count,
        preparation_host,
    )?;
    let reference = input
        .bones
        .first()
        .ok_or("root-only evaluation requires a reference root")?;
    let mut root = PreparedRootPose {
        root: RootTransform {
            rotation: state.buffers.rotations[0],
            position: state.buffers.positions[0],
        },
        matrix: state.buffers.matrices[0],
        byte_60: state.bounds.byte_60,
        byte_61: state.bounds.byte_61,
    };
    let result = apply_root_prepared(
        &mut root,
        RootTransform {
            rotation: reference.rotation,
            position: reference.position,
        },
        channels,
        input.word_11c,
        root_host,
    );
    // GetRotPos may commit quaternion/position before failing; matrix commits only on success.
    state.buffers.rotations[0] = root.root.rotation;
    state.buffers.positions[0] = root.root.position;
    state.buffers.matrices[0] = root.matrix;
    Ok(AnimationRootResult {
        preparation,
        sampled: result?,
    })
}
/// Mesh-owned inverse cache and global/shared scratch are supplied separately from instance state.
/// Full branch only: root-only evaluation, actor ticking and skinning remain separate.
pub fn apply_animation_full<
    H: AnimationPreparationHost,
    C: PreparedChannelStackHost,
    R: DirectorRotationHost,
    P: PoseBoundsHost,
>(
    state: &mut AnimationInstance,
    input: AnimationFullInput<'_>,
    inverse: &mut Vec<[u32; 16]>,
    scratch: &mut ChannelScratchBuffers,
    channels: &mut [AnimationChannel],
    directors: &mut [BoneDirector],
    hosts: AnimationHosts<'_, H, C, R, P>,
) -> Result<AnimationFullResult, String> {
    let preparation = prepare_animation_instance(
        &mut state.buffers,
        input.bones,
        inverse,
        &mut state.bounds.byte_61,
        input.linkup_count,
        hosts.preparation,
    )?;
    if state.bounds.byte_61 != 0 && !input.editor {
        return Ok(AnimationFullResult {
            preparation,
            scratch_resized: false,
            pose: FullPoseResult {
                channels: ChannelStackResult::Cached,
                directors: 0,
            },
        });
    }
    let count = input.bones.len();
    let scratch_resized = prepare_channel_scratch(scratch, count);
    if scratch.positions.len() < count {
        return Err("native scratch position buffer is shorter than the skeleton".into());
    }
    let reference: Vec<_> = input
        .bones
        .iter()
        .map(|b| RootTransform {
            rotation: b.rotation,
            position: b.position,
        })
        .collect();
    let mut local: Vec<_> = state
        .buffers
        .rotations
        .iter()
        .zip(&state.buffers.positions)
        .map(|(&rotation, &position)| RootTransform { rotation, position })
        .collect();
    // Preserve all paired tail entries, and leave any unpaired native-array tail untouched.
    let mut working: Vec<_> = scratch
        .rotations
        .iter()
        .zip(&scratch.positions)
        .map(|(&rotation, &position)| RootTransform { rotation, position })
        .collect();
    let result = apply_full_pose_to_buffers(
        FullPoseBuffers {
            local: &mut local,
            scratch: &mut working,
            matrices: &mut state.buffers.matrices,
            bounds: &mut state.bounds,
        },
        FullPoseInput {
            reference: &reference,
            hierarchy: input.hierarchy,
            editor: input.editor,
            word_11c: input.word_11c,
            mesh_to_world: state.buffers.mesh_to_world,
            actor_scale: input.actor_scale,
            padding: input.padding,
        },
        channels,
        directors,
        hosts.pose,
    );
    // Copy back even on error: native channel history/scratch writes precede pose commit.
    for (i, p) in local.iter().enumerate() {
        state.buffers.rotations[i] = p.rotation;
        state.buffers.positions[i] = p.position;
    }
    for (i, p) in working.iter().enumerate() {
        scratch.rotations[i] = p.rotation;
        scratch.positions[i] = p.position;
    }
    Ok(AnimationFullResult {
        preparation,
        scratch_resized,
        pose: result?,
    })
}
#[cfg(test)]
#[path = "skeletal_animation_entry_tests.rs"]
mod tests;
