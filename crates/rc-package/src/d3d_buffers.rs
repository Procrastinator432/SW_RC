//! Resolved static buffer/declaration handoff, D3DDrv 100201c0/10020220/10020840.
//! Cache lookup, allocation and uploads are external; inputs describe their resolved results.
use crate::d3d_bindings::Deferred;
use serde::{Deserialize, Serialize};

#[derive(Clone, Debug, Serialize, Deserialize, PartialEq, Eq)]
pub struct State {
    pub declaration: u32,
    pub hardware_vertex: u32,
    pub declarations: [[u32; 5]; 16],
    pub stream_count: i32,
    pub wrappers: [u32; 16],
    pub strides: [u8; 16],
    pub previous_stream_count: u8,
    pub index_wrapper: u32,
    pub base_vertex: u32,
}
#[derive(Clone, Copy, Debug, Serialize, Deserialize, PartialEq, Eq)]
pub struct Shader {
    pub address: u32,
    pub handle: u32,
}
#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct Stream {
    /// Four GetComponents payload words followed by its returned component count.
    pub declaration: [u32; 5],
    pub source_revision: u32,
    pub cached_revision: u32,
    /// Source virtual +14 result, added only when revisions differ.
    pub upload_size: u32,
    pub wrapper: u32,
    /// Resolved wrapper+30 after any external upload.
    pub handle: u32,
    pub stride: u32,
}
#[derive(Clone, Copy, Debug, Serialize, Deserialize)]
pub struct Index {
    pub source: u32,
    /// Source virtual +10 result. Zero follows the native unbind path.
    pub size: u32,
    pub source_revision: u32,
    pub cached_revision: u32,
    pub wrapper: u32,
    pub handle: u32,
}
#[derive(Clone, Debug, Serialize, Deserialize, PartialEq, Eq)]
pub struct Usage {
    pub wrapper: u32,
    /// Device+46a8 copied to wrapper+14 after cache resolution/upload.
    pub frame: u32,
}
#[derive(Clone, Debug, Serialize, PartialEq, Eq)]
pub struct StreamPlan {
    pub upload_slots: Vec<usize>,
    pub upload_size: u32,
    pub usage: Vec<Usage>,
    pub fixed_shader: bool,
}
#[derive(Clone, Debug, Serialize, PartialEq, Eq)]
pub struct IndexPlan {
    pub upload: bool,
    pub upload_size: u32,
    pub usage: Option<Usage>,
}
impl State {
    fn validate_shader(&self, shader: Option<Shader>) -> Result<(), String> {
        if self.stream_count > 0 && shader.is_none_or(|s| s.address == 0) {
            return Err("Missing resolved fixed vertex shader".into());
        }
        Ok(())
    }
    /// Original standalone restore gate uses signed stream_count > 0.
    pub fn restore_fixed(
        &mut self,
        deferred: &mut Deferred,
        shader: Option<Shader>,
    ) -> Result<bool, String> {
        self.validate_shader(shader)?;
        if self.stream_count <= 0 {
            return Ok(false);
        }
        let shader = shader.unwrap();
        self.declaration = shader.address;
        self.hardware_vertex = 0;
        deferred.bindings.desired.vertex_shader = shader.handle;
        deferred.states.dirty |= 4;
        Ok(true)
    }
    pub fn set_streams(
        &mut self,
        deferred: &mut Deferred,
        streams: &[Stream],
        shader_kind: u32,
        shader: Option<Shader>,
        frame: u32,
    ) -> Result<StreamPlan, String> {
        if streams.len() > 16 || self.previous_stream_count > 16 {
            return Err("Static stream count exceeds bounded sixteen slots".into());
        }
        if streams.iter().any(|s| s.wrapper == 0) {
            return Err("Missing resolved static stream wrapper".into());
        }
        if shader_kind == 0 && !streams.is_empty() && shader.is_none_or(|s| s.address == 0) {
            return Err("Missing resolved fixed vertex shader".into());
        }
        // Native tail clearing is conditional on the renderer wrapper, not the desired handle.
        for slot in streams.len()..usize::from(self.previous_stream_count) {
            if self.wrappers[slot] != 0 {
                self.wrappers[slot] = 0;
                deferred.bindings.desired.streams[slot] = [0, 0];
                deferred.states.dirty |= 8;
            }
        }
        self.stream_count = streams.len() as i32;
        for (slot, stream) in streams.iter().enumerate() {
            self.declarations[slot] = stream.declaration;
        }
        let mut plan = StreamPlan {
            upload_slots: vec![],
            upload_size: 0,
            usage: vec![],
            fixed_shader: false,
        };
        for (slot, stream) in streams.iter().enumerate() {
            if stream.source_revision != stream.cached_revision {
                plan.upload_slots.push(slot);
                plan.upload_size = plan.upload_size.wrapping_add(stream.upload_size);
            }
            plan.usage.push(Usage {
                wrapper: stream.wrapper,
                frame,
            });
            deferred.bindings.desired.streams[slot] = [stream.handle, stream.stride];
            deferred.states.dirty |= 8;
            self.wrappers[slot] = stream.wrapper;
            self.strides[slot] = stream.stride as u8;
        }
        self.previous_stream_count = streams.len() as u8;
        if shader_kind == 0 {
            plan.fixed_shader = self.restore_fixed(deferred, shader)?;
        }
        Ok(plan)
    }
    /// Null source and zero-sized source share the conditional unbind path.
    pub fn set_index(
        &mut self,
        deferred: &mut Deferred,
        index: Index,
        base: u32,
        frame: u32,
    ) -> Result<IndexPlan, String> {
        let mut plan = IndexPlan {
            upload: false,
            upload_size: 0,
            usage: None,
        };
        if index.source == 0 || index.size == 0 {
            if self.index_wrapper != 0 {
                self.index_wrapper = 0;
                self.base_vertex = 0;
                deferred.bindings.desired.indices = [0, 0];
                deferred.states.dirty |= 8;
            }
            return Ok(plan);
        }
        if index.wrapper == 0 {
            return Err("Missing resolved static index wrapper".into());
        }
        plan.upload = index.source_revision != index.cached_revision;
        plan.upload_size = if plan.upload { index.size } else { 0 };
        plan.usage = Some(Usage {
            wrapper: index.wrapper,
            frame,
        });
        self.index_wrapper = index.wrapper;
        self.base_vertex = base;
        deferred.bindings.desired.indices = [index.handle, base];
        deferred.states.dirty |= 8;
        Ok(plan)
    }
}

