//! Original skeletal section/index selection with mesh-local diagnostic geometry.
//! Native actor transforms, material effects and GPU arithmetic remain external.
use crate::{
    skeletal_lod::StoredLod,
    skeletal_render_product::rigid_render_product,
    skeletal_skin::{SkinStreamVertex, SkinVertex},
};
use serde::Serialize;
#[derive(Clone, Debug, Serialize, PartialEq, Eq)]
pub struct DrawSection {
    pub bank: usize,
    pub section_index: usize,
    pub material: u16,
    pub bone: Option<u16>,
    pub first_index: usize,
    pub triangle_count: usize,
    pub min_vertex: u16,
    pub max_vertex: u16,
}
#[derive(Debug)]
pub struct DrawTriangle {
    pub section: usize,
    pub indices: [u16; 3],
    pub vertices: [SkinStreamVertex; 3],
}
#[derive(Debug)]
pub struct SkeletalDraw {
    pub sections: Vec<DrawSection>,
    pub triangles: Vec<DrawTriangle>,
    pub unused_indices: [usize; 2],
}
/// Soft first indices are cumulative; rigid first indices are stored at native +2.
/// Native +0/+c/+12 are material/bone/primitive count. +4/+6 are vertex bounds.
pub fn draw_sections(lod: &StoredLod) -> Vec<DrawSection> {
    let mut result = vec![];
    for bank in 0..2 {
        let mut cursor = 0;
        for (i, row) in lod.sections[bank].iter().enumerate() {
            let count = row[8] as usize;
            result.push(DrawSection {
                bank,
                section_index: i,
                material: row[0],
                bone: if bank == 1 { Some(row[5]) } else { None },
                first_index: if bank == 0 { cursor } else { row[1] as usize },
                triangle_count: count,
                min_vertex: row[2],
                max_vertex: row[3],
            });
            cursor += count * 3;
        }
    }
    result
}
/// CPU diagnostic point/vector transform; GPU rounding/normal treatment is not asserted.
pub fn transform_rigid_stream_vertex(words: [u32; 8], matrix: [u32; 16]) -> SkinStreamVertex {
    let p = [words[0], words[1], words[2]].map(f32::from_bits);
    let n = [words[3], words[4], words[5]].map(f32::from_bits);
    let m = matrix.map(f32::from_bits);
    SkinStreamVertex {
        vertex: SkinVertex {
            position: std::array::from_fn(|i| {
                (((p[0] * m[i] + p[1] * m[4 + i]) + p[2] * m[8 + i]) + m[12 + i]).to_bits()
            }),
            normal: std::array::from_fn(|i| {
                ((n[0] * m[i] + n[1] * m[4 + i]) + n[2] * m[8 + i]).to_bits()
            }),
        },
        uv: [words[6], words[7]],
    }
}
pub fn build_skeletal_draw(
    lod: &StoredLod,
    soft: &[SkinStreamVertex],
    poses: &[[u32; 16]],
    inverse: &[[u32; 16]],
) -> Result<SkeletalDraw, String> {
    let sections = draw_sections(lod);
    let mut triangles = vec![];
    let mut used = [
        vec![false; lod.indices[0].len()],
        vec![false; lod.indices[1].len()],
    ];
    for (si, s) in sections.iter().enumerate() {
        if s.triangle_count == 0 {
            continue;
        }
        if s.min_vertex > s.max_vertex {
            return Err("skeletal section has reversed vertex bounds".into());
        }
        let end = s
            .first_index
            .checked_add(s.triangle_count * 3)
            .ok_or("skeletal section index overflow")?;
        let indices = lod.indices[s.bank]
            .get(s.first_index..end)
            .ok_or("skeletal section index range exceeds buffer")?;
        let matrix = if let Some(bone) = s.bone {
            Some(rigid_render_product(
                *inverse
                    .get(bone as usize)
                    .ok_or("rigid section lacks inverse bone")?,
                *poses
                    .get(bone as usize)
                    .ok_or("rigid section lacks pose bone")?,
            ))
        } else {
            None
        };
        for raw in indices.chunks_exact(3) {
            let mut vertices = [SkinStreamVertex::default(); 3];
            for (i, &index) in raw.iter().enumerate() {
                if index < s.min_vertex || index > s.max_vertex {
                    return Err("skeletal index exceeds section vertex bounds".into());
                }
                vertices[i] = if let Some(m) = matrix {
                    transform_rigid_stream_vertex(
                        *lod.stream_vertices
                            .get(index as usize)
                            .ok_or("rigid index exceeds vertex stream")?,
                        m,
                    )
                } else {
                    *soft
                        .get(index as usize)
                        .ok_or("soft index exceeds skinned output")?
                };
            }
            triangles.push(DrawTriangle {
                section: si,
                indices: raw.try_into().unwrap(),
                vertices,
            });
        }
        used[s.bank][s.first_index..end].fill(true);
    }
    Ok(SkeletalDraw {
        sections,
        triangles,
        unused_indices: used.map(|a| a.iter().filter(|&&v| !v).count()),
    })
}

