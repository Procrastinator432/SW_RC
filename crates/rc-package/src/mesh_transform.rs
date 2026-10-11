//! Core rotator matrix and Engine ULodMeshInstance::MeshToWorld (1044e760).
//! Stable actor snapshots; default nearest IEEE arithmetic, finite values only.
use crate::{
    d3d_dynamic_draw::Runtime, material_panner::sin_tab, shader_constants::Matrix, vertmesh_render,
};
use serde::{Deserialize, Serialize};

/// Core 10115130 tests low 16 bits, independently of sine-table quantization.
pub fn rotation_is_zero(rotation: [i32; 3]) -> bool {
    rotation.iter().all(|v| v & 65535 == 0)
}
/// Core 101444a0. Translation words are copied, including signed zero.
pub fn rotation_matrix(rotation: [i32; 3], translation: [u32; 3]) -> Result<Matrix, String> {
    finite(&translation)?;
    let [p, y, r] = rotation;
    let (sp, sy, sr) = (sin_tab(p), sin_tab(y), sin_tab(r));
    let (cp, cy, cr) = (
        sin_tab(p.wrapping_add(16384)),
        sin_tab(y.wrapping_add(16384)),
        sin_tab(r.wrapping_add(16384)),
    );
    Ok([
        [cy * cp, cp * sy, sp, 0.0].map(f32::to_bits),
        [
            ((cy * sp) * sr) - cr * sy,
            ((sy * sp) * sr) + cy * cr,
            0.0 - cp * sr,
            0.0,
        ]
        .map(f32::to_bits),
        [
            0.0 - ((cy * cr) * sp + sy * sr),
            cy * sr - (cr * sy) * sp,
            cp * cr,
            0.0,
        ]
        .map(f32::to_bits),
        [
            translation[0],
            translation[1],
            translation[2],
            1.0f32.to_bits(),
        ],
    ])
}
fn finite(words: &[u32]) -> Result<(), String> {
    if words.iter().any(|w| !f32::from_bits(*w).is_finite()) {
        Err("Nonfinite MeshToWorld value".into())
    } else {
        Ok(())
    }
}
fn checked(value: f32) -> Result<f32, String> {
    if value.is_finite() {
        Ok(value)
    } else {
        Err("MeshToWorld arithmetic overflow".into())
    }
}
// Scalar MULSS/ADDSS orders differ in the mesh and actor products. Do not use a
// conventional four-term dot product or fold zeros away (signed zero matters).
const MESH_ORDER: [[[usize; 4]; 4]; 4] = [
    [[0, 1, 2, 3]; 4],
    [[1, 0, 2, 3]; 4],
    [[2, 0, 1, 3]; 4],
    [[0, 1, 2, 3], [0, 3, 2, 1], [2, 0, 1, 3], [1, 2, 0, 3]],
];
const ACTOR_ORDER: [[[usize; 4]; 4]; 4] = [
    [[0, 1, 3, 2], [3, 1, 0, 2], [3, 2, 0, 1], [0, 2, 3, 1]],
    [[1, 0, 2, 3], [0, 1, 3, 2], [0, 3, 2, 1], [2, 0, 1, 3]],
    [[1, 0, 2, 3], [0, 1, 3, 2], [0, 3, 2, 1], [2, 0, 1, 3]],
    [[1, 0, 2, 3], [0, 1, 3, 2], [0, 3, 2, 1], [2, 0, 1, 3]],
];
fn multiply(a: Matrix, b: Matrix, order: &[[[usize; 4]; 4]; 4]) -> Result<Matrix, String> {
    let a = a.map(|r| r.map(f32::from_bits));
    let b = b.map(|r| r.map(f32::from_bits));
    let mut out = [[0; 4]; 4];
    for r in 0..4 {
        for c in 0..4 {
            let k = order[r][c];
            let mut v = checked(a[r][k[0]] * b[k[0]][c])?;
            for i in &k[1..] {
                v = checked(v + checked(a[r][*i] * b[*i][c])?)?;
            }
            out[r][c] = v.to_bits();
        }
    }
    Ok(out)
}
#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct Input {
    pub actor_location: [u32; 3],
    pub actor_rotation: [i32; 3],
    pub actor_draw_scale: u32,
    pub actor_draw_scale_3d: [u32; 3],
    pub actor_prepivot: [u32; 3],
    pub actor_flags: u32,
    pub actor_height: u32,
    pub mesh_scale: [u32; 3],
    pub mesh_origin: [u32; 3],
    pub mesh_rotation: [i32; 3],
    /// Full DWORD at instance+74, and byte at +78, respectively.
    pub suppress_origin: u32,
    pub bypass_mesh_adjustments: u8,
}
#[derive(Clone, Debug, Serialize)]
pub struct Constructor {
    pub rotation: [i32; 3],
    pub translation: [u32; 3],
    pub matrix: Matrix,
}
#[derive(Clone, Debug, Serialize)]
pub struct Output {
    pub effective_scale: [u32; 3],
    pub base_matrix: Matrix,
    pub adjusted_location: [u32; 3],
    pub constructors: Vec<Constructor>,
    pub matrix: Matrix,
}
impl Input {
    pub fn build(&self) -> Result<Output, String> {
        finite(&self.actor_location)?;
        finite(&[self.actor_draw_scale])?;
        finite(&self.actor_draw_scale_3d)?;
        finite(&self.mesh_scale)?;
        let mut effective_scale = [0; 3];
        let mut base = [[0; 4]; 4];
        for i in 0..3 {
            let scaled = checked(
                f32::from_bits(self.actor_draw_scale_3d[i]) * f32::from_bits(self.actor_draw_scale),
            )?;
            effective_scale[i] = checked(f32::from_bits(self.mesh_scale[i]) * scaled)?.to_bits();
            base[i][i] = effective_scale[i];
        }
        base[3][3] = 1.0f32.to_bits();
        let mut location = self.actor_location;
        let mut constructors = vec![];
        let mut construct = |rotation, translation| -> Result<Matrix, String> {
            let matrix = rotation_matrix(rotation, translation)?;
            constructors.push(Constructor {
                rotation,
                translation,
                matrix,
            });
            Ok(matrix)
        };
        if self.bypass_mesh_adjustments == 0 {
            if self.suppress_origin == 0 {
                finite(&self.mesh_origin)?;
                for (word, origin) in base[3][..3].iter_mut().zip(self.mesh_origin) {
                    *word = (0.0 - f32::from_bits(origin)).to_bits();
                }
            }
            if !rotation_is_zero(self.mesh_rotation) {
                base = multiply(base, construct(self.mesh_rotation, [0; 3])?, &MESH_ORDER)?;
            }
            if self.actor_flags & 0x800 == 0 {
                finite(&self.actor_prepivot)?;
                for (word, pivot) in location.iter_mut().zip(self.actor_prepivot) {
                    *word = checked(f32::from_bits(pivot) + f32::from_bits(*word))?.to_bits();
                }
            } else {
                finite(&[self.actor_height])?;
                let axis = construct(self.actor_rotation, [0; 3])?[2];
                // FLD f32 / FADD stored double / FSTP f32 at 1044ed25..43.
                let height = checked(
                    (f64::from(f32::from_bits(self.actor_height)) + 5.850000090897083_f64) as f32,
                )?;
                for i in 0..3 {
                    location[i] = checked(
                        f32::from_bits(location[i]) - checked(f32::from_bits(axis[i]) * height)?,
                    )?
                    .to_bits();
                }
            }
        }
        let matrix = multiply(
            base,
            construct(self.actor_rotation, location)?,
            &ACTOR_ORDER,
        )?;
        Ok(Output {
            effective_scale,
            base_matrix: base,
            adjusted_location: location,
            constructors,
            matrix,
        })
    }
}
#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct Request {
    pub transform: Input,
    pub render: vertmesh_render::Request,
}
/// Original renderer virtual +48 arguments: transform slot zero and 16 copied words.
#[derive(Clone, Debug, Serialize)]
pub struct Handoff {
    pub receiver: u32,
    pub vtable_offset: u32,
    pub slot: u32,
    pub matrix: Matrix,
}
#[derive(Clone, Debug, Serialize)]
pub struct Plan {
    pub transform: Output,
    pub handoff: Handoff,
    pub render: vertmesh_render::Plan,
}
impl Runtime {
    /// The renderer transform is a recorded boundary. No live renderer is called;
    /// the entire request/runtime update rolls back on any downstream error.
    pub fn submit_transformed_vertmesh(&mut self, request: &mut Request) -> Result<Plan, String> {
        let transform = request.transform.build()?;
        let mut q = request.clone();
        let receiver = q.render.geometry.mesh.materials.renderer_address;
        if receiver == 0 || q.render.vertex.instance == 0 {
            return Err("Null transform renderer or mesh instance".into());
        }
        q.render.actor_position = q.transform.actor_location;
        q.render.actor_draw_scale = q.transform.actor_draw_scale_3d;
        q.render.camera.actor_draw_scale = q.transform.actor_draw_scale;
        let render = self.submit_vertmesh_render(&mut q.render)?;
        let handoff = Handoff {
            receiver,
            vtable_offset: 0x48,
            slot: 0,
            matrix: transform.matrix,
        };
        *request = q;
        Ok(Plan {
            transform,
            handoff,
            render,
        })
    }
}
