//! D3D8 shader/stream/index cache tail, original 10029dd9..10029ef0.
use crate::{
    d3d_state::{Cache, Call},
    d3d_transforms::{self, Transforms},
};
use serde::{Deserialize, Serialize};

#[derive(Clone, Debug, Serialize, Deserialize, PartialEq, Eq)]
pub struct Values {
    /// Native cache+628/+62c. Raw handles/FVF, not shader programs.
    pub vertex_shader: u32,
    pub pixel_shader: u32,
    /// Native cache+630: sixteen (buffer, stride) pairs.
    pub streams: [[u32; 2]; 16],
    /// Native cache+6b0: (index buffer, base vertex index).
    pub indices: [u32; 2],
}
#[derive(Clone, Debug, Serialize, Deserialize, PartialEq, Eq)]
pub struct Bindings {
    pub desired: Values,
    pub applied: Values,
}
impl Bindings {
    /// Does not clear the outer dirty word. Dirty 4 gates both shaders, 8 streams/indices.
    pub fn tail(&mut self, dirty: u32, stream_capacity: i32) -> Vec<Call> {
        let mut calls = Vec::new();
        if dirty & 4 != 0 && self.desired.vertex_shader != self.applied.vertex_shader {
            self.applied.vertex_shader = self.desired.vertex_shader;
            calls.push(Call {
                vtable_offset: 0x130,
                arguments: vec![self.desired.vertex_shader],
            });
        }
        if dirty & 8 != 0 {
            for index in 0..stream_capacity.clamp(0, 16) as usize {
                let pair = self.desired.streams[index];
                if pair != self.applied.streams[index] {
                    self.applied.streams[index] = pair;
                    calls.push(Call {
                        vtable_offset: 0x14c,
                        arguments: vec![index as u32, pair[0], pair[1]],
                    });
                }
            }
        }
        if dirty & 4 != 0 && self.desired.pixel_shader != self.applied.pixel_shader {
            self.applied.pixel_shader = self.desired.pixel_shader;
            calls.push(Call {
                vtable_offset: 0x160,
                arguments: vec![self.desired.pixel_shader],
            });
        }
        if dirty & 8 != 0 && self.desired.indices != self.applied.indices {
            self.applied.indices = self.desired.indices;
            calls.push(Call {
                vtable_offset: 0x154,
                arguments: self.desired.indices.to_vec(),
            });
        }
        calls
    }
}
#[derive(Clone, Debug, Serialize, Deserialize, PartialEq, Eq)]
pub struct Deferred {
    pub states: Cache,
    pub transforms: Transforms,
    pub bindings: Bindings,
}
#[derive(Clone, Debug, Serialize, PartialEq, Eq)]
pub struct Plan {
    /// Ordered before -> transforms -> after, followed by bindings.
    pub states: d3d_transforms::Plan,
    pub bindings: Vec<Call>,
}
impl Deferred {
    /// Combined supported groups 1/2/4/8/10/20/40, excluding lights (80) and unknown bits.
    pub fn flush(
        &mut self,
        texture_capacity: usize,
        stream_capacity: i32,
        stencil_gate: bool,
    ) -> Result<Plan, String> {
        if texture_capacity > 8 || self.states.dirty & !0x7f != 0 {
            return Err("Unsupported dirty group/texture capacity in deferred plan".into());
        }
        if self.states.dirty == 0 {
            return Ok(Plan {
                states: self
                    .transforms
                    .flush(&mut self.states, texture_capacity, stencil_gate)?,
                bindings: vec![],
            });
        }
        self.flush_body(texture_capacity, stream_capacity, stencil_gate)
    }
    /// Called after an outer planner's nonzero entry check, before subgroup masking.
    pub(crate) fn flush_body(
        &mut self,
        texture_capacity: usize,
        stream_capacity: i32,
        stencil_gate: bool,
    ) -> Result<Plan, String> {
        if texture_capacity > 8 || self.states.dirty & !0x7f != 0 {
            return Err("Unsupported dirty group/texture capacity in deferred plan".into());
        }
        let dirty = self.states.dirty;
        self.states.dirty = dirty & 0x73;
        let states =
            self.transforms
                .flush_body(&mut self.states, texture_capacity, stencil_gate)?;
        let bindings = self.bindings.tail(dirty, stream_capacity);
        Ok(Plan { states, bindings })
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    fn bindings() -> Bindings {
        let v = Values {
            vertex_shader: 3,
            pixel_shader: 4,
            streams: [[5, 6]; 16],
            indices: [7, 8],
        };
        Bindings {
            desired: v.clone(),
            applied: v,
        }
    }
    fn deferred() -> Deferred {
        let v = crate::d3d_state::Values {
            render: [0; 32],
            stages: [[0; 21]; 8],
            textures: [0; 8],
        };
        Deferred {
            states: Cache {
                desired: v.clone(),
                applied: v,
                dirty: 0,
            },
            transforms: Transforms {
                words: [[0; 16]; 11],
                mask: 0,
            },
            bindings: bindings(),
        }
    }
    #[test]
    fn shared_shader_gate_emits_raw_vertex_then_pixel_values() {
        let mut b = bindings();
        b.desired.vertex_shader = 0xdeadbeef;
        b.desired.pixel_shader = 0;
        assert!(b.tail(8, 0).is_empty());
        let c = b.tail(4, 0);
        assert_eq!(
            c.iter().map(|c| c.vtable_offset).collect::<Vec<_>>(),
            [0x130, 0x160]
        );
        assert_eq!(c[0].arguments, [0xdeadbeef]);
        assert_eq!(c[1].arguments, [0]);
        assert!(b.tail(4, 0).is_empty());
    }
    #[test]
    fn stride_alone_and_null_stream_are_not_suppressed() {
        let mut b = bindings();
        b.desired.streams[0] = [5, 99];
        b.desired.streams[1] = [0, 6];
        let c = b.tail(8, 2);
        assert_eq!(c.len(), 2);
        assert_eq!(c[0].arguments, [0, 5, 99]);
        assert_eq!(c[1].arguments, [1, 0, 6]);
        assert_eq!(b.applied.streams[2], [5, 6]);
    }
    #[test]
    fn index_base_alone_updates_even_without_stream_capacity() {
        let mut b = bindings();
        b.desired.indices[1] = u32::MAX;
        let c = b.tail(8, 0);
        assert_eq!(c.len(), 1);
        assert_eq!(c[0].vtable_offset, 0x154);
        assert_eq!(c[0].arguments, [7, u32::MAX]);
    }
    #[test]
    fn signed_stream_capacity_is_clamped_to_sixteen() {
        let mut b = bindings();
        b.desired.streams = [[0, 0]; 16];
        assert!(b.tail(8, -1).is_empty());
        assert_eq!(b.tail(8, i32::MAX).len(), 16);
        assert!(b.tail(8, 16).is_empty());
    }
    #[test]
    fn native_order_interleaves_shaders_with_streams_and_indices() {
        let mut b = bindings();
        b.desired.vertex_shader = 0;
        b.desired.pixel_shader = 0;
        b.desired.streams[0] = [0, 0];
        b.desired.indices = [0, 0];
        let c = b.tail(12, 1);
        assert_eq!(
            c.iter().map(|c| c.vtable_offset).collect::<Vec<_>>(),
            [0x130, 0x14c, 0x160, 0x154]
        );
    }
    #[test]
    fn combined_plan_keeps_transform_texture_and_binding_order_and_clears_dirty() {
        let mut d = deferred();
        d.states.dirty = 0x7f;
        d.states.desired.render[29] = 15;
        d.states.desired.textures[0] = 99;
        d.transforms.mask = 1;
        d.bindings.desired.pixel_shader = 0;
        let p = d.flush(1, 0, false).unwrap();
        assert_eq!(p.states.before[0].vtable_offset, 0xc8);
        assert_eq!(p.states.transforms[0].vtable_offset, 0x94);
        assert_eq!(p.states.after[0].vtable_offset, 0xf4);
        assert_eq!(p.bindings[0].vtable_offset, 0x160);
        assert_eq!(d.states.dirty, 0);
        assert_eq!(d.transforms.mask, 0);
        let p = d.flush(1, 0, false).unwrap();
        assert!(p.bindings.is_empty());
        assert!(p.states.transforms.is_empty());
    }
    #[test]
    fn combined_unknown_dirty_group_or_capacity_is_atomic() {
        let mut d = deferred();
        d.states.dirty = 0x84;
        d.transforms.mask = 1;
        d.bindings.desired.pixel_shader = 0;
        let before = d.clone();
        assert!(d.flush(8, 16, false).is_err());
        assert_eq!(d, before);
        d.states.dirty = 4;
        let before = d.clone();
        assert!(d.flush(9, 16, false).is_err());
        assert_eq!(d, before);
    }
    #[test]
    fn binding_only_dirty_clears_mask_but_zero_dirty_preserves_it() {
        let mut d = deferred();
        d.transforms.mask = 1;
        d.flush(8, 16, false).unwrap();
        assert_eq!(d.transforms.mask, 1);
        d.states.dirty = 4;
        d.flush(8, 16, false).unwrap();
        assert_eq!(d.transforms.mask, 0);
    }
}
