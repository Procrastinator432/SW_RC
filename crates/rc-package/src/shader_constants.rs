//! Native matrix/camera upload and bounded persistent constant-bank dispatch.
use crate::material_constants::{
    cos_time, rotator, shader_time, sin_time, tan_time, xy_circle, Flicker,
};
use crate::shader_lights::{self, LightCache, Lighting};
use serde::Serialize;
#[path = "shader_matrix_orders.rs"]
mod orders;
#[path = "shader_scene.rs"]
pub mod scene;
pub type Matrix = [[u32; 4]; 4];
pub fn transpose(matrix: Matrix) -> Matrix {
    std::array::from_fn(|i| std::array::from_fn(|j| matrix[j][i]))
}
// Each original MULSS and ADDSS rounds separately. Orders differ between cases,
// including ObjectToCamera and the first ObjectToScreen multiplication.
fn multiply(a: Matrix, b: Matrix, order: &[[usize; 4]; 16]) -> Result<Matrix, String> {
    let a = a.map(|row| row.map(f32::from_bits));
    let b = b.map(|row| row.map(f32::from_bits));
    if a.iter().chain(&b).flatten().any(|v| !v.is_finite()) {
        return Err("Nonfinite matrix composition input excluded by host contract".into());
    }
    let mut result = [[0; 4]; 4];
    for row in 0..4 {
        for col in 0..4 {
            let terms = order[row * 4 + col].map(|k| a[row][k] * b[k][col]);
            let mut value = terms[0];
            if terms.iter().any(|v| !v.is_finite()) {
                return Err(
                    "Nonfinite matrix composition product excluded by host contract".into(),
                );
            }
            for term in &terms[1..] {
                value += term;
                if !value.is_finite() {
                    return Err("Nonfinite matrix composition sum excluded by host contract".into());
                }
            }
            result[row][col] = value.to_bits();
        }
    }
    Ok(result)
}
/// Case 2: transpose(WorldToCamera * Projection), original per-element order.
pub fn world_to_screen(view: Matrix, projection: Matrix) -> Result<Matrix, String> {
    multiply(view, projection, &orders::WORLD_SCREEN).map(transpose)
}
/// Case 3: transpose((ObjectToWorld * WorldToCamera) * Projection).
/// The first product is rounded/stored as f32 before the second multiplication.
pub fn object_to_screen(
    object: Matrix,
    view: Matrix,
    projection: Matrix,
) -> Result<Matrix, String> {
    let camera = multiply(object, view, &orders::OBJECT_SCREEN_VIEW)?;
    multiply(camera, projection, &orders::OBJECT_SCREEN_PROJECTION).map(transpose)
}
/// Case 32: transpose(ObjectToWorld * WorldToCamera), with its own native order.
pub fn object_to_camera(object: Matrix, view: Matrix) -> Result<Matrix, String> {
    multiply(object, view, &orders::OBJECT_CAMERA).map(transpose)
}
#[derive(Clone, Copy, Debug, Default, Serialize)]
pub struct Constant {
    pub kind: u8,
    pub words: [u32; 4],
}
#[derive(Clone, Debug, Serialize)]
pub struct Bank {
    pub words: Vec<[u32; 4]>,
    pub count: i32,
}
#[derive(Clone, Copy, Debug)]
pub struct Host {
    pub scene: scene::Scene,
    pub lighting: Lighting,
    pub object_to_world: Matrix,
    pub world_to_camera: Matrix,
    pub projection: Matrix,
    pub camera_to_world: Option<Matrix>,
    pub editor: bool,
    pub engine_time: f32,
}
fn span(kind: u8) -> usize {
    match kind {
        2..=7 | 32 => 4,
        34 => 2,
        _ => 1,
    }
}
impl Bank {
    /// Raw initial register words are a host decision; no zero-initialization inference.
    pub fn new(words: Vec<[u32; 4]>) -> Result<Self, String> {
        let count = match words.len() {
            96 => -2,
            8 => -3,
            _ => return Err("Constant bank must have 96 or 8 slots".into()),
        };
        Ok(Self { words, count })
    }
    /// Cache native Num*Constants from the last nonzero kind. All-unused still yields 1.
    pub fn initialize_count(&mut self, bindings: &[Constant]) -> Result<usize, String> {
        if bindings.len() != self.words.len() {
            return Err("Constant binding/buffer length mismatch".into());
        }
        if self.count < 0 {
            let limit = match self.count {
                -2 => 96,
                -3 => 8,
                _ => return Err("Unknown constant count marker".into()),
            };
            if bindings.len() < limit {
                return Err("Count scan exceeds bindings".into());
            }
            let last = bindings[..limit]
                .iter()
                .rposition(|b| b.kind != 0)
                .unwrap_or(0);
            self.count = (last + span(bindings[last].kind)) as i32;
        }
        let count = self.count as usize;
        if count > self.words.len() {
            return Err("Native constant count exceeds safe buffer".into());
        }
        Ok(count)
    }
    /// All native constant kinds 0..34 on bounded supplied host snapshots.
    /// Object inverse case7 and light positions share one cache per update.
    /// Writes/cached count/flicker already performed remain visible after host errors.
    pub fn update(
        &mut self,
        bindings: &[Constant],
        host: Host,
        flicker: &mut Flicker,
        inverse: &mut dyn FnMut(Matrix) -> Result<Matrix, String>,
        random: &mut dyn FnMut() -> Result<u16, String>,
    ) -> Result<(), String> {
        let count = self.initialize_count(bindings)?;
        let mut cached_camera = None;
        let mut cached_lights = None;
        let mut cached_object_inverse = None;
        let mut slot = 0;
        while slot < count {
            let binding = bindings[slot];
            let matrix = match binding.kind {
                30 => {
                    self.words[slot] = scene::draw_scale(host.scene, host.object_to_world)?;
                    None
                }
                34 => {
                    let end = slot
                        .checked_add(2)
                        .filter(|end| *end <= self.words.len())
                        .ok_or("Rotator upload exceeds safe buffer")?;
                    let rows = rotator(host.engine_time, f32::from_bits(binding.words[0]))?;
                    self.words[slot..end].copy_from_slice(&rows);
                    slot = end;
                    continue;
                }
                7 => {
                    if cached_object_inverse.is_none() {
                        cached_object_inverse = Some(inverse(host.object_to_world)?);
                    }
                    Some(transpose(cached_object_inverse.unwrap()))
                }
                14 | 17 | 20 | 23 => {
                    let cache =
                        *cached_lights.get_or_insert_with(|| LightCache::new(host.lighting));
                    let light = cache.select(host.lighting, (binding.kind as usize - 14) / 3)?;
                    self.words[slot] = if let Some(light) = light {
                        // Native directional preparation precedes the lazy inverse call.
                        let prepared = if light.kind == 0x13 {
                            Some(shader_lights::position_point(light, host.object_to_world)?)
                        } else {
                            None
                        };
                        if cached_object_inverse.is_none() {
                            cached_object_inverse = Some(inverse(host.object_to_world)?);
                        }
                        let point = match prepared {
                            Some(point) => point,
                            None => shader_lights::position_point(light, host.object_to_world)?,
                        };
                        shader_lights::transform_position(
                            point,
                            cached_object_inverse.unwrap(),
                            light.kind == 0x13,
                        )?
                    } else {
                        shader_lights::missing_position()
                    };
                    None
                }
                15 | 18 | 21 | 24 | 28 | 29 => {
                    let cache =
                        *cached_lights.get_or_insert_with(|| LightCache::new(host.lighting));
                    let index = if binding.kind >= 28 {
                        0
                    } else {
                        (binding.kind as usize - 15) / 3
                    };
                    let light = cache.select(host.lighting, index)?;
                    self.words[slot] = match binding.kind {
                        28 => shader_lights::spotlight_direction(light),
                        29 => shader_lights::spotlight_cone(light),
                        _ => shader_lights::color(light, host.lighting.alpha_gate)?,
                    };
                    None
                }
                16 | 19 | 22 | 25 => {
                    let cache =
                        *cached_lights.get_or_insert_with(|| LightCache::new(host.lighting));
                    let light = cache.select(host.lighting, (binding.kind as usize - 16) / 3)?;
                    self.words[slot] = shader_lights::inverse_radius(
                        light,
                        host.object_to_world,
                        host.scene.reciprocal_sqrt_seed,
                    )?;
                    None
                }
                31 => {
                    self.words[slot] = scene::fog(host.scene.fog);
                    None
                }
                33 => {
                    if cached_object_inverse.is_none() {
                        cached_object_inverse = Some(inverse(host.object_to_world)?);
                    }
                    let eye = if host.editor {
                        host.scene.editor_eye
                    } else {
                        host.scene.runtime_eye.ok_or("Native EyePositionObjectSpace runtime null dereference excluded by host contract")?
                    };
                    self.words[slot] =
                        scene::eye_position(eye, cached_object_inverse.unwrap(), host.editor)?;
                    None
                }
                26 => {
                    self.words[slot] = shader_lights::ambient(host.lighting.ambient_bgra);
                    None
                }
                2 => Some(world_to_screen(host.world_to_camera, host.projection)?),
                3 => Some(object_to_screen(
                    host.object_to_world,
                    host.world_to_camera,
                    host.projection,
                )?),
                4 => Some(transpose(host.object_to_world)),
                6 => Some(transpose(host.world_to_camera)),
                32 => Some(object_to_camera(
                    host.object_to_world,
                    host.world_to_camera,
                )?),
                5 | 12 => {
                    if cached_camera.is_none() {
                        cached_camera = Some(if !host.editor {
                            if let Some(camera) = host.camera_to_world {
                                camera
                            } else if binding.kind == 5 {
                                return Err("Native CameraToWorld runtime null dereference excluded by host contract".into());
                            } else {
                                inverse(host.world_to_camera)?
                            }
                        } else {
                            inverse(host.world_to_camera)?
                        });
                    }
                    let camera = cached_camera.unwrap();
                    if binding.kind == 12 {
                        self.words[slot] = camera[3];
                        None
                    } else {
                        Some(transpose(camera))
                    }
                }
                0 => None,
                1 => {
                    self.words[slot] = binding.words;
                    None
                }
                8 => {
                    self.words[slot] = shader_time(host.engine_time)?.map(f32::to_bits);
                    None
                }
                9 => {
                    self.words[slot] =
                        cos_time(host.engine_time, f32::from_bits(binding.words[0]))?
                            .map(f32::to_bits);
                    None
                }
                10 | 11 => {
                    let function = if binding.kind == 10 {
                        sin_time
                    } else {
                        tan_time
                    };
                    self.words[slot] =
                        function(host.engine_time, f32::from_bits(binding.words[0]))?
                            .map(f32::to_bits);
                    None
                }
                13 => {
                    self.words[slot] = xy_circle(host.engine_time)?.map(f32::to_bits);
                    None
                }
                27 => {
                    self.words[slot] = flicker
                        .evaluate(host.engine_time, binding.words.map(f32::from_bits), random)?
                        .map(f32::to_bits);
                    None
                }
                kind => {
                    return Err(format!(
                        "Unreconstructed shader constant kind {kind} at slot {slot}"
                    ))
                }
            };
            if let Some(matrix) = matrix {
                let end = slot
                    .checked_add(4)
                    .filter(|end| *end <= self.words.len())
                    .ok_or("Matrix upload exceeds safe buffer")?;
                self.words[slot..end].copy_from_slice(&matrix);
                slot = end;
            } else {
                slot += 1;
            }
        }
        Ok(())
    }
}
#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn draw_scale_seed_failure_keeps_current_register_and_prior_writes() {
        fn fail(_: f32) -> Result<f32, String> {
            Err("seed unavailable".into())
        }
        let mut h = host();
        h.object_to_world = identity();
        h.scene.reciprocal_sqrt_seed = Some(fail);
        let mut c = [Constant::default(); 8];
        c[0] = Constant {
            kind: 1,
            words: [1, 2, 3, 4],
        };
        c[1].kind = 30;
        let mut b = Bank::new(vec![[9; 4]; 8]).unwrap();
        assert!(b
            .update(
                &c,
                h,
                &mut Flicker::default(),
                &mut |_| panic!(),
                &mut || panic!()
            )
            .is_err());
        assert_eq!(b.words[0], [1, 2, 3, 4]);
        assert_eq!(b.words[1], [9; 4]);
    }
    #[test]
    fn rotator_skips_second_binding_and_checked_span_preserves_bank() {
        let mut c = [Constant::default(); 8];
        c[6] = Constant {
            kind: 34,
            words: [0; 4],
        };
        c[7].kind = 255;
        let mut b = Bank::new(vec![[9; 4]; 8]).unwrap();
        b.count = 7;
        b.update(
            &c,
            host(),
            &mut Flicker::default(),
            &mut |_| panic!(),
            &mut || panic!(),
        )
        .unwrap();
        assert_eq!(&b.words[6..], &rotator(0., 0.).unwrap());
        c[6].kind = 0;
        c[7].kind = 34;
        b.count = 8;
        let before = b.words.clone();
        assert!(b
            .update(
                &c,
                host(),
                &mut Flicker::default(),
                &mut |_| panic!(),
                &mut || panic!()
            )
            .is_err());
        assert_eq!(before, b.words);
    }
    #[test]
    fn wave_dispatch_ignores_unused_words_and_error_keeps_previous_writes() {
        let mut c = [Constant::default(); 8];
        c[0] = Constant {
            kind: 10,
            words: [0, f32::NAN.to_bits(), 7, 8],
        };
        c[1] = Constant {
            kind: 13,
            words: [f32::NAN.to_bits(); 4],
        };
        c[2] = Constant {
            kind: 11,
            words: [f32::NAN.to_bits(); 4],
        };
        let mut h = host();
        h.engine_time = 0.5;
        let mut bank = Bank::new(vec![[9; 4]; 8]).unwrap();
        assert!(bank
            .update(
                &c,
                h,
                &mut Flicker::default(),
                &mut |_| panic!(),
                &mut || panic!()
            )
            .is_err());
        assert_eq!(bank.words[0], sin_time(0.5, 1.).unwrap().map(f32::to_bits));
        assert_eq!(bank.words[1], xy_circle(0.5).unwrap().map(f32::to_bits));
        assert_eq!(bank.words[2], [9; 4]);
        assert_eq!(bank.count, 3);
    }
    #[test]
    fn object_eye_shares_inverse_but_null_runtime_camera_errors_after_inverse() {
        let mut h = host();
        h.scene.editor_eye = [1f32.to_bits(); 3];
        h.editor = true;
        let mut c = [Constant::default(); 8];
        c[0].kind = 33;
        c[1].kind = 7;
        c[5].kind = 33;
        let mut b = Bank::new(vec![[9; 4]; 8]).unwrap();
        let mut calls = 0;
        b.update(
            &c,
            h,
            &mut Flicker::default(),
            &mut |_| {
                calls += 1;
                Ok(identity())
            },
            &mut || panic!(),
        )
        .unwrap();
        assert_eq!(calls, 1);
        assert_eq!(b.words[0], [1f32.to_bits(); 4]);
        assert_eq!(b.words[5], b.words[0]);
        h.editor = false;
        b.words[0] = [9; 4];
        assert!(b
            .update(
                &c,
                h,
                &mut Flicker::default(),
                &mut |_| {
                    calls += 1;
                    Ok(identity())
                },
                &mut || panic!()
            )
            .is_err());
        assert_eq!(calls, 2);
        assert_eq!(b.words[0], [9; 4]);
    }
    #[test]
    fn object_inverse_is_shared_with_four_light_positions_and_resets_next_dispatch() {
        let mut h = host();
        h.object_to_world = identity();
        h.lighting.slots = [Some(shader_lights::Light {
            actor_present: true,
            position: [2f32.to_bits(); 3],
            ..Default::default()
        }); 4];
        let mut c = [Constant::default(); 8];
        for (i, k) in [14, 17, 20, 23, 7].into_iter().enumerate() {
            c[i].kind = k;
        }
        let mut b = Bank::new(vec![[9; 4]; 8]).unwrap();
        let mut calls = 0;
        for _ in 0..2 {
            b.update(
                &c,
                h,
                &mut Flicker::default(),
                &mut |m| {
                    assert_eq!(m, identity());
                    calls += 1;
                    Ok(identity())
                },
                &mut || panic!(),
            )
            .unwrap();
        }
        assert_eq!(calls, 2);
        assert_eq!(&b.words[4..], &identity());
        c[0].kind = 7;
        c[4].kind = 14;
        b.update(
            &c,
            h,
            &mut Flicker::default(),
            &mut |_| {
                calls += 1;
                Ok(identity())
            },
            &mut || panic!(),
        )
        .unwrap();
        assert_eq!(calls, 3);
    }
    #[test]
    fn missing_and_sparse_positions_do_not_request_inverse_and_error_preserves_prior_words() {
        let mut h = host();
        let mut c = [Constant::default(); 8];
        c[0].kind = 1;
        c[1].kind = 14;
        let mut b = Bank::new(vec![[9; 4]; 8]).unwrap();
        b.update(
            &c,
            h,
            &mut Flicker::default(),
            &mut |_| panic!(),
            &mut || panic!(),
        )
        .unwrap();
        assert_eq!(b.words[1], shader_lights::missing_position());
        h.lighting.slots[1] = Some(shader_lights::Light {
            actor_present: true,
            ..Default::default()
        });
        assert!(b
            .update(
                &c,
                h,
                &mut Flicker::default(),
                &mut |_| panic!(),
                &mut || panic!()
            )
            .is_err());
        assert_eq!(b.words[1], shader_lights::missing_position());
        h.lighting.slots[0] = h.lighting.slots[1];
        assert!(b
            .update(
                &c,
                h,
                &mut Flicker::default(),
                &mut |_| Err("inverse host error".into()),
                &mut || panic!()
            )
            .is_err());
        assert_eq!(b.words[0], [0; 4]);
        assert_eq!(b.count, 2);
    }
    #[test]
    fn directional_invalid_point_precedes_inverse_but_positional_validation_follows_it() {
        for kind in [0, 0x13] {
            let mut h = host();
            h.lighting.slots[0] = Some(shader_lights::Light {
                actor_present: true,
                kind,
                position: [f32::NAN.to_bits(); 3],
                direction: [f32::NAN.to_bits(); 3],
                ..Default::default()
            });
            let mut c = [Constant::default(); 8];
            c[0].kind = 14;
            let mut b = Bank::new(vec![[9; 4]; 8]).unwrap();
            let mut calls = 0;
            assert!(b
                .update(
                    &c,
                    h,
                    &mut Flicker::default(),
                    &mut |_| {
                        calls += 1;
                        Ok(identity())
                    },
                    &mut || panic!()
                )
                .is_err());
            assert_eq!(calls, if kind == 0 { 1 } else { 0 });
            assert_eq!(b.words[0], [9; 4]);
        }
    }
    #[test]
    fn light_dispatch_uses_source_slots_and_rebuilds_selection_next_update() {
        let light = crate::shader_lights::Light {
            radius: [0; 2],
            kind: 0,
            position: [0; 3],
            actor_present: true,
            cone: 255,
            color: [0.25f32.to_bits(); 4],
            brightness: 3f32.to_bits(),
            direction: [7, 8, 9],
            flags: [0; 2],
        };
        let mut h = host();
        h.lighting.slots = [Some(light); 4];
        h.lighting.ambient_bgra = 0x99ff0000;
        let mut c = [Constant::default(); 8];
        for (i, kind) in [15, 18, 21, 24, 26, 28, 29].into_iter().enumerate() {
            c[i].kind = kind;
        }
        let mut b = Bank::new(vec![[99; 4]; 8]).unwrap();
        b.update(
            &c,
            h,
            &mut Flicker::default(),
            &mut |_| panic!(),
            &mut || panic!(),
        )
        .unwrap();
        assert_eq!(
            b.words[0],
            shader_lights::color(Some(light), false).unwrap()
        );
        assert_eq!(b.words[4], shader_lights::ambient(h.lighting.ambient_bgra));
        assert_eq!(b.words[5], [7, 8, 9, 1f32.to_bits()]);
        assert_eq!(b.words[7], [99; 4]);
        h.lighting.slots = [None; 4];
        b.update(
            &c,
            h,
            &mut Flicker::default(),
            &mut |_| panic!(),
            &mut || panic!(),
        )
        .unwrap();
        assert_eq!(b.words[0], [0; 4]);
        assert_eq!(b.words[5], [1f32.to_bits(), 0, 0, 1f32.to_bits()]);
    }
    #[test]
    fn sparse_light_error_keeps_prior_ambient_write_and_cached_bank_count() {
        let mut h = host();
        h.lighting.slots[1] = Some(crate::shader_lights::Light {
            actor_present: true,
            ..Default::default()
        });
        let mut c = [Constant::default(); 8];
        c[0].kind = 26;
        c[1].kind = 15;
        let mut b = Bank::new(vec![[99; 4]; 8]).unwrap();
        assert!(b
            .update(
                &c,
                h,
                &mut Flicker::default(),
                &mut |_| panic!(),
                &mut || panic!()
            )
            .is_err());
        assert_eq!(b.count, 2);
        assert_eq!(b.words[0], shader_lights::ambient(0));
        assert_eq!(b.words[1], [99; 4]);
    }
    fn words(m: [[f32; 4]; 4]) -> Matrix {
        m.map(|r| r.map(f32::to_bits))
    }
    fn identity() -> Matrix {
        words(std::array::from_fn(|r| {
            std::array::from_fn(|c| if r == c { 1. } else { 0. })
        }))
    }
    #[test]
    fn composition_preserves_native_case_specific_cancellation_order() {
        let a = words([[1.; 4]; 4]);
        let b = words([[16777216.; 4], [1.; 4], [-16777216.; 4], [1.; 4]]);
        assert_eq!(world_to_screen(a, b).unwrap()[0][0], 2f32.to_bits());
        assert_eq!(object_to_camera(a, b).unwrap()[0][0], 1f32.to_bits());
        assert_eq!(
            object_to_screen(a, b, identity()).unwrap()[0][0],
            2f32.to_bits()
        );
    }
    #[test]
    fn object_screen_multiplies_in_row_order_then_transposes_including_perspective() {
        let object = words([
            [2., 0., 0., 0.],
            [0., 3., 0., 0.],
            [0., 0., 4., 0.],
            [5., 6., 7., 1.],
        ]);
        let view = words([
            [0., 1., 0., 0.],
            [-1., 0., 0., 0.],
            [0., 0., 1., 0.],
            [8., 9., 10., 1.],
        ]);
        let projection = words([
            [2., 0., 0., 0.],
            [0., 3., 0., 0.],
            [0., 0., 4., 1.],
            [0., 0., -5., 0.],
        ]);
        let expected = words([
            [0., -6., 0., 4.],
            [6., 0., 0., 42.],
            [0., 0., 16., 63.],
            [0., 0., 4., 17.],
        ]);
        assert_eq!(
            object_to_screen(object, view, projection).unwrap(),
            expected
        );
        assert_eq!(
            object_to_camera(object, view).unwrap(),
            words([
                [0., -3., 0., 2.],
                [2., 0., 0., 14.],
                [0., 0., 4., 17.],
                [0., 0., 0., 1.]
            ])
        );
    }
    #[test]
    fn composed_dispatch_packs_matrices_and_skips_continuation_bindings() {
        let mut h = host();
        h.object_to_world = identity();
        h.world_to_camera = identity();
        h.projection = identity();
        let mut b = Bank::new(vec![[7; 4]; 96]).unwrap();
        let mut c = vec![Constant::default(); 96];
        for (slot, kind) in [(0, 2), (4, 3), (8, 32)] {
            c[slot].kind = kind;
            c[slot + 1].kind = 255;
        }
        c[12] = Constant {
            kind: 1,
            words: [8; 4],
        };
        b.update(
            &c,
            h,
            &mut Flicker::default(),
            &mut |_| panic!(),
            &mut || panic!(),
        )
        .unwrap();
        assert_eq!(b.count, 13);
        for chunk in b.words[..12].chunks_exact(4) {
            assert_eq!(chunk, identity());
        }
        assert_eq!(b.words[12], [8; 4]);
        assert_eq!(b.words[13], [7; 4]);
    }
    #[test]
    fn composition_errors_do_not_upload_a_partial_matrix_but_retain_prior_registers() {
        let mut h = host();
        h.object_to_world = identity();
        h.world_to_camera = identity();
        h.projection = identity();
        h.projection[0][0] = f32::NAN.to_bits();
        let mut b = Bank::new(vec![[7; 4]; 8]).unwrap();
        let mut c = [Constant::default(); 8];
        c[0] = Constant {
            kind: 1,
            words: [8; 4],
        };
        c[1].kind = 3;
        assert!(b
            .update(
                &c,
                h,
                &mut Flicker::default(),
                &mut |_| panic!(),
                &mut || panic!()
            )
            .is_err());
        assert_eq!(b.count, 5);
        assert_eq!(b.words[0], [8; 4]);
        assert_eq!(b.words[1..], [[7; 4]; 7]);
        let large = words([[f32::MAX; 4]; 4]);
        assert!(object_to_camera(large, large).is_err());
        // Finite products can overflow during addition too.
        assert!(world_to_screen(large, words([[1.; 4]; 4])).is_err());
    }
    fn matrix() -> Matrix {
        std::array::from_fn(|r| std::array::from_fn(|c| (r * 4 + c) as u32))
    }
    fn host() -> Host {
        Host {
            scene: Default::default(),
            lighting: Lighting::default(),
            object_to_world: matrix(),
            world_to_camera: matrix(),
            projection: matrix(),
            camera_to_world: Some(matrix()),
            editor: false,
            engine_time: 0.,
        }
    }
    #[test]
    fn raw_transpose_preserves_signed_zero_and_nan_payloads() {
        let mut m = matrix();
        m[1][2] = 0x7fc12345;
        m[3][0] = 0x80000000;
        assert_eq!(transpose(m)[2][1], 0x7fc12345);
        assert_eq!(transpose(transpose(m)), m);
    }
    #[test]
    fn matrix_upload_skips_continuations_and_unused_registers_persist() {
        let mut b = Bank::new(vec![[0xdeadbeef; 4]; 8]).unwrap();
        let mut c = [Constant::default(); 8];
        c[0].kind = 4;
        c[1].kind = 255;
        c[5] = Constant {
            kind: 1,
            words: [0x80000000, 0x7fc12345, 3, 4],
        };
        b.update(
            &c,
            host(),
            &mut Flicker::default(),
            &mut |_| panic!(),
            &mut || panic!(),
        )
        .unwrap();
        assert_eq!(b.count, 6);
        assert_eq!(&b.words[..4], &transpose(matrix()));
        assert_eq!(b.words[4], [0xdeadbeef; 4]);
        assert_eq!(b.words[5], c[5].words);
        c[5] = Constant::default();
        b.update(
            &c,
            host(),
            &mut Flicker::default(),
            &mut |_| panic!(),
            &mut || panic!(),
        )
        .unwrap();
        assert_eq!(b.count, 6);
        assert_eq!(b.words[5], [0x80000000, 0x7fc12345, 3, 4]);
    }
    #[test]
    fn camera_selection_cache_shared_by_eye_and_matrix_and_resets_each_update() {
        let mut b = Bank::new(vec![[0; 4]; 8]).unwrap();
        let mut c = [Constant::default(); 8];
        c[0].kind = 12;
        c[1].kind = 5;
        let mut h = host();
        h.editor = true;
        let mut calls = 0;
        let mut inv = |input| {
            assert_eq!(input, matrix());
            calls += 1;
            Ok(transpose(matrix()))
        };
        for _ in 0..2 {
            b.update(&c, h, &mut Flicker::default(), &mut inv, &mut || panic!())
                .unwrap();
            assert_eq!(b.words[0], transpose(matrix())[3]);
            assert_eq!(&b.words[1..5], &matrix());
        }
        assert_eq!(calls, 2);
    }
    #[test]
    fn runtime_eye_null_fallback_then_camera_reuses_cached_inverse() {
        let mut b = Bank::new(vec![[0; 4]; 8]).unwrap();
        let mut c = [Constant::default(); 8];
        c[0].kind = 12;
        c[1].kind = 5;
        let mut h = host();
        h.camera_to_world = None;
        b.update(
            &c,
            h,
            &mut Flicker::default(),
            &mut |_| Ok(matrix()),
            &mut || panic!(),
        )
        .unwrap();
        assert_eq!(b.words[0], matrix()[3]);
        c[0].kind = 5;
        assert!(b
            .update(
                &c,
                h,
                &mut Flicker::default(),
                &mut |_| panic!(),
                &mut || panic!()
            )
            .is_err());
    }
    #[test]
    fn empty_banks_cached_counts_and_out_of_bounds_matrix_rejected() {
        let c = [Constant::default(); 8];
        let mut b = Bank::new(vec![[7; 4]; 8]).unwrap();
        assert_eq!(b.initialize_count(&c).unwrap(), 1);
        let mut c = c;
        c[7].kind = 4;
        assert_eq!(b.initialize_count(&c).unwrap(), 1);
        b.count = -3;
        assert!(b.initialize_count(&c).is_err());
        assert_eq!(b.count, 11);
        assert!(Bank::new(vec![[0; 4]; 9]).is_err());
    }
    #[test]
    fn host_errors_leave_prior_writes_and_eye_copies_all_four_words() {
        let mut b = Bank::new(vec![[7; 4]; 8]).unwrap();
        let mut c = [Constant::default(); 8];
        c[0] = Constant {
            kind: 1,
            words: [0x7fc11111; 4],
        };
        c[1].kind = 35;
        assert!(b
            .update(
                &c,
                host(),
                &mut Flicker::default(),
                &mut |_| panic!(),
                &mut || panic!()
            )
            .is_err());
        assert_eq!(b.words[0], c[0].words);
        assert_eq!(b.words[1], [7; 4]);
        c[1].kind = 12;
        let mut h = host();
        let mut m = matrix();
        m[3][3] = 0x80000000;
        h.camera_to_world = Some(m);
        b.update(
            &c,
            h,
            &mut Flicker::default(),
            &mut |_| panic!(),
            &mut || panic!(),
        )
        .unwrap();
        assert_eq!(b.words[1], m[3]);
    }
}
