//! GetFrame cache lifecycle (1055f0c5..1055f1f2) and shared GetSequence integration.
//! Logical array resize is modeled; native storage addresses/allocator behavior are external.
use crate::{
    d3d_dynamic_draw::Runtime, mesh_animation::AnimationChannel, skeletal_sequence,
    vertmesh_animation, vertmesh_frame, vertmesh_indices,
};
use serde::{Deserialize, Serialize};

#[derive(Clone, Debug, Serialize, Deserialize, PartialEq, Eq)]
pub struct Array {
    pub packed_count: u32,
    pub values: Vec<[u32; 3]>,
}
impl Array {
    fn validate(&self) -> Result<(), String> {
        let count = (self.packed_count.wrapping_shl(3) as i32) >> 3;
        if !(0..=65536).contains(&count) || self.values.len() != count as usize {
            return Err("Invalid GetFrame cache array snapshot".into());
        }
        Ok(())
    }
    fn resize(&mut self, count: usize) -> Result<(), String> {
        if count > 65536 {
            return Err("GetFrame cache resize exceeds bound".into());
        }
        self.values.resize(count, [0; 3]);
        self.packed_count = (self.packed_count & 0xe0000000) | count as u32;
        Ok(())
    }
}
#[derive(Clone, Debug, Serialize, Deserialize, PartialEq, Eq)]
pub struct Cache {
    pub cached_mesh: u32,
    pub positions: Array,
    pub normals: Array,
    pub word_9c: u32,
    pub word_90: u32,
    pub word_8c: u32,
    pub last_count: i32,
    pub flags: [u8; 2],
}
#[derive(Clone, Debug, Serialize, PartialEq, Eq)]
pub struct PreparedCache {
    pub initialized: bool,
    pub mesh_changed: bool,
    /// Logical 10332670 calls in order: array (0 positions / 1 normals), new count.
    pub resizes: Vec<[u32; 2]>,
    pub cache: Cache,
}
impl Cache {
    pub fn prepare(&self, mesh: u32, vertex_count: u32) -> Result<PreparedCache, String> {
        if mesh == 0 || vertex_count > 65536 {
            return Err("Invalid mesh identity/count".into());
        }
        self.positions.validate()?;
        self.normals.validate()?;
        let mesh_changed = self.cached_mesh != mesh;
        let initialized = self.positions.packed_count & 0x1fffffff == 0 || mesh_changed;
        let mut cache = self.clone();
        let mut resizes = vec![];
        if initialized {
            if mesh_changed {
                cache.positions.resize(0)?;
                cache.normals.resize(0)?;
                resizes.extend([[0, 0], [1, 0]]);
            }
            for (index, array) in [&mut cache.positions, &mut cache.normals]
                .into_iter()
                .enumerate()
            {
                let old = array.values.len();
                let new = old + vertex_count as usize;
                array.resize(new)?;
                // Original REP STOS zeroes only the appended range.
                array.values[old..].fill([0; 3]);
                resizes.push([index as u32, new as u32]);
            }
            cache.word_9c = 1.0f32.to_bits();
            cache.cached_mesh = mesh;
            cache.word_90 = 0;
            cache.word_8c = 0;
        }
        Ok(PreparedCache {
            initialized,
            mesh_changed,
            resizes,
            cache,
        })
    }
}
#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct SequenceRecord {
    pub identity: u32,
    pub sequence: vertmesh_animation::Sequence,
}
#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct Input {
    pub mesh: u32,
    pub cache: Cache,
    pub channels: Vec<AnimationChannel>,
    pub editor: bool,
    /// Supplied virtual mesh +ac lookup bindings; asset/name loading remains external.
    pub bindings: Vec<(u32, u32)>,
    pub sequences: Vec<SequenceRecord>,
    /// Sequence/time/cache fields are replaced from channels and this instance cache.
    pub animation: vertmesh_animation::Input,
}
pub struct Prepared {
    pub preparation: PreparedCache,
    pub channels: Vec<AnimationChannel>,
    pub identity: u32,
    pub animation: vertmesh_animation::Input,
}
#[derive(Clone, Debug, Serialize)]
pub struct Output {
    pub preparation: PreparedCache,
    pub cache: Cache,
    pub channels: Vec<AnimationChannel>,
    pub sequence_identity: u32,
    pub animation: vertmesh_animation::Output,
}
impl Input {
    pub fn prepare(&self) -> Result<Prepared, String> {
        let vc = self.animation.clock.vertex_count as usize;
        let preparation = self
            .cache
            .prepare(self.mesh, self.animation.clock.vertex_count)?;
        if self.channels.len() > 256 || self.bindings.len() > 65536 || self.sequences.len() > 65536
        {
            return Err("Instance channel/sequence snapshot exceeds bound".into());
        }
        let mut channels = self.channels.clone();
        let mut host = skeletal_sequence::SnapshotSequenceLookup {
            bindings: &self.bindings,
        };
        let identity = skeletal_sequence::get_sequence(&mut channels, 0, self.editor, &mut host)?;
        let sequence = if identity == 0 {
            None
        } else {
            Some(
                self.sequences
                    .iter()
                    .find(|s| s.identity == identity)
                    .ok_or("Cached sequence identity has no supplied record")?
                    .sequence
                    .clone(),
            )
        };
        let mut animation = self.animation.clone();
        animation.clock.sequence = sequence;
        animation.clock.time = channels[0].words[7];
        animation.cache = vertmesh_frame::Cache {
            positions: preparation
                .cache
                .positions
                .values
                .get(..vc)
                .ok_or("Short reused position cache")?
                .to_vec(),
            normals: preparation
                .cache
                .normals
                .values
                .get(..vc)
                .ok_or("Short reused normal cache")?
                .to_vec(),
            last_count: preparation.cache.last_count,
            flags: preparation.cache.flags,
        };
        Ok(Prepared {
            preparation,
            channels,
            identity,
            animation,
        })
    }
    pub fn build(&self) -> Result<Output, String> {
        let prepared = self.prepare()?;
        let animation = prepared.animation.build()?;
        Ok(prepared.finish(animation))
    }
    fn commit(&mut self, output: &Output) {
        self.cache = output.cache.clone();
        self.channels = output.channels.clone();
        self.animation.bytes = output.animation.frame.bytes.clone();
        // Keep the redundant legacy fields synchronized for an inspectable snapshot.
        self.animation.cache = output.animation.frame.cache.clone();
        self.animation.clock.time = self.channels[0].words[7];
        self.animation.clock.sequence = if output.sequence_identity == 0 {
            None
        } else {
            self.sequences
                .iter()
                .find(|s| s.identity == output.sequence_identity)
                .map(|s| s.sequence.clone())
        };
    }
}
impl Prepared {
    fn finish(self, animation: vertmesh_animation::Output) -> Output {
        let mut cache = self.preparation.cache.clone();
        let frame = &animation.frame.cache;
        cache.positions.values[..frame.positions.len()].copy_from_slice(&frame.positions);
        cache.normals.values[..frame.normals.len()].copy_from_slice(&frame.normals);
        cache.last_count = frame.last_count;
        cache.flags = frame.flags;
        Output {
            preparation: self.preparation,
            cache,
            channels: self.channels,
            sequence_identity: self.identity,
            animation,
        }
    }
}
#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct Request {
    pub instance: Input,
    pub geometry: vertmesh_indices::Request,
}
#[derive(Clone, Debug, Serialize)]
pub struct Plan {
    pub instance: Output,
    pub draw: vertmesh_indices::Plan,
}
impl Runtime {
    pub fn submit_vertmesh_instance(&mut self, request: &mut Request) -> Result<Plan, String> {
        let prepared = request.instance.prepare()?;
        let mut nested = vertmesh_animation::Request {
            animation: prepared.animation.clone(),
            geometry: request.geometry.clone(),
        };
        let result = self.submit_animated_frame(&mut nested)?;
        let instance = prepared.finish(result.animation);
        request.instance.commit(&instance);
        request.geometry = nested.geometry;
        Ok(Plan {
            instance,
            draw: result.draw,
        })
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    fn cache(n: usize) -> Cache {
        Cache {
            cached_mesh: 7,
            positions: Array {
                packed_count: 0xa0000000 | n as u32,
                values: vec![[123; 3]; n],
            },
            normals: Array {
                packed_count: 0x60000000 | n as u32,
                values: vec![[456; 3]; n],
            },
            word_9c: 9,
            word_90: 10,
            word_8c: 11,
            last_count: 12,
            flags: [13, 14],
        }
    }
    #[test]
    fn matching_nonempty_cache_preserves_metadata() {
        let c = cache(2);
        let p = c.prepare(7, 2).unwrap();
        assert!(!p.initialized);
        assert_eq!(p.cache, c);
        assert!(p.resizes.is_empty());
    }
    #[test]
    fn mesh_change_resets_both_arrays_before_append() {
        let p = cache(2).prepare(8, 3).unwrap();
        assert_eq!(p.resizes, [[0, 0], [1, 0], [0, 3], [1, 3]]);
        assert_eq!(p.cache.positions.values, [[0; 3]; 3]);
        assert_eq!(p.cache.normals.values, [[0; 3]; 3]);
    }
    #[test]
    fn resize_preserves_three_descriptor_flag_bits() {
        let p = cache(2).prepare(8, 3).unwrap();
        assert_eq!(p.cache.positions.packed_count, 0xa0000003);
        assert_eq!(p.cache.normals.packed_count, 0x60000003);
    }
    #[test]
    fn initialization_resets_only_documented_metadata() {
        let p = cache(2).prepare(8, 3).unwrap();
        assert_eq!(
            (p.cache.word_9c, p.cache.word_90, p.cache.word_8c),
            (0x3f800000, 0, 0)
        );
        assert_eq!((p.cache.last_count, p.cache.flags), (12, [13, 14]));
    }
    #[test]
    fn empty_positions_triggers_even_with_same_mesh() {
        let p = cache(0).prepare(7, 2).unwrap();
        assert!(p.initialized);
        assert!(!p.mesh_changed);
        assert_eq!(p.resizes, [[0, 2], [1, 2]]);
    }
    #[test]
    fn nonempty_normals_prefix_survives_empty_positions_initialization() {
        let mut c = cache(0);
        c.normals = cache(2).normals;
        let p = c.prepare(7, 1).unwrap();
        assert_eq!(p.cache.normals.values, [[456; 3], [456; 3], [0; 3]]);
    }
    #[test]
    fn zero_vertex_count_still_initializes_metadata() {
        let p = cache(0).prepare(7, 0).unwrap();
        assert!(p.initialized);
        assert_eq!(p.cache.word_9c, 0x3f800000);
    }
    #[test]
    fn zero_vertex_count_reinitializes_next_call() {
        let p = cache(0).prepare(7, 0).unwrap();
        assert!(p.cache.prepare(7, 0).unwrap().initialized);
    }
    #[test]
    fn nonempty_position_gate_does_not_depend_on_normals_count() {
        let mut c = cache(1);
        c.normals = cache(0).normals;
        assert!(!c.prepare(7, 1).unwrap().initialized);
    }
    #[test]
    fn signed_count_and_storage_conflicts_are_rejected() {
        let mut c = cache(1);
        c.positions.packed_count = 0x10000000;
        assert!(c.prepare(7, 1).is_err());
        c = cache(1);
        c.normals.values.clear();
        assert!(c.prepare(7, 1).is_err());
    }
    #[test]
    fn oversized_append_is_rejected_without_mutating_cache() {
        let mut c = cache(0);
        c.normals = cache(65536).normals;
        let before = c.clone();
        assert!(c.prepare(7, 1).is_err());
        assert_eq!(c, before);
    }
    #[test]
    fn null_mesh_and_oversized_vertex_count_are_rejected() {
        assert!(cache(1).prepare(0, 1).is_err());
        assert!(cache(1).prepare(7, 65537).is_err());
    }
}
