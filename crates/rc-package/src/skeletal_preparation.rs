//! ApplyAnimation instance buffers/transform/inverse/linkups before branch selection.
use crate::{skeletal_mesh::StoredBone, skeletal_reference_cache::ensure_inverse_reference_cache};
use serde::{Deserialize, Serialize};
#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct InstanceAnimationBuffers {
    pub rotations: Vec<[u32; 4]>,
    pub positions: Vec<[u32; 3]>,
    pub matrices: Vec<[u32; 16]>,
    pub mesh_to_world: [u32; 16],
}
#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct ChannelScratchBuffers {
    pub rotations: Vec<[u32; 4]>,
    pub positions: Vec<[u32; 3]>,
}
#[derive(Debug, Serialize, PartialEq, Eq)]
pub struct BufferPreparation {
    pub local_resized: bool,
    pub matrices_resized: bool,
}
/// Native helpers preserve each existing prefix and zero only appended words.
/// Heap ownership/capacity flags and allocator failure are outside this content model.
pub fn prepare_instance_buffers(
    state: &mut InstanceAnimationBuffers,
    count: usize,
    cache_byte: &mut u8,
) -> BufferPreparation {
    let local_resized = state.rotations.len() != count || state.positions.len() != count;
    if local_resized {
        *cache_byte = 0;
        state.rotations.resize(count, [0; 4]);
        state.positions.resize(count, [0; 3]);
    }
    let matrices_resized = state.matrices.len() != count;
    if matrices_resized {
        *cache_byte = 0;
        state.matrices.resize(count, [0; 16]);
    }
    BufferPreparation {
        local_resized,
        matrices_resized,
    }
}
/// Only called after full-pose cache gate. Native checks quaternion count alone.
/// Adequate quaternion count leaves even an inconsistent position count untouched.
pub fn prepare_channel_scratch(scratch: &mut ChannelScratchBuffers, count: usize) -> bool {
    if scratch.rotations.len() >= count {
        return false;
    }
    scratch.rotations.resize(count, [0; 4]);
    scratch.positions.resize(count, [0; 3]);
    true
}
pub trait AnimationPreparationHost {
    /// Original virtual call +e8, before inverse-reference preparation.
    fn mesh_to_world(&mut self) -> Result<[u32; 16], String>;
    /// Ordered mesh linkup refresh, after inverse-reference preparation.
    fn refresh_linkup(&mut self, index: usize) -> Result<(), String>;
}
#[derive(Debug, Serialize, PartialEq, Eq)]
pub struct AnimationPreparationResult {
    pub buffers: BufferPreparation,
    pub inverse_built: bool,
    pub linkups: usize,
}
/// Runs before BOTH root-only selection and the full-pose cache gate.
/// Therefore a cached pose still obtains a transform and refreshes linkups.
pub fn prepare_animation_instance(
    state: &mut InstanceAnimationBuffers,
    bones: &[StoredBone],
    inverse: &mut Vec<[u32; 16]>,
    cache_byte: &mut u8,
    linkup_count: usize,
    host: &mut impl AnimationPreparationHost,
) -> Result<AnimationPreparationResult, String> {
    let buffers = prepare_instance_buffers(state, bones.len(), cache_byte);
    state.mesh_to_world = host.mesh_to_world()?;
    let inverse_built = ensure_inverse_reference_cache(bones, inverse, cache_byte)?;
    for index in 0..linkup_count {
        host.refresh_linkup(index)?;
    }
    Ok(AnimationPreparationResult {
        buffers,
        inverse_built,
        linkups: linkup_count,
    })
}
#[cfg(test)]
#[path = "skeletal_preparation_tests.rs"]
mod tests;
