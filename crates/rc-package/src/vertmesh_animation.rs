//! GetFrame scheduling at Engine!1055f217..1055f28f, with supplied GetSequence result.
use crate::{d3d_dynamic_draw::Runtime, vertmesh_frame, vertmesh_indices};
use serde::{Deserialize, Serialize};

#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct Sequence {
    /// FMeshAnimSeq +0c and +10.
    pub start: i32,
    pub count: i32,
}
#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct Clock {
    pub vertex_count: u32,
    pub sequence: Option<Sequence>,
    /// AnimChannel +1c, raw float32 bits. Unused when GetSequence returns null.
    pub time: u32,
}
#[derive(Clone, Debug, Serialize, PartialEq, Eq)]
pub struct Selection {
    pub first_offset: u32,
    pub next_offset: u32,
    pub fraction: u32,
    pub interpolated: bool,
}
impl Clock {
    pub fn select(&self) -> Result<Selection, String> {
        if self.vertex_count > 65536 {
            return Err("Animation vertex count exceeds bounded mesh contract".into());
        }
        let Some(sequence) = &self.sequence else {
            return Ok(Selection {
                first_offset: 0,
                next_offset: 0,
                fraction: 0,
                interpolated: false,
            });
        };
        let mut time = f32::from_bits(self.time);
        if sequence.count <= 0 || sequence.start < 0 || !time.is_finite() {
            return Err("Invalid animation sequence or time".into());
        }
        if time < 0.0 {
            time = 0.0;
        }
        let product = sequence.count as f32 * time;
        // Native LDMXCSR 3f80 selects round toward -infinity for CVTSS2SI.
        if !product.is_finite() || product >= 2147483648.0 {
            return Err("Animation time exceeds signed conversion range".into());
        }
        let integer = product.floor() as i32;
        let fraction = product - integer as f32;
        let offset = |integer: i32| -> Result<u32, String> {
            let frame = (integer % sequence.count)
                .checked_add(sequence.start)
                .ok_or("Animation frame index overflow")?;
            let offset = (frame as u32)
                .checked_mul(self.vertex_count)
                .ok_or("Animation vertex offset overflow")?;
            if offset > u32::MAX / 4 {
                return Err("Animation byte offset overflow".into());
            }
            Ok(offset)
        };
        Ok(Selection {
            first_offset: offset(integer)?,
            next_offset: offset(integer + 1)?,
            fraction: fraction.to_bits(),
            interpolated: fraction > 0.0,
        })
    }
}
#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct Input {
    pub clock: Clock,
    /// Flattened mesh +60 and +130 packed word arrays, in frame order.
    pub positions: Vec<u32>,
    pub normals: Vec<u32>,
    pub requested_count: i32,
    pub cache: vertmesh_frame::Cache,
    pub wedges: Vec<u16>,
    pub mode: u32,
    pub stride: u32,
    pub bytes: Vec<u8>,
}
#[derive(Clone, Debug, Serialize, PartialEq, Eq)]
pub struct Output {
    pub selection: Selection,
    pub frame: vertmesh_frame::Output,
}
impl Input {
    pub fn resolve(&self) -> Result<(Selection, vertmesh_frame::Input), String> {
        let selection = self.clock.select()?;
        if !(0..=65536).contains(&self.requested_count)
            || self.positions.len() > 4194304
            || self.normals.len() > 4194304
        {
            return Err("Animation source exceeds bounded contract".into());
        }
        let count = (self.requested_count as u32).min(self.clock.vertex_count) as usize;
        let frame = |offset: u32| -> Result<vertmesh_frame::Frame, String> {
            // No native source read occurs for a zero vertex count.
            if count == 0 {
                return Ok(vertmesh_frame::Frame {
                    positions: vec![],
                    normals: vec![],
                });
            }
            let start = offset as usize;
            let end = start
                .checked_add(count)
                .ok_or("Animation source range overflow")?;
            Ok(vertmesh_frame::Frame {
                positions: self
                    .positions
                    .get(start..end)
                    .ok_or("Short animation position bank")?
                    .to_vec(),
                normals: self
                    .normals
                    .get(start..end)
                    .ok_or("Short animation normal bank")?
                    .to_vec(),
            })
        };
        let first = frame(selection.first_offset)?;
        let next = if selection.interpolated {
            Some(frame(selection.next_offset)?)
        } else {
            None
        };
        let decoded = vertmesh_frame::Input {
            vertex_count: self.clock.vertex_count,
            requested_count: self.requested_count,
            first,
            next,
            fraction: selection.fraction,
            cache: self.cache.clone(),
            wedges: self.wedges.clone(),
            mode: self.mode,
            stride: self.stride,
            bytes: self.bytes.clone(),
        };
        Ok((selection, decoded))
    }
    pub fn build(&self) -> Result<Output, String> {
        let (selection, frame) = self.resolve()?;
        Ok(Output {
            selection,
            frame: frame.build()?,
        })
    }
}
#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct Request {
    pub animation: Input,
    pub geometry: vertmesh_indices::Request,
}
#[derive(Clone, Debug, Serialize)]
pub struct Plan {
    pub animation: Output,
    pub draw: vertmesh_indices::Plan,
}
impl Runtime {
    pub fn submit_animated_frame(&mut self, request: &mut Request) -> Result<Plan, String> {
        let (selection, frame) = request.animation.resolve()?;
        let mut nested = vertmesh_frame::Request {
            frame,
            geometry: request.geometry.clone(),
        };
        let result = self.submit_frame(&mut nested)?;
        request.animation.cache = result.frame.cache.clone();
        request.animation.bytes = result.frame.bytes.clone();
        request.geometry = nested.geometry;
        Ok(Plan {
            animation: Output {
                selection,
                frame: result.frame,
            },
            draw: result.draw,
        })
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    fn clock(time: f32) -> Clock {
        Clock {
            vertex_count: 3,
            sequence: Some(Sequence { start: 2, count: 4 }),
            time: time.to_bits(),
        }
    }
    fn input(time: f32) -> Input {
        Input {
            clock: clock(time),
            positions: (0..18).collect(),
            normals: vec![0; 18],
            requested_count: 1,
            cache: vertmesh_frame::Cache {
                positions: vec![[9; 3]; 3],
                normals: vec![[8; 3]; 3],
                last_count: 0,
                flags: [0, 0],
            },
            wedges: vec![0],
            mode: 0,
            stride: 32,
            bytes: vec![0xa5; 32],
        }
    }
    #[test]
    fn null_sequence_ignores_time() {
        let mut q = clock(f32::NAN);
        q.sequence = None;
        assert_eq!(q.select().unwrap().first_offset, 0);
    }
    #[test]
    fn negative_time_clamps_to_first_sequence_frame() {
        assert_eq!(clock(-0.75).select().unwrap().first_offset, 6);
    }
    #[test]
    fn negative_zero_fraction_survives() {
        assert_eq!(clock(-0.0).select().unwrap().fraction, 0x80000000);
    }
    #[test]
    fn floor_does_not_round_to_nearest() {
        let p = clock(0.2).select().unwrap();
        assert_eq!(p.first_offset, 6);
        assert_eq!(p.fraction, 0.8f32.to_bits());
    }
    #[test]
    fn last_frame_interpolates_to_sequence_start() {
        let p = clock(0.875).select().unwrap();
        assert_eq!((p.first_offset, p.next_offset), (15, 6));
        assert_eq!(p.fraction, 0.5f32.to_bits());
    }
    #[test]
    fn whole_cycles_wrap() {
        assert_eq!(clock(3.0).select().unwrap().first_offset, 6);
    }
    #[test]
    fn one_frame_sequence_can_interpolate_with_itself() {
        let mut q = clock(0.5);
        q.sequence.as_mut().unwrap().count = 1;
        let p = q.select().unwrap();
        assert_eq!(p.first_offset, p.next_offset);
        assert!(p.interpolated);
    }
    #[test]
    fn invalid_divisor_and_start_are_rejected() {
        let mut q = clock(0.0);
        q.sequence.as_mut().unwrap().count = 0;
        assert!(q.select().is_err());
        q.sequence = Some(Sequence {
            start: -1,
            count: 4,
        });
        assert!(q.select().is_err());
    }
    #[test]
    fn nonfinite_and_conversion_overflow_are_rejected() {
        for v in [f32::NAN, f32::INFINITY, 536870912.0] {
            assert!(clock(v).select().is_err());
        }
    }
    #[test]
    fn native_float_product_precedes_floor() {
        let mut q = clock(0.5);
        q.sequence.as_mut().unwrap().count = 16777217;
        q.vertex_count = 0;
        assert_eq!(q.select().unwrap().fraction, 0);
    }
    #[test]
    fn byte_offset_overflow_is_rejected() {
        let mut q = clock(0.0);
        q.vertex_count = 65536;
        q.sequence.as_mut().unwrap().start = 65536;
        assert!(q.select().is_err());
    }
    #[test]
    fn bank_selection_drives_decoded_positions() {
        let p = input(0.875).build().unwrap();
        assert_eq!(p.frame.cache.positions[0][0], 10.5f32.to_bits());
        assert_eq!(&p.frame.bytes[24..], &[0xa5; 8]);
    }
    #[test]
    fn inactive_next_frame_is_not_read() {
        let mut q = input(0.0);
        q.positions.truncate(7);
        q.normals.truncate(7);
        assert!(q.build().is_ok());
        q.clock.time = 0.125f32.to_bits();
        assert!(q.build().is_err());
    }
    #[test]
    fn zero_count_ignores_empty_banks() {
        let mut q = input(0.5);
        q.positions.clear();
        q.normals.clear();
        q.requested_count = 0;
        assert_eq!(q.build().unwrap().frame.cache.flags, [1, 1]);
    }
    #[test]
    fn short_bank_does_not_mutate_input() {
        let mut q = input(0.5);
        q.normals.clear();
        let before = serde_json::to_value(&q).unwrap();
        assert!(q.build().is_err());
        assert_eq!(serde_json::to_value(q).unwrap(), before);
    }
}
