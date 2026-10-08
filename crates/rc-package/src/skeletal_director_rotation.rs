//! ApplyDirector 105020b3..105022f2: prepared rotation, limits and mutable history.
//! Does not apply rows, actor scale, relative composition or ancestor correction.
use crate::{
    skeletal_director::BoneDirector,
    skeletal_root_pose::{quaternion_translation_matrix, RootTransform},
};
use serde::{Deserialize, Serialize};

pub trait DirectorRotationHost {
    fn matrix_quaternion(&mut self, matrix: [u32; 16]) -> Result<[u32; 4], String>;
    fn angle_difference(&mut self, a: [u32; 4], b: [u32; 4]) -> Result<f32, String>;
    fn slerp(&mut self, a: [u32; 4], b: [u32; 4], alpha: f32) -> Result<[u32; 4], String>;
    fn rotation_angle_fast(&mut self, q: [u32; 4]) -> Result<f32, String>;
    fn power(&mut self, q: [u32; 4], exponent: f32) -> Result<[u32; 4], String>;
}
#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct PreparedDirectorRotation {
    pub quaternion: [u32; 4],
    pub matrix: [u32; 16],
    pub scale: [u32; 3],
    pub temporal_limited: bool,
    pub absolute_limited: bool,
}
/// Caller supplies the already-local director matrix and actor scale snapshot.
/// Native history initialization is committed before later host calls may fail.
pub fn prepare_director_rotation(
    director: &mut BoneDirector,
    current: [u32; 16],
    supplied: [u32; 16],
    actor_scale: [u32; 3],
    host: &mut impl DirectorRotationHost,
) -> Result<PreparedDirectorRotation, String> {
    if director.words[18] & 0xff00 == 0 {
        return Err("rotation preparation requires enabled director rotation".into());
    }
    if director.words[27] & 0xff == 0 {
        let initial = host.matrix_quaternion(current)?;
        director.words[22..26].copy_from_slice(&initial);
        director.words[27] = (director.words[27] & 0xffffff00) | 1;
    }
    let mut q = host.matrix_quaternion(supplied)?;
    let mut output = PreparedDirectorRotation {
        quaternion: q,
        matrix: supplied,
        scale: actor_scale,
        temporal_limited: false,
        absolute_limited: false,
    };
    let rebuild = |out: &mut PreparedDirectorRotation, q| {
        out.matrix = quaternion_translation_matrix(RootTransform {
            rotation: q,
            position: supplied[12..15].try_into().unwrap(),
        });
        out.scale = [1f32.to_bits(); 3];
    };
    if f32::from_bits(director.words[21]) >= 0.0 {
        let history = director.words[22..26].try_into().unwrap();
        let distance = host.angle_difference(history, q)?;
        let budget = f32::from_bits(director.words[26]);
        if distance > budget {
            q = host.slerp(history, q, budget / distance)?;
            rebuild(&mut output, q);
            output.temporal_limited = true;
        }
        director.words[26] = 0;
    }
    let maximum = f32::from_bits(director.words[20]);
    if maximum >= 0.0 {
        let angle = host.rotation_angle_fast(q)?;
        if angle > maximum {
            q = host.power(q, maximum / angle)?;
            rebuild(&mut output, q);
            output.absolute_limited = true;
        }
    }
    director.words[22..26].copy_from_slice(&q);
    output.quaternion = q;
    Ok(output)
}

/// f64 transcendental/x87 approximation; native scalar SSE stages remain f32.
pub struct PortableDirectorRotationHost<M> {
    pub math: M,
}
impl<M: crate::quaternion_animation::QuaternionMath> DirectorRotationHost
    for PortableDirectorRotationHost<M>
{
    fn matrix_quaternion(&mut self, m: [u32; 16]) -> Result<[u32; 4], String> {
        crate::quaternion_matrix::quaternion_from_matrix(m, &mut self.math)
    }
    fn angle_difference(&mut self, a: [u32; 4], b: [u32; 4]) -> Result<f32, String> {
        let a = a.map(|v| f32::from_bits(v) as f64);
        let b = b.map(|v| f32::from_bits(v) as f64);
        let dot = (((a[3] * b[3]) + a[2] * b[2]) + a[1] * b[1]) + a[0] * b[0];
        let absolute = dot.abs();
        // Original x87 upper clamp also handles unordered as 1.
        let clamped = if absolute <= 1.0 { absolute } else { 1.0 };
        Ok((clamped.acos() * 2.0) as f32)
    }
    fn slerp(&mut self, a: [u32; 4], b: [u32; 4], alpha: f32) -> Result<[u32; 4], String> {
        crate::quaternion_animation::slerp_rotation(a, b, alpha, &mut self.math)
    }
    fn rotation_angle_fast(&mut self, q: [u32; 4]) -> Result<f32, String> {
        let w = f32::from_bits(q[3]);
        let absolute = if w >= 0.0 { w } else { 0.0f32 - w };
        let clamped = if absolute <= 1.0 { absolute } else { 1.0 };
        Ok(((clamped as f64).acos() * 2.0) as f32)
    }
    fn power(&mut self, q: [u32; 4], exponent: f32) -> Result<[u32; 4], String> {
        let v = q.map(f32::from_bits);
        let n = (v[0] * v[0] + v[1] * v[1]) + v[2] * v[2];
        let seed = self.math.reciprocal_sqrt_seed(n)?;
        let root = ((3.0f32 - (seed * n) * seed) * (seed * 0.5f32)) * n;
        let root = if n == 0.0 { 0.0f32 } else { root };
        let clamped = if root <= 1.0 { root } else { 1.0 };
        // Unlike the caller's Fast-angle stores, RotationAngle stays in x87 ST0
        // throughout operator^. Approximate that extended value with f64 here.
        let angle = (clamped as f64).asin() * 2.0;
        if angle.partial_cmp(&0.0) != Some(std::cmp::Ordering::Greater) {
            return Ok(q);
        }
        let half = angle * 0.5;
        let scaled = exponent as f64 * angle * 0.5;
        let factor = scaled.sin() / half.sin();
        Ok([
            (factor * v[0] as f64) as f32,
            (factor * v[1] as f64) as f32,
            (factor * v[2] as f64) as f32,
            scaled.cos() as f32,
        ]
        .map(f32::to_bits))
    }
}

#[cfg(test)]
#[path = "skeletal_director_rotation_tests.rs"]
mod tests;
