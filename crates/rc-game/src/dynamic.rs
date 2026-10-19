//! Selected removable actors. Static GPU packets stay immutable; visible actor
//! geometry travels in the shared CPU/GPU frame, and collision follows State.
use crate::{bundle, Game};
use rc_package::{
    hull::HullSet,
    world_collision::{BodyActor, BodyShape},
};
use serde::{Deserialize, Serialize};
use std::sync::Arc;
#[derive(Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Object {
    pub original: u32,
    pub collider: bundle::Collider,
    /// Optional original mesh snapshot; blocking volumes have no visible mesh.
    pub snapshot: Vec<u8>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub released: Option<Released>,
}
#[derive(Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Released {
    pub collider: bundle::Collider,
    pub snapshot: Vec<u8>,
}
pub struct PreparedObject {
    pub original: u32,
    pub actor: usize,
    pub hulls: Arc<HullSet>,
    pub triangles: Vec<rc_package::geometry::Triangle>,
    pub appearance: Vec<Option<rc_render::FaceTexture>>,
    pub released_hulls: Option<Arc<HullSet>>,
    pub released_triangles: Vec<rc_package::geometry::Triangle>,
    pub released_appearance: Vec<Option<rc_render::FaceTexture>>,
}
pub struct Prepared {
    pub objects: Vec<PreparedObject>,
}
impl Game {
    pub fn attach_dynamic(&mut self, objects: Vec<Object>) -> Result<(), String> {
        if self.dynamic.is_some() {
            return Err("Dynamic actors already attached".into());
        }
        self.append_dynamic(objects)
    }
    /// Append a validated mission extension without changing the earlier actor order.
    pub fn append_dynamic(&mut self, objects: Vec<Object>) -> Result<(), String> {
        let existing = self.dynamic.as_ref().map_or(0, |p| p.objects.len());
        if objects.is_empty()
            || existing + objects.len() > 32
            || self.state.dynamic.as_ref().map_or(0, Vec::len) != existing
        {
            return Err("Invalid dynamic actor set".into());
        }
        let mut ids: std::collections::BTreeSet<_> = self
            .dynamic
            .as_ref()
            .into_iter()
            .flat_map(|p| p.objects.iter().map(|p| p.original))
            .collect();
        let mut prepared = vec![];
        let mut textures = vec![];
        for object in objects {
            if object.original == 0
                || !ids.insert(object.original)
                || object.snapshot.len() > 8 * 1024 * 1024
            {
                return Err("Invalid dynamic actor".into());
            }
            let hulls = object.collider.runtime()?;
            if hulls.hulls.is_empty() {
                return Err("Empty dynamic collider".into());
            }
            let mut decode = |snapshot: &[u8]| -> Result<_, String> {
                if snapshot.is_empty() {
                    return Ok((vec![], vec![]));
                }
                if snapshot.len() > 8 * 1024 * 1024 {
                    return Err("Dynamic snapshot exceeds budget".into());
                }
                let scene = rc_render::Scene::from_snapshot(snapshot)?;
                if scene.triangles.len() > 10000 {
                    return Err("Dynamic actor geometry exceeds budget".into());
                }
                let base = self.scene.textures.len() + textures.len();
                let appearance = if scene.appearance.is_empty() {
                    vec![None; scene.triangles.len()]
                } else {
                    scene
                        .appearance
                        .into_iter()
                        .map(|f| {
                            f.map(|mut f| {
                                f.texture += base;
                                f
                            })
                        })
                        .collect()
                };
                textures.extend(scene.textures);
                Ok((scene.triangles, appearance))
            };
            let (triangles, appearance) = decode(&object.snapshot)?;
            let (released_hulls, released_triangles, released_appearance) =
                if let Some(r) = object.released {
                    let hulls = r.collider.runtime()?;
                    if hulls.hulls.is_empty() {
                        return Err("Empty released collider".into());
                    }
                    let (triangles, appearance) = decode(&r.snapshot)?;
                    (Some(hulls), triangles, appearance)
                } else {
                    (None, vec![], vec![])
                };
            prepared.push(PreparedObject {
                original: object.original,
                actor: self.world.actors.len() + prepared.len(),
                hulls,
                triangles,
                appearance,
                released_hulls,
                released_triangles,
                released_appearance,
            });
        }
        // All validation precedes mutation, including snapshot and hull parsing.
        for p in &prepared {
            self.world.actors.push(BodyActor {
                name: format!("Dynamic.{}", p.original),
                shape: BodyShape::Hulls(p.hulls.clone()),
            });
        }
        self.scene.textures.extend(textures);
        self.state
            .dynamic
            .get_or_insert_with(Vec::new)
            .extend(vec![false; prepared.len()]);
        self.dynamic
            .get_or_insert_with(|| Prepared { objects: vec![] })
            .objects
            .extend(prepared);
        self.initial = self.state.clone();
        Ok(())
    }
    pub fn disable_dynamic(&mut self, original: u32) -> Result<(), String> {
        let wake = self
            .opening
            .as_ref()
            .is_some_and(|p| p.assets.rigid_interactions.is_some());
        let p = self.dynamic.as_ref().ok_or("No dynamic actors")?;
        let i = p
            .objects
            .iter()
            .position(|p| p.original == original)
            .ok_or("Unknown dynamic actor")?;
        self.state
            .dynamic
            .as_mut()
            .ok_or("Missing dynamic actor state")?[i] = true;
        if let Some(bodies) = self.state.opening.as_mut().and_then(|o| o.rigid.as_mut()) {
            if let Some(body) = bodies.iter_mut().find(|b| b.original == original) {
                body.active = true;
                if wake {
                    body.wake();
                }
            }
        }
        self.sync_dynamic();
        Ok(())
    }
    pub(crate) fn sync_dynamic(&mut self) {
        if let (Some(p), Some(state)) = (&self.dynamic, &self.state.dynamic) {
            for (p, disabled) in p.objects.iter().zip(state) {
                self.world.actors[p.actor].shape = if let Some(h) = self.rigid_collider(p.original)
                {
                    BodyShape::Hulls(Arc::new(h))
                } else if *disabled {
                    p.released_hulls
                        .as_ref()
                        .map_or(BodyShape::Clear, |h| BodyShape::Hulls(h.clone()))
                } else {
                    BodyShape::Hulls(p.hulls.clone())
                };
            }
        }
        self.sync_spire_doors();
    }
    pub(crate) fn dynamic_geometry(
        &self,
    ) -> (
        Vec<rc_package::geometry::Triangle>,
        Vec<Option<rc_render::FaceTexture>>,
    ) {
        let mut triangles = vec![];
        let mut faces = vec![];
        if let (Some(p), Some(state)) = (&self.dynamic, &self.state.dynamic) {
            for (p, disabled) in p.objects.iter().zip(state) {
                {
                    let rigid = self.rigid_pose(p.original);
                    let (geometry, appearance) = if *disabled && rigid.is_none() {
                        (&p.released_triangles, &p.released_appearance)
                    } else {
                        (&p.triangles, &p.appearance)
                    };
                    triangles.extend(geometry.iter().map(|t| rc_package::geometry::Triangle {
                        points: if let Some((b, s)) = rigid {
                            t.points
                                .map(|v| s.point(b, v.map(f64::from)).map(|v| v as f32))
                        } else if !*disabled {
                            self.spire_door_offset(p.original)
                                .map_or(t.points, |delta| {
                                    t.points.map(|v| std::array::from_fn(|i| v[i] + delta[i]))
                                })
                        } else {
                            t.points
                        },
                        surface: t.surface,
                    }));
                    faces.extend(appearance);
                }
            }
        }
        if let (Some(p), Some(s)) = (
            &self.pickup,
            self.state
                .mission
                .as_ref()
                .and_then(|m| m.grenades.as_ref()),
        ) {
            if s.flow.respawn_remaining <= 0. {
                triangles.extend(p.triangles.iter().map(|t| rc_package::geometry::Triangle {
                    points: t.points,
                    surface: t.surface,
                }));
                faces.extend(&p.appearance);
            }
        }
        (triangles, faces)
    }
}
