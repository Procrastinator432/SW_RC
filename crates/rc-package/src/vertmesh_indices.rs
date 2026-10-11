//! Engine!10560378..105604c6: LOD-filtered vertex-mesh index construction.
//! The LOD integer, source arrays and generated vertex stream are inputs.
use crate::{d3d_dynamic_draw::Runtime, d3d_mesh_draw as mesh};
use serde::{Deserialize, Serialize};

#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct Input {
    pub packed_count: u32,
    pub section_bytes: Vec<u8>,
    pub lod: i32,
    pub face_thresholds: Vec<u16>,
    /// Original eight-byte face records. Word 3 is not used by this loop.
    pub faces: Vec<[u16; 4]>,
    pub collapse: Vec<u16>,
    /// Original vertex record's WORD at vertex*12; remaining bytes are unused.
    pub vertex_thresholds: Vec<u16>,
}
#[derive(Clone, Debug, Serialize)]
pub struct Geometry {
    pub sections: mesh::Sections,
    pub indices: Vec<u16>,
}
impl Geometry {
    pub fn payload(&self) -> Vec<u8> {
        self.indices.iter().flat_map(|i| i.to_le_bytes()).collect()
    }
}
fn count(packed: u32) -> usize {
    (((packed.wrapping_shl(3) as i32) >> 3).max(0)) as usize
}
impl Input {
    pub fn resolve(&self, mut vertex: u16) -> Result<u16, String> {
        let mut visited = vec![];
        loop {
            let threshold = *self
                .vertex_thresholds
                .get(usize::from(vertex))
                .ok_or("Missing vertex LOD threshold")?;
            if i32::from(threshold) < self.lod {
                return Ok(vertex);
            }
            if visited.len() >= 1024 || visited.contains(&vertex) {
                return Err("Cyclic or excessive vertex collapse chain".into());
            }
            visited.push(vertex);
            vertex = *self
                .collapse
                .get(usize::from(vertex))
                .ok_or("Missing vertex collapse target")?;
        }
    }
    pub fn build(&self) -> Result<Geometry, String> {
        let n = count(self.packed_count);
        if n > 256 || self.section_bytes.len() < n * 80 {
            return Err("Invalid bounded vertex-mesh section image".into());
        }
        let mut sections = mesh::Sections {
            packed_count: self.packed_count,
            bytes: self.section_bytes.clone(),
            primitive_counts: vec![],
        };
        let mut indices = vec![];
        let mut candidates = 0usize;
        for section in sections.bytes.chunks_exact_mut(80).take(n) {
            let start = u16::from_le_bytes([section[14], section[15]]) as usize;
            let len = u16::from_le_bytes([section[16], section[17]]) as usize;
            candidates += len;
            if candidates > 65536 {
                return Err("Mesh face iteration exceeds safe limit".into());
            }
            // Native MOV word ptr truncates the accumulated index count.
            section[2..4].copy_from_slice(&(indices.len() as u16).to_le_bytes());
            let mut emitted = 0u32;
            for face in start..start + len {
                let threshold = *self
                    .face_thresholds
                    .get(face)
                    .ok_or("Missing face LOD threshold")?;
                if i32::from(threshold) > self.lod {
                    continue;
                }
                let words = self
                    .faces
                    .get(face)
                    .ok_or("Missing vertex-mesh face record")?;
                for vertex in [words[2], words[1], words[0]] {
                    indices.push(self.resolve(vertex)?);
                }
                emitted += 1;
            }
            sections.primitive_counts.push(emitted);
        }
        Ok(Geometry { sections, indices })
    }
}
#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct Request {
    pub geometry: Input,
    pub mesh: mesh::Request,
}
#[derive(Clone, Debug, Serialize)]
pub struct Plan {
    pub geometry: Geometry,
    pub draw: mesh::Plan,
}
impl Runtime {
    /// Generated raw u16 indices replace the supplied index callback payload.
    /// All geometry/header, upload, material and pass mutations commit together.
    pub fn submit_geometry(&mut self, request: &mut Request) -> Result<Plan, String> {
        let geometry = request.geometry.build()?;
        let mut q = request.clone();
        let mut next = self.clone();
        q.mesh.sections = geometry.sections.clone();
        let payload = geometry.payload();
        let size = payload.len() as u32;
        q.mesh.index.source.width = 2;
        q.mesh.index.source.size = size;
        q.mesh.index.source.payload = payload;
        q.mesh.index.reported_size = size;
        let draw = next.submit_mesh(&mut q.mesh)?;
        *self = next;
        *request = q;
        Ok(Plan { geometry, draw })
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    fn input() -> Input {
        let mut section_bytes = vec![0xa5; 80];
        section_bytes[14..18].copy_from_slice(&[0, 0, 2, 0]);
        Input {
            packed_count: 1,
            section_bytes,
            lod: 3,
            face_thresholds: vec![3, 4],
            faces: vec![[0, 1, 2, 65535], [2, 1, 0, 0]],
            collapse: vec![0, 0, 1],
            vertex_thresholds: vec![0, 2, 3],
        }
    }
    #[test]
    fn threshold_equal_to_lod_includes_face() {
        assert_eq!(input().build().unwrap().sections.primitive_counts, vec![1]);
    }
    #[test]
    fn original_face_words_are_emitted_in_reverse_order() {
        let mut i = input();
        i.vertex_thresholds = vec![0; 3];
        assert_eq!(i.build().unwrap().indices, vec![2, 1, 0]);
    }
    #[test]
    fn vertex_threshold_equal_to_lod_collapses() {
        assert_eq!(input().resolve(2).unwrap(), 1);
    }
    #[test]
    fn multi_hop_chain_stops_strictly_below_lod() {
        let mut i = input();
        i.vertex_thresholds[1] = 4;
        assert_eq!(i.resolve(2).unwrap(), 0);
    }
    #[test]
    fn collapse_cycle_is_rejected() {
        let mut i = input();
        i.collapse[2] = 2;
        assert!(i.resolve(2).is_err());
    }
    #[test]
    fn missing_collapse_link_is_rejected() {
        let mut i = input();
        i.collapse.clear();
        assert!(i.resolve(2).is_err());
    }
    #[test]
    fn negative_lod_filters_without_reading_face_vertices() {
        let mut i = input();
        i.lod = -1;
        i.faces.clear();
        assert!(i.build().unwrap().indices.is_empty());
    }
    #[test]
    fn signed_nonpositive_count_does_not_access_arrays() {
        let mut i = input();
        i.packed_count = 0x1fffffff;
        i.section_bytes.clear();
        assert!(i.build().unwrap().indices.is_empty());
    }
    #[test]
    fn section_starts_and_counts_accumulate_separately() {
        let mut i = input();
        i.section_bytes.extend(i.section_bytes.clone());
        i.packed_count = 0xe0000002;
        let g = i.build().unwrap();
        assert_eq!(g.sections.primitive_counts, vec![1, 1]);
        assert_eq!(&g.sections.bytes[82..84], &[3, 0]);
    }
    #[test]
    fn empty_section_still_gets_current_first_index() {
        let mut i = input();
        i.section_bytes.extend(i.section_bytes.clone());
        i.packed_count = 2;
        i.section_bytes[96] = 0;
        let g = i.build().unwrap();
        assert_eq!(g.sections.primitive_counts, vec![1, 0]);
        assert_eq!(&g.sections.bytes[82..84], &[3, 0]);
    }
    #[test]
    fn build_preserves_other_section_bytes() {
        let i = input();
        let mut g = i.build().unwrap().sections.bytes;
        g[2..4].copy_from_slice(&i.section_bytes[2..4]);
        assert_eq!(g, i.section_bytes);
    }
    #[test]
    fn payload_is_little_endian_u16() {
        let mut i = input();
        i.faces[0] = [256, 257, 258, 0];
        i.vertex_thresholds = vec![0; 259];
        assert_eq!(i.build().unwrap().payload(), vec![2, 1, 1, 1, 0, 1]);
    }
    #[test]
    fn missing_face_threshold_and_record_are_checked() {
        let mut i = input();
        i.face_thresholds.clear();
        assert!(i.build().is_err());
        i.face_thresholds = vec![0, 0];
        i.faces.clear();
        assert!(i.build().is_err());
    }
    #[test]
    fn first_index_truncates_to_native_word() {
        let mut i = input();
        i.section_bytes[16..18].copy_from_slice(&21846u16.to_le_bytes());
        i.section_bytes.extend(i.section_bytes.clone());
        i.section_bytes[96..98].copy_from_slice(&[0, 0]);
        i.packed_count = 2;
        i.face_thresholds = vec![0; 21846];
        i.faces = vec![[0; 4]; 21846];
        let g = i.build().unwrap();
        assert_eq!(&g.sections.bytes[82..84], &[2, 0]);
    }
    #[test]
    fn section_image_and_positive_count_are_bounded() {
        let mut i = input();
        i.section_bytes.clear();
        assert!(i.build().is_err());
        i.packed_count = 257;
        assert!(i.build().is_err());
    }
}
