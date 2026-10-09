//! Bounded D3D8 deferred render/stage/texture state, from 1001ed70 and 10028f10.
//! Numeric API IDs are preserved. Matrices, shaders and stream/index buffers excluded.
use crate::hardware_stages::Stage;
use serde::{Deserialize, Serialize};
#[derive(Clone, Debug, Serialize, Deserialize, PartialEq, Eq)]
pub struct Values {
    /// Native deferred words 1..32; word 32 is padding, never emitted.
    pub render: [u32; 32],
    pub stages: [[u32; 21]; 8],
    pub textures: [u32; 8],
}
#[derive(Clone, Debug, Serialize, Deserialize, PartialEq, Eq)]
pub struct Cache {
    pub desired: Values,
    pub applied: Values,
    pub dirty: u32,
}
#[derive(Clone, Debug, Serialize, PartialEq, Eq)]
pub struct Call {
    pub vtable_offset: u16,
    pub arguments: Vec<u32>,
}
impl Cache {
    /// Unused-stage tail 1001f59f..1001f68e. Does not reset unrelated stage fields.
    pub fn disable_unused(
        &mut self,
        stages: &mut [Stage; 8],
        active: usize,
        capacity: usize,
        hardware_shader: bool,
    ) -> Result<(), String> {
        if active > 8 || capacity > 8 {
            return Err("Pass stage count/capacity exceeds storage".into());
        }
        for (index, stage) in stages.iter_mut().enumerate().take(capacity).skip(active) {
            stage[0] = 0;
            let s = &mut self.desired.stages[index];
            if !hardware_shader {
                stage[2] = (stage[2] & 0xffc21fff) | 0x21000;
                s[0] = 1;
                s[3] = 1;
            }
            s[6] = index as u32;
            s[9] = 0;
            self.desired.textures[index] = 0;
            self.dirty |= 0x12;
        }
        Ok(())
    }
    /// Final raw ColorWrite word and pass identity, 1001f68e..1001f6ae.
    pub fn finish_pass(&mut self, color_write: u32, address: u32, last_pass: &mut u32) {
        self.desired.render[29] = color_write;
        self.dirty |= 1;
        *last_pass = address;
    }
    /// Common render prefix 1001efb7..1001f165. Shader selection and pass tail excluded.
    pub fn render_prefix(&mut self, pass: &[u8; 28], cull_mode: u32) {
        let word = |offset| u32::from_le_bytes(pass[offset..offset + 4].try_into().unwrap());
        let flags = word(4);
        let r = &mut self.desired.render;
        r[30] = (flags >> 2) & 1;
        r[10] = flags & 1;
        r[8] = pass[8].into();
        r[9] = 5;
        r[3] = (flags >> 1) & 1;
        match word(12) {
            0 => {
                r[0] = 2;
                self.desired.stages[0][0] = 4;
                self.dirty |= 2;
            }
            1 => {
                r[0] = 3;
                self.desired.stages[0][0] = 3;
                self.dirty |= 2;
            }
            2 => {
                r[0] = 3;
                self.desired.stages[0][0] = 4;
                self.dirty |= 2;
            }
            _ => {}
        }
        r[7] = if flags & 8 != 0 { 4 } else { 8 };
        r[1] = (flags >> 4) & 1;
        r[6] = if flags & 0x20 != 0 { 1 } else { cull_mode };
        r[4] = word(20);
        r[5] = word(24);
        r[24] = word(16);
        self.dirty |= 1;
    }
    /// One stage's non-matrix translation. Resource supplies raw cache words +38/+3c.
    /// Original stage +4 (LOD bias) is changed in place only if below native -100.
    pub fn stage(
        &mut self,
        index: usize,
        stage: &mut Stage,
        resource: Option<[u32; 2]>,
        hardware_shader: bool,
        lod_bias: u32,
    ) -> Result<(), String> {
        if index >= 8 || stage[4] & 0x40 != 0 {
            return Err("Stage index/matrix transform outside bounded state path".into());
        }
        let texture = if stage[0] == 0 {
            0
        } else {
            let r = resource.ok_or("Missing original texture resource fields")?;
            if r[1] != 0 {
                r[1]
            } else {
                r[0]
            }
        };
        self.desired.textures[index] = texture;
        self.dirty |= 0x10;
        let s = &mut self.desired.stages[index];
        let flags = stage[2];
        let arguments = stage[3];
        let coords = stage[4];
        s[7] = flags & 15;
        s[8] = (flags >> 4) & 15;
        s[10] = (flags >> 8) & 15;
        if f32::from_bits(stage[1]) < -100. {
            stage[1] = lod_bias;
        }
        s[14] = stage[1];
        s[0] = (flags >> 12) & 31;
        s[3] = (flags >> 17) & 31;
        if !hardware_shader {
            s[11] = (flags >> 22) & 63;
            s[1] = arguments & 63;
            s[2] = (arguments >> 6) & 63;
            s[12] = (arguments >> 12) & 63;
            s[4] = (arguments >> 18) & 63;
            s[5] = (arguments >> 24) & 63;
            s[13] = coords & 63;
        }
        s[6] = (coords >> 7) & 0x3ffff;
        s[9] = 0;
        s[15..21].copy_from_slice(&stage[22..28]);
        self.dirty |= 2;
        Ok(())
    }
    /// Emit the supported native dirty groups, updating cache before each queued call.
    /// No GPU is invoked. HRESULT handling is external; native ignores these HRESULTs.
    pub fn flush(&mut self, capacity: usize, stencil_gate: bool) -> Result<Vec<Call>, String> {
        if capacity > 8 || self.dirty & !0x13 != 0 {
            return Err("Unimplemented D3D dirty group/capacity".into());
        }
        let mut calls = Vec::new();
        if self.dirty & 1 != 0 {
            for (word, id) in [
                (1, 8),
                (2, 14),
                (3, 7),
                (30, 168),
                (4, 15),
                (5, 19),
                (6, 20),
                (7, 22),
                (8, 23),
                (9, 24),
                (10, 25),
                (11, 27),
                (12, 28),
                (13, 34),
                (14, 36),
                (15, 37),
                (31, 29),
                (16, 47),
                (17, 52),
                (18, 53),
                (19, 54),
                (20, 55),
                (21, 56),
                (22, 57),
                (23, 58),
                (24, 59),
                (25, 60),
                (26, 137),
                (27, 139),
                (28, 141),
                (29, 164),
                (25, 60),
            ] {
                if (13..=15).contains(&word) && self.desired.render[11] == 0 {
                    continue;
                }
                if (17..=24).contains(&word) && !stencil_gate {
                    continue;
                }
                let index = word - 1;
                let v = self.desired.render[index];
                if v != self.applied.render[index] {
                    self.applied.render[index] = v;
                    calls.push(Call {
                        vtable_offset: 0xc8,
                        arguments: vec![id, v],
                    });
                }
            }
        }
        if self.dirty & 2 != 0 {
            let ids = [
                1, 2, 3, 4, 5, 6, 11, 13, 14, 24, 25, 26, 27, 28, 19, 7, 8, 9, 10, 22, 23,
            ];
            for stage in 0..capacity {
                for (index, id) in ids.into_iter().enumerate() {
                    let v = self.desired.stages[stage][index];
                    if v != self.applied.stages[stage][index] {
                        self.applied.stages[stage][index] = v;
                        calls.push(Call {
                            vtable_offset: 0xfc,
                            arguments: vec![stage as u32, id, v],
                        });
                    }
                }
            }
        }
        if self.dirty & 0x10 != 0 {
            for stage in 0..capacity {
                let v = self.desired.textures[stage];
                if v != self.applied.textures[stage] {
                    self.applied.textures[stage] = v;
                    calls.push(Call {
                        vtable_offset: 0xf4,
                        arguments: vec![stage as u32, v],
                    });
                }
            }
        }
        self.dirty = 0;
        Ok(calls)
    }
}
#[cfg(test)]
mod tests {
    use super::*;
    fn cache() -> Cache {
        let values = Values {
            render: [9; 32],
            stages: [[9; 21]; 8],
            textures: [9; 8],
        };
        Cache {
            desired: values.clone(),
            applied: values,
            dirty: 0,
        }
    }
    #[test]
    fn render_flags_and_fill_selectors_preserve_unrelated_state() {
        let mut c = cache();
        let mut p = [0; 28];
        p[4] = 0x3f;
        p[8] = 77;
        c.render_prefix(&p, 3);
        assert_eq!(c.desired.render[0], 2);
        assert_eq!(c.desired.render[7], 4);
        assert_eq!(c.desired.render[8], 77);
        assert_eq!(c.desired.render[6], 1);
        assert_eq!(c.desired.render[11], 9);
        assert_eq!(c.dirty, 3);
        assert_eq!(c.desired.stages[0][0], 4);
    }
    #[test]
    fn unknown_fill_selector_keeps_render_and_stage_history() {
        let mut c = cache();
        let mut p = [0; 28];
        p[12..16].copy_from_slice(&255u32.to_le_bytes());
        c.render_prefix(&p, 0xdeadbeef);
        assert_eq!(c.desired.render[0], 9);
        assert_eq!(c.desired.stages[0][0], 9);
        assert_eq!(c.dirty, 1);
    }
    #[test]
    fn stage_lod_sentinel_and_hardware_shader_preserve_fixed_function_fields() {
        let mut c = cache();
        let mut s = [0; 28];
        s[0] = 7;
        s[1] = (-101f32).to_bits();
        s[2] = 0x1f;
        s[22..28].copy_from_slice(&[1, 2, 3, 4, 5, 6]);
        c.stage(2, &mut s, Some([11, 12]), true, (-2f32).to_bits())
            .unwrap();
        assert_eq!(s[1], (-2f32).to_bits());
        assert_eq!(c.desired.textures[2], 12);
        assert_eq!(c.desired.stages[2][1], 9);
        assert_eq!(c.desired.stages[2][7], 15);
        assert_eq!(c.desired.stages[2][15..21], [1, 2, 3, 4, 5, 6]);
        s[1] = 0x7fc12345;
        c.stage(2, &mut s, Some([11, 0]), false, 0).unwrap();
        assert_eq!(s[1], 0x7fc12345);
        assert_eq!(c.desired.textures[2], 11);
    }
    #[test]
    fn excluded_stage_inputs_fail_before_mutation() {
        let mut c = cache();
        let before = c.clone();
        let mut s = [0; 28];
        s[0] = 1;
        assert!(c.stage(0, &mut s, None, true, 0).is_err());
        s[4] = 0x40;
        assert!(c.stage(0, &mut s, Some([1, 2]), true, 0).is_err());
        assert_eq!(c, before);
    }
    #[test]
    fn flush_suppresses_repeats_and_defers_fog_and_stencil_gate_fields() {
        let mut c = cache();
        c.desired.render = [0; 32];
        c.dirty = 1;
        let calls = c.flush(0, false).unwrap();
        assert!(!calls.is_empty());
        assert_eq!(c.applied.render[12], 9);
        assert_eq!(c.applied.render[16], 9);
        c.dirty = 1;
        assert!(c.flush(0, false).unwrap().is_empty());
        c.desired.render[11] = 1;
        c.dirty = 1;
        let calls = c.flush(0, true).unwrap();
        assert!(calls.iter().any(|v| v.arguments[0] == 34));
    }
    #[test]
    fn flush_capacity_and_dirty_groups_are_checked_before_cache_changes() {
        let mut c = cache();
        c.desired.textures[0] = 13;
        c.dirty = 0x50;
        let before = c.clone();
        assert!(c.flush(8, false).is_err());
        assert_eq!(c, before);
        c.dirty = 0x10;
        assert!(c.flush(9, false).is_err());
        assert_eq!(
            c.flush(1, false).unwrap(),
            [Call {
                vtable_offset: 0xf4,
                arguments: vec![0, 13]
            }]
        );
    }
}
