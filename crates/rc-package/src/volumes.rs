//! Static actor-center volume diagnostic. Runtime hash/touch order and callbacks absent.
use crate::{collision::SolidBsp, geometry::Bsp, mesh_query::ActorTransform};
use serde::Serialize;
#[derive(Debug, Clone, Serialize)]
pub struct VolumeSettings {
    pub gravity: [f32; 3],
    pub terminal_speed: f32,
    pub ground_friction: f32,
    pub water: bool,
    pub zone_velocity: [f32; 3],
}
impl VolumeSettings {
    pub fn apply(
        &self,
        mut physics: crate::physics::PhysicsOptions,
    ) -> Result<crate::physics::PhysicsOptions, String> {
        if self.water
            || self.zone_velocity != [0.0; 3]
            || self.gravity[0] != 0.0
            || self.gravity[1] != 0.0
            || self.gravity[2] > 0.0
            || self.gravity.iter().any(|v| !v.is_finite())
            || !self.terminal_speed.is_finite()
            || self.terminal_speed <= 0.0
        {
            return Err(
                "selected volume requires unsupported water, zone velocity or gravity physics"
                    .into(),
            );
        }
        physics.gravity = -f64::from(self.gravity[2]);
        physics.terminal_speed = f64::from(self.terminal_speed);
        Ok(physics)
    }
}
pub enum VolumeShape {
    Empty,
    Brush {
        bsp: Box<Bsp>,
        solid: SolidBsp,
        transform: ActorTransform,
    },
    Unsupported(String),
}
pub struct PhysicsVolume {
    pub name: String,
    pub priority: i32,
    pub settings: VolumeSettings,
    pub shape: VolumeShape,
}
pub struct VolumeWorld {
    pub default: VolumeSettings,
    pub volumes: Vec<PhysicsVolume>,
}
#[derive(Debug, Serialize)]
pub struct VolumeSelection {
    pub object: String,
    pub settings: VolumeSettings,
    pub priority: i32,
    pub complete: bool,
    pub contained: Vec<String>,
    pub priority_ties: Vec<String>,
    pub errors: Vec<String>,
}
impl VolumeShape {
    pub fn contains(&self, point: [f32; 3]) -> Result<bool, String> {
        if point.iter().any(|v| !v.is_finite() || v.abs() > 1e9) {
            return Err("invalid volume point".into());
        }
        match self {
            Self::Empty => Ok(false),
            Self::Unsupported(e) => Err(e.clone()),
            Self::Brush {
                bsp,
                solid,
                transform,
            } => {
                solid.validate()?;
                if bsp.nodes.len() != solid.nodes.len() {
                    return Err("volume node mismatch".into());
                }
                let local = transform.local_point(point)?;
                let mut outside = solid.root_outside;
                let mut index = if bsp.nodes.is_empty() { -1 } else { 0 };
                for _ in 0..=bsp.nodes.len() {
                    if index == -1 {
                        return Ok(!outside);
                    }
                    let n = bsp
                        .nodes
                        .get(usize::try_from(index).map_err(|_| "invalid volume child")?)
                        .ok_or("invalid volume child")?;
                    let distance =
                        local[0] * n.plane[0] + local[1] * n.plane[1] + local[2] * n.plane[2]
                            - n.plane[3];
                    if !distance.is_finite() {
                        return Err("volume plane arithmetic overflow".into());
                    }
                    // Native zero-extent PointCheck chooses back at exact plane equality.
                    let front = distance > 0.0;
                    let csg = n.vertex_count != 0 && (n.flags & 0x1f) & 0x21 == 0;
                    if csg {
                        outside = front;
                    }
                    index = if front { n.front } else { n.back };
                }
                Err("volume BSP cycle".into())
            }
        }
    }
}
impl VolumeWorld {
    /// Recompute the static center selection before a tick. Callbacks and crossing substeps absent.
    pub fn control_tick(
        &self,
        world: &crate::world_collision::StaticBodyWorld,
        state: &crate::controller::ControlledBody,
        input: crate::controller::MovementInput,
        dt: f64,
        controller: crate::controller::ControllerOptions,
        physics: crate::physics::PhysicsOptions,
    ) -> Result<(VolumeSelection, crate::controller::ControllerFrame), String> {
        let selection = self.select(state.body.position.map(|v| v as f32))?;
        if !selection.complete {
            return Err("volume selection incomplete or tied runtime order unknown".into());
        }
        let motion = world.control_tick(
            state,
            input,
            dt,
            controller,
            selection.settings.apply(physics)?,
        )?;
        Ok((selection, motion))
    }
    pub fn select(&self, point: [f32; 3]) -> Result<VolumeSelection, String> {
        if point.iter().any(|v| !v.is_finite() || v.abs() > 1e9) {
            return Err("invalid volume selection point".into());
        }
        let mut result = VolumeSelection {
            object: "DefaultPhysicsVolume".into(),
            settings: self.default.clone(),
            priority: -1_000_000,
            complete: true,
            contained: vec![],
            priority_ties: vec![],
            errors: vec![],
        };
        let mut selected_volume = false;
        for volume in &self.volumes {
            match volume.shape.contains(point) {
                Err(e) => result.errors.push(format!("{}: {e}", volume.name)),
                Ok(false) => {}
                Ok(true) => {
                    result.contained.push(volume.name.clone());
                    if volume.priority > result.priority {
                        result.object = volume.name.clone();
                        result.priority = volume.priority;
                        result.settings = volume.settings.clone();
                        result.priority_ties.clear();
                        selected_volume = true;
                    } else if selected_volume && volume.priority == result.priority {
                        result.priority_ties.push(volume.name.clone());
                    }
                }
            }
        }
        // Stored export order does not prove the engine's runtime hash/touch traversal order.
        result.complete = result.errors.is_empty() && result.priority_ties.is_empty();
        Ok(result)
    }
}
#[cfg(test)]
mod tests {
    use super::*;
    fn settings(g: f32) -> VolumeSettings {
        VolumeSettings {
            gravity: [0.0, 0.0, g],
            terminal_speed: 1000.0,
            ground_friction: 8.0,
            water: false,
            zone_velocity: [0.0; 3],
        }
    }
    fn full() -> VolumeShape {
        VolumeShape::Brush {
            bsp: Box::new(Bsp {
                vectors: vec![],
                points: vec![],
                nodes: vec![],
                surfaces: vec![],
                vertices: vec![],
                geometry_bytes: 0,
                unparsed_tail_bytes: 0,
            }),
            solid: SolidBsp {
                nodes: vec![],
                root_outside: false,
                linked: false,
                zones: 0,
                bounds: 0,
                hull_indices: 0,
                hull_words: vec![],
                leaves: 0,
                lights: 0,
                decoded_bytes: 0,
                remaining_bytes: 0,
            },
            transform: ActorTransform {
                location: [0.0; 3],
                rotation: [0; 3],
                scale: [1.0; 3],
                pivot: [0.0; 3],
            },
        }
    }
    #[test]
    fn priority_default_ties_and_errors_are_not_silently_accepted() {
        let mut w = VolumeWorld {
            default: settings(-100.0),
            volumes: vec![],
        };
        assert_eq!(w.select([0.0; 3]).unwrap().priority, -1_000_000);
        w.volumes.push(PhysicsVolume {
            name: "default tie".into(),
            priority: -1_000_000,
            settings: settings(-200.0),
            shape: full(),
        });
        let unchanged = w.select([0.0; 3]).unwrap();
        assert!(unchanged.complete);
        assert_eq!(unchanged.object, "DefaultPhysicsVolume");
        w.volumes.clear();
        for (name, priority) in [("low", 1), ("high", 2), ("tie", 2)] {
            w.volumes.push(PhysicsVolume {
                name: name.into(),
                priority,
                settings: settings(-200.0),
                shape: full(),
            });
        }
        let r = w.select([0.0; 3]).unwrap();
        assert_eq!(r.object, "high");
        assert!(!r.complete);
        assert_eq!(r.priority_ties, vec!["tie"]);
        w.volumes[2].priority = 3;
        assert!(w.select([0.0; 3]).unwrap().complete);
        w.volumes.push(PhysicsVolume {
            name: "unknown".into(),
            priority: 0,
            settings: settings(-200.0),
            shape: VolumeShape::Unsupported("missing model".into()),
        });
        assert!(!w.select([0.0; 3]).unwrap().complete);
        assert!(w.select([f32::NAN; 3]).is_err());
    }
    #[test]
    fn center_boundary_csg_and_transformed_brush() {
        let mut shape = full();
        if let VolumeShape::Brush {
            bsp,
            solid,
            transform,
        } = &mut shape
        {
            bsp.nodes.push(crate::geometry::Node {
                plane: [1.0, 0.0, 0.0, 0.0],
                vertex_start: 0,
                surface: 0,
                back: -1,
                front: -1,
                coplanar: -1,
                collision_bound: -1,
                vertex_count: 3,
                flags: 0,
            });
            solid.nodes.push(crate::collision::SolidNode {
                plane: [1.0, 0.0, 0.0, 0.0],
                back: -1,
                front: -1,
                flags: 0,
            });
            solid.root_outside = true;
            transform.location = [10.0, 0.0, 0.0];
            transform.scale = [-2.0, 1.0, 1.0];
        }
        assert!(shape.contains([10.0, 0.0, 0.0]).unwrap());
        assert!(shape.contains([11.0, 0.0, 0.0]).unwrap());
        assert!(!shape.contains([9.0, 0.0, 0.0]).unwrap());
        if let VolumeShape::Brush { bsp, .. } = &mut shape {
            bsp.nodes[0].vertex_count = 0;
        }
        assert!(!shape.contains([11.0, 0.0, 0.0]).unwrap());
    }
    #[test]
    fn crossings_change_gravity_on_the_following_tick() {
        use crate::{
            controller::{ControlledBody, ControllerOptions, MovementInput},
            hull::HullSet,
            physics::{BodyState, PhysicsOptions},
            world_collision::StaticBodyWorld,
        };
        let world = StaticBodyWorld {
            world: Ok(std::sync::Arc::new(HullSet {
                hulls: vec![],
                missing_solid_leaves: 0,
                empty_root_solid: false,
            })),
            actors: vec![],
        };
        let mut shape = full();
        if let VolumeShape::Brush { bsp, solid, .. } = &mut shape {
            bsp.nodes.push(crate::geometry::Node {
                plane: [1.0, 0.0, 0.0, 0.0],
                vertex_start: 0,
                surface: 0,
                back: -1,
                front: -1,
                coplanar: -1,
                collision_bound: -1,
                vertex_count: 3,
                flags: 0,
            });
            solid.nodes.push(crate::collision::SolidNode {
                plane: [1.0, 0.0, 0.0, 0.0],
                back: -1,
                front: -1,
                flags: 0,
            });
            solid.root_outside = true;
        }
        let volumes = VolumeWorld {
            default: settings(-100.0),
            volumes: vec![PhysicsVolume {
                name: "low gravity".into(),
                priority: 1,
                settings: settings(-20.0),
                shape,
            }],
        };
        let s = ControlledBody {
            body: BodyState {
                position: [-0.5, 0.0, 20.0],
                velocity: [30.0, 0.0, 0.0],
                grounded: false,
            },
            jump_held: false,
        };
        let p = PhysicsOptions {
            extent: [1.0; 3],
            gravity: 100.0,
            terminal_speed: 1000.0,
            skin: 0.1,
            support_distance: 0.5,
            minimum_up: 0.7,
            max_iterations: 8,
            step_height: 0.0,
        };
        let c = ControllerOptions {
            speed: 30.0,
            acceleration: 1200.0,
            braking: 100.0,
            air_control: 1.0,
            jump_speed: 20.0,
        };
        let forward = MovementInput {
            direction: [1.0, 0.0],
            jump: false,
        };
        let back = MovementInput {
            direction: [-1.0, 0.0],
            jump: false,
        };
        let (a, f) = volumes
            .control_tick(&world, &s, forward, 0.05, c, p)
            .unwrap();
        assert_eq!(a.object, "low gravity");
        assert!(f.state.body.position[0] > 0.0);
        assert_eq!(f.state.body.velocity[2], -1.0);
        let (b, g) = volumes
            .control_tick(&world, &f.state, back, 0.05, c, p)
            .unwrap();
        assert_eq!(b.object, "DefaultPhysicsVolume");
        assert!(g.state.body.position[0] < 0.0);
        assert_eq!(g.state.body.velocity[2], -6.0);
        let (d, h) = volumes
            .control_tick(&world, &g.state, back, 0.05, c, p)
            .unwrap();
        assert_eq!(d.object, "low gravity");
        assert_eq!(h.state.body.velocity[2], -7.0);
    }
    #[test]
    fn selected_gravity_is_applied_and_unknown_or_water_has_no_candidate() {
        use crate::{
            controller::{ControlledBody, ControllerOptions, MovementInput},
            hull::HullSet,
            physics::{BodyState, PhysicsOptions},
            world_collision::StaticBodyWorld,
        };
        let world = StaticBodyWorld {
            world: Ok(std::sync::Arc::new(HullSet {
                hulls: vec![],
                missing_solid_leaves: 0,
                empty_root_solid: false,
            })),
            actors: vec![],
        };
        let mut volumes = VolumeWorld {
            default: settings(-100.0),
            volumes: vec![PhysicsVolume {
                name: "strong".into(),
                priority: 1,
                settings: settings(-200.0),
                shape: full(),
            }],
        };
        let state = ControlledBody {
            body: BodyState {
                position: [0.0, 0.0, 20.0],
                velocity: [0.0; 3],
                grounded: false,
            },
            jump_held: false,
        };
        let physics = PhysicsOptions {
            extent: [1.0; 3],
            gravity: 100.0,
            terminal_speed: 1000.0,
            skin: 0.1,
            support_distance: 0.5,
            minimum_up: 0.7,
            max_iterations: 8,
            step_height: 0.0,
        };
        let controller = ControllerOptions {
            speed: 10.0,
            acceleration: 20.0,
            braking: 20.0,
            air_control: 0.2,
            jump_speed: 20.0,
        };
        let input = MovementInput {
            direction: [0.0; 2],
            jump: false,
        };
        let (selection, frame) = volumes
            .control_tick(&world, &state, input, 0.05, controller, physics)
            .unwrap();
        assert_eq!(selection.object, "strong");
        assert_eq!(frame.state.body.velocity[2], -10.0);
        volumes.volumes[0].settings.water = true;
        assert!(volumes
            .control_tick(&world, &state, input, 0.05, controller, physics)
            .is_err());
        volumes.volumes[0].shape = VolumeShape::Unsupported("unknown".into());
        assert!(volumes
            .control_tick(&world, &state, input, 0.05, controller, physics)
            .is_err());
        assert_eq!(state.body.velocity, [0.0; 3]);
    }
}
