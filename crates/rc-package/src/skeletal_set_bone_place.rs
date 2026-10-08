//! engine 10509270 SetBonePlace: resolve, invalidate, update prefix or append history.
use crate::mesh_animation::SkeletonSnapshot;
use crate::skeletal_director::BoneDirector;
/// Resolves a positive global name index through native name/MatchRefBone semantics.
/// Alias-table and global FName identity binding remain caller responsibilities.
pub trait BonePlaceNames {
    fn match_name(&mut self, global_name_index: i32) -> Result<i32, String>;
}
/// Supplied global name-index -> FName identity table plus prepared mesh names/aliases.
/// Dynamic runtime registration and alias archive decoding are not inferred here.
pub struct SnapshotBonePlaceNames<'a> {
    pub global_names: &'a [u32],
    pub skeleton: &'a SkeletonSnapshot,
}
impl BonePlaceNames for SnapshotBonePlaceNames<'_> {
    fn match_name(&mut self, global_name_index: i32) -> Result<i32, String> {
        let index =
            usize::try_from(global_name_index).map_err(|_| "negative global bone name index")?;
        let handle = *self
            .global_names
            .get(index)
            .ok_or("global bone name index unavailable")?;
        self.skeleton.match_ref_bone(handle).map_or(Ok(-1), |bone| {
            i32::try_from(bone).map_err(|_| "reference bone index exceeds signed range".into())
        })
    }
}
pub struct BonePlaceDefaults {
    pub identity_quaternion: [u32; 4],
    /// Native initializes only byte +6c; +6d..+6f are supplied stack-padding bytes.
    pub append_padding: [u8; 3],
}
fn signed31(word: u32) -> i32 {
    ((word << 1) as i32) >> 1
}
fn resolve(word: u32, names: &mut impl BonePlaceNames) -> Result<u32, String> {
    let index = signed31(word);
    if index < 0 {
        return Err("negative global bone name index".into());
    }
    let matched = if index == 0 {
        -1
    } else {
        names.match_name(index)?
    };
    Ok(matched as u32 & 0x7fffffff)
}
/// Mesh/bone count has already been supplied. Null-mesh invalid memory behavior is not emulated.
pub fn set_bone_place(
    input: [u32; 22],
    bone_count: usize,
    cache_byte_61: &mut u8,
    directors: &mut Vec<BoneDirector>,
    defaults: &BonePlaceDefaults,
    names: &mut impl BonePlaceNames,
) -> Result<bool, String> {
    let mut words = input;
    if (words[0] as i32) < 0 {
        words[0] = resolve(words[0], names)?;
    }
    let bone = signed31(words[0]);
    if bone < 0 || bone as usize >= bone_count {
        return Ok(false);
    }
    *cache_byte_61 = 0;
    if (words[1] as i32) < 0 {
        words[1] = resolve(words[1], names)?;
    }
    if signed31(words[1]) < 0 {
        words[1] = bone as u32 & 0x7fffffff;
    }
    if let Some(existing) = directors.iter_mut().find(|d| d.bone() == bone) {
        // REP MOVSD count 0x16: preserve +58 quaternion, +68 budget and +6c history byte.
        existing.words[..22].copy_from_slice(&words);
        return Ok(true);
    }
    let mut track = [0; 28];
    track[..22].copy_from_slice(&words);
    track[22..26].copy_from_slice(&defaults.identity_quaternion);
    let p = defaults.append_padding;
    track[27] = u32::from_le_bytes([0, p[0], p[1], p[2]]);
    directors.push(BoneDirector { words: track });
    Ok(true)
}
#[cfg(test)]
mod tests {
    use super::*;
    struct Names {
        calls: Vec<i32>,
        fail: bool,
    }
    impl BonePlaceNames for Names {
        fn match_name(&mut self, index: i32) -> Result<i32, String> {
            self.calls.push(index);
            if self.fail {
                Err("name boundary".into())
            } else {
                Ok(if index == 1 { 2 } else { -1 })
            }
        }
    }
    fn defaults() -> BonePlaceDefaults {
        BonePlaceDefaults {
            identity_quaternion: [0, 0, 0, 1f32.to_bits()],
            append_padding: [0xaa, 0xbb, 0xcc],
        }
    }
    fn names() -> Names {
        Names {
            calls: vec![],
            fail: false,
        }
    }
    #[test]
    fn numeric_append_initializes_history_and_only_low_padding_byte() {
        let mut input = [0x12345678; 22];
        input[0] = 2;
        input[1] = 7;
        let mut cache = 9;
        let mut ds = vec![];
        assert!(set_bone_place(input, 4, &mut cache, &mut ds, &defaults(), &mut names()).unwrap());
        assert_eq!(&ds[0].words[..22], &input);
        assert_eq!(&ds[0].words[22..26], &defaults().identity_quaternion);
        assert_eq!(ds[0].words[26], 0);
        assert_eq!(ds[0].words[27], 0xccbbaa00);
        assert_eq!(cache, 0);
    }
    #[test]
    fn replacement_copies_only_prefix_of_first_matching_track() {
        let mut track = [99; 28];
        track[0] = 0x80000002;
        let duplicate = BoneDirector { words: track };
        let mut ds = vec![duplicate.clone(), duplicate.clone()];
        let mut input = [77; 22];
        input[0] = 2;
        input[1] = 0;
        assert!(set_bone_place(input, 4, &mut 7, &mut ds, &defaults(), &mut names()).unwrap());
        assert_eq!(&ds[0].words[..22], &input);
        assert_eq!(&ds[0].words[22..], &track[22..]);
        assert_eq!(ds[1].words, duplicate.words);
    }
    #[test]
    fn tagged_target_resolves_and_missing_first_falls_back_to_target() {
        let mut input = [0; 22];
        input[0] = 0x80000001;
        input[1] = 0x80000002;
        let mut n = names();
        let mut ds = vec![];
        assert!(set_bone_place(input, 4, &mut 7, &mut ds, &defaults(), &mut n).unwrap());
        assert_eq!(n.calls, [1, 2]);
        assert_eq!(ds[0].words[0], 2);
        assert_eq!(ds[0].words[1], 2);
    }
    #[test]
    fn invalid_target_and_null_name_do_not_invalidate_or_resolve_first() {
        for target in [8, 0x40000000, 0x80000000] {
            let mut input = [0; 22];
            input[0] = target;
            input[1] = 0x80000001;
            let mut cache = 7;
            let mut n = names();
            let mut ds = vec![];
            assert!(!set_bone_place(input, 4, &mut cache, &mut ds, &defaults(), &mut n).unwrap());
            assert_eq!(cache, 7);
            assert!(ds.is_empty() && n.calls.is_empty());
        }
    }
    #[test]
    fn failures_before_and_after_target_validation_have_distinct_cache_writes() {
        for fail_first in [false, true] {
            let mut input = [0; 22];
            input[0] = if fail_first { 2 } else { 0x80000001 };
            input[1] = 0x80000001;
            let mut cache = 7;
            let mut ds = vec![];
            let mut n = Names {
                calls: vec![],
                fail: true,
            };
            assert!(set_bone_place(input, 4, &mut cache, &mut ds, &defaults(), &mut n).is_err());
            assert_eq!(cache, if fail_first { 0 } else { 7 });
            assert!(ds.is_empty());
        }
    }
    #[test]
    fn negative_first_index_falls_back_but_positive_out_of_range_is_preserved() {
        for first in [0x40000000, 123] {
            let mut input = [0; 22];
            input[0] = 2;
            input[1] = first;
            let mut ds = vec![];
            assert!(set_bone_place(input, 4, &mut 7, &mut ds, &defaults(), &mut names()).unwrap());
            assert_eq!(ds[0].words[1], if first == 123 { 123 } else { 2 });
        }
    }
}
