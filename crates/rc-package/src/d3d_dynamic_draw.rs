//! Engine caller offset routing and composition with dynamic uploads/material passes.
//! Caller geometry generation, source callbacks and material selection remain inputs.
use crate::{
    d3d_buffers::{Index, State},
    d3d_complete::{Complete, Lights},
    d3d_draw::{self, Counters, Draw, PreparedPass, RenderedPass},
    d3d_pools::{self, IndexRequest, VertexRequest},
};
use serde::{Deserialize, Serialize};
/// Resolved section inputs; geometry generation and per-section material lookup
/// happen outside this adapter. Native zero count/word gates are retained.
#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct MeshSection {
    pub first_index: u16,
    pub min_vertex: u16,
    pub max_vertex: u16,
    pub primitive_count: u32,
    pub enabled_word: u16,
}
#[derive(Clone, Debug, Serialize, Deserialize)]
#[serde(tag = "kind", rename_all = "snake_case")]
pub enum Caller {
    Grid {
        columns: u32,
        rows: u32,
    },
    Fluid {
        columns: u32,
        rows: u32,
        quad: bool,
    },
    Beam {
        segments: u32,
        copies: u32,
        vertices: u32,
        indices: u32,
        primitives: u32,
    },
    Canvas {
        primitive: u32,
        primitive_count: u32,
        packed_vertices: u32,
        alternate_vertices: u32,
        alternate: bool,
    },
    Lines {
        packed_vertices: u32,
    },
    Ribbon {
        pairs: u32,
    },
    Spark {
        sparks: u32,
        points: u32,
    },
    Sprite {
        sprites: u32,
    },
    Trail {
        primitives: u32,
        vertices: u32,
    },
    VertMesh {
        sections: Vec<MeshSection>,
    },
}
fn count(packed: u32) -> u32 {
    ((packed.wrapping_shl(3) as i32) >> 3) as u32
}
fn division(n: u32, d: u32) -> Result<u32, String> {
    (n as i32)
        .checked_div(d as i32)
        .map(|v| v as u32)
        .ok_or("Native caller IDIV zero divisor or overflow".into())
}
impl Caller {
    pub fn enabled(&self) -> bool {
        match self {
            Self::Canvas {
                primitive_count, ..
            } => (*primitive_count as i32) > 0,
            Self::Lines { packed_vertices } => packed_vertices & 0x1fffffff != 0,
            _ => true,
        }
    }
    pub fn indexed(&self) -> bool {
        matches!(
            self,
            Self::Grid { .. }
                | Self::Fluid { .. }
                | Self::Beam { .. }
                | Self::Ribbon { .. }
                | Self::Sprite { .. }
                | Self::Trail { .. }
                | Self::VertMesh { .. }
        )
    }
    pub fn draws(&self, vertex: u32, index: Option<u32>) -> Result<Vec<Draw>, String> {
        if !self.enabled() {
            return Ok(vec![]);
        }
        let start = if self.indexed() {
            index.ok_or("Indexed caller requires dynamic index upload")?
        } else {
            vertex
        };
        let make = |primitive, primitive_count, max_vertex, start| Draw {
            primitive,
            start,
            primitive_count,
            min_vertex: 0,
            max_vertex,
            indexed: self.indexed(),
        };
        if let Self::VertMesh { sections } = self {
            if sections.len() > 256 {
                return Err("Mesh section count exceeds bounded draw list".into());
            }
            return Ok(sections
                .iter()
                .filter(|s| s.primitive_count != 0 && s.enabled_word != 0)
                .map(|s| Draw {
                    primitive: 5,
                    start: start.wrapping_add(u32::from(s.first_index)),
                    primitive_count: s.primitive_count,
                    min_vertex: u32::from(s.min_vertex),
                    max_vertex: u32::from(s.max_vertex),
                    indexed: true,
                })
                .collect());
        }
        Ok(match *self {
            Self::Grid { columns, rows }
            | Self::Fluid {
                columns,
                rows,
                quad: false,
            } => vec![make(
                5,
                rows.wrapping_mul(2)
                    .wrapping_sub(2)
                    .wrapping_mul(columns.wrapping_sub(1)),
                columns.wrapping_mul(rows).wrapping_sub(1),
                start,
            )],
            Self::Fluid { quad: true, .. } => vec![make(5, 2, 3, start)],
            Self::Beam {
                segments,
                copies,
                vertices,
                indices,
                primitives,
            } => {
                if (segments as i32) <= 0 {
                    vec![]
                } else {
                    if segments > 256 {
                        return Err("Beam segment count exceeds bounded draw list".into());
                    }
                    let primitive_count = division(primitives.wrapping_mul(copies), segments)?;
                    let stride = division(indices.wrapping_mul(copies), segments)?;
                    (0..segments)
                        .map(|j| {
                            make(
                                5,
                                primitive_count,
                                vertices.wrapping_mul(copies).wrapping_sub(1),
                                start.wrapping_add(stride.wrapping_mul(j)),
                            )
                        })
                        .collect()
                }
            }
            Self::Canvas {
                primitive,
                primitive_count,
                packed_vertices,
                alternate_vertices,
                alternate,
            } => vec![make(
                primitive,
                primitive_count,
                count(if alternate {
                    alternate_vertices
                } else {
                    packed_vertices
                })
                .wrapping_sub(1),
                start,
            )],
            Self::Lines { packed_vertices } => vec![make(
                2,
                division(count(packed_vertices), 2)?,
                count(packed_vertices).wrapping_sub(1),
                start,
            )],
            Self::Ribbon { pairs } => vec![make(
                5,
                pairs.wrapping_mul(2),
                pairs.wrapping_mul(2).wrapping_sub(1),
                start,
            )],
            Self::Spark { sparks, points } => vec![Draw {
                min_vertex: u32::MAX,
                ..make(2, sparks.wrapping_mul(points), u32::MAX, start)
            }],
            Self::Sprite { sprites } => vec![make(
                5,
                sprites.wrapping_mul(2),
                sprites.wrapping_mul(4).wrapping_sub(1),
                start,
            )],
            Self::Trail {
                primitives,
                vertices,
            } => vec![make(5, primitives, vertices.wrapping_sub(1), start)],
            Self::VertMesh { .. } => unreachable!(),
        })
    }
}
#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct Request {
    pub caller: Caller,
    pub vertex: VertexRequest,
    pub index: Option<IndexRequest>,
    pub passes: Vec<PreparedPass>,
    pub context: d3d_draw::Context,
}
#[derive(Clone, Debug, Serialize, Deserialize, PartialEq, Eq)]
pub struct Runtime {
    pub buffers: d3d_pools::Runtime,
    pub state: State,
    pub lights: Lights,
    pub last_pass: u32,
    pub counters: Counters,
    pub vertex_target: Vec<u8>,
    pub index_target: Vec<u8>,
}
#[derive(Clone, Debug, Serialize)]
pub struct Plan {
    pub vertex: Option<d3d_pools::Transfer>,
    pub index: Option<d3d_pools::Transfer>,
    pub draws: Vec<Draw>,
    pub rendered: Vec<Vec<RenderedPass>>,
}
impl Runtime {
    /// Caller scope begins after geometry/material decisions. Entire model transaction
    /// includes both uploads, explicit nonindexed unbind and every material pass.
    pub fn submit(&mut self, request: &mut Request) -> Result<Plan, String> {
        if !request.caller.enabled() {
            return Ok(Plan {
                vertex: None,
                index: None,
                draws: vec![],
                rendered: vec![],
            });
        }
        if request.caller.indexed() != request.index.is_some() {
            return Err("Caller/index request mismatch".into());
        }
        // Check arithmetic and bounds before cloning or planning allocations.
        request.caller.draws(0, Some(0))?;
        let mut next = self.clone();
        let mut q = request.clone();
        let vertex = next
            .buffers
            .vertex(&mut next.state, &q.vertex, &mut next.vertex_target)?;
        let index = if let Some(i) = &mut q.index {
            i.base = vertex.offset;
            Some(
                next.buffers
                    .index(&mut next.state, i, &mut next.index_target)?,
            )
        } else {
            next.state.set_index(
                &mut next.buffers.deferred,
                Index {
                    source: 0,
                    size: 0,
                    source_revision: 0,
                    cached_revision: 0,
                    wrapper: 0,
                    handle: 0,
                },
                0,
                0,
            )?;
            None
        };
        let draws = q
            .caller
            .draws(vertex.offset, index.as_ref().map(|i| i.offset))?;
        let mut complete = Complete {
            deferred: next.buffers.deferred.clone(),
            lights: next.lights.clone(),
        };
        let mut rendered = vec![];
        for draw in &draws {
            rendered.push(d3d_draw::render(
                &mut complete,
                &mut next.last_pass,
                &mut next.counters,
                &mut q.passes,
                q.context,
                *draw,
            )?);
        }
        next.buffers.deferred = complete.deferred;
        next.lights = complete.lights;
        *self = next;
        *request = q;
        Ok(Plan {
            vertex: Some(vertex),
            index,
            draws,
            rendered,
        })
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn grid_routes_index_offset_and_local_range() {
        let d = Caller::Grid {
            columns: 5,
            rows: 3,
        }
        .draws(100, Some(17))
        .unwrap()[0];
        assert_eq!(
            (d.start, d.primitive_count, d.min_vertex, d.max_vertex),
            (17, 16, 0, 14)
        );
        assert!(d.indexed);
    }
    #[test]
    fn grid_native_wrapping_arithmetic() {
        let d = Caller::Grid {
            columns: 0,
            rows: 0,
        }
        .draws(0, Some(0))
        .unwrap()[0];
        assert_eq!((d.primitive_count, d.max_vertex), (2, u32::MAX));
    }
    #[test]
    fn fluid_grid_matches_regular_grid() {
        assert_eq!(
            Caller::Fluid {
                columns: 4,
                rows: 7,
                quad: false
            }
            .draws(1, Some(2))
            .unwrap(),
            Caller::Grid {
                columns: 4,
                rows: 7
            }
            .draws(1, Some(2))
            .unwrap()
        );
    }
    #[test]
    fn fluid_quad_uses_fixed_range() {
        let d = Caller::Fluid {
            columns: 0,
            rows: 99,
            quad: true,
        }
        .draws(100, Some(3))
        .unwrap()[0];
        assert_eq!((d.start, d.primitive_count, d.max_vertex), (3, 2, 3));
    }
    #[test]
    fn beam_offsets_truncate_signed_division() {
        let d = Caller::Beam {
            segments: 3,
            copies: 2,
            vertices: 7,
            indices: 10,
            primitives: 5,
        }
        .draws(100, Some(17))
        .unwrap();
        assert_eq!(
            d.iter().map(|d| d.start).collect::<Vec<_>>(),
            vec![17, 23, 29]
        );
        assert!(d
            .iter()
            .all(|d| d.primitive_count == 3 && d.max_vertex == 13));
    }
    #[test]
    fn beam_zero_segments_has_no_draws() {
        assert!(Caller::Beam {
            segments: 0,
            copies: 1,
            vertices: 7,
            indices: 0,
            primitives: 1
        }
        .draws(1, Some(2))
        .unwrap()
        .is_empty());
    }
    #[test]
    fn beam_negative_segments_has_no_draws() {
        assert!(Caller::Beam {
            segments: u32::MAX,
            copies: 1,
            vertices: 7,
            indices: 0,
            primitives: 1
        }
        .draws(1, Some(2))
        .unwrap()
        .is_empty());
    }
    #[test]
    fn beam_positive_segments_are_bounded() {
        assert!(Caller::Beam {
            segments: 257,
            copies: 1,
            vertices: 7,
            indices: 1,
            primitives: 1
        }
        .draws(1, Some(2))
        .is_err());
    }
    #[test]
    fn beam_product_wraps_before_signed_division() {
        let d = Caller::Beam {
            segments: 3,
            copies: 2,
            vertices: 7,
            indices: 0x80000001,
            primitives: u32::MAX,
        }
        .draws(100, Some(u32::MAX))
        .unwrap();
        assert_eq!(d[0].primitive_count, 0);
        assert_eq!(d[2].start, u32::MAX);
    }
    #[test]
    fn canvas_uses_vertex_offset_and_primary_count() {
        let d = Caller::Canvas {
            primitive: 5,
            primitive_count: 2,
            packed_vertices: 0xe0000006,
            alternate_vertices: 99,
            alternate: false,
        }
        .draws(123, None)
        .unwrap()[0];
        assert_eq!((d.start, d.max_vertex), (123, 5));
        assert!(!d.indexed);
    }
    #[test]
    fn canvas_alternate_count_is_independent() {
        let d = Caller::Canvas {
            primitive: 6,
            primitive_count: 2,
            packed_vertices: 6,
            alternate_vertices: 0xa0000009,
            alternate: true,
        }
        .draws(123, None)
        .unwrap()[0];
        assert_eq!((d.primitive, d.max_vertex), (6, 8));
    }
    #[test]
    fn canvas_nonpositive_primitive_count_skips() {
        assert!(Caller::Canvas {
            primitive: 5,
            primitive_count: u32::MAX,
            packed_vertices: 6,
            alternate_vertices: 0,
            alternate: false
        }
        .draws(0, None)
        .unwrap()
        .is_empty());
    }
    #[test]
    fn lines_divide_signed_count_toward_zero() {
        let d = Caller::Lines {
            packed_vertices: 0x1ffffffb,
        }
        .draws(7, None)
        .unwrap()[0];
        assert_eq!((d.primitive_count, d.max_vertex), (0xfffffffe, 0xfffffffa));
        assert!(!d.indexed);
    }
    #[test]
    fn lines_zero_lower_count_skips_regardless_flags() {
        assert!(Caller::Lines {
            packed_vertices: 0xe0000000
        }
        .draws(7, None)
        .unwrap()
        .is_empty());
    }
    #[test]
    fn indexed_caller_requires_index_offset() {
        assert!(Caller::Grid {
            columns: 2,
            rows: 2
        }
        .draws(7, None)
        .is_err());
    }
    fn one(caller: Caller) -> Draw {
        caller.draws(101, Some(17)).unwrap()[0]
    }
    fn section() -> MeshSection {
        MeshSection {
            first_index: 3,
            min_vertex: 7,
            max_vertex: 19,
            primitive_count: 4,
            enabled_word: 1,
        }
    }
    #[test]
    fn ribbon_uses_two_vertices_and_primitives_per_pair() {
        let d = one(Caller::Ribbon { pairs: 9 });
        assert_eq!((d.start, d.primitive_count, d.max_vertex), (17, 18, 17));
    }
    #[test]
    fn ribbon_zero_still_submits_wrapped_range() {
        let d = one(Caller::Ribbon { pairs: 0 });
        assert_eq!((d.primitive_count, d.max_vertex), (0, u32::MAX));
    }
    #[test]
    fn spark_uses_vertex_offset_and_sentinel_bounds() {
        let d = one(Caller::Spark {
            sparks: 3,
            points: 7,
        });
        assert_eq!(
            (d.start, d.primitive_count, d.min_vertex, d.max_vertex),
            (101, 21, u32::MAX, u32::MAX)
        );
        assert!(!d.indexed);
    }
    #[test]
    fn spark_product_wraps_without_signed_gate() {
        assert_eq!(
            one(Caller::Spark {
                sparks: u32::MAX,
                points: 2
            })
            .primitive_count,
            u32::MAX - 1
        );
    }
    #[test]
    fn sprite_has_four_vertices_per_quad() {
        let d = one(Caller::Sprite { sprites: 6 });
        assert_eq!((d.primitive_count, d.max_vertex), (12, 23));
    }
    #[test]
    fn sprite_products_wrap_independently() {
        let d = one(Caller::Sprite {
            sprites: 0x40000001,
        });
        assert_eq!((d.primitive_count, d.max_vertex), (0x80000002, 3));
    }
    #[test]
    fn trail_keeps_independent_primitive_and_vertex_counts() {
        let d = one(Caller::Trail {
            primitives: 13,
            vertices: 8,
        });
        assert_eq!((d.start, d.primitive_count, d.max_vertex), (17, 13, 7));
    }
    #[test]
    fn trail_zero_vertices_wraps_maximum() {
        assert_eq!(
            one(Caller::Trail {
                primitives: 0,
                vertices: 0
            })
            .max_vertex,
            u32::MAX
        );
    }
    #[test]
    fn mesh_section_adds_index_offset_but_preserves_local_bounds() {
        let d = one(Caller::VertMesh {
            sections: vec![section()],
        });
        assert_eq!(
            (d.start, d.min_vertex, d.max_vertex, d.primitive_count),
            (20, 7, 19, 4)
        );
    }
    #[test]
    fn mesh_zero_count_or_word_skips_after_uploads() {
        let mut a = section();
        a.primitive_count = 0;
        let mut b = section();
        b.enabled_word = 0;
        let caller = Caller::VertMesh {
            sections: vec![a, b],
        };
        assert!(caller.enabled() && caller.indexed());
        assert!(caller.draws(1, Some(2)).unwrap().is_empty());
    }
    #[test]
    fn mesh_unsigned_words_and_start_wrapping_are_preserved() {
        let mut s = section();
        s.first_index = u16::MAX;
        s.min_vertex = u16::MAX;
        s.max_vertex = 0;
        s.enabled_word = 0x8000;
        s.primitive_count = u32::MAX;
        let d = Caller::VertMesh { sections: vec![s] }
            .draws(1, Some(u32::MAX))
            .unwrap()[0];
        assert_eq!(
            (d.start, d.min_vertex, d.max_vertex, d.primitive_count),
            (65534, 65535, 0, u32::MAX)
        );
    }
    #[test]
    fn mesh_sections_preserve_order_and_have_a_safe_bound() {
        let mut s = section();
        s.first_index = 11;
        let d = Caller::VertMesh {
            sections: vec![section(), s],
        }
        .draws(0, Some(2))
        .unwrap();
        assert_eq!(d.iter().map(|d| d.start).collect::<Vec<_>>(), vec![5, 13]);
        assert!(Caller::VertMesh {
            sections: vec![section(); 257]
        }
        .draws(0, Some(0))
        .is_err());
    }
}
