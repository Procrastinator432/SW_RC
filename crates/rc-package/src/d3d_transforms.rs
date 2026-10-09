//! Native raw transform state: pass stage branch and deferred D3D8 transform calls.
use crate::{
    d3d_pass::{Device, Pass},
    d3d_state::{Cache, Call},
    hardware_stages::Stage,
};
use serde::{Deserialize, Serialize};

#[derive(Clone, Debug, Serialize, Deserialize, PartialEq, Eq)]
pub struct Transforms {
    /// Native cache+324, eleven consecutive raw 4x4 matrices.
    pub words: [[u32; 16]; 11],
    /// Native cache+624. Cleared at flush end, independently of group gates.
    pub mask: u32,
}
#[derive(Clone, Debug, Serialize, PartialEq, Eq)]
pub struct TransformCall {
    pub vtable_offset: u16,
    pub transform: u32,
    /// Copied payload pointed to by the original call, not sixteen API arguments.
    pub matrix: [u32; 16],
}
#[derive(Clone, Debug, Serialize, PartialEq, Eq)]
pub struct Plan {
    pub before: Vec<Call>,
    pub transforms: Vec<TransformCall>,
    pub after: Vec<Call>,
}
impl Transforms {
    /// Stage branch 1001f176..1001f58e, including raw MOVSD.REP matrix copy.
    pub fn stage(
        &mut self,
        cache: &mut Cache,
        index: usize,
        stage: &mut Stage,
        resource: Option<[u32; 2]>,
        hardware: bool,
        lod_bias: u32,
    ) -> Result<(), String> {
        if index >= 8 {
            return Err("Transform stage index exceeds eight".into());
        }
        let matrix = stage[4] & 0x40 != 0;
        // Reuse common translation without changing the original packed flag word.
        let mut common = *stage;
        common[4] &= !0x40;
        cache.stage(index, &mut common, resource, hardware, lod_bias)?;
        stage[1] = common[1];
        if matrix {
            cache.desired.stages[index][9] = stage[5] & 0x1ff;
            self.words[index].copy_from_slice(&stage[6..22]);
            self.mask |= 1 << index;
            cache.dirty |= 0x40;
        }
        Ok(())
    }
    /// Render/stage -> transforms -> textures, preserving native gates and order.
    /// Native has no applied-matrix comparison; marking an identical matrix re-emits it.
    pub fn flush(
        &mut self,
        cache: &mut Cache,
        capacity: usize,
        stencil_gate: bool,
    ) -> Result<Plan, String> {
        if capacity > 8 || cache.dirty & !0x73 != 0 {
            return Err("Unsupported dirty group/capacity in transform plan".into());
        }
        if cache.dirty == 0 {
            return Ok(Plan {
                before: vec![],
                transforms: vec![],
                after: vec![],
            });
        }
        self.flush_body(cache, capacity, stencil_gate)
    }
    /// Outer composite planners already established a nonzero original dirty word.
    pub(crate) fn flush_body(
        &mut self,
        cache: &mut Cache,
        capacity: usize,
        stencil_gate: bool,
    ) -> Result<Plan, String> {
        if capacity > 8 || cache.dirty & !0x73 != 0 {
            return Err("Unsupported dirty group/capacity in transform plan".into());
        }
        let dirty = cache.dirty;
        cache.dirty = dirty & 3;
        let before = cache.flush(capacity, stencil_gate)?;
        let mut transforms = Vec::new();
        for (index, id) in [16, 17, 18, 19, 20, 21, 22, 23, 2, 3, 256]
            .into_iter()
            .enumerate()
        {
            let group = if index == 10 { 0x20 } else { 0x40 };
            if dirty & group != 0 && self.mask & (1 << index) != 0 {
                transforms.push(TransformCall {
                    vtable_offset: 0x94,
                    transform: id,
                    matrix: self.words[index],
                });
            }
        }
        cache.dirty = dirty & 0x10;
        let after = cache.flush(capacity, stencil_gate)?;
        self.mask = 0;
        Ok(Plan {
            before,
            transforms,
            after,
        })
    }
}
/// HardwareShader branch only, with pre-resolved resource words. Invalid inputs are atomic.
pub fn apply_hardware_pass(
    cache: &mut Cache,
    transforms: &mut Transforms,
    last_pass: &mut u32,
    pass: &mut Pass,
    resources: [Option<[u32; 2]>; 8],
    device: Device,
) -> Result<bool, String> {
    if *last_pass == pass.address {
        return Ok(false);
    }
    let active = usize::from(pass.header[9]);
    if pass.address == 0 || active > 8 || device.capacity > 8 {
        return Err("Invalid bounded hardware pass address/count/capacity".into());
    }
    for (stage, resource) in pass.stages.iter().zip(resources).take(active) {
        if stage[0] != 0 && resource.is_none() {
            return Err("Missing active texture resource".into());
        }
    }
    cache.render_prefix(&pass.header, device.cull_mode);
    for (index, resource) in resources.into_iter().enumerate().take(active) {
        transforms.stage(
            cache,
            index,
            &mut pass.stages[index],
            resource,
            true,
            device.lod_bias,
        )?;
    }
    cache.disable_unused(&mut pass.stages, active, device.capacity, true)?;
    cache.finish_pass(pass.color_write, pass.address, last_pass);
    Ok(true)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::d3d_state::Values;
    fn fixture() -> (Cache, Transforms) {
        let v = Values {
            render: [0; 32],
            stages: [[0; 21]; 8],
            textures: [0; 8],
        };
        (
            Cache {
                desired: v.clone(),
                applied: v,
                dirty: 0,
            },
            Transforms {
                words: [[7; 16]; 11],
                mask: 0,
            },
        )
    }
    #[test]
    fn matrix_payload_flags_and_nan_are_raw() {
        let (mut c, mut t) = fixture();
        let mut s = [0; 28];
        s[4] = 0x40;
        s[5] = 0xfffff9ab;
        s[6..22].fill(0x7fc12345);
        s[1] = (-101f32).to_bits();
        t.stage(&mut c, 7, &mut s, None, true, 0x80000000).unwrap();
        assert_eq!(s[4], 0x40);
        assert_eq!(s[1], 0x80000000);
        assert_eq!(c.desired.stages[7][9], 0x1ab);
        assert_eq!(t.words[7], [0x7fc12345; 16]);
        assert_eq!(t.mask, 0x80);
        assert_eq!(c.dirty, 0x52);
    }
    #[test]
    fn no_matrix_branch_keeps_history_and_existing_mask() {
        let (mut c, mut t) = fixture();
        t.mask = 8;
        t.stage(&mut c, 3, &mut [0; 28], None, false, 0).unwrap();
        assert_eq!(t.words, [[7; 16]; 11]);
        assert_eq!(t.mask, 8);
        assert_eq!(c.dirty, 0x12);
    }
    #[test]
    fn missing_resource_and_invalid_stage_do_not_write() {
        let (mut c, mut t) = fixture();
        let before = (c.clone(), t.clone());
        let mut s = [1; 28];
        s[4] = 0x40;
        assert!(t.stage(&mut c, 0, &mut s, None, true, 0).is_err());
        assert!(t.stage(&mut c, 8, &mut s, Some([1, 2]), true, 0).is_err());
        assert_eq!((c, t), before);
        assert_eq!(s[1], 1);
    }
    #[test]
    fn matrix_gates_order_and_mask_clear_do_not_use_capacity() {
        let (mut c, mut t) = fixture();
        c.dirty = 0x60;
        t.mask = 0x800007ff;
        let p = t.flush(&mut c, 0, false).unwrap();
        assert_eq!(
            p.transforms.iter().map(|v| v.transform).collect::<Vec<_>>(),
            [16, 17, 18, 19, 20, 21, 22, 23, 2, 3, 256]
        );
        assert_eq!(t.mask, 0);
        assert_eq!(c.dirty, 0);
        c.dirty = 0x40;
        t.mask = 0x400;
        assert!(t.flush(&mut c, 8, false).unwrap().transforms.is_empty());
        assert_eq!(t.mask, 0);
    }
    #[test]
    fn identical_marked_matrix_is_emitted_again_but_unmarked_repeat_is_empty() {
        let (mut c, mut t) = fixture();
        c.dirty = 0x40;
        t.mask = 1;
        let first = t.flush(&mut c, 8, false).unwrap();
        assert!(t.flush(&mut c, 8, false).unwrap().transforms.is_empty());
        c.dirty = 0x40;
        t.mask = 1;
        assert_eq!(t.flush(&mut c, 8, false).unwrap(), first);
    }
    #[test]
    fn invalid_flush_preserves_entire_state() {
        let (mut c, mut t) = fixture();
        c.dirty = 0x44;
        t.mask = 1;
        let before = (c.clone(), t.clone());
        assert!(t.flush(&mut c, 8, false).is_err());
        assert!(t.flush(&mut c, 9, false).is_err());
        assert_eq!((c, t), before);
    }
    #[test]
    fn zero_dirty_entry_preserves_pending_matrix_mask() {
        let (mut c, mut t) = fixture();
        t.mask = 1;
        let before = (c.clone(), t.clone());
        assert!(t.flush(&mut c, 8, false).unwrap().transforms.is_empty());
        assert_eq!((c.clone(), t.clone()), before);
        c.dirty = 1;
        t.flush(&mut c, 8, false).unwrap();
        assert_eq!(t.mask, 0);
    }
    #[test]
    fn pipeline_preflight_and_identity_skip_include_matrix_state() {
        let (mut c, mut t) = fixture();
        let mut p = Pass {
            address: 99,
            header: [0; 28],
            color_write: 15,
            stages: [[0; 28]; 8],
        };
        p.header[9] = 8;
        p.stages[7][0] = 1;
        let before = (c.clone(), t.clone(), p.clone());
        let mut last = 0;
        let d = Device {
            cull_mode: 3,
            lod_bias: 0,
            capacity: 8,
        };
        assert!(apply_hardware_pass(&mut c, &mut t, &mut last, &mut p, [None; 8], d).is_err());
        assert_eq!((c.clone(), t.clone(), p.clone()), before);
        assert_eq!(last, 0);
        p.stages[7][0] = 0;
        p.stages[7][4] = 0x40;
        assert!(apply_hardware_pass(&mut c, &mut t, &mut last, &mut p, [None; 8], d).unwrap());
        assert_eq!(last, 99);
        assert_eq!(t.mask, 0x80);
        let before = (c.clone(), t.clone());
        p.header[9] = 255;
        assert!(!apply_hardware_pass(&mut c, &mut t, &mut last, &mut p, [None; 8], d).unwrap());
        assert_eq!((c, t), before);
    }
}