#[derive(Clone, Debug, Serialize, PartialEq, Eq)]
pub struct Preparation {
    pub streams: StreamPlan,
    pub index: IndexPlan,
}
#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct Request {
    pub streams: Vec<Stream>,
    pub shader_kind: u32,
    pub shader: Option<Shader>,
    pub index: Index,
    pub base_vertex: u32,
    pub frame: u32,
}
/// Commit both resolved handoffs together; errors preserve caller state and deferred cache.
pub fn prepare(
    state: &mut State,
    deferred: &mut Deferred,
    request: &Request,
) -> Result<Preparation, String> {
    let mut next = state.clone();
    let mut cache = deferred.clone();
    let streams = next.set_streams(
        &mut cache,
        &request.streams,
        request.shader_kind,
        request.shader,
        request.frame,
    )?;
    let index = next.set_index(
        &mut cache,
        request.index,
        request.base_vertex,
        request.frame,
    )?;
    *state = next;
    *deferred = cache;
    Ok(Preparation { streams, index })
}

#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct Submission {
    pub state: State,
    pub complete: crate::d3d_complete::Complete,
    pub last_pass: u32,
    pub counters: crate::d3d_draw::Counters,
    pub passes: Vec<crate::d3d_draw::PreparedPass>,
}
#[derive(Clone, Debug, Serialize, PartialEq, Eq)]
pub struct SubmissionPlan {
    pub preparation: Preparation,
    pub draw: crate::d3d_draw::Draw,
    pub passes: Vec<crate::d3d_draw::RenderedPass>,
}
/// Resolved static handoff followed by bounded pass execution planning.
/// Index mode comes from the renderer wrapper (even when its device handle is zero).
pub fn submit(
    submission: &mut Submission,
    request: &Request,
    context: crate::d3d_draw::Context,
    mut draw: crate::d3d_draw::Draw,
) -> Result<SubmissionPlan, String> {
    let mut next = submission.clone();
    let preparation = prepare(&mut next.state, &mut next.complete.deferred, request)?;
    draw.indexed = next.state.index_wrapper != 0;
    let passes = crate::d3d_draw::render(
        &mut next.complete,
        &mut next.last_pass,
        &mut next.counters,
        &mut next.passes,
        context,
        draw,
    )?;
    *submission = next;
    Ok(SubmissionPlan {
        preparation,
        draw,
        passes,
    })
}

