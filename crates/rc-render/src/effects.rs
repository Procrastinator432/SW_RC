//! Explicit port render policy for transparent world effects. These are supplied
//! triangles, not an implementation of the original particle spawn simulation.
use crate::FaceTexture;
use rc_package::geometry::Triangle;

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
#[repr(u32)]
pub enum Blend {
    Alpha = 1,
    SourceAlphaAdditive = 2,
}

pub struct TriangleEffect {
    pub triangle: Triangle,
    pub face: FaceTexture,
    /// Unlit RGBA multiplier, independent of triangle orientation.
    pub color: [f32; 4],
    pub blend: Blend,
}

impl TriangleEffect {
    /// Same nine-float vertex layout as the existing world transfer.
    pub fn vertices(&self, out: &mut Vec<u8>) {
        for i in 0..3 {
            for value in self.triangle.points[i]
                .into_iter()
                .chain(self.face.uv[i])
                .chain(self.color)
            {
                out.extend_from_slice(&value.to_le_bytes());
            }
        }
    }
}

pub fn composite(destination: u32, source: [f32; 4], blend: Blend) -> u32 {
    let alpha = source[3].clamp(0., 1.);
    let mut result = 0xff000000;
    for (axis, shift) in [16, 8, 0].into_iter().enumerate() {
        let old = ((destination >> shift) & 255) as f32 / 255.;
        let gain = if blend == Blend::Alpha {
            1. - alpha
        } else {
            1.
        };
        let value = source[axis].clamp(0., 1.) * alpha + old * gain;
        result |= ((value.clamp(0., 1.) * 255.).round() as u32) << shift;
    }
    result
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::{Scene, Texture, View};
    fn triangle(x: f32) -> Triangle {
        Triangle {
            points: [[x, -10., -10.], [x, 10., -10.], [x, 0., 10.]],
            surface: 0,
        }
    }
    fn fixture() -> (Scene, View) {
        let mut scene = Scene::new(vec![triangle(10.)]).unwrap();
        scene.textures = [0xff0000ff, 0x40ff0000, 0xff00ff00]
            .map(|p| Texture {
                width: 1,
                height: 1,
                pixels: vec![p],
            })
            .to_vec();
        scene.appearance = vec![Some(face(0))];
        let view = View {
            yaw: std::f32::consts::PI,
            pitch: 0.,
            zoom: 1.,
            position: Some(scene.center.map(|v| -v / scene.radius)),
        };
        (scene, view)
    }
    fn face(texture: usize) -> FaceTexture {
        FaceTexture {
            texture,
            uv: [[0.; 2]; 3],
        }
    }
    fn effect(x: f32, blend: Blend) -> TriangleEffect {
        TriangleEffect {
            triangle: triangle(x),
            face: face(1),
            color: [1.; 4],
            blend,
        }
    }
    fn center(scene: &Scene, view: View, effects: &[TriangleEffect]) -> u32 {
        scene
            .render_animated_effects([64, 64], view, &[], &[], 0, effects)
            .unwrap()[32 * 64 + 32]
    }
    #[test]
    fn low_alpha_effects_are_unlit_and_occluded_by_world() {
        let (scene, view) = fixture();
        let base = center(&scene, view, &[]);
        assert_eq!(center(&scene, view, &[effect(20., Blend::Alpha)]), base);
        let shown = center(&scene, view, &[effect(5., Blend::Alpha)]);
        assert_eq!((shown >> 16) & 255, 64); // original alpha below old 128 cutoff
        assert_eq!(
            shown & 255,
            (((base & 255) as f32) * (1. - 64. / 255.)).round() as u32
        );
    }
    #[test]
    fn effects_do_not_occlude_later_effects_and_weapon_remains_separate() {
        let (scene, view) = fixture();
        let base = center(&scene, view, &[]);
        let effects = [
            effect(5., Blend::SourceAlphaAdditive),
            effect(6., Blend::SourceAlphaAdditive),
        ];
        assert_eq!(center(&scene, view, &effects), base | 0x800000);
        let weapon = scene
            .render_animated_effects(
                [64, 64],
                view,
                &[triangle(20.)],
                &[Some(face(2))],
                0,
                &effects,
            )
            .unwrap();
        assert_eq!(weapon[32 * 64 + 32] & 0xff00ff, 0);
        assert!(weapon[32 * 64 + 32] & 0xff00 > 0);
    }
    #[test]
    fn transparent_quad_shared_edge_is_covered_once_for_both_windings() {
        let (scene, view) = fixture();
        let points = [[5., -5., -5.], [5., 5., -5.], [5., 5., 5.], [5., -5., 5.]];
        let quad = |reverse: bool| {
            [[0, 1, 2], [0, 2, 3]].map(|mut ids| {
                if reverse {
                    ids.reverse();
                }
                let mut e = effect(5., Blend::SourceAlphaAdditive);
                e.triangle.points = ids.map(|i| points[i]);
                e
            })
        };
        let render = |effects: &[TriangleEffect]| {
            scene
                .render_animated_effects([64, 64], view, &[], &[], 0, effects)
                .unwrap()
        };
        let image = render(&quad(false));
        assert_eq!(image, render(&quad(true)));
        assert!(image.iter().any(|p| (p >> 16) & 255 == 64));
        let base = render(&[]);
        assert!(image
            .iter()
            .zip(base)
            .all(|(p, b)| ((p >> 16) & 255) - ((b >> 16) & 255) <= 64));
    }
    #[test]
    fn zero_alpha_near_clip_and_invalid_effects() {
        let (scene, view) = fixture();
        let base = center(&scene, view, &[]);
        let mut e = effect(5., Blend::Alpha);
        e.color[3] = 0.;
        assert_eq!(center(&scene, view, &[e]), base);
        assert_eq!(center(&scene, view, &[effect(2., Blend::Alpha)]), base);
        let mut e = effect(5., Blend::Alpha);
        e.triangle.points[0][0] = 2.;
        assert!(scene
            .render_animated_effects([64, 64], view, &[], &[], 0, &[e])
            .unwrap()
            .iter()
            .any(|p| (p >> 16) & 255 > 0));
        for field in 0..3 {
            let mut e = effect(5., Blend::Alpha);
            match field {
                0 => e.color[0] = f32::NAN,
                1 => e.face.texture = 3,
                _ => e.face.uv[0][0] = f32::INFINITY,
            }
            assert!(scene
                .render_animated_effects([64, 64], view, &[], &[], 0, &[e])
                .is_err());
        }
    }
    #[test]
    fn blend_clamps_and_preserves_destination_for_zero_alpha() {
        assert_eq!(
            composite(0xff123456, [1., 1., 1., 0.], Blend::Alpha),
            0xff123456
        );
        assert_eq!(
            composite(0xffcccccc, [1., 1., 1., 0.5], Blend::SourceAlphaAdditive),
            0xffffffff
        );
        assert_eq!(
            composite(0xff000000, [1., 0., 0., 0.5], Blend::Alpha),
            0xff800000
        );
    }
}
