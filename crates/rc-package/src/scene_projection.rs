//! Engine FSceneNode::Project (104a3860) and Deproject (104a3990).
//! Supplied row-major matrix snapshots; default IEEE/SSE rounding, no FMA.
use crate::{d3d_dynamic_draw::Runtime, shader_constants::Matrix, vertmesh_lod};
use serde::{Deserialize, Serialize};

fn finite(words: impl IntoIterator<Item = u32>) -> Result<(), String> {
    if words.into_iter().any(|v| !f32::from_bits(v).is_finite()) {
        Err("Nonfinite scene projection value".into())
    } else {
        Ok(())
    }
}

/// Scene +114..+150. XYZ are divided by W; returned W is the original clip W.
pub fn project(matrix: Matrix, position: [u32; 3]) -> Result<[u32; 4], String> {
    finite(matrix.into_iter().flatten().chain(position))?;
    let m = matrix.map(|r| r.map(f32::from_bits));
    let [x, y, z] = position.map(f32::from_bits);
    let clip: [f32; 4] =
        std::array::from_fn(|c| ((m[1][c] * y + m[2][c] * z) + m[0][c] * x) + m[3][c]);
    finite(clip.map(f32::to_bits))?;
    if clip[3] == 0.0 {
        return Err("Zero scene projection W".into());
    }
    let reciprocal = 1.0f32 / clip[3];
    finite([reciprocal.to_bits()])?;
    let result = [
        clip[0] * reciprocal,
        clip[1] * reciprocal,
        clip[2] * reciprocal,
        clip[3],
    ];
    finite(result.map(f32::to_bits))?;
    Ok(result.map(f32::to_bits))
}

/// Scene +154..+18c. Uses stored W, with distinct X vs Y/Z addition orders.
/// The caller supplies the inverse snapshot; no matrix inversion is implied.
pub fn deproject(matrix: Matrix, plane: [u32; 4]) -> Result<[u32; 3], String> {
    finite(matrix.into_iter().flatten().chain(plane))?;
    let m = matrix.map(|r| r.map(f32::from_bits));
    let [px, py, pz, w] = plane.map(f32::from_bits);
    let [x, y, z] = [px * w, py * w, pz * w];
    finite([x.to_bits(), y.to_bits(), z.to_bits()])?;
    let a = m[2][0] * z + m[3][0] * w;
    let b = m[2][1] * z + m[3][1] * w;
    let c = m[2][2] * z + m[3][2] * w;
    let result = [
        (a + m[1][0] * y) + m[0][0] * x,
        (b + m[0][1] * x) + m[1][1] * y,
        (c + m[0][2] * x) + m[1][2] * y,
    ];
    finite(result.map(f32::to_bits))?;
    Ok(result.map(f32::to_bits))
}

