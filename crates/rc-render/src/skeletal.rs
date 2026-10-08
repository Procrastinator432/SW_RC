//! Diagnostic material-slot checker view for reconstructed skeletal triangles.
use crate::{FaceTexture, Scene, Texture};
use rc_package::{geometry::Triangle, skeletal_draw::SkeletalDraw};
impl Scene {
    /// Apply a sampled diagonal UV matrix by material slot, atomically.
    /// Call on fresh source UVs each frame; repeated calls accumulate transforms.
    pub fn apply_skeletal_uv_transforms(
        &mut self,
        slots: &[Option<rc_package::material_uv::UvTransform>],
    ) -> Result<usize, String> {
        if slots.len() > 1024 || self.appearance.len() != self.triangles.len() {
            return Err("invalid skeletal UV transform slots".into());
        }
        if slots
            .iter()
            .flatten()
            .any(|t| t.scale.iter().chain(&t.offset).any(|v| !v.is_finite()))
        {
            return Err("nonfinite skeletal UV matrix".into());
        }
        let mut appearance = self.appearance.clone();
        let mut count = 0;
        for (triangle, face) in self.triangles.iter().zip(&mut appearance) {
            if let Some(transform) = slots.get(triangle.surface).copied().flatten() {
                let face = face
                    .as_mut()
                    .ok_or("UV transform lacks resolved diffuse UV")?;
                face.uv = face.uv.map(|uv| transform.apply(uv));
                if face.uv.iter().flatten().any(|v| !v.is_finite()) {
                    return Err("skeletal UV matrix overflow".into());
                }
                count += 1;
            }
        }
        self.appearance = appearance;
        Ok(count)
    }
    /// Apply resolved base diffuse textures to a prepared skeletal scene.
    /// Missing slots retain flat diagnostic shading; source UVs are scaled once.
    /// Commit only after all supplied dimensions/scales and resulting UVs validate.
    pub fn apply_skeletal_diffuse_slots(
        &mut self,
        materials: &[Option<(Texture, f32)>],
    ) -> Result<usize, String> {
        if self.appearance.len() != self.triangles.len() {
            return Err("skeletal diffuse needs one UV record per triangle".into());
        }
        if materials.len() > 1024 {
            return Err("skeletal diffuse material slot limit exceeded".into());
        }
        let mut textures = vec![];
        let mut slots = vec![];
        let mut total = 0;
        for material in materials {
            slots.push(if let Some((texture, scale)) = material {
                let (w, h) = (texture.width, texture.height);
                if w == 0
                    || h == 0
                    || w > 4096
                    || h > 4096
                    || w * h != texture.pixels.len()
                    || !scale.is_finite()
                {
                    return Err("invalid skeletal diffuse texture or scale".into());
                }
                total += w * h;
                if total > 16_777_216 {
                    return Err("skeletal diffuse texture budget exceeded".into());
                }
                let index = textures.len();
                textures.push(texture.clone());
                Some((index, *scale))
            } else {
                None
            });
        }
        let mut appearance = vec![];
        let mut resolved = 0;
        for (triangle, source) in self.triangles.iter().zip(&self.appearance) {
            let binding = slots.get(triangle.surface).copied().flatten();
            appearance.push(if let Some((texture, scale)) = binding {
                let source = source.ok_or("resolved skeletal diffuse lacks source UV")?;
                let uv = source.uv.map(|pair| pair.map(|v| v * scale));
                if uv.iter().flatten().any(|v| !v.is_finite()) {
                    return Err("nonfinite scaled skeletal UV".into());
                }
                resolved += 1;
                Some(FaceTexture { uv, texture })
            } else {
                None
            });
        }
        self.textures = textures;
        self.appearance = appearance;
        Ok(resolved)
    }
    pub fn from_skeletal_draw(draw: &SkeletalDraw) -> Result<Self, String> {
        let triangles = draw
            .triangles
            .iter()
            .map(|t| Triangle {
                points: t.vertices.map(|v| v.vertex.position.map(f32::from_bits)),
                surface: draw.sections[t.section].material as usize,
            })
            .collect();
        let mut scene = Self::new(triangles)?;
        let slots = draw
            .sections
            .iter()
            .map(|s| s.material as usize + 1)
            .max()
            .unwrap_or(0);
        if slots > 1024 {
            return Err("diagnostic skeletal material slot limit exceeded".into());
        }
        for slot in 0..slots {
            let color = 0xff000000
                | (((slot as u32 * 71 + 90) % 156 + 80) << 16)
                | (((slot as u32 * 31 + 40) % 156 + 80) << 8)
                | ((slot as u32 * 97 + 20) % 156 + 80);
            scene.textures.push(Texture {
                width: 4,
                height: 4,
                pixels: (0..16)
                    .map(|i| {
                        if (i / 4 + i % 4) % 2 == 0 {
                            color
                        } else {
                            0xff303840
                        }
                    })
                    .collect(),
            });
        }
        for t in &draw.triangles {
            let uv = t.vertices.map(|v| v.uv.map(f32::from_bits));
            if uv.iter().flatten().any(|v| !v.is_finite()) {
                return Err("nonfinite skeletal UV in diagnostic scene".into());
            }
            scene.appearance.push(Some(FaceTexture {
                uv,
                texture: draw.sections[t.section].material as usize,
            }));
        }
        Ok(scene)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use rc_package::{
        skeletal_draw::{DrawSection, DrawTriangle},
        skeletal_skin::{SkinStreamVertex, SkinVertex},
    };
    fn fixture() -> SkeletalDraw {
        let points = [[0f32, 0., 0.], [1., 0., 0.], [0., 1., 0.]];
        SkeletalDraw {
            sections: vec![DrawSection {
                bank: 0,
                section_index: 0,
                material: 2,
                bone: None,
                first_index: 0,
                triangle_count: 1,
                min_vertex: 0,
                max_vertex: 2,
            }],
            triangles: vec![DrawTriangle {
                section: 0,
                indices: [0, 1, 2],
                vertices: points.map(|p| SkinStreamVertex {
                    vertex: SkinVertex {
                        position: p.map(f32::to_bits),
                        normal: [0; 3],
                    },
                    uv: [0.5f32.to_bits(), 1f32.to_bits()],
                }),
            }],
            unused_indices: [0; 2],
        }
    }
    #[test]
    fn sampled_uv_matrix_uses_material_slot_and_preserves_snapshot() {
        use rc_package::material_uv::UvTransform;
        let mut scene = Scene::from_skeletal_draw(&fixture()).unwrap();
        assert_eq!(
            scene
                .apply_skeletal_uv_transforms(&[
                    None,
                    None,
                    Some(UvTransform {
                        scale: [2., 3.],
                        offset: [0.25, -0.5]
                    })
                ])
                .unwrap(),
            1
        );
        assert_eq!(scene.appearance[0].unwrap().uv, [[1.25, 2.5]; 3]);
        let loaded = Scene::from_snapshot(&scene.snapshot().unwrap()).unwrap();
        assert_eq!(
            loaded.appearance[0].unwrap().uv,
            scene.appearance[0].unwrap().uv
        );
    }
    #[test]
    fn sampled_uv_overflow_is_atomic() {
        use rc_package::material_uv::UvTransform;
        let mut scene = Scene::from_skeletal_draw(&fixture()).unwrap();
        let before = scene.snapshot().unwrap();
        assert!(scene
            .apply_skeletal_uv_transforms(&[
                None,
                None,
                Some(UvTransform {
                    scale: [f32::MAX; 2],
                    offset: [f32::MAX; 2]
                })
            ])
            .is_err());
        assert_eq!(before, scene.snapshot().unwrap());
    }
    #[test]
    fn skeletal_scene_preserves_mesh_positions_material_slot_and_uv() {
        let scene = Scene::from_skeletal_draw(&fixture()).unwrap();
        assert_eq!(scene.triangles.len(), 1);
        assert_eq!(scene.triangles[0].points[1], [1., 0., 0.]);
        assert_eq!(scene.textures.len(), 3);
        assert_eq!(scene.appearance[0].unwrap().texture, 2);
        assert_eq!(scene.appearance[0].unwrap().uv, [[0.5, 1.]; 3]);
        assert_eq!(
            Scene::from_snapshot(&scene.snapshot().unwrap())
                .unwrap()
                .triangles
                .len(),
            1
        );
    }
    #[test]
    fn skeletal_scene_rejects_nonfinite_uv_and_excessive_material_slot() {
        let mut draw = fixture();
        draw.triangles[0].vertices[0].uv[0] = f32::NAN.to_bits();
        assert!(Scene::from_skeletal_draw(&draw).is_err());
        draw = fixture();
        draw.sections[0].material = 65535;
        assert!(Scene::from_skeletal_draw(&draw).is_err());
    }
    #[test]
    fn original_diffuse_slot_replaces_checker_and_scales_uv() {
        let mut scene = Scene::from_skeletal_draw(&fixture()).unwrap();
        let tex = Texture {
            width: 1,
            height: 1,
            pixels: vec![0xff112233],
        };
        assert_eq!(
            scene
                .apply_skeletal_diffuse_slots(&[None, None, Some((tex, 2.))])
                .unwrap(),
            1
        );
        assert_eq!(scene.textures[0].pixels, [0xff112233]);
        assert_eq!(scene.appearance[0].unwrap().uv, [[1., 2.]; 3]);
        assert_eq!(scene.appearance[0].unwrap().texture, 0);
    }
    #[test]
    fn omitted_diffuse_slot_retains_geometry_and_flat_shading() {
        let mut scene = Scene::from_skeletal_draw(&fixture()).unwrap();
        assert_eq!(scene.apply_skeletal_diffuse_slots(&[]).unwrap(), 0);
        assert!(scene.textures.is_empty());
        assert!(scene.appearance[0].is_none());
        assert_eq!(scene.triangles.len(), 1);
    }
    #[test]
    fn invalid_diffuse_input_does_not_mutate_scene() {
        let mut scene = Scene::from_skeletal_draw(&fixture()).unwrap();
        let before = scene.snapshot().unwrap();
        let tex = Texture {
            width: 1,
            height: 1,
            pixels: vec![],
        };
        assert!(scene
            .apply_skeletal_diffuse_slots(&[Some((tex, 1.))])
            .is_err());
        assert_eq!(scene.snapshot().unwrap(), before);
        let tex = Texture {
            width: 1,
            height: 1,
            pixels: vec![0],
        };
        assert!(scene
            .apply_skeletal_diffuse_slots(&[Some((tex, f32::NAN))])
            .is_err());
        assert_eq!(scene.snapshot().unwrap(), before);
    }
    #[test]
    fn scaled_uv_overflow_rejected_without_mutation() {
        let mut scene = Scene::from_skeletal_draw(&fixture()).unwrap();
        let before = scene.snapshot().unwrap();
        scene.appearance[0].as_mut().unwrap().uv = [[f32::MAX; 2]; 3];
        let tex = Texture {
            width: 1,
            height: 1,
            pixels: vec![0],
        };
        assert!(scene
            .apply_skeletal_diffuse_slots(&[None, None, Some((tex, 2.))])
            .is_err());
        assert_eq!(scene.textures.len(), 3);
        scene.appearance[0].as_mut().unwrap().uv = [[0.5, 1.]; 3];
        assert_eq!(scene.snapshot().unwrap(), before);
    }
}
