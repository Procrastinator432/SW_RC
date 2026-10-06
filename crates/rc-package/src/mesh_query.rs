//! Boolean zero-extent mesh probes; no hit records, cylinder or extent sweeps.
use crate::{collision::SolidBsp, mesh::transform_point, mesh_collision::MeshCollision};
use serde::Serialize;

#[derive(Debug, PartialEq, Eq, Serialize)]
pub enum LineRoute {
    Cylinder,
    SimpleModel,
    ComplexTree,
    Clear,
}
impl MeshCollision {
    /// Same native LineCheck dispatch for a nonzero extent, using the Box flag.
    pub fn box_route(&self, cylinder: bool, trace_flags: u32, tree_loaded: bool) -> LineRoute {
        if cylinder {
            LineRoute::Cylinder
        } else if self.simple_model != 0 && self.use_simple_box && trace_flags & 0x100 == 0 {
            LineRoute::SimpleModel
        } else if tree_loaded && (!self.use_simple_box || trace_flags & 0x100 != 0) {
            LineRoute::ComplexTree
        } else {
            LineRoute::Clear
        }
    }
    /// UStaticMesh::LineCheck zero-extent dispatch (engine.dll 0x10530500).
    /// `tree_loaded` describes runtime data, not merely serialized data.
    pub fn line_route(&self, cylinder: bool, trace_flags: u32, tree_loaded: bool) -> LineRoute {
        if cylinder {
            LineRoute::Cylinder
        } else if self.simple_model != 0 && self.use_simple_line && trace_flags & 0x100 == 0 {
            LineRoute::SimpleModel
        } else if tree_loaded && (!self.use_simple_line || trace_flags & 0x100 != 0) {
            LineRoute::ComplexTree
        } else {
            LineRoute::Clear
        }
    }
}

#[derive(Clone, Debug)]
pub struct ActorTransform {
    pub location: [f32; 3],
    pub rotation: [i32; 3],
    pub scale: [f32; 3],
    pub pivot: [f32; 3],
}
impl ActorTransform {
    pub fn local_point(&self, world: [f32; 3]) -> Result<[f32; 3], String> {
        if world
            .into_iter()
            .chain(self.location)
            .chain(self.scale)
            .chain(self.pivot)
            .any(|v| !v.is_finite())
            || self.scale.contains(&0.0)
        {
            return Err("invalid or singular actor transform".into());
        }
        let point = std::array::from_fn(|i| {
            let mut basis = [0.0; 3];
            basis[i] = 1.0;
            let axis = transform_point(basis, [0.0; 3], self.rotation, [1.0; 3], [0.0; 3]);
            self.pivot[i]
                + (0..3)
                    .map(|j| (world[j] - self.location[j]) * axis[j])
                    .sum::<f32>()
                    / self.scale[i]
        });
        if point.iter().any(|v: &f32| !v.is_finite()) {
            return Err("actor inverse transform overflow".into());
        }
        Ok(point)
    }
}

