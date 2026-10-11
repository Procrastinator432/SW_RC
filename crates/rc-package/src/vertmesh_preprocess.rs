//! Engine RenderPreProcess 1055fbe0 and Core FVector::GetAbsMax 1011e5b0.
//! Allocator bytes are supplied; vertex position/animation generation is external.
use crate::{d3d_dynamic_draw::Runtime, scene_projection};
use serde::{Deserialize, Serialize};

pub fn abs_max(words: [u32; 3]) -> Result<u32, String> {
    let [x, y, z] = words.map(f32::from_bits);
    if [x, y, z].iter().any(|v| !v.is_finite()) {
        return Err("Nonfinite actor DrawScale3D".into());
    }
    // Native COMISS/JC, not sign-bit masking: negative zero remains negative.
    let magnitude = |v: f32| if v < 0.0 { 0.0 - v } else { v };
    let (x, y, z) = (magnitude(x), magnitude(y), magnitude(z));
    let xy = if x < y { y } else { x };
    Ok((if xy < z { z } else { xy }).to_bits())
}
fn count(packed: u32) -> i32 {
    (packed.wrapping_shl(3) as i32) >> 3
}
fn store(bytes: &mut [u8], offset: usize, word: u32) {
    bytes[offset..offset + 4].copy_from_slice(&word.to_le_bytes());
}
fn word(bytes: &mut [u8], offset: usize, word: u16) {
    bytes[offset..offset + 2].copy_from_slice(&word.to_le_bytes());
}
#[derive(Clone, Debug, Serialize, Deserialize, PartialEq, Eq)]
pub struct State {
    pub packed_sections: u32,
    pub section_bytes: Vec<u8>,
    pub packed_stream_count: u32,
    pub stream: Vec<u8>,
    pub revision: u32,
    pub cache_index: [u32; 2],
}
#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct Input {
    pub state: State,
    pub packed_wedges: u32,
    /// Twelve-byte records: threshold/padding word, then raw U/V words.
    pub wedges: Vec<[u32; 3]>,
    pub packed_faces: u32,
    /// Eight-byte face records; word 3 selects the material group.
    pub faces: Vec<[u16; 4]>,
    /// Complete allocator answer for all new 80-byte sections, including untouched bytes.
    pub section_allocation: Vec<u8>,
}
#[derive(Clone, Debug, Serialize, PartialEq, Eq)]
pub struct Prepared {
    pub processed: bool,
    pub state: State,
}
impl Input {
    pub fn prepare(&self) -> Result<Prepared, String> {
        if self.state.packed_sections & 0x1fffffff != 0 {
            return Ok(Prepared {
                processed: false,
                state: self.state.clone(),
            });
        }
        let nw = count(self.packed_wedges);
        let nf = count(self.packed_faces);
        if !(0..=65536).contains(&nw)
            || !(0..=65536).contains(&nf)
            || !(0..=65536).contains(&count(self.state.packed_stream_count))
            || self.wedges.len() < nw as usize
            || self.faces.len() < nf as usize
        {
            return Err("Invalid bounded preprocessing source arrays".into());
        }
        let faces = &self.faces[..nf as usize];
        let runs = faces
            .iter()
            .enumerate()
            .filter(|(i, f)| *i == 0 || faces[*i - 1][3] != f[3])
            .count();
        if runs > 256 || self.section_allocation.len() != runs * 80 {
            return Err("Invalid preprocessing section allocation".into());
        }
        let mut state = self.state.clone();
        state.stream = vec![0; nw as usize * 32];
        state.packed_stream_count = (state.packed_stream_count & 0xe0000000) | nw as u32;
        state.revision = state.revision.wrapping_add(1);
        for (record, wedge) in state.stream.chunks_exact_mut(32).zip(&self.wedges) {
            store(record, 24, wedge[1]);
            store(record, 28, wedge[2]);
        }
        if runs != 0 {
            state.section_bytes = self.section_allocation.clone();
        }
        let mut cache = u64::from(state.cache_index[0]) | (u64::from(state.cache_index[1]) << 32);
        let mut section = 0usize;
        for (i, face) in faces.iter().enumerate() {
            if i == 0 || faces[i - 1][3] != face[3] {
                let record = &mut state.section_bytes[section * 80..(section + 1) * 80];
                store(record, 0x14, 0x1066a27c);
                let vertex_key = cache.wrapping_mul(256).wrapping_add(0xe1);
                cache = cache.wrapping_add(1);
                store(record, 0x18, vertex_key as u32);
                store(record, 0x1c, (vertex_key >> 32) as u32);
                for offset in [0x20, 0x24, 0x28, 0x2c, 0x30] {
                    store(record, offset, 0);
                }
                store(record, 0x38, 0x10665540);
                let index_key = cache.wrapping_mul(256).wrapping_add(0xe2);
                cache = cache.wrapping_add(1);
                store(record, 0x3c, index_key as u32);
                store(record, 0x40, (index_key >> 32) as u32);
                for offset in [0x44, 0x48, 0x4c] {
                    store(record, offset, 0);
                }
                word(record, 0, face[3]);
                word(record, 0xe, i as u16);
                word(record, 4, 0);
                word(record, 6, (nw as u16).wrapping_sub(1));
                word(record, 0x10, 0);
                section += 1;
            }
            let record = &mut state.section_bytes[(section - 1) * 80..section * 80];
            let length = u16::from_le_bytes([record[16], record[17]]).wrapping_add(1);
            word(record, 16, length);
        }
        state.packed_sections = (state.packed_sections & 0xe0000000) | runs as u32;
        state.cache_index = [cache as u32, (cache >> 32) as u32];
        Ok(Prepared {
            processed: true,
            state,
        })
    }
}
#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct Request {
    pub preprocessing: Input,
    pub actor_draw_scale: [u32; 3],
    pub mesh: scene_projection::Request,
}
#[derive(Clone, Debug, Serialize)]
pub struct Plan {
    pub preprocessing: Prepared,
    pub actor_abs_max: u32,
    pub mesh: scene_projection::Plan,
}
impl Runtime {
    pub fn submit_preprocessed_mesh(&mut self, request: &mut Request) -> Result<Plan, String> {
        let preprocessing = request.preprocessing.prepare()?;
        let actor_abs_max = abs_max(request.actor_draw_scale)?;
        let mut q = request.clone();
        q.preprocessing.state = preprocessing.state.clone();
        q.mesh.mesh.camera.actor_abs_max = actor_abs_max;
        let state = &preprocessing.state;
        q.mesh.mesh.gates.packed_sections = state.packed_sections;
        let geometry = &mut q.mesh.mesh.geometry.geometry;
        geometry.packed_count = state.packed_sections;
        geometry.section_bytes = state.section_bytes.clone();
        if preprocessing.processed {
            geometry.faces =
                q.preprocessing.faces[..count(q.preprocessing.packed_faces) as usize].to_vec();
            geometry.vertex_thresholds = q.preprocessing.wedges
                [..count(q.preprocessing.packed_wedges) as usize]
                .iter()
                .map(|w| w[0] as u16)
                .collect();
        }
        // UV preparation does not supply animated positions or replace GetFrame bytes.
        let mesh = self.submit_projected_mesh(&mut q.mesh)?;
        *request = q;
        Ok(Plan {
            preprocessing,
            actor_abs_max,
            mesh,
        })
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    fn input() -> Input {
        Input {
            state: State {
                packed_sections: 0xe0000000,
                section_bytes: vec![7; 80],
                packed_stream_count: 0xa0000003,
                stream: vec![9; 96],
                revision: u32::MAX,
                cache_index: [u32::MAX, 7],
            },
            packed_wedges: 2,
            wedges: vec![[0x12340001, 0x80000000, 0x7fc01234], [2, 3, 4]],
            packed_faces: 3,
            faces: vec![[0, 1, 0, 4], [1, 0, 1, 4], [0, 0, 1, 9]],
            section_allocation: vec![0xa5; 160],
        }
    }
    #[test]
    fn stream_reset_uv_and_revision() {
        let p = input().prepare().unwrap();
        assert_eq!(p.state.revision, 0);
        assert_eq!(p.state.packed_stream_count, 0xa0000002);
        assert_eq!(&p.state.stream[..24], &[0; 24]);
        assert_eq!(&p.state.stream[24..28], &0x80000000u32.to_le_bytes());
        assert_eq!(&p.state.stream[28..32], &0x7fc01234u32.to_le_bytes());
    }
    #[test]
    fn consecutive_groups_form_sections() {
        let p = input().prepare().unwrap();
        assert_eq!(p.state.packed_sections, 0xe0000002);
        assert_eq!(&p.state.section_bytes[14..18], &[0, 0, 2, 0]);
        assert_eq!(&p.state.section_bytes[94..98], &[2, 0, 1, 0]);
    }
    #[test]
    fn material_reappearance_is_a_new_section() {
        let mut i = input();
        i.faces[1][3] = 9;
        i.faces[2][3] = 4;
        i.section_allocation = vec![0xa5; 240];
        assert_eq!(i.prepare().unwrap().state.packed_sections, 0xe0000003);
    }
    #[test]
    fn untouched_bytes_are_preserved() {
        let p = input().prepare().unwrap();
        for offset in [2, 3, 8, 9, 10, 11, 12, 13, 18, 19, 52, 53, 54, 55] {
            assert_eq!(p.state.section_bytes[offset], 0xa5);
        }
    }
    #[test]
    fn cache_counter_carries_and_keys_have_distinct_tags() {
        let p = input().prepare().unwrap();
        assert_eq!(p.state.cache_index, [3, 8]);
        let key = |o| u64::from_le_bytes(p.state.section_bytes[o..o + 8].try_into().unwrap());
        assert_eq!(key(24), 0x7ffffffffu64.wrapping_mul(256) + 0xe1);
        assert_eq!(key(60), 0x800000000u64 * 256 + 0xe2);
    }
    #[test]
    fn cache_counter_wraps_u64() {
        let mut i = input();
        i.state.cache_index = [u32::MAX; 2];
        assert_eq!(i.prepare().unwrap().state.cache_index, [3, 0]);
    }
    #[test]
    fn existing_sections_bypass_invalid_sources() {
        let mut i = input();
        i.state.packed_sections = 0x10000000;
        i.packed_wedges = u32::MAX;
        i.wedges.clear();
        let p = i.prepare().unwrap();
        assert!(!p.processed);
        assert_eq!(p.state, i.state);
    }
    #[test]
    fn empty_sources_still_increment_revision() {
        let mut i = input();
        i.packed_wedges = 0;
        i.packed_faces = 0;
        i.section_allocation.clear();
        let p = i.prepare().unwrap();
        assert!(p.state.stream.is_empty());
        assert_eq!(p.state.revision, 0);
        assert_eq!(p.state.packed_sections, 0xe0000000);
    }
    #[test]
    fn zero_wedges_give_word_max_vertex() {
        let mut i = input();
        i.packed_wedges = 0;
        let p = i.prepare().unwrap();
        assert_eq!(&p.state.section_bytes[6..8], &[255, 255]);
    }
    #[test]
    fn negative_counts_and_short_allocations_are_errors() {
        let mut i = input();
        i.packed_wedges = 0x1fffffff;
        assert!(i.prepare().is_err());
        i = input();
        i.section_allocation.pop();
        assert!(i.prepare().is_err());
    }
    #[test]
    fn abs_max_handles_negative_and_ties() {
        assert_eq!(
            abs_max([(-3f32).to_bits(), 3f32.to_bits(), 2f32.to_bits()]).unwrap(),
            3f32.to_bits()
        );
    }
    #[test]
    fn abs_max_preserves_x_zero_on_equal_comparisons() {
        assert_eq!(abs_max([0x80000000, 0, 0]).unwrap(), 0x80000000);
        assert_eq!(abs_max([0, 0x80000000, 0]).unwrap(), 0);
    }
    #[test]
    fn abs_max_rejects_nonfinite() {
        assert!(abs_max([0, 0x7f800000, 0]).is_err());
    }
    #[test]
    fn run_length_wraps_to_word_zero() {
        let mut i = input();
        i.packed_faces = 65536;
        i.faces = vec![[0, 0, 0, 1]; 65536];
        i.section_allocation = vec![7; 80];
        assert_eq!(&i.prepare().unwrap().state.section_bytes[16..18], &[0, 0]);
    }
}