#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct Request {
    pub world_to_screen: Matrix,
    pub actor_position: [u32; 3],
    pub mesh: vertmesh_lod::Request,
}
#[derive(Clone, Debug, Serialize)]
pub struct Plan {
    pub projection: [u32; 4],
    pub mesh: vertmesh_lod::Plan,
}
impl Runtime {
    /// Derives W before LOD. Commits the derived camera snapshot only on success.
    pub fn submit_projected_mesh(&mut self, request: &mut Request) -> Result<Plan, String> {
        let projection = project(request.world_to_screen, request.actor_position)?;
        let mut q = request.clone();
        q.mesh.camera.projected_w = projection[3];
        // submit_lod itself is transactional, including later upload/material errors.
        let mesh = self.submit_lod(&mut q.mesh)?;
        *request = q;
        Ok(Plan { projection, mesh })
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    fn identity() -> Matrix {
        std::array::from_fn(|r| std::array::from_fn(|c| if r == c { 1f32.to_bits() } else { 0 }))
    }
    #[test]
    fn identity_roundtrip() {
        let p = [2f32.to_bits(), (-3f32).to_bits(), 4f32.to_bits()];
        assert_eq!(
            deproject(identity(), project(identity(), p).unwrap()).unwrap(),
            p
        );
    }
    #[test]
    fn perspective_roundtrip_uses_preserved_clip_w() {
        // Swap homogeneous Z/W; this matrix is its own inverse.
        let mut m = identity();
        m[2][2] = 0;
        m[3][3] = 0;
        m[2][3] = 1f32.to_bits();
        m[3][2] = 1f32.to_bits();
        let p = [1.5f32.to_bits(), 2f32.to_bits(), (-4f32).to_bits()];
        let projected = project(m, p).unwrap();
        assert_eq!(projected[3], p[2]);
        assert_eq!(deproject(m, projected).unwrap(), p);
    }
    #[test]
    fn reciprocal_is_multiplied_before_output() {
        let mut m = identity();
        m[3][3] = 3f32.to_bits();
        let p = project(m, [7f32.to_bits(), 0, 0]).unwrap();
        assert_eq!(p[0], (7f32 * (1f32 / 3f32)).to_bits());
        assert_eq!(p[3], 3f32.to_bits());
    }
    #[test]
    fn forward_order_is_y_z_x_translation() {
        let mut m = identity();
        m[1][0] = 1e20f32.to_bits();
        m[2][0] = (-1e20f32).to_bits();
        assert_eq!(project(m, [1f32.to_bits(); 3]).unwrap()[0], 1f32.to_bits());
    }
    #[test]
    fn inverse_x_order_differs_from_y_z() {
        let mut m = [[0; 4]; 4];
        m[2][..3].fill(1e20f32.to_bits());
        m[0][..3].fill(1f32.to_bits());
        m[1][..3].fill((-1e20f32).to_bits());
        assert_eq!(
            deproject(m, [1f32.to_bits(); 4]).unwrap(),
            [1f32.to_bits(), 0, 0]
        );
    }
    #[test]
    fn negative_w_is_retained() {
        let mut m = identity();
        m[3][3] = (-2f32).to_bits();
        assert_eq!(
            project(m, [2f32.to_bits(); 3]).unwrap(),
            [
                (-1f32).to_bits(),
                (-1f32).to_bits(),
                (-1f32).to_bits(),
                (-2f32).to_bits()
            ]
        );
    }
    #[test]
    fn inverse_translation_is_scaled_by_w() {
        let mut m = identity();
        m[3][0] = 3f32.to_bits();
        assert_eq!(
            deproject(m, [1f32.to_bits(), 0, 0, 2f32.to_bits()]).unwrap()[0],
            8f32.to_bits()
        );
    }
    #[test]
    fn signed_zero_w_is_rejected() {
        for w in [0, 0x80000000] {
            let mut m = identity();
            m[3][3] = w;
            assert!(project(m, [0; 3]).is_err());
        }
    }
    #[test]
    fn nonfinite_input_is_rejected() {
        assert!(project(identity(), [0x7fc00000, 0, 0]).is_err());
        assert!(deproject(identity(), [0, 0, 0, 0x7f800000]).is_err());
    }
    #[test]
    fn intermediate_overflow_is_rejected() {
        let mut m = identity();
        m[0][0] = f32::MAX.to_bits();
        assert!(project(m, [2f32.to_bits(), 0, 0]).is_err());
        assert!(deproject(identity(), [f32::MAX.to_bits(), 0, 0, 2f32.to_bits()]).is_err());
    }
    #[test]
    fn reciprocal_overflow_is_rejected() {
        let mut m = identity();
        m[3][3] = 1;
        assert!(project(m, [0; 3]).is_err());
    }
    #[test]
    fn inverse_zero_w_is_allowed() {
        assert_eq!(
            deproject(
                identity(),
                [1f32.to_bits(), 2f32.to_bits(), 3f32.to_bits(), 0]
            )
            .unwrap(),
            [0; 3]
        );
    }
}
