//! Bounded complete deferred groups, raw lights and resolved fixed/hardware pass selection.
use crate::{
    d3d_bindings::{Bindings, Deferred},
    d3d_pass::{Device, Pass},
    d3d_state::Call,
    d3d_transforms,
};
use serde::{Deserialize, Serialize};

#[derive(Clone, Debug, Serialize, Deserialize, PartialEq, Eq)]
pub struct Lights {
    /// Native cache+6d8, eight consecutive 26-word light structures.
    pub words: [[u32; 26]; 8],
    /// Native cache+a18, with applied enable words at +144c.
    pub enabled: [u32; 8],
    pub applied_enabled: [u32; 8],
}
#[derive(Clone, Debug, Serialize, PartialEq, Eq)]
#[serde(untagged)]
pub enum LightCall {
    Set {
        vtable_offset: u16,
        index: u32,
        words: [u32; 26],
    },
    Enable(Call),
}
impl Lights {
    /// Dirty 80: enabled payloads always emitted; enable values compared separately.
    pub fn tail(&mut self, dirty: u32, capacity: i32) -> Vec<LightCall> {
        let mut calls = Vec::new();
        if dirty & 0x80 == 0 {
            return calls;
        }
        for index in 0..capacity.clamp(0, 8) as usize {
            let enabled = self.enabled[index];
            if enabled != 0 {
                calls.push(LightCall::Set {
                    vtable_offset: 0xb0,
                    index: index as u32,
                    words: self.words[index],
                });
            }
            if enabled != self.applied_enabled[index] {
                self.applied_enabled[index] = enabled;
                calls.push(LightCall::Enable(Call {
                    vtable_offset: 0xb8,
                    arguments: vec![index as u32, enabled],
                }));
            }
        }
        calls
    }
}
#[derive(Clone, Debug, Serialize, Deserialize, PartialEq, Eq)]
pub struct Complete {
    pub deferred: Deferred,
    pub lights: Lights,
}
#[derive(Clone, Debug, Serialize, PartialEq, Eq)]
pub struct Plan {
    /// before -> transforms -> after, then lights, then bindings.
    pub states: d3d_transforms::Plan,
    pub lights: Vec<LightCall>,
    pub bindings: Vec<Call>,
}
impl Complete {
    /// All native low-byte dirty groups; profiling/statistics and GPU execution excluded.
    pub fn flush(
        &mut self,
        texture_capacity: usize,
        stream_capacity: i32,
        light_capacity: i32,
        stencil_gate: bool,
    ) -> Result<Plan, String> {
        let dirty = self.deferred.states.dirty;
        if texture_capacity > 8 || dirty & !0xff != 0 {
            return Err("Unsupported complete cache dirty group/texture capacity".into());
        }
        if dirty == 0 {
            let plan = self
                .deferred
                .flush(texture_capacity, stream_capacity, stencil_gate)?;
            return Ok(Plan {
                states: plan.states,
                lights: vec![],
                bindings: plan.bindings,
            });
        }
        self.deferred.states.dirty = dirty & 0x7f;
        let plan = self
            .deferred
            .flush_body(texture_capacity, stream_capacity, stencil_gate)?;
        let lights = self.lights.tail(dirty, light_capacity);
        Ok(Plan {
            states: plan.states,
            lights,
            bindings: plan.bindings,
        })
    }
}
#[derive(Clone, Copy, Debug, Serialize, Deserialize, PartialEq, Eq)]
pub struct ShaderChoice {
    /// Original pass+3a8 is nonzero: skip fixed pixelshader selection.
    pub hardware: bool,
    /// Already resolved GetPixelShader result+4 for nonzero fixed kinds.
    pub resolved: Option<u32>,
}
#[derive(Clone, Debug, Serialize, PartialEq, Eq)]
pub struct PixelConstants {
    pub vtable_offset: u16,
    pub first: u32,
    pub count: u32,
    /// Original pointer payload: three planes, not twelve API arguments.
    pub words: [u32; 12],
}
/// Selection branches from 1001ee08..1001efb7. Resource lookup/static allocation external.
pub fn select_pixel(
    bindings: &mut Bindings,
    dirty: &mut u32,
    kind: u32,
    choice: ShaderChoice,
) -> Result<Option<PixelConstants>, String> {
    if choice.hardware {
        return Ok(None);
    }
    let handle = if kind == 0 {
        0
    } else {
        choice
            .resolved
            .ok_or("Missing resolved fixed pixelshader handle")?
    };
    bindings.desired.pixel_shader = handle;
    *dirty |= 4;
    Ok((kind == 1).then_some(PixelConstants {
        vtable_offset: 0x16c,
        first: 0,
        count: 3,
        words: [
            0x3f800000, 0, 0, 0, 0, 0x3f800000, 0, 0, 0, 0, 0x3f800000, 0,
        ],
    }))
}
#[derive(Clone, Debug, Serialize, PartialEq, Eq)]
pub struct PassPlan {
    pub changed: bool,
    pub immediate: Option<PixelConstants>,
}
/// Both pass branches, with matrix stages and resolved resources. Preflight is atomic.
pub fn apply_pass(
    complete: &mut Complete,
    last_pass: &mut u32,
    pass: &mut Pass,
    resources: [Option<[u32; 2]>; 8],
    device: Device,
    choice: ShaderChoice,
) -> Result<PassPlan, String> {
    if *last_pass == pass.address {
        return Ok(PassPlan {
            changed: false,
            immediate: None,
        });
    }
    let active = usize::from(pass.header[9]);
    if pass.address == 0 || active > 8 || device.capacity > 8 {
        return Err("Invalid bounded pass address/count/capacity".into());
    }
    for (stage, resource) in pass.stages.iter().zip(resources).take(active) {
        if stage[0] != 0 && resource.is_none() {
            return Err("Missing active texture resource".into());
        }
    }
    let kind = u32::from_le_bytes(pass.header[..4].try_into().unwrap());
    if !choice.hardware && kind != 0 && choice.resolved.is_none() {
        return Err("Missing resolved fixed pixelshader handle".into());
    }
    let d = &mut complete.deferred;
    let immediate = select_pixel(&mut d.bindings, &mut d.states.dirty, kind, choice)?;
    d.states.render_prefix(&pass.header, device.cull_mode);
    for (index, resource) in resources.into_iter().enumerate().take(active) {
        d.transforms.stage(
            &mut d.states,
            index,
            &mut pass.stages[index],
            resource,
            choice.hardware,
            device.lod_bias,
        )?;
    }
    d.states
        .disable_unused(&mut pass.stages, active, device.capacity, choice.hardware)?;
    d.states
        .finish_pass(pass.color_write, pass.address, last_pass);
    Ok(PassPlan {
        changed: true,
        immediate,
    })
}