#[derive(Clone, Copy, Debug)]
pub struct ActorBlocking {
    pub collide_actors: bool,
    pub block_actors: bool,
    pub block_players: bool,
    pub cylinder: bool,
}
/// A diagnostic player line: caller supplies the selected local Model if available.
/// Unsupported selected shapes return an error instead of pretending to be clear.
pub fn player_line_clear(
    mesh: &MeshCollision,
    simple: Option<&SolidBsp>,
    transform: &ActorTransform,
    blocking: ActorBlocking,
    start: [f32; 3],
    end: [f32; 3],
) -> Result<bool, String> {
    if start.into_iter().chain(end).any(|v| !v.is_finite()) {
        return Err("invalid mesh query endpoint".into());
    }
    if !blocking.collide_actors || !blocking.block_players {
        return Ok(true);
    }
    // Stored complex data is never executed here; conservatively reject this
    // selection even if a native non-editor loader might have discarded it.
    match mesh.line_route(blocking.cylinder, 0, !mesh.nodes.is_empty()) {
        LineRoute::Clear => Ok(true),
        LineRoute::SimpleModel => simple
            .ok_or("selected simple Model unavailable")?
            .line_clear(transform.local_point(start)?, transform.local_point(end)?),
        LineRoute::Cylinder => Err("selected cylinder query unsupported".into()),
        LineRoute::ComplexTree => Err("selected complex tree query unsupported".into()),
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::collision::SolidNode;
    fn mesh() -> MeshCollision {
        MeshCollision {
            simple_model: 1,
            simple_model_path: "Model".into(),
            serialized_shadow_only: Some(1),
            use_simple_line: true,
            use_simple_box: true,
            use_simple_karma: true,
            triangles: vec![],
            nodes: vec![],
            bounds_scale: [1.0; 3],
            decoded_bytes: 0,
            remaining_bytes: 0,
        }
    }
    #[test]
    fn native_dispatch_matrix_has_no_implicit_complex_fallback() {
        for model in [0, 1] {
            for simple in [false, true] {
                for cylinder in [false, true] {
                    for force in [0, 0x100] {
                        for loaded in [false, true] {
                            let mut m = mesh();
                            m.simple_model = model;
                            m.use_simple_line = simple;
                            let expected = if cylinder {
                                LineRoute::Cylinder
                            } else if model != 0 && simple && force == 0 {
                                LineRoute::SimpleModel
                            } else if loaded && (!simple || force != 0) {
                                LineRoute::ComplexTree
                            } else {
                                LineRoute::Clear
                            };
                            assert_eq!(m.line_route(cylinder, force, loaded), expected);
                        }
                    }
                }
            }
        }
        let mut m = mesh();
        m.use_simple_line = false;
        assert_eq!(m.line_route(false, 0, true), LineRoute::ComplexTree);
        assert_eq!(m.box_route(false, 0, true), LineRoute::SimpleModel);
        m.use_simple_line = true;
        m.use_simple_box = false;
        assert_eq!(m.line_route(false, 0, true), LineRoute::SimpleModel);
        assert_eq!(m.box_route(false, 0, true), LineRoute::ComplexTree);
        assert_eq!(m.box_route(true, 0, true), LineRoute::Cylinder);
        m.simple_model = 0;
        m.use_simple_box = true;
        assert_eq!(m.box_route(false, 0, true), LineRoute::Clear);
    }
    #[test]
    fn inverse_actor_transform_handles_pivot_negative_scale_and_quantized_rotators() {
        let mut t = ActorTransform {
            location: [100.0, -20.0, 3.0],
            rotation: [12345, -9815, 6789],
            scale: [2.0, -3.0, 0.25],
            pivot: [4.0, 5.0, 6.0],
        };
        let point = [-2.0, 3.0, 10.0];
        let world = transform_point(point, t.location, t.rotation, t.scale, t.pivot);
        for (a, b) in t.local_point(world).unwrap().into_iter().zip(point) {
            assert!((a - b).abs() < 0.001);
        }
        t.scale[0] = 0.0;
        assert!(t.local_point(world).is_err());
        t.scale[0] = f32::NAN;
        assert!(t.local_point(world).is_err());
    }
    #[test]
    fn transformed_simple_solid_and_player_flags() {
        let solid = SolidBsp {
            nodes: vec![SolidNode {
                plane: [1.0, 0.0, 0.0, 0.0],
                back: -1,
                front: -1,
                flags: 0,
            }],
            root_outside: true,
            linked: false,
            zones: 0,
            bounds: 0,
            hull_indices: 0,
            hull_words: vec![],
            leaves: 0,
            lights: 0,
            decoded_bytes: 0,
            remaining_bytes: 0,
        };
        let t = ActorTransform {
            location: [10.0, 20.0, 0.0],
            rotation: [0, 16384, 0],
            scale: [2.0, 1.0, 1.0],
            pivot: [1.0, 0.0, 0.0],
        };
        let mut flags = ActorBlocking {
            collide_actors: true,
            block_actors: false,
            block_players: true,
            cylinder: false,
        };
        assert!(!player_line_clear(
            &mesh(),
            Some(&solid),
            &t,
            flags,
            [10.0, 20.0, 0.0],
            [10.0, 16.0, 0.0]
        )
        .unwrap());
        assert!(player_line_clear(
            &mesh(),
            Some(&solid),
            &t,
            flags,
            [10.0, 20.0, 0.0],
            [10.0, 24.0, 0.0]
        )
        .unwrap());
        assert!(player_line_clear(&mesh(), None, &t, flags, [0.0; 3], [1.0; 3]).is_err());
        flags.block_players = false;
        assert!(player_line_clear(&mesh(), None, &t, flags, [0.0; 3], [1.0; 3]).unwrap());
        flags.block_players = true;
        flags.cylinder = true;
        assert!(player_line_clear(&mesh(), Some(&solid), &t, flags, [0.0; 3], [1.0; 3]).is_err());
        flags.cylinder = false;
        let mut complex = mesh();
        complex.use_simple_line = false;
        complex.nodes.push(crate::mesh_collision::CollisionNode {
            triangle: 0,
            children: [u16::MAX; 3],
            compressed_bounds: [[0; 3]; 2],
        });
        assert!(player_line_clear(&complex, Some(&solid), &t, flags, [0.0; 3], [1.0; 3]).is_err());
    }
}
