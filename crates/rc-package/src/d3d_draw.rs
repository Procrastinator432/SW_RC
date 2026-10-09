//! Original bounded multipass draw consumer, 10020cb0/10020cf0/10021063.
use crate::{
    d3d_complete::{self, Complete, PassPlan, ShaderChoice},
    d3d_pass::{Device, Pass},
    d3d_state::{Cache, Call},
};
use serde::{Deserialize, Serialize};
#[derive(Clone, Copy, Debug, Serialize, Deserialize, PartialEq, Eq)]
pub struct Draw {
    pub primitive: u32,
    pub start: u32,
    pub primitive_count: u32,
    pub min_vertex: u32,
    pub max_vertex: u32,
    /// Native renderer state+484 is nonzero.
    pub indexed: bool,
}
pub fn primitive_id(kind: u32) -> Option<u32> {
    match kind {
        1 => Some(1),
        2 => Some(2),
        5 => Some(4),
        6 => Some(5),
        7 => Some(6),
        _ => None,
    }
}
impl Draw {
    /// Original skeletal section triangle-list request; buffers/materials remain external.
    pub fn from_section(
        section: &crate::skeletal_draw::DrawSection,
        indexed: bool,
    ) -> Result<Self, String> {
        Ok(Self {
            primitive: 5,
            start: u32::try_from(section.first_index)
                .map_err(|_| "Section first index exceeds native DWORD")?,
            primitive_count: u32::try_from(section.triangle_count)
                .map_err(|_| "Section triangle count exceeds native DWORD")?,
            min_vertex: section.min_vertex.into(),
            max_vertex: section.max_vertex.into(),
            indexed,
        })
    }
    /// Raw native wrapping range arithmetic; this does not validate buffer bounds.
    pub fn call(self) -> Option<Call> {
        let id = primitive_id(self.primitive)?;
        Some(if self.indexed {
            Call {
                vtable_offset: 0x11c,
                arguments: vec![
                    id,
                    self.min_vertex,
                    self.max_vertex
                        .wrapping_sub(self.min_vertex)
                        .wrapping_add(1),
                    self.start,
                    self.primitive_count,
                ],
            }
        } else {
            Call {
                vtable_offset: 0x118,
                arguments: vec![id, self.start, self.primitive_count],
            }
        })
    }
    fn vertices(self) -> u32 {
        if primitive_id(self.primitive).is_none() {
            return 0;
        }
        if self.indexed {
            return self
                .max_vertex
                .wrapping_sub(self.min_vertex)
                .wrapping_add(1);
        }
        match self.primitive {
            1 => self.primitive_count,
            2 => self.primitive_count.wrapping_mul(2),
            5 => self.primitive_count.wrapping_mul(3),
            6 | 7 => self.primitive_count.wrapping_add(2),
            _ => 0,
        }
    }
}
#[derive(Clone, Debug, Serialize, Deserialize, PartialEq, Eq)]
pub struct Counters {
    /// Device+fec0: incremented per material pass, even for an unknown primitive kind.
    pub draw_passes: u32,
    /// Device+fc90: primitive_count added after every processed pass.
    pub submitted_primitives: u32,
    /// Device+fcb8: native dispatch-specific vertex statistic, not distinct vertices.
    pub submitted_vertices: u32,
}
impl Counters {
    pub fn submit(&mut self, draw: Draw) -> Option<Call> {
        self.draw_passes = self.draw_passes.wrapping_add(1);
        self.submitted_primitives = self.submitted_primitives.wrapping_add(draw.primitive_count);
        self.submitted_vertices = self.submitted_vertices.wrapping_add(draw.vertices());
        draw.call()
    }
}
#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct PreparedPass {
    pub pass: Pass,
    pub resources: [Option<[u32; 2]>; 8],
    pub shader: ShaderChoice,
    /// Raw FColor word at original pass+1c. Engine!10338670 returns it unchanged.
    pub fog_color: u32,
}
#[derive(Clone, Copy, Debug, Serialize, Deserialize)]
pub struct Context {
    pub device: Device,
    pub stream_capacity: i32,
    pub light_capacity: i32,
    pub stencil_gate: bool,
    /// Native state+17 byte; separate from cached render-state fog enable.
    pub fog_enabled: u8,
    /// Raw FColor word at native state+2f8, restored after an overridden pass.
    pub restore_fog: u32,
}
pub fn fog_begin(cache: &mut Cache, enabled: u8, pass: &PreparedPass) -> bool {
    let overridden = enabled != 0 && pass.pass.header[4] & 0x40 != 0;
    if overridden {
        cache.desired.render[12] = pass.fog_color;
        cache.dirty |= 1;
    }
    overridden
}
pub fn fog_end(cache: &mut Cache, overridden: bool, restore: u32) {
    if overridden {
        cache.desired.render[12] = restore;
        cache.dirty |= 1;
    }
}
#[derive(Clone, Debug, Serialize, PartialEq, Eq)]
pub struct RenderedPass {
    pub address: u32,
    /// Immediate upload precedes commands, and commands precede draw.
    pub selection: PassPlan,
    pub commands: d3d_complete::Plan,
    pub draw: Option<Call>,
    /// Restoration remains pending in the desired cache after this draw.
    pub fog_override: bool,
}
/// Bounded active pass array (max eight). No trailing flush after the final restoration.
/// Invalid inputs are checked across the whole array before any writes.
pub fn render(
    complete: &mut Complete,
    last_pass: &mut u32,
    counters: &mut Counters,
    passes: &mut [PreparedPass],
    context: Context,
    draw: Draw,
) -> Result<Vec<RenderedPass>, String> {
    if passes.is_empty() {
        return Ok(vec![]);
    }
    if passes.len() > 8
        || context.device.capacity > 8
        || complete.deferred.states.dirty & !0xff != 0
    {
        return Err("Invalid bounded draw pass count/capacity/dirty group".into());
    }
    let mut previous = *last_pass;
    for (index, p) in passes.iter().enumerate() {
        if let Some(alias) = passes[..index]
            .iter()
            .find(|q| q.pass.address == p.pass.address)
        {
            if alias.pass != p.pass
                || alias.resources != p.resources
                || alias.shader != p.shader
                || alias.fog_color != p.fog_color
            {
                return Err("Conflicting snapshots for aliased draw pass".into());
            }
        }
        if p.pass.address == previous {
            continue;
        }
        let active = usize::from(p.pass.header[9]);
        if p.pass.address == 0 || active > 8 {
            return Err("Invalid bounded draw pass address/stage count".into());
        }
        if p.pass
            .stages
            .iter()
            .zip(p.resources)
            .take(active)
            .any(|(s, r)| s[0] != 0 && r.is_none())
        {
            return Err("Missing draw-pass texture resource".into());
        }
        let kind = u32::from_le_bytes(p.pass.header[..4].try_into().unwrap());
        if !p.shader.hardware && kind != 0 && p.shader.resolved.is_none() {
            return Err("Missing draw-pass fixed shader handle".into());
        }
        previous = p.pass.address;
    }
    let mut result = Vec::new();
    for index in 0..passes.len() {
        let p = &mut passes[index];
        let selection = d3d_complete::apply_pass(
            complete,
            last_pass,
            &mut p.pass,
            p.resources,
            context.device,
            p.shader,
        )?;
        let fog_override = fog_begin(&mut complete.deferred.states, context.fog_enabled, p);
        let commands = complete.flush(
            context.device.capacity,
            context.stream_capacity,
            context.light_capacity,
            context.stencil_gate,
        )?;
        let call = counters.submit(draw);
        fog_end(
            &mut complete.deferred.states,
            fog_override,
            context.restore_fog,
        );
        result.push(RenderedPass {
            address: p.pass.address,
            selection,
            commands,
            draw: call,
            fog_override,
        });
        let snapshot = p.pass.clone();
        for alias in passes
            .iter_mut()
            .filter(|p| p.pass.address == snapshot.address)
        {
            alias.pass = snapshot.clone();
        }
    }
    Ok(result)
}
#[cfg(test)]
mod tests {
    use super::*;
    fn draw() -> Draw {
        Draw {
            primitive: 5,
            start: 13,
            primitive_count: 7,
            min_vertex: 3,
            max_vertex: 11,
            indexed: false,
        }
    }
    fn fixture() -> (Complete, PreparedPass, Context) {
        let v = crate::d3d_state::Values {
            render: [0; 32],
            stages: [[0; 21]; 8],
            textures: [0; 8],
        };
        let b = crate::d3d_bindings::Values {
            vertex_shader: 0,
            pixel_shader: 0,
            streams: [[0; 2]; 16],
            indices: [0; 2],
        };
        let c = Complete {
            deferred: crate::d3d_bindings::Deferred {
                states: Cache {
                    desired: v.clone(),
                    applied: v,
                    dirty: 0,
                },
                transforms: crate::d3d_transforms::Transforms {
                    words: [[0; 16]; 11],
                    mask: 0,
                },
                bindings: crate::d3d_bindings::Bindings {
                    desired: b.clone(),
                    applied: b,
                },
            },
            lights: crate::d3d_complete::Lights {
                words: [[0; 26]; 8],
                enabled: [0; 8],
                applied_enabled: [0; 8],
            },
        };
        let p = PreparedPass {
            pass: Pass {
                address: 99,
                header: [0; 28],
                color_write: 15,
                stages: [[0; 28]; 8],
            },
            resources: [None; 8],
            shader: ShaderChoice {
                hardware: true,
                resolved: None,
            },
            fog_color: 0x12345678,
        };
        (
            c,
            p,
            Context {
                device: Device {
                    cull_mode: 3,
                    lod_bias: 0,
                    capacity: 8,
                },
                stream_capacity: 16,
                light_capacity: 8,
                stencil_gate: false,
                fog_enabled: 1,
                restore_fog: 0xaabbccdd,
            },
        )
    }
    fn counters() -> Counters {
        Counters {
            draw_passes: 0,
            submitted_primitives: 0,
            submitted_vertices: 0,
        }
    }
    #[test]
    fn native_primitive_mapping_excludes_unused_kinds() {
        assert_eq!(
            (0..9).map(primitive_id).collect::<Vec<_>>(),
            [
                None,
                Some(1),
                Some(2),
                None,
                None,
                Some(4),
                Some(5),
                Some(6),
                None
            ]
        );
    }
    #[test]
    fn indexed_argument_order_and_range_wrap_are_preserved() {
        let d = Draw {
            indexed: true,
            min_vertex: 9,
            max_vertex: 2,
            ..draw()
        };
        let c = d.call().unwrap();
        assert_eq!(c.vtable_offset, 0x11c);
        assert_eq!(c.arguments, [4, 9, u32::MAX - 5, 13, 7]);
    }
    #[test]
    fn nonindexed_zero_count_still_plans_call_and_strip_vertex_statistic() {
        let d = Draw {
            primitive: 6,
            primitive_count: 0,
            ..draw()
        };
        let mut c = counters();
        assert_eq!(c.submit(d).unwrap().arguments, [5, 13, 0]);
        assert_eq!(c.submitted_vertices, 2);
    }
    #[test]
    fn unknown_primitive_updates_pass_and_primitive_counters_only() {
        let mut c = counters();
        assert!(c
            .submit(Draw {
                primitive: 4,
                ..draw()
            })
            .is_none());
        assert_eq!(
            c,
            Counters {
                draw_passes: 1,
                submitted_primitives: 7,
                submitted_vertices: 0
            }
        );
    }
    #[test]
    fn counters_wrap_like_native_words() {
        let mut c = Counters {
            draw_passes: u32::MAX,
            submitted_primitives: u32::MAX,
            submitted_vertices: u32::MAX,
        };
        c.submit(draw());
        assert_eq!(
            c,
            Counters {
                draw_passes: 0,
                submitted_primitives: 6,
                submitted_vertices: 20
            }
        );
    }
    #[test]
    fn fog_gate_needs_both_flags_and_restoration_keeps_dirty() {
        let (mut c, mut p, x) = fixture();
        assert!(!fog_begin(&mut c.deferred.states, 1, &p));
        p.pass.header[4] = 0x40;
        assert!(!fog_begin(&mut c.deferred.states, 0, &p));
        assert!(fog_begin(&mut c.deferred.states, 255, &p));
        assert_eq!(c.deferred.states.desired.render[12], p.fog_color);
        c.flush(8, 16, 8, false).unwrap();
        fog_end(&mut c.deferred.states, true, x.restore_fog);
        assert_eq!(c.deferred.states.desired.render[12], x.restore_fog);
        assert_eq!(c.deferred.states.dirty, 1);
    }
    #[test]
    fn final_fog_restore_is_not_flushed_and_next_pass_emits_it() {
        let (mut c, mut p, x) = fixture();
        c.deferred.states.desired.render[11] = 1;
        p.pass.header[4] = 0x40;
        let mut q = p.clone();
        q.pass.address = 100;
        q.pass.header[4] = 0;
        let result = render(&mut c, &mut 0, &mut counters(), &mut [p, q], x, draw()).unwrap();
        assert!(result[0].fog_override);
        assert!(!result[1].fog_override);
        assert!(result[1]
            .commands
            .states
            .before
            .iter()
            .any(|v| v.arguments == [34, x.restore_fog]));
        assert_eq!(c.deferred.states.dirty, 0);
    }
    #[test]
    fn zero_passes_skip_even_invalid_context_and_preserve_state() {
        let (mut c, _, mut x) = fixture();
        x.device.capacity = 99;
        c.deferred.transforms.mask = 1;
        let before = c.clone();
        assert!(render(&mut c, &mut 0, &mut counters(), &mut [], x, draw())
            .unwrap()
            .is_empty());
        assert_eq!(c, before);
    }
    #[test]
    fn failure_in_last_pass_preflights_entire_array_without_writes() {
        let (mut c, p, x) = fixture();
        let mut q = p.clone();
        q.pass.address = 100;
        q.pass.header[9] = 1;
        q.pass.stages[0][0] = 1;
        let before = c.clone();
        let mut last = 77;
        let mut stats = counters();
        assert!(render(&mut c, &mut last, &mut stats, &mut [p, q], x, draw()).is_err());
        assert_eq!(c, before);
        assert_eq!(last, 77);
        assert_eq!(stats, counters());
    }
    #[test]
    fn repeated_pass_identity_skips_translation_but_still_draws_and_counts() {
        let (mut c, p, x) = fixture();
        let mut last = 0;
        let mut stats = counters();
        let r = render(
            &mut c,
            &mut last,
            &mut stats,
            &mut [p.clone(), p],
            x,
            draw(),
        )
        .unwrap();
        assert!(r[0].selection.changed);
        assert!(!r[1].selection.changed);
        assert!(r[1].draw.is_some());
        assert_eq!(stats.draw_passes, 2);
    }
    #[test]
    fn skeletal_section_request_preserves_stored_bounds_and_start() {
        let section = crate::skeletal_draw::DrawSection {
            bank: 1,
            section_index: 0,
            material: 0,
            bone: Some(3),
            first_index: 10521,
            triangle_count: 602,
            min_vertex: 17,
            max_vertex: 2234,
        };
        assert_eq!(
            Draw::from_section(&section, true)
                .unwrap()
                .call()
                .unwrap()
                .arguments,
            [4, 17, 2218, 10521, 602]
        );
    }
    #[test]
    fn aliased_pass_slots_share_lod_and_unused_stage_mutations() {
        let (mut c, mut p, x) = fixture();
        p.pass.header[9] = 1;
        p.pass.stages[0][1] = (-101f32).to_bits();
        p.pass.stages[1][0] = 99;
        let mut passes = [p.clone(), p];
        render(&mut c, &mut 0, &mut counters(), &mut passes, x, draw()).unwrap();
        assert_eq!(passes[0].pass, passes[1].pass);
        assert_eq!(passes[1].pass.stages[0][1], x.device.lod_bias);
        assert_eq!(passes[1].pass.stages[1][0], 0);
    }
    #[test]
    fn conflicting_alias_snapshots_fail_before_any_writes() {
        let (mut c, p, x) = fixture();
        let mut q = p.clone();
        q.fog_color ^= 1;
        let before = c.clone();
        assert!(render(&mut c, &mut 0, &mut counters(), &mut [p, q], x, draw()).is_err());
        assert_eq!(c, before);
    }
}
