//! Complete bounded hardware-pass state translation; shader/resource acquisition is external.
use crate::{d3d_state::Cache, hardware_stages::Stage};
use serde::{Deserialize, Serialize};

#[derive(Clone, Debug, Serialize, Deserialize, PartialEq, Eq)]
pub struct Pass {
    /// Original pointer identity. Equal pointers skip even if their contents changed.
    pub address: u32,
    pub header: [u8; 28],
    /// Original pass+20, with no semantic mask or normalization.
    pub color_write: u32,
    pub stages: [Stage; 8],
}
#[derive(Clone, Copy, Debug, Serialize, Deserialize)]
pub struct Device {
    pub cull_mode: u32,
    pub lod_bias: u32,
    pub capacity: usize,
}
/// Requires native pass+3a8 nonzero. No shader selection occurs in that branch.
/// Resource words +38/+3c must already be resolved for each nonnull active stage.
/// Unsupported inputs are preflighted before writes; native valid-input behavior is preserved.
pub fn apply_hardware_pass(
    cache: &mut Cache,
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
        if stage[4] & 0x40 != 0 || (stage[0] != 0 && resource.is_none()) {
            return Err("Unsupported matrix or missing active texture resource".into());
        }
    }
    cache.render_prefix(&pass.header, device.cull_mode);
    for (index, resource) in resources.into_iter().enumerate().take(active) {
        cache.stage(
            index,
            &mut pass.stages[index],
            resource,
            true,
            device.lod_bias,
        )?;
    }
    cache.disable_unused(&mut pass.stages, active, device.capacity, true)?;
    let color_write = pass.color_write;
    cache.finish_pass(color_write, pass.address, last_pass);
    Ok(true)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::d3d_state::Values;
    fn fixture() -> (Cache, Pass) {
        let v = Values {
            render: [7; 32],
            stages: [[7; 21]; 8],
            textures: [7; 8],
        };
        (
            Cache {
                desired: v.clone(),
                applied: v,
                dirty: 0,
            },
            Pass {
                address: 0x1234,
                header: [0; 28],
                color_write: 0,
                stages: [[0; 28]; 8],
            },
        )
    }
    #[test]
    fn unused_fixed_stages_keep_non_operation_fields() {
        let (mut c, mut p) = fixture();
        p.stages = [[u32::MAX; 28]; 8];
        c.disable_unused(&mut p.stages, 2, 5, false).unwrap();
        assert_eq!(p.stages[2][2], 0xffc21fff | 0x21000);
        assert_eq!(c.desired.stages[2][0], 1);
        assert_eq!(c.desired.stages[2][3], 1);
        assert_eq!(c.desired.stages[2][7], 7);
        assert_eq!(p.stages[5], [u32::MAX; 28]);
        assert_eq!(c.dirty, 0x12);
    }
    #[test]
    fn unused_hardware_stages_preserve_operations_and_packed_words() {
        let (mut c, mut p) = fixture();
        p.stages = [[123; 28]; 8];
        c.disable_unused(&mut p.stages, 0, 1, true).unwrap();
        assert_eq!(p.stages[0][0], 0);
        assert_eq!(p.stages[0][2], 123);
        assert_eq!(c.desired.stages[0][0], 7);
        assert_eq!(c.desired.stages[0][9], 0);
        assert_eq!(c.desired.textures[0], 0);
    }
    #[test]
    fn tail_empty_range_and_invalid_capacity_do_not_write() {
        let (mut c, mut p) = fixture();
        let before = (c.clone(), p.clone());
        c.disable_unused(&mut p.stages, 8, 0, false).unwrap();
        assert!(c.disable_unused(&mut p.stages, 9, 8, false).is_err());
        assert!(c.disable_unused(&mut p.stages, 0, 9, false).is_err());
        assert_eq!((c, p), before);
    }
    #[test]
    fn finish_keeps_raw_color_mask_and_updates_identity() {
        let (mut c, _) = fixture();
        let mut last = 99;
        c.finish_pass(0xdeadbeef, 123, &mut last);
        assert_eq!(last, 123);
        let calls = c.flush(0, false).unwrap();
        assert_eq!(calls.len(), 1);
        assert_eq!(calls[0].arguments, [168, 0xdeadbeef]);
    }
    #[test]
    fn whole_pass_failure_is_atomic_even_at_last_active_stage() {
        let (mut c, mut p) = fixture();
        p.header[9] = 8;
        p.stages[7][0] = 1;
        let before = (c.clone(), p.clone());
        let mut last = 77;
        assert!(apply_hardware_pass(
            &mut c,
            &mut last,
            &mut p,
            [None; 8],
            Device {
                cull_mode: 3,
                lod_bias: 0,
                capacity: 8
            }
        )
        .is_err());
        assert_eq!((c, p), before);
        assert_eq!(last, 77);
    }
    #[test]
    fn wrapped_negative_setup_count_is_rejected_without_writes() {
        let (mut c, mut p) = fixture();
        p.header[9] = 254;
        let before = (c.clone(), p.clone());
        let mut last = 0;
        assert!(apply_hardware_pass(
            &mut c,
            &mut last,
            &mut p,
            [None; 8],
            Device {
                cull_mode: 3,
                lod_bias: 0,
                capacity: 8,
            }
        )
        .is_err());
        assert_eq!((c, p), before);
        assert_eq!(last, 0);
    }
    #[test]
    fn identical_pointer_skips_changed_invalid_contents_and_device() {
        let (mut c, mut p) = fixture();
        let mut last = 0;
        let d = Device {
            cull_mode: 3,
            lod_bias: 0,
            capacity: 8,
        };
        assert!(apply_hardware_pass(&mut c, &mut last, &mut p, [None; 8], d).unwrap());
        p.header[9] = 255;
        let before = c.clone();
        assert!(!apply_hardware_pass(
            &mut c,
            &mut last,
            &mut p,
            [None; 8],
            Device { capacity: 99, ..d }
        )
        .unwrap());
        assert_eq!(c, before);
    }
}
