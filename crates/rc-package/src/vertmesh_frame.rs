//! GetFrame packed frame decode/interpolation and strided output, Engine!1055f2b0..1055f650.
//! Sequence/time selection and cache allocation are supplied snapshots.
use crate::{d3d_dynamic_draw::Runtime, vertmesh_indices};
use serde::{Deserialize, Serialize};

pub fn position(word: u32) -> [u32; 3] {
    [
        ((word << 21) as i32) >> 21,
        ((word << 10) as i32) >> 21,
        (word as i32) >> 22,
    ]
    .map(|v| (v as f32).to_bits())
}
pub fn normal(word: u32) -> [u32; 3] {
    [word & 1023, (word >> 10) & 1023, (word >> 20) & 1023].map(|v| (v as f32 - 512.0).to_bits())
}
fn interpolate(a: [u32; 3], b: [u32; 3], fraction: f32) -> Result<[u32; 3], String> {
    let result = std::array::from_fn(|c| {
        let a = f32::from_bits(a[c]);
        let b = f32::from_bits(b[c]);
        ((b - a) * fraction + a).to_bits()
    });
    if result.iter().any(|&v| !f32::from_bits(v).is_finite()) {
        return Err("Nonfinite interpolated vertex".into());
    }
    Ok(result)
}
#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct Frame {
    pub positions: Vec<u32>,
    pub normals: Vec<u32>,
}
#[derive(Clone, Debug, Serialize, Deserialize, PartialEq, Eq)]
pub struct Cache {
    pub positions: Vec<[u32; 3]>,
    pub normals: Vec<[u32; 3]>,
    pub last_count: i32,
    /// Instance +60 and +61, both set even when no vertices are written.
    pub flags: [u8; 2],
}
#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct Input {
    pub vertex_count: u32,
    pub requested_count: i32,
    pub first: Frame,
    /// Some supplies an animation sequence's next frame; None uses direct decode.
    pub next: Option<Frame>,
    pub fraction: u32,
    pub cache: Cache,
    pub wedges: Vec<u16>,
    /// Exactly 1 copies positions only; every other value scatters wedge XYZ/normal.
    pub mode: u32,
    pub stride: u32,
    pub bytes: Vec<u8>,
}
#[derive(Clone, Debug, Serialize, PartialEq, Eq)]
pub struct Output {
    pub count: i32,
    pub interpolated: bool,
    pub cache: Cache,
    pub bytes: Vec<u8>,
}
impl Input {
    pub fn build(&self) -> Result<Output, String> {
        if self.vertex_count > 65536
            || self.requested_count < 0
            || self.requested_count > 65536
            || self.wedges.len() > 65536
            || self.bytes.len() > 16 * 1024 * 1024
            || self.cache.positions.len() != self.vertex_count as usize
            || self.cache.normals.len() != self.vertex_count as usize
        {
            return Err("Invalid bounded GetFrame buffers/count".into());
        }
        let count = self.requested_count.min(self.vertex_count as i32);
        let n = count as usize;
        let fraction = f32::from_bits(self.fraction);
        if !fraction.is_finite() {
            return Err("Nonfinite frame fraction".into());
        }
        let interpolated = self.next.is_some() && fraction > 0.0;
        if self.first.positions.len() < n
            || self.first.normals.len() < n
            || (interpolated
                && self
                    .next
                    .as_ref()
                    .is_some_and(|f| f.positions.len() < n || f.normals.len() < n))
        {
            return Err("Truncated packed frame".into());
        }
        let records = if self.mode == 1 { n } else { self.wedges.len() };
        let width = if self.mode == 1 { 12 } else { 24 };
        let stride = self.stride as usize;
        if records != 0
            && (stride < width
                || stride > 4096
                || (records - 1) * stride + width > self.bytes.len())
        {
            return Err("Invalid strided GetFrame destination".into());
        }
        let mut cache = self.cache.clone();
        for i in 0..n {
            let p = position(self.first.positions[i]);
            let normal_a = normal(self.first.normals[i]);
            if let Some(next) = self.next.as_ref().filter(|_| interpolated) {
                cache.positions[i] = interpolate(p, position(next.positions[i]), fraction)?;
                cache.normals[i] = interpolate(normal_a, normal(next.normals[i]), fraction)?;
            } else {
                cache.positions[i] = p;
                cache.normals[i] = normal_a;
            }
        }
        cache.last_count = count;
        cache.flags = [1, 1];
        let mut bytes = self.bytes.clone();
        for i in 0..records {
            let vertex = if self.mode == 1 {
                i
            } else {
                usize::from(self.wedges[i])
            };
            if vertex >= n {
                continue;
            }
            let words = cache.positions[vertex].into_iter().chain(
                cache.normals[vertex]
                    .into_iter()
                    .take(if self.mode == 1 { 0 } else { 3 }),
            );
            for (c, v) in words.enumerate() {
                bytes[i * stride + c * 4..i * stride + c * 4 + 4].copy_from_slice(&v.to_le_bytes());
            }
        }
        Ok(Output {
            count,
            interpolated,
            cache,
            bytes,
        })
    }
}
#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct Request {
    pub frame: Input,
    pub geometry: vertmesh_indices::Request,
}
#[derive(Clone, Debug, Serialize)]
pub struct Plan {
    pub frame: Output,
    pub draw: vertmesh_indices::Plan,
}
impl Runtime {
    /// Full vertex-mesh output (mode 0, stride 32) replaces external upload bytes.
    pub fn submit_frame(&mut self, request: &mut Request) -> Result<Plan, String> {
        if request.frame.mode != 0
            || request.frame.stride != 32
            || request.frame.requested_count != request.geometry.geometry.lod
            || request.frame.wedges != request.geometry.geometry.vertex_thresholds
        {
            return Err("GetFrame/index snapshot or vertex-mesh layout mismatch".into());
        }
        let frame = request.frame.build()?;
        let mut q = request.clone();
        q.frame.cache = frame.cache.clone();
        q.frame.bytes = frame.bytes.clone();
        let source = &mut q.geometry.mesh.vertex.source;
        source.stride = 32;
        source.size_before = frame.bytes.len() as u32;
        source.size_again = source.size_before;
        source.payload = frame.bytes.clone();
        let draw = self.submit_geometry(&mut q.geometry)?;
        *request = q;
        Ok(Plan { frame, draw })
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    fn input() -> Input {
        Input {
            vertex_count: 2,
            requested_count: 1,
            first: Frame {
                positions: vec![0xffffffff, 0],
                normals: vec![0, 0],
            },
            next: None,
            fraction: 0,
            cache: Cache {
                positions: vec![[7; 3]; 2],
                normals: vec![[8; 3]; 2],
                last_count: 9,
                flags: [2, 3],
            },
            wedges: vec![0, 1, 0],
            mode: 0,
            stride: 32,
            bytes: vec![0xa5; 96],
        }
    }
    #[test]
    fn signed_position_bit_boundaries() {
        assert_eq!(position(0xffffffff), [(-1f32).to_bits(); 3]);
        assert_eq!(
            position(0x80000400),
            [(-1024f32).to_bits(), 0, (-512f32).to_bits()]
        );
    }
    #[test]
    fn position_axes_are_independent() {
        assert_eq!(
            position(1 | (2 << 11) | (3 << 22)),
            [1f32.to_bits(), 2f32.to_bits(), 3f32.to_bits()]
        );
    }
    #[test]
    fn normal_components_are_centered_unsigned_ten_bits() {
        assert_eq!(normal(0), [(-512f32).to_bits(); 3]);
        assert_eq!(normal(0xffffffff), [511f32.to_bits(); 3]);
    }
    #[test]
    fn normal_top_two_bits_are_ignored() {
        assert_eq!(normal(0x40000000), normal(0));
    }
    #[test]
    fn cache_tail_is_not_replaced() {
        let p = input().build().unwrap();
        assert_eq!(p.cache.positions[1], [7; 3]);
        assert_eq!(p.cache.normals[1], [8; 3]);
        assert_eq!(p.cache.last_count, 1);
        assert_eq!(p.cache.flags, [1, 1]);
    }
    #[test]
    fn invalid_wedges_and_uv_padding_keep_original_bytes() {
        let p = input().build().unwrap();
        assert_eq!(&p.bytes[32..64], &[0xa5; 32]);
        assert_eq!(&p.bytes[24..32], &[0xa5; 8]);
        assert_eq!(&p.bytes[64..88], &p.bytes[..24]);
    }
    #[test]
    fn position_only_mode_ignores_wedge_mapping_and_normals_output() {
        let mut q = input();
        q.mode = 1;
        q.wedges = vec![65535];
        let p = q.build().unwrap();
        assert_eq!(&p.bytes[12..], &q.bytes[12..]);
        assert_ne!(&p.bytes[..12], &q.bytes[..12]);
    }
    #[test]
    fn requested_count_has_signed_upper_clamp() {
        let mut q = input();
        q.requested_count = 3;
        assert_eq!(q.build().unwrap().count, 2);
    }
    #[test]
    fn zero_count_still_marks_flags() {
        let mut q = input();
        q.requested_count = 0;
        let p = q.build().unwrap();
        assert_eq!(p.cache.flags, [1, 1]);
        assert_eq!(p.bytes, q.bytes);
    }
    #[test]
    fn linear_interpolation_does_not_normalize_normals() {
        let mut q = input();
        q.next = Some(Frame {
            positions: vec![1 | (1 << 11) | (1 << 22)],
            normals: vec![0x3fffffff],
        });
        q.fraction = 0.5f32.to_bits();
        let p = q.build().unwrap();
        assert_eq!(p.cache.positions[0], [0; 3]);
        assert_eq!(p.cache.normals[0], [(-0.5f32).to_bits(); 3]);
    }
    #[test]
    fn nonpositive_fraction_does_not_read_next_frame() {
        let mut q = input();
        q.next = Some(Frame {
            positions: vec![],
            normals: vec![],
        });
        q.fraction = (-1f32).to_bits();
        assert!(!q.build().unwrap().interpolated);
    }
    #[test]
    fn short_frame_or_destination_is_rejected() {
        let mut q = input();
        q.first.normals.clear();
        assert!(q.build().is_err());
        q = input();
        q.bytes.truncate(70);
        assert!(q.build().is_err());
    }
    #[test]
    fn negative_count_or_nonfinite_fraction_is_rejected() {
        let mut q = input();
        q.requested_count = -1;
        assert!(q.build().is_err());
        q = input();
        q.fraction = f32::NAN.to_bits();
        assert!(q.build().is_err());
    }
    #[test]
    fn interpolation_overflow_is_rejected() {
        let mut q = input();
        q.first.positions[0] = 0;
        q.next = Some(Frame {
            positions: vec![1023],
            normals: vec![0],
        });
        q.fraction = f32::MAX.to_bits();
        assert!(q.build().is_err());
    }
}
