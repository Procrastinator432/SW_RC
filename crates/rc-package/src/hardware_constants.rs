//! D3DDrv 1000dbe0: PS then VS through one persistent 96-plane scratch buffer.
//! Counts belong to the material; uploaded device registers have separate history.
use crate::{
    material_constants::Flicker,
    shader_constants::{Bank, Constant, Host, Matrix},
};
use serde::Serialize;

#[derive(Clone, Copy, Debug, Serialize)]
pub struct Counts {
    pub pixel: i32,
    pub vertex: i32,
}
impl Default for Counts {
    fn default() -> Self {
        Self {
            pixel: -3,
            vertex: -2,
        }
    }
}
#[derive(Clone, Debug, Serialize)]
pub struct DeviceConstants {
    scratch: Bank,
    pub pixel: Bank,
    pub vertex: Bank,
}
impl DeviceConstants {
    /// All initial words are explicit host fixtures, including device registers.
    pub fn new(scratch: [[u32; 4]; 96], pixel: [[u32; 4]; 8], vertex: [[u32; 4]; 96]) -> Self {
        Self {
            scratch: Bank {
                words: scratch.to_vec(),
                count: 0,
            },
            pixel: Bank {
                words: pixel.to_vec(),
                count: 0,
            },
            vertex: Bank {
                words: vertex.to_vec(),
                count: 0,
            },
        }
    }
    #[allow(clippy::too_many_arguments)]
    pub fn update(
        &mut self,
        pixel: &[Constant; 8],
        vertex: &[Constant; 96],
        counts: &mut Counts,
        host: Host,
        flicker: &mut Flicker,
        inverse: &mut dyn FnMut(Matrix) -> Result<Matrix, String>,
        random: &mut dyn FnMut() -> Result<u16, String>,
    ) -> Result<(), String> {
        if self.scratch.words.len() != 96
            || self.pixel.words.len() != 8
            || self.vertex.words.len() != 96
        {
            return Err("Invalid hardware constant storage".into());
        }
        // Structural safety checks precede mutation. Native malformed counts are excluded.
        for (count, marker, capacity) in [(counts.pixel, -3, 8), (counts.vertex, -2, 96)] {
            if count != marker && !(0..=capacity).contains(&count) {
                return Err("Invalid material constant count".into());
            }
        }
        let mut padded = [Constant::default(); 96];
        padded[..8].copy_from_slice(pixel);
        for (bindings, count, uploaded, capacity) in [
            (&padded, &mut counts.pixel, &mut self.pixel, 8usize),
            (vertex, &mut counts.vertex, &mut self.vertex, 96usize),
        ] {
            if *count == 0 {
                continue;
            }
            self.scratch.count = *count;
            let result = self
                .scratch
                .update(bindings, host, flicker, inverse, random);
            *count = self.scratch.count;
            result?; // retain scratch/count/earlier upload if a safe host error occurs
            let n = usize::try_from(*count).map_err(|_| "Negative upload count")?;
            if n > capacity {
                return Err("Constant upload exceeds device stage capacity".into());
            }
            if n != 0 {
                uploaded.words[..n].copy_from_slice(&self.scratch.words[..n]);
                uploaded.count = *count;
            }
        }
        Ok(())
    }
}
#[cfg(test)]
mod tests {
    use super::*;
    fn host() -> Host {
        Host {
            scene: Default::default(),
            lighting: Default::default(),
            object_to_world: [[0; 4]; 4],
            world_to_camera: [[0; 4]; 4],
            projection: [[0; 4]; 4],
            camera_to_world: None,
            editor: false,
            engine_time: 0.,
        }
    }
    fn apply(
        d: &mut DeviceConstants,
        p: &[Constant; 8],
        v: &[Constant; 96],
        c: &mut Counts,
    ) -> Result<(), String> {
        d.update(
            p,
            v,
            c,
            host(),
            &mut Flicker::default(),
            &mut |_| panic!(),
            &mut || panic!(),
        )
    }
    fn initial() -> DeviceConstants {
        DeviceConstants::new([[7; 4]; 96], [[8; 4]; 8], [[9; 4]; 96])
    }
    #[test]
    fn pixel_precedes_vertex_and_only_uploaded_ranges_change() {
        let mut d = initial();
        let mut p = [Constant::default(); 8];
        let mut v = [Constant::default(); 96];
        p[0] = Constant {
            kind: 1,
            words: [11; 4],
        };
        v[1] = Constant {
            kind: 1,
            words: [12; 4],
        };
        let mut c = Counts::default();
        apply(&mut d, &p, &v, &mut c).unwrap();
        assert_eq!(d.pixel.words[0], [11; 4]);
        assert_eq!(d.pixel.words[1], [8; 4]);
        assert_eq!(d.vertex.words[..3], [[11; 4], [12; 4], [9; 4]]);
        assert_eq!((c.pixel, c.vertex), (1, 2));
    }
    #[test]
    fn zero_counts_skip_and_material_switch_keeps_shared_scratch() {
        let mut d = initial();
        let mut p = [Constant::default(); 8];
        let mut v = [Constant::default(); 96];
        p[0] = Constant {
            kind: 1,
            words: [11; 4],
        };
        v[1] = Constant {
            kind: 1,
            words: [12; 4],
        };
        v[0] = Constant {
            kind: 1,
            words: [13; 4],
        };
        apply(&mut d, &p, &v, &mut Counts::default()).unwrap();
        let before = serde_json::to_value(&d).unwrap();
        apply(
            &mut d,
            &p,
            &v,
            &mut Counts {
                pixel: 0,
                vertex: 0,
            },
        )
        .unwrap();
        assert_eq!(serde_json::to_value(&d).unwrap(), before);
        apply(
            &mut d,
            &[Constant::default(); 8],
            &[Constant::default(); 96],
            &mut Counts::default(),
        )
        .unwrap();
        assert_eq!(d.pixel.words[0], [13; 4]);
        assert_eq!(d.vertex.words[1], [12; 4]);
    }
    #[test]
    fn vertex_error_preserves_pixel_upload_and_partial_scratch() {
        let mut d = initial();
        let mut p = [Constant::default(); 8];
        let mut v = [Constant::default(); 96];
        p[0] = Constant {
            kind: 1,
            words: [11; 4],
        };
        v[0] = Constant {
            kind: 1,
            words: [12; 4],
        };
        v[1].kind = 35;
        let mut c = Counts::default();
        assert!(apply(&mut d, &p, &v, &mut c).is_err());
        assert_eq!(d.pixel.words[0], [11; 4]);
        assert_eq!(d.vertex.words[0], [9; 4]);
        assert_eq!(d.scratch.words[0], [12; 4]);
        assert_eq!((c.pixel, c.vertex), (1, 2));
    }
    #[test]
    fn device_registers_beyond_upload_count_keep_device_history() {
        let mut d = initial();
        let mut v = [Constant::default(); 96];
        v[17] = Constant {
            kind: 1,
            words: [99; 4],
        };
        apply(
            &mut d,
            &[Constant::default(); 8],
            &v,
            &mut Counts::default(),
        )
        .unwrap();
        apply(
            &mut d,
            &[Constant::default(); 8],
            &[Constant::default(); 96],
            &mut Counts::default(),
        )
        .unwrap();
        assert_eq!(d.vertex.words[17], [99; 4]);
        assert_eq!(d.vertex.count, 1);
        assert_eq!(d.scratch.words[17], [99; 4]);
    }
}