#[cfg(test)]
mod tests {
    use super::*;
    fn identity() -> [u32; 16] {
        std::array::from_fn(|i| if i / 4 == i % 4 { 1f32.to_bits() } else { 0 })
    }
    fn fixture() -> StoredLod {
        StoredLod {
            offset: 0,
            end_offset: 0,
            version: 1,
            commands: vec![],
            bind_vertices: vec![],
            word_14: 3,
            sections: [
                vec![[2, 99, 0, 2, 0, 99, 0, 0, 1]],
                vec![[4, 3, 0, 2, 0, 0, 0, 0, 1]],
            ],
            indices: [vec![2, 0, 1], vec![99, 99, 99, 0, 1, 2, 88]],
            index_words: [0; 2],
            stream_words: [0; 3],
            stream_vertices: vec![
                [0, 0, 0, 0, 0, 1f32.to_bits(), 0, 0],
                [
                    1f32.to_bits(),
                    0,
                    0,
                    0,
                    0,
                    1f32.to_bits(),
                    1f32.to_bits(),
                    0,
                ],
                [
                    0,
                    1f32.to_bits(),
                    0,
                    0,
                    0,
                    1f32.to_bits(),
                    0,
                    1f32.to_bits(),
                ],
            ],
            lazy_offsets: [0; 4],
            lazy_ends: [0; 4],
            influences: vec![],
            wedges: vec![],
            faces: vec![],
            points: vec![],
            tail_words: [0; 6],
        }
    }
    fn soft() -> [SkinStreamVertex; 3] {
        [SkinStreamVertex {
            vertex: SkinVertex {
                position: [0, 0, 2f32.to_bits()],
                normal: [0; 3],
            },
            uv: [17, 19],
        }; 3]
    }
    #[test]
    fn native_section_fields_and_cumulative_soft_indices() {
        let mut lod = fixture();
        lod.sections[0].push([7, 999, 0, 2, 0, 999, 0, 0, 2]);
        let s = draw_sections(&lod);
        assert_eq!((s[0].material, s[0].bone, s[0].first_index), (2, None, 0));
        assert_eq!(
            (s[1].material, s[1].first_index, s[1].triangle_count),
            (7, 3, 2)
        );
        assert_eq!(
            (s[2].material, s[2].bone, s[2].first_index),
            (4, Some(0), 3)
        );
    }
    #[test]
    fn both_banks_select_correct_vertices_and_retain_unused_indices() {
        let draw = build_skeletal_draw(&fixture(), &soft(), &[identity()], &[identity()]).unwrap();
        assert_eq!(draw.triangles.len(), 2);
        assert_eq!(draw.unused_indices, [0, 4]);
        assert_eq!(draw.triangles[0].indices, [2, 0, 1]);
        assert_eq!(draw.triangles[0].vertices[0].uv, [17, 19]);
        assert_eq!(draw.triangles[1].indices, [0, 1, 2]);
        assert_eq!(
            draw.triangles[1].vertices[1].vertex.position,
            [1f32.to_bits(), 0, 0]
        );
    }
    #[test]
    fn empty_command_rigid_only_lod_needs_no_soft_vertices() {
        let mut lod = fixture();
        lod.sections[0].clear();
        let draw = build_skeletal_draw(&lod, &[], &[identity()], &[identity()]).unwrap();
        assert_eq!(draw.triangles.len(), 1);
        assert_eq!(draw.unused_indices, [3, 4]);
    }
    #[test]
    fn section_index_range_and_vertex_bounds_rejected() {
        let mut lod = fixture();
        lod.sections[0][0][8] = 2;
        assert!(build_skeletal_draw(&lod, &soft(), &[identity()], &[identity()]).is_err());
        lod = fixture();
        lod.sections[0][0][3] = 1;
        assert!(build_skeletal_draw(&lod, &soft(), &[identity()], &[identity()]).is_err());
        lod.sections[0][0][2] = 2;
        assert!(build_skeletal_draw(&lod, &soft(), &[identity()], &[identity()]).is_err());
    }
    #[test]
    fn missing_soft_and_rigid_stream_vertices_rejected() {
        assert!(build_skeletal_draw(&fixture(), &[], &[identity()], &[identity()]).is_err());
        let mut lod = fixture();
        lod.stream_vertices.clear();
        assert!(build_skeletal_draw(&lod, &soft(), &[identity()], &[identity()]).is_err());
    }
    #[test]
    fn missing_rigid_pose_or_inverse_rejected() {
        assert!(build_skeletal_draw(&fixture(), &soft(), &[], &[identity()]).is_err());
        assert!(build_skeletal_draw(&fixture(), &soft(), &[identity()], &[]).is_err());
    }
    #[test]
    fn zero_triangle_sections_do_not_dereference_bones_or_ranges() {
        let mut lod = fixture();
        lod.sections[0].clear();
        lod.sections[1][0][8] = 0;
        lod.sections[1][0][5] = 65535;
        let draw = build_skeletal_draw(&lod, &[], &[], &[]).unwrap();
        assert!(draw.triangles.is_empty());
        assert_eq!(draw.unused_indices, [3, 7]);
    }
    #[test]
    fn rigid_inverse_pose_order_translation_and_vector_uv_behavior() {
        let mut inv = identity();
        inv[12] = 2f32.to_bits();
        let mut pose = identity();
        pose[0] = 3f32.to_bits();
        let product = rigid_render_product(inv, pose);
        assert_eq!(product[12], 6f32.to_bits());
        let v = transform_rigid_stream_vertex(
            [
                1f32.to_bits(),
                0,
                0,
                1f32.to_bits(),
                0,
                0,
                0x80000000,
                0x7fc01234,
            ],
            product,
        );
        assert_eq!(v.vertex.position, [9f32.to_bits(), 0, 0]);
        assert_eq!(v.vertex.normal, [3f32.to_bits(), 0, 0]);
        assert_eq!(v.uv, [0x80000000, 0x7fc01234]);
    }
}
