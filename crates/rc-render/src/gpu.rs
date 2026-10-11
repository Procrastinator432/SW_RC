//! Versioned, little-endian graphics transfer. No driver objects or game state.
use crate::{Scene, View};
use rc_package::geometry::Triangle;

pub const VERTEX_FLOATS: usize = 9; // position, UV, RGBA
fn word(out: &mut Vec<u8>, value: u32) {
    out.extend_from_slice(&value.to_le_bytes());
}
fn floats(out: &mut Vec<u8>, values: &[f32]) {
    for value in values {
        out.extend_from_slice(&value.to_le_bytes());
    }
}
pub fn basis(scene: &Scene, view: View) -> [[f32; 3]; 4] {
    let (sy, cy) = view.yaw.sin_cos();
    let (sp, cp) = view.pitch.clamp(-1.5, 1.5).sin_cos();
    let direction = [cy * cp, sy * cp, sp];
    let eye = std::array::from_fn(|i| {
        scene.center[i]
            + view
                .position
                .map_or(direction[i] * 2.8 / view.zoom.clamp(0.5, 32.), |p| p[i])
                * scene.radius
    });
    [
        eye,
        [-sy, cy, 0.],
        [-cy * sp, -sy * sp, cp],
        direction.map(|v| -v),
    ]
}
pub fn vertices(
    out: &mut Vec<u8>,
    triangle: &Triangle,
    uv: Option<[[f32; 2]; 3]>,
    transient: bool,
) {
    let a: [f32; 3] = std::array::from_fn(|i| triangle.points[1][i] - triangle.points[0][i]);
    let b: [f32; 3] = std::array::from_fn(|i| triangle.points[2][i] - triangle.points[0][i]);
    let n = [
        a[1] * b[2] - a[2] * b[1],
        a[2] * b[0] - a[0] * b[2],
        a[0] * b[1] - a[1] * b[0],
    ];
    let length = n.iter().map(|v| v * v).sum::<f32>().sqrt().max(0.001);
    let lighting =
        ((n[0] * 0.3 + n[1] * 0.4 + n[2] * 0.85).abs() / length).clamp(0., 1.) * 0.6 + 0.4;
    let seed = (triangle.surface % 100) as u32;
    let rgb = if transient {
        [
            (triangle.surface as u32 >> 16) & 255,
            (triangle.surface as u32 >> 8) & 255,
            triangle.surface as u32 & 255,
        ]
    } else {
        [
            90 + seed * 37 % 100,
            130 + seed * 19 % 100,
            140 + seed * 11 % 100,
        ]
    };
    let color = if uv.is_some() {
        [lighting; 3]
    } else {
        rgb.map(|v| ((v as f32 * lighting) as u32) as f32 / 255.)
    };
    for i in 0..3 {
        floats(out, &triangle.points[i]);
        floats(out, &uv.map_or([0.; 2], |p| p[i]));
        floats(out, &[color[0], color[1], color[2], 1.]);
    }
}
/// RCGPU001, texture count, (width,height,RGBA8)*, batch count,
/// (texture index or -1, vertex count, interleaved float vertices)*.
pub fn world(scene: &Scene) -> Result<Vec<u8>, String> {
    // Validate the same public scene invariants as its persisted representation.
    scene.snapshot()?;
    let mut out = b"RCGPU001".to_vec();
    word(&mut out, scene.textures.len() as u32);
    for texture in &scene.textures {
        word(&mut out, texture.width as u32);
        word(&mut out, texture.height as u32);
        for &p in &texture.pixels {
            out.extend_from_slice(&[(p >> 16) as u8, (p >> 8) as u8, p as u8, (p >> 24) as u8]);
        }
    }
    let mut batches = vec![Vec::new(); scene.textures.len() + 1];
    for (i, triangle) in scene.triangles.iter().enumerate() {
        let face = scene.appearance.get(i).copied().flatten();
        let index = face.map_or(0, |f| f.texture + 1);
        vertices(&mut batches[index], triangle, face.map(|f| f.uv), false);
    }
    word(
        &mut out,
        batches.iter().filter(|b| !b.is_empty()).count() as u32,
    );
    for (i, batch) in batches
        .into_iter()
        .enumerate()
        .filter(|(_, b)| !b.is_empty())
    {
        word(&mut out, (i as i32 - 1) as u32);
        word(&mut out, (batch.len() / (VERTEX_FLOATS * 4)) as u32);
        out.extend_from_slice(&batch);
    }
    Ok(out)
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn transfer_preserves_uv_alpha_and_camera_handedness() {
        let mut scene = Scene::new(vec![Triangle {
            points: [[0., 0., 0.], [0., 1., 0.], [0., 0., 1.]],
            surface: 0,
        }])
        .unwrap();
        scene.textures.push(crate::Texture {
            width: 1,
            height: 1,
            pixels: vec![0x7f123456],
        });
        scene.appearance = vec![Some(crate::FaceTexture {
            uv: [[0., 0.], [2., 0.], [0., -1.]],
            texture: 0,
        })];
        let data = world(&scene).unwrap();
        assert_eq!(&data[20..24], &[0x12, 0x34, 0x56, 0x7f]);
        assert_eq!(u32::from_le_bytes(data[32..36].try_into().unwrap()), 3);
        let uv = f32::from_le_bytes(data[36 + 36 + 12..36 + 36 + 16].try_into().unwrap());
        assert_eq!(uv, 2.);
        let camera = basis(
            &scene,
            View {
                yaw: std::f32::consts::PI,
                pitch: 0.,
                zoom: 1.,
                position: Some([0.; 3]),
            },
        );
        assert!(camera[3][0] > 0.999);
        assert!(camera[1][1] < -0.999);
        assert_eq!(camera[2], [0., 0., 1.]);
    }
}