#[cfg(test)]
mod tests {
    use super::*;
    fn fixture() -> Complete {
        let v = crate::d3d_state::Values {
            render: [0; 32],
            stages: [[0; 21]; 8],
            textures: [0; 8],
        };
        let b = crate::d3d_bindings::Values {
            vertex_shader: 2,
            pixel_shader: 3,
            streams: [[0; 2]; 16],
            indices: [0; 2],
        };
        Complete {
            deferred: Deferred {
                states: crate::d3d_state::Cache {
                    desired: v.clone(),
                    applied: v,
                    dirty: 0,
                },
                transforms: d3d_transforms::Transforms {
                    words: [[0; 16]; 11],
                    mask: 0,
                },
                bindings: Bindings {
                    desired: b.clone(),
                    applied: b,
                },
            },
            lights: Lights {
                words: [[0; 26]; 8],
                enabled: [0; 8],
                applied_enabled: [0; 8],
            },
        }
    }
    #[test]
    fn enabled_payload_is_raw_and_reemitted_without_enable_change() {
        let mut c = fixture();
        c.lights.words[0] = [0x7fc12345; 26];
        c.lights.enabled[0] = 0x80000000;
        c.lights.applied_enabled[0] = 0x80000000;
        let calls = c.lights.tail(0x80, 1);
        assert_eq!(
            calls,
            [LightCall::Set {
                vtable_offset: 0xb0,
                index: 0,
                words: [0x7fc12345; 26]
            }]
        );
        assert_eq!(c.lights.tail(0x80, 1), calls);
    }
    #[test]
    fn disable_skips_payload_and_keeps_raw_enable_history() {
        let mut c = fixture();
        c.lights.applied_enabled[0] = 7;
        assert_eq!(
            c.lights.tail(0x80, 1),
            [LightCall::Enable(Call {
                vtable_offset: 0xb8,
                arguments: vec![0, 0]
            })]
        );
        assert!(c.lights.tail(0x80, 1).is_empty());
    }
    #[test]
    fn light_capacity_is_signed_and_clamped_independently() {
        let mut c = fixture();
        c.lights.enabled = [1; 8];
        assert!(c.lights.tail(0x80, -1).is_empty());
        assert_eq!(c.lights.tail(0x80, 99).len(), 16);
        assert_eq!(c.lights.tail(0x80, 8).len(), 8);
        assert!(c.lights.tail(0, 8).is_empty());
    }
    #[test]
    fn each_light_payload_precedes_its_enable_call() {
        let mut c = fixture();
        c.lights.enabled[0] = 1;
        c.lights.enabled[1] = 2;
        let calls = c.lights.tail(0x80, 2);
        assert!(matches!(calls[0], LightCall::Set { index: 0, .. }));
        assert!(matches!(calls[1], LightCall::Enable(_)));
        assert!(matches!(calls[2], LightCall::Set { index: 1, .. }));
    }
    #[test]
    fn complete_plan_includes_lights_and_rejects_unknown_bits_atomically() {
        let mut c = fixture();
        c.deferred.states.dirty = 0x84;
        c.lights.enabled[0] = 1;
        c.deferred.bindings.desired.pixel_shader = 0;
        let p = c.flush(0, 0, 1, false).unwrap();
        assert_eq!(p.lights.len(), 2);
        assert_eq!(p.bindings[0].vtable_offset, 0x160);
        assert_eq!(c.deferred.states.dirty, 0);
        c.deferred.states.dirty = 0x100;
        let before = c.clone();
        assert!(c.flush(0, 0, 1, false).is_err());
        assert_eq!(c, before);
    }
    #[test]
    fn null_fixed_kind_does_not_require_resource_and_hardware_keeps_shader() {
        let mut c = fixture();
        let d = &mut c.deferred;
        assert_eq!(
            select_pixel(
                &mut d.bindings,
                &mut d.states.dirty,
                0,
                ShaderChoice {
                    hardware: false,
                    resolved: None
                }
            )
            .unwrap(),
            None
        );
        assert_eq!(d.bindings.desired.pixel_shader, 0);
        assert_eq!(d.states.dirty, 4);
        d.states.dirty = 0;
        d.bindings.desired.pixel_shader = 99;
        assert_eq!(
            select_pixel(
                &mut d.bindings,
                &mut d.states.dirty,
                1,
                ShaderChoice {
                    hardware: true,
                    resolved: None
                }
            )
            .unwrap(),
            None
        );
        assert_eq!(d.bindings.desired.pixel_shader, 99);
        assert_eq!(d.states.dirty, 0);
    }
    #[test]
    fn fixed_kind_one_plans_three_identity_planes_each_time() {
        let mut c = fixture();
        let d = &mut c.deferred;
        let choice = ShaderChoice {
            hardware: false,
            resolved: Some(0),
        };
        let p = select_pixel(&mut d.bindings, &mut d.states.dirty, 1, choice)
            .unwrap()
            .unwrap();
        assert_eq!(p.count, 3);
        assert_eq!(p.first, 0);
        assert_eq!(p.words[10], 0x3f800000);
        assert_eq!(
            select_pixel(&mut d.bindings, &mut d.states.dirty, 1, choice).unwrap(),
            Some(p)
        );
    }
    #[test]
    fn missing_fixed_handle_is_atomic_but_other_kinds_accept_raw_handle() {
        let mut c = fixture();
        let before = c.clone();
        let d = &mut c.deferred;
        assert!(select_pixel(
            &mut d.bindings,
            &mut d.states.dirty,
            9,
            ShaderChoice {
                hardware: false,
                resolved: None
            }
        )
        .is_err());
        assert_eq!(c, before);
        let d = &mut c.deferred;
        assert_eq!(
            select_pixel(
                &mut d.bindings,
                &mut d.states.dirty,
                9,
                ShaderChoice {
                    hardware: false,
                    resolved: Some(u32::MAX)
                }
            )
            .unwrap(),
            None
        );
        assert_eq!(d.bindings.desired.pixel_shader, u32::MAX);
    }
    #[test]
    fn integrated_fixed_pass_preflights_all_resources_and_preserves_identity_skip() {
        let mut c = fixture();
        let mut pass = Pass {
            address: 99,
            header: [0; 28],
            color_write: 15,
            stages: [[0; 28]; 8],
        };
        pass.header[0] = 1;
        pass.header[9] = 8;
        pass.stages[7][0] = 1;
        let mut last = 0;
        let before = (c.clone(), pass.clone());
        let d = Device {
            cull_mode: 3,
            lod_bias: 0,
            capacity: 8,
        };
        let choice = ShaderChoice {
            hardware: false,
            resolved: Some(77),
        };
        assert!(apply_pass(&mut c, &mut last, &mut pass, [None; 8], d, choice).is_err());
        assert_eq!((c.clone(), pass.clone()), before);
        assert_eq!(last, 0);
        pass.stages[7][0] = 0;
        pass.stages[7][4] = 0x40;
        assert!(
            apply_pass(&mut c, &mut last, &mut pass, [None; 8], d, choice)
                .unwrap()
                .immediate
                .is_some()
        );
        assert_eq!(c.deferred.bindings.desired.pixel_shader, 77);
        pass.header[9] = 255;
        let before = c.clone();
        assert!(
            !apply_pass(
                &mut c,
                &mut last,
                &mut pass,
                [None; 8],
                d,
                ShaderChoice {
                    hardware: false,
                    resolved: None
                }
            )
            .unwrap()
            .changed
        );
        assert_eq!(c, before);
    }
    #[test]
    fn integrated_hardware_branch_preserves_fixed_function_fields() {
        let mut c = fixture();
        c.deferred.states.desired.stages[0][1] = 99;
        let mut pass = Pass {
            address: 99,
            header: [0; 28],
            color_write: 15,
            stages: [[0; 28]; 8],
        };
        pass.header[9] = 1;
        apply_pass(
            &mut c,
            &mut 0,
            &mut pass,
            [None; 8],
            Device {
                cull_mode: 3,
                lod_bias: 0,
                capacity: 8,
            },
            ShaderChoice {
                hardware: true,
                resolved: None,
            },
        )
        .unwrap();
        assert_eq!(c.deferred.states.desired.stages[0][1], 99);
        assert_eq!(c.deferred.bindings.desired.pixel_shader, 3);
    }
    #[test]
    fn complete_entry_gate_preserves_mask_only_when_original_dirty_is_zero() {
        let mut c = fixture();
        c.deferred.transforms.mask = 0xdeadbeef;
        let before = c.clone();
        c.flush(8, 16, 8, false).unwrap();
        assert_eq!(c, before);
        c.deferred.states.dirty = 0x80;
        c.flush(8, 16, 8, false).unwrap();
        assert_eq!(c.deferred.transforms.mask, 0);
    }
}
