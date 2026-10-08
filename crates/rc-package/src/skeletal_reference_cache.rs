//! ApplyAnimation empty inverse-reference-cache preparation, 10509f3c..1050a43b.
use crate::{
    skeletal_matrix_inverse::inverse_matrix,
    skeletal_mesh::StoredBone,
    skeletal_reference_product::compose_reference_matrix,
    skeletal_root_pose::{quaternion_translation_matrix, RootTransform},
};
/// Native only checks whether the cache is empty; a nonempty stale cache is reused.
/// On build, invalidate +61 and allocate all inverse slots before filling them.
pub fn ensure_inverse_reference_cache(
    bones: &[StoredBone],
    cache: &mut Vec<[u32; 16]>,
    cache_byte_61: &mut u8,
) -> Result<bool, String> {
    if !cache.is_empty() {
        return Ok(false);
    }
    *cache_byte_61 = 0;
    cache.resize(bones.len(), [0; 16]);
    let mut reference = Vec::with_capacity(bones.len());
    for (index, bone) in bones.iter().enumerate() {
        let mut matrix = quaternion_translation_matrix(RootTransform {
            rotation: bone.rotation,
            position: bone.position,
        });
        if index > 0 {
            let parent = usize::try_from(bone.word_34).map_err(|_| "negative reference parent")?;
            if parent >= index {
                return Err("reference parent must precede bone".into());
            }
            // Reference preparation has no Move detachment or editor gate.
            matrix = compose_reference_matrix(matrix, reference[parent]);
        }
        reference.push(matrix);
        cache[index] = inverse_matrix(matrix);
    }
    Ok(true)
}
#[cfg(test)]
mod tests {
    use super::*;
    fn bone(name: &str, parent: i32, x: f32) -> StoredBone {
        StoredBone {
            name: crate::skeletal_animation::AnimationName {
                index: 0,
                name: name.into(),
            },
            flags: 0,
            rotation: [0, 0, 0, 1f32.to_bits()],
            position: [x.to_bits(), 0, 0],
            word_24: 0,
            word_28_first: 0,
            word_28: 0,
            word_30: 0,
            word_38: 0,
            word_34: parent,
        }
    }
    fn identity() -> [u32; 16] {
        quaternion_translation_matrix(RootTransform {
            rotation: [0, 0, 0, 1f32.to_bits()],
            position: [0; 3],
        })
    }
    #[test]
    fn move_parent_is_composed_in_reference_cache() {
        let bones = [bone("Move", 123, 10.), bone("Child", 0, 2.)];
        let mut cache = vec![];
        let mut flag = 7;
        assert!(ensure_inverse_reference_cache(&bones, &mut cache, &mut flag).unwrap());
        assert_eq!(flag, 0);
        assert_eq!(cache[0][12], (-10f32).to_bits());
        assert_eq!(cache[1][12], (-12f32).to_bits());
    }
    #[test]
    fn nonempty_cache_bypasses_size_parent_and_invalidation_checks() {
        let bones = [bone("Root", 0, 1.), bone("Bad", 2, 2.)];
        let mut cache = vec![[99; 16]];
        let mut flag = 7;
        assert!(!ensure_inverse_reference_cache(&bones, &mut cache, &mut flag).unwrap());
        assert_eq!(cache, vec![[99; 16]]);
        assert_eq!(flag, 7);
    }
    #[test]
    fn parent_error_preserves_allocated_slots_and_completed_inverses() {
        let bones = [
            bone("Root", 0, 5.),
            bone("Bad", 1, 2.),
            bone("Later", 0, 3.),
        ];
        let mut cache = vec![];
        let mut flag = 7;
        assert!(ensure_inverse_reference_cache(&bones, &mut cache, &mut flag).is_err());
        assert_eq!(flag, 0);
        assert_eq!(cache.len(), 3);
        assert_eq!(cache[0][12], (-5f32).to_bits());
        assert_eq!(cache[1], [0; 16]);
        assert_eq!(cache[2], [0; 16]);
        // Native's next empty check will also reuse this partially populated cache.
        assert!(!ensure_inverse_reference_cache(&bones, &mut cache, &mut flag).unwrap());
    }
    #[test]
    fn singular_reference_uses_native_identity_inverse_fallback() {
        let mut b = bone("Root", 0, 0.);
        b.rotation = [0.5f32, 0.5, 0., 0.].map(f32::to_bits);
        let mut cache = vec![];
        assert!(ensure_inverse_reference_cache(&[b], &mut cache, &mut 7).unwrap());
        assert_eq!(cache, [identity()]);
    }
    #[test]
    fn empty_skeleton_stays_empty_and_invalidates_on_each_call() {
        let mut cache = vec![];
        let mut flag = 7;
        assert!(ensure_inverse_reference_cache(&[], &mut cache, &mut flag).unwrap());
        assert_eq!(flag, 0);
        flag = 9;
        assert!(ensure_inverse_reference_cache(&[], &mut cache, &mut flag).unwrap());
        assert_eq!(flag, 0);
        assert!(cache.is_empty());
    }
}