#[cfg(test)]
mod tests {
    use super::*;
    fn fixture() -> (State, Deferred, Request) {
        let cache = crate::d3d_state::Values {
            render: [0; 32],
            stages: [[0; 21]; 8],
            textures: [0; 8],
        };
        let bindings = crate::d3d_bindings::Values {
            vertex_shader: 0,
            pixel_shader: 0,
            streams: [[0; 2]; 16],
            indices: [0; 2],
        };
        (
            State {
                declaration: 9,
                hardware_vertex: 8,
                declarations: [[99; 5]; 16],
                stream_count: 0,
                wrappers: [0; 16],
                strides: [255; 16],
                previous_stream_count: 0,
                index_wrapper: 0,
                base_vertex: 7,
            },
            Deferred {
                states: crate::d3d_state::Cache {
                    desired: cache.clone(),
                    applied: cache,
                    dirty: 0,
                },
                transforms: crate::d3d_transforms::Transforms {
                    words: [[0; 16]; 11],
                    mask: 0,
                },
                bindings: crate::d3d_bindings::Bindings {
                    desired: bindings.clone(),
                    applied: bindings,
                },
            },
            Request {
                streams: vec![Stream {
                    declaration: [1, 2, 3, 4, 5],
                    source_revision: 1,
                    cached_revision: 0,
                    upload_size: u32::MAX,
                    wrapper: 10,
                    handle: 11,
                    stride: 0x123,
                }],
                shader_kind: 0,
                shader: Some(Shader {
                    address: 12,
                    handle: 13,
                }),
                index: Index {
                    source: 14,
                    size: 15,
                    source_revision: 1,
                    cached_revision: 0,
                    wrapper: 16,
                    handle: 17,
                },
                base_vertex: 18,
                frame: 19,
            },
        )
    }
    #[test]
    fn resolved_handoff_preserves_applied_and_orders_binding_output() {
        let (mut s, mut d, r) = fixture();
        let old = d.bindings.applied.clone();
        let p = prepare(&mut s, &mut d, &r).unwrap();
        assert_eq!(p.streams.upload_size, u32::MAX);
        assert_eq!(p.index.upload_size, 15);
        assert_eq!(s.declarations[0], [1, 2, 3, 4, 5]);
        assert_eq!(s.strides[0], 0x23);
        assert_eq!(d.bindings.applied, old);
        assert_eq!(d.states.dirty, 12);
        let calls = d.bindings.tail(d.states.dirty, 16);
        assert_eq!(
            calls.iter().map(|c| c.vtable_offset).collect::<Vec<_>>(),
            vec![0x130, 0x14c, 0x154]
        );
    }
    #[test]
    fn upload_sum_wraps_and_matching_revision_skips() {
        let (mut s, mut d, mut r) = fixture();
        r.streams.push(r.streams[0].clone());
        r.streams[1].upload_size = 2;
        r.streams.push(r.streams[0].clone());
        r.streams[2].cached_revision = 1;
        r.index.cached_revision = 1;
        let p = prepare(&mut s, &mut d, &r).unwrap();
        assert_eq!(p.streams.upload_size, 1);
        assert_eq!(p.streams.upload_slots, vec![0, 1]);
        assert!(!p.index.upload);
        assert_eq!(p.index.upload_size, 0);
        assert_eq!(p.streams.usage.len(), 3);
    }
    #[test]
    fn empty_streams_only_clear_nonnull_old_wrappers() {
        let (mut s, mut d, mut r) = fixture();
        s.previous_stream_count = 3;
        s.wrappers[1] = 4;
        d.bindings.desired.streams[0] = [7, 8];
        d.bindings.desired.streams[1] = [9, 10];
        r.streams.clear();
        r.shader = None;
        prepare(&mut s, &mut d, &r).unwrap();
        assert_eq!(d.bindings.desired.streams[0], [7, 8]);
        assert_eq!(d.bindings.desired.streams[1], [0, 0]);
        assert_eq!(s.strides[1], 255);
        assert_eq!(s.declarations[1], [99; 5]);
        assert_eq!(s.declaration, 9);
    }
    #[test]
    fn null_index_preserves_inconsistent_cache_when_already_unbound() {
        let (mut s, mut d, mut r) = fixture();
        r.index.source = 0;
        d.bindings.desired.indices = [99, 88];
        s.set_index(&mut d, r.index, 1, 2).unwrap();
        assert_eq!(d.bindings.desired.indices, [99, 88]);
        assert_eq!(s.base_vertex, 7);
        assert_eq!(d.states.dirty, 0);
    }
    #[test]
    fn zero_size_unbinds_and_clears_base() {
        let (mut s, mut d, mut r) = fixture();
        s.index_wrapper = 1;
        r.index.size = 0;
        r.index.wrapper = 0;
        let p = s.set_index(&mut d, r.index, 8, 9).unwrap();
        assert_eq!(s.index_wrapper, 0);
        assert_eq!(s.base_vertex, 0);
        assert_eq!(d.states.dirty, 8);
        assert!(p.usage.is_none());
    }
    #[test]
    fn hardware_kind_preserves_vertex_selection_and_full_stride() {
        let (mut s, mut d, mut r) = fixture();
        r.shader_kind = u32::MAX;
        r.shader = None;
        r.streams[0].handle = 0;
        r.index.handle = 0;
        prepare(&mut s, &mut d, &r).unwrap();
        assert_eq!(s.hardware_vertex, 8);
        assert_eq!(s.declaration, 9);
        assert_eq!(d.bindings.desired.streams[0], [0, 0x123]);
        assert_eq!(s.index_wrapper, 16);
    }
    #[test]
    fn signed_restore_gate_ignores_missing_shader_without_streams() {
        let (mut s, mut d, _) = fixture();
        for count in [i32::MIN, -1, 0] {
            s.stream_count = count;
            assert!(!s.restore_fixed(&mut d, None).unwrap());
        }
        assert_eq!(d.states.dirty, 0);
        s.stream_count = 1;
        assert!(s.restore_fixed(&mut d, None).is_err());
    }
    #[test]
    fn late_invalid_index_preserves_both_inputs() {
        let (mut s, mut d, mut r) = fixture();
        let old = (s.clone(), d.clone());
        r.index.wrapper = 0;
        assert!(prepare(&mut s, &mut d, &r).is_err());
        assert_eq!((s, d), old);
    }
    #[test]
    fn invalid_counts_and_shader_fail_before_stream_writes() {
        let (mut s, mut d, mut r) = fixture();
        let old = (s.clone(), d.clone());
        r.shader = None;
        assert!(prepare(&mut s, &mut d, &r).is_err());
        assert_eq!((s.clone(), d.clone()), old);
        s.previous_stream_count = 17;
        assert!(prepare(&mut s, &mut d, &r).is_err());
        r.streams = vec![r.streams[0].clone(); 17];
        s.previous_stream_count = 0;
        assert!(prepare(&mut s, &mut d, &r).is_err());
    }
    #[test]
    fn all_sixteen_slots_and_zero_handle_fixed_shader_are_supported() {
        let (mut s, mut d, mut r) = fixture();
        r.streams = vec![r.streams[0].clone(); 16];
        r.shader.as_mut().unwrap().handle = 0;
        let p = prepare(&mut s, &mut d, &r).unwrap();
        assert_eq!(s.previous_stream_count, 16);
        assert_eq!(s.stream_count, 16);
        assert_eq!(s.declarations[15], [1, 2, 3, 4, 5]);
        assert!(p.streams.fixed_shader);
        assert_eq!(d.bindings.desired.vertex_shader, 0);
    }
    fn submission() -> (
        Submission,
        Request,
        crate::d3d_draw::Context,
        crate::d3d_draw::Draw,
    ) {
        let (state, deferred, request) = fixture();
        (
            Submission {
                state,
                complete: crate::d3d_complete::Complete {
                    deferred,
                    lights: crate::d3d_complete::Lights {
                        words: [[0; 26]; 8],
                        enabled: [0; 8],
                        applied_enabled: [0; 8],
                    },
                },
                last_pass: 0,
                counters: crate::d3d_draw::Counters {
                    draw_passes: 0,
                    submitted_primitives: 0,
                    submitted_vertices: 0,
                },
                passes: vec![],
            },
            request,
            crate::d3d_draw::Context {
                device: crate::d3d_pass::Device {
                    cull_mode: 0,
                    lod_bias: 0,
                    capacity: 8,
                },
                stream_capacity: 16,
                light_capacity: 8,
                stencil_gate: false,
                fog_enabled: 0,
                restore_fog: 0,
            },
            crate::d3d_draw::Draw {
                primitive: 5,
                start: 0,
                primitive_count: 1,
                min_vertex: 0,
                max_vertex: 2,
                indexed: false,
            },
        )
    }
    #[test]
    fn submission_uses_wrapper_for_index_mode_even_with_null_device_handle() {
        let (mut s, mut r, c, d) = submission();
        r.index.handle = 0;
        let p = submit(&mut s, &r, c, d).unwrap();
        assert!(p.draw.indexed);
        assert_eq!(s.complete.deferred.bindings.desired.indices, [0, 18]);
        assert!(p.passes.is_empty());
        assert_eq!(s.counters.draw_passes, 0);
    }
    #[test]
    fn submission_draw_failure_rolls_back_buffer_preparation() {
        let (mut s, r, mut c, d) = submission();
        s.passes.push(crate::d3d_draw::PreparedPass {
            pass: crate::d3d_pass::Pass {
                address: 1,
                header: [0; 28],
                color_write: 0,
                stages: [[0; 28]; 8],
            },
            resources: [None; 8],
            shader: crate::d3d_complete::ShaderChoice {
                hardware: true,
                resolved: None,
            },
            fog_color: 0,
        });
        c.device.capacity = 9;
        let before = serde_json::to_value(&s).unwrap();
        assert!(submit(&mut s, &r, c, d).is_err());
        assert_eq!(serde_json::to_value(s).unwrap(), before);
    }
}
