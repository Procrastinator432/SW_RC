//! Versioned self-contained first-scene bundle; contains locally derived game assets.
use crate::{Game, Point};
use rc_package::{
    collision::{SolidBsp, SolidNode},
    controller::{ControllerOptions, MovementRatios},
    geometry::{Bsp, Node},
    hull::{Hull, HullSet},
    mesh_query::ActorTransform,
    movement_profile::MovementProfile,
    physics::PhysicsOptions,
    volumes::{PhysicsVolume, VolumeSettings, VolumeShape, VolumeWorld},
    world_collision::{BodyActor, BodyShape, StaticBodyWorld},
};
use serde::{Deserialize, Serialize};
use std::{collections::BTreeMap, sync::Arc};
const MAGIC: &[u8; 8] = b"RCPLAY01";
#[derive(Serialize, Deserialize)]
pub struct HullData {
    pub offset: usize,
    pub planes: Vec<[f64; 4]>,
    pub bounds: [Point; 2],
}
#[derive(Serialize, Deserialize)]
pub struct Collider {
    pub name: String,
    pub hulls: Vec<HullData>,
}
#[derive(Serialize, Deserialize)]
pub struct Movement {
    pub speed: f64,
    pub acceleration: f64,
    pub braking: f64,
    pub air_control: f64,
    pub jump_speed: f64,
    pub ratios: [f64; 3],
    pub extent: Point,
    pub gravity: f64,
    pub terminal_speed: f64,
    pub skin: f64,
    pub support_distance: f64,
    pub step_height: f64,
    pub minimum_up: f64,
}
#[derive(Serialize, Deserialize)]
pub struct Settings {
    pub gravity: [f32; 3],
    pub terminal_speed: f32,
    pub ground_friction: f32,
    pub water: bool,
    pub zone_velocity: [f32; 3],
}
#[derive(Serialize, Deserialize)]
pub struct VolumeNode {
    pub plane: [f32; 4],
    pub front: i32,
    pub back: i32,
    pub flags: u8,
    pub vertex_count: u8,
}
#[derive(Serialize, Deserialize)]
pub struct Transform {
    pub location: [f32; 3],
    pub rotation: [i32; 3],
    pub scale: [f32; 3],
    pub pivot: [f32; 3],
}
#[derive(Serialize, Deserialize)]
pub struct Volume {
    pub name: String,
    pub priority: i32,
    pub settings: Settings,
    pub nodes: Option<Vec<VolumeNode>>,
    pub root_outside: bool,
    pub transform: Transform,
}
#[derive(Serialize, Deserialize)]
pub struct Manifest {
    pub name: String,
    pub provenance: serde_json::Value,
    pub movement: Movement,
    /// First collider is the Level-referenced world; following ones are blocking static actors.
    pub colliders: Vec<Collider>,
    pub default_volume: Settings,
    pub volumes: Vec<Volume>,
    pub spawn: Point,
    pub yaw: f64,
    pub enemies: Vec<Point>,
    pub exit: Point,
}
impl Settings {
    fn from_runtime(s: &VolumeSettings) -> Self {
        Self {
            gravity: s.gravity,
            terminal_speed: s.terminal_speed,
            ground_friction: s.ground_friction,
            water: s.water,
            zone_velocity: s.zone_velocity,
        }
    }
    fn runtime(&self) -> Result<VolumeSettings, String> {
        if self
            .gravity
            .iter()
            .chain(&self.zone_velocity)
            .any(|v| !v.is_finite())
            || !self.terminal_speed.is_finite()
            || self.terminal_speed <= 0.
            || !self.ground_friction.is_finite()
            || self.ground_friction < 0.
        {
            return Err("Invalid volume settings in game bundle".into());
        }
        Ok(VolumeSettings {
            gravity: self.gravity,
            terminal_speed: self.terminal_speed,
            ground_friction: self.ground_friction,
            water: self.water,
            zone_velocity: self.zone_velocity,
        })
    }
}
impl Collider {
    fn capture(name: String, h: &HullSet) -> Result<Self, String> {
        if h.missing_solid_leaves != 0 || h.empty_root_solid {
            return Err(format!("Incomplete collider: {name}"));
        }
        Ok(Self {
            name,
            hulls: h
                .hulls
                .iter()
                .map(|h| HullData {
                    offset: h.offset,
                    planes: h.planes.clone(),
                    bounds: h.bounds,
                })
                .collect(),
        })
    }
    pub(crate) fn runtime(self) -> Result<Arc<HullSet>, String> {
        if self.hulls.len() > 100_000 {
            return Err("Too many collision hulls".into());
        }
        if self.hulls.iter().any(|h| {
            h.planes.len() > 70
                || h.planes
                    .iter()
                    .flatten()
                    .chain(h.bounds.iter().flatten())
                    .any(|v| !v.is_finite() || v.abs() > 1e9)
        }) {
            return Err("Invalid collision coordinates in game bundle".into());
        }
        let hulls = self
            .hulls
            .into_iter()
            .map(|h| Hull {
                offset: h.offset,
                planes: h.planes,
                bounds: h.bounds,
            })
            .collect();
        let result = HullSet {
            hulls,
            missing_solid_leaves: 0,
            empty_root_solid: false,
        };
        // Validation occurs even for bounds outside the spawn region.
        result.sweep([0.; 3], [0.; 3], [0.; 3])?;
        Ok(Arc::new(result))
    }
}
impl Manifest {
    pub fn capture(game: &Game, provenance: serde_json::Value) -> Result<Self, String> {
        let p = &game.profile;
        let movement = Movement {
            speed: p.controller.speed,
            acceleration: p.controller.acceleration,
            braking: p.controller.braking,
            air_control: p.controller.air_control,
            jump_speed: p.controller.jump_speed,
            ratios: [p.ratios.walk, p.ratios.back, p.ratios.side],
            extent: p.physics.extent,
            gravity: p.physics.gravity,
            terminal_speed: p.physics.terminal_speed,
            skin: p.physics.skin,
            support_distance: p.physics.support_distance,
            step_height: p.physics.step_height,
            minimum_up: p.physics.minimum_up,
        };
        let mut colliders = vec![Collider::capture(
            "World.BSP".into(),
            game.world.world.as_ref().map_err(Clone::clone)?,
        )?];
        for actor in &game.world.actors {
            match &actor.shape {
                BodyShape::Hulls(h) => colliders.push(Collider::capture(actor.name.clone(), h)?),
                BodyShape::Clear => {}
                BodyShape::Unsupported(e) => return Err(format!("{}: {e}", actor.name)),
            }
        }
        let mut volumes = vec![];
        for v in &game.volumes.volumes {
            let (nodes, root_outside, transform) = match &v.shape {
                VolumeShape::Empty => (
                    None,
                    true,
                    Transform {
                        location: [0.; 3],
                        rotation: [0; 3],
                        scale: [1.; 3],
                        pivot: [0.; 3],
                    },
                ),
                VolumeShape::Unsupported(e) => return Err(format!("{}: {e}", v.name)),
                VolumeShape::Brush {
                    bsp,
                    solid,
                    transform,
                } => (
                    Some(
                        bsp.nodes
                            .iter()
                            .map(|n| VolumeNode {
                                plane: n.plane,
                                front: n.front,
                                back: n.back,
                                flags: n.flags,
                                vertex_count: n.vertex_count,
                            })
                            .collect(),
                    ),
                    solid.root_outside,
                    Transform {
                        location: transform.location,
                        rotation: transform.rotation,
                        scale: transform.scale,
                        pivot: transform.pivot,
                    },
                ),
            };
            volumes.push(Volume {
                name: v.name.clone(),
                priority: v.priority,
                settings: Settings::from_runtime(&v.settings),
                nodes,
                root_outside,
                transform,
            });
        }
        Ok(Self {
            name: "geo_01a".into(),
            provenance,
            movement,
            colliders,
            default_volume: Settings::from_runtime(&game.volumes.default),
            volumes,
            spawn: game.initial.pawn.body.position,
            yaw: game.initial.yaw,
            enemies: game.initial.enemies.iter().map(|e| e.position).collect(),
            exit: game.exit,
        })
    }
    pub fn into_game(self, scene: rc_render::Scene) -> Result<Game, String> {
        let m = self.movement;
        if [
            m.speed,
            m.acceleration,
            m.braking,
            m.jump_speed,
            m.gravity,
            m.terminal_speed,
            m.skin,
            m.support_distance,
            m.step_height,
            m.minimum_up,
        ]
        .iter()
        .any(|v| !v.is_finite() || *v < 0. || *v > 1e6)
            || m.extent
                .iter()
                .any(|v| !v.is_finite() || *v <= 0. || *v > 10000.)
            || m.ratios
                .iter()
                .chain([m.air_control, m.minimum_up].iter())
                .any(|v| !v.is_finite() || !(0.0..=1.0).contains(v))
            || m.speed == 0.
            || m.jump_speed == 0.
            || self.colliders.is_empty()
            || self.colliders.len() > 10000
            || self.volumes.len() > 1024
        {
            return Err("Invalid movement/collider manifest".into());
        }
        let profile = MovementProfile {
            controller: ControllerOptions {
                speed: m.speed,
                acceleration: m.acceleration,
                braking: m.braking,
                air_control: m.air_control,
                jump_speed: m.jump_speed,
            },
            ratios: MovementRatios {
                walk: m.ratios[0],
                back: m.ratios[1],
                side: m.ratios[2],
            },
            physics: PhysicsOptions {
                extent: m.extent,
                gravity: m.gravity,
                terminal_speed: m.terminal_speed,
                skin: m.skin,
                support_distance: m.support_distance,
                step_height: m.step_height,
                minimum_up: m.minimum_up,
                max_iterations: 8,
            },
            properties: BTreeMap::new(),
        };
        let mut colliders = self.colliders.into_iter();
        let mut world = StaticBodyWorld {
            world: Ok(colliders.next().unwrap().runtime()?),
            actors: vec![],
        };
        for collider in colliders {
            let name = collider.name.clone();
            world.actors.push(BodyActor {
                name,
                shape: BodyShape::Hulls(collider.runtime()?),
            });
        }
        let mut volumes = VolumeWorld {
            default: self.default_volume.runtime()?,
            volumes: vec![],
        };
        for v in self.volumes {
            let shape = if let Some(nodes) = v.nodes {
                if nodes.len() > 100_000
                    || v.transform.scale.contains(&0.)
                    || v.transform
                        .location
                        .iter()
                        .chain(&v.transform.scale)
                        .chain(&v.transform.pivot)
                        .any(|x| !x.is_finite())
                {
                    return Err("Invalid volume transform/nodes".into());
                }
                let solid = SolidBsp {
                    nodes: nodes
                        .iter()
                        .map(|n| SolidNode {
                            plane: n.plane,
                            back: n.back,
                            front: n.front,
                            flags: n.flags,
                        })
                        .collect(),
                    root_outside: v.root_outside,
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
                solid.validate()?;
                let bsp = Bsp {
                    nodes: nodes
                        .into_iter()
                        .map(|n| Node {
                            plane: n.plane,
                            back: n.back,
                            front: n.front,
                            flags: n.flags,
                            vertex_count: n.vertex_count,
                            vertex_start: 0,
                            surface: 0,
                            coplanar: -1,
                            collision_bound: -1,
                        })
                        .collect(),
                    vectors: vec![],
                    points: vec![],
                    surfaces: vec![],
                    vertices: vec![],
                    geometry_bytes: 0,
                    unparsed_tail_bytes: 0,
                };
                VolumeShape::Brush {
                    bsp: Box::new(bsp),
                    solid,
                    transform: ActorTransform {
                        location: v.transform.location,
                        rotation: v.transform.rotation,
                        scale: v.transform.scale,
                        pivot: v.transform.pivot,
                    },
                }
            } else {
                VolumeShape::Empty
            };
            volumes.volumes.push(PhysicsVolume {
                name: v.name,
                priority: v.priority,
                settings: v.settings.runtime()?,
                shape,
            });
        }
        let mut game = Game::new(scene, world, volumes, profile, self.spawn, self.yaw)?;
        if self.enemies.is_empty() {
            if self.exit.iter().any(|v| !v.is_finite() || v.abs() > 1e8) {
                return Err("Invalid empty encounter exit".into());
            }
            game.exit = self.exit;
        } else {
            game.set_encounter(&self.enemies, self.exit)?;
        }
        Ok(game)
    }
}
pub fn encode(manifest: &Manifest, scene: &rc_render::Scene) -> Result<Vec<u8>, String> {
    let metadata = serde_json::to_vec(manifest).map_err(|e| e.to_string())?;
    let snapshot = scene.snapshot()?;
    if metadata.len() > 32 * 1024 * 1024 || snapshot.len() > 64 * 1024 * 1024 {
        return Err("Game bundle exceeds size budget".into());
    }
    let mut bytes = MAGIC.to_vec();
    bytes.extend_from_slice(&(metadata.len() as u32).to_le_bytes());
    bytes.extend_from_slice(&metadata);
    bytes.extend_from_slice(&snapshot);
    Ok(bytes)
}
pub fn decode(bytes: &[u8]) -> Result<Game, String> {
    if bytes.starts_with(b"RCPLAY04") {
        if bytes.len() < 16 || bytes.len() > 96 * 1024 * 1024 {
            return Err("Invalid mission bundle size".into());
        }
        let size = u32::from_le_bytes(bytes[8..12].try_into().unwrap()) as usize;
        if size > 24 * 1024 * 1024 {
            return Err("Mission metadata exceeds budget".into());
        }
        let assets = serde_json::from_slice(
            bytes
                .get(12..12 + size)
                .ok_or("Truncated mission metadata")?,
        )
        .map_err(|e| e.to_string())?;
        let base = bytes.get(12 + size..).ok_or("Missing mission base")?;
        if !base.starts_with(b"RCPLAY02") {
            return Err("Mission requires one visual-only base".into());
        }
        let mut game = decode(base)?;
        game.attach_mission(assets)?;
        return Ok(game);
    }
    if bytes.starts_with(b"RCPLAY03") {
        if bytes.len() < 16 || bytes.len() > 96 * 1024 * 1024 {
            return Err("Invalid combat bundle size".into());
        }
        let size = u32::from_le_bytes(bytes[8..12].try_into().unwrap()) as usize;
        if size > 16 * 1024 * 1024 {
            return Err("Combat assets exceed budget".into());
        }
        let assets =
            serde_json::from_slice(bytes.get(12..12 + size).ok_or("Truncated combat assets")?)
                .map_err(|e| e.to_string())?;
        let base = bytes.get(12 + size..).ok_or("Missing animated base")?;
        if !base.starts_with(b"RCPLAY02") {
            return Err("Combat bundle requires one RCPLAY02 base".into());
        }
        let mut game = decode(base)?;
        game.attach_combat(assets)?;
        return Ok(game);
    }
    if bytes.starts_with(b"RCPLAY02") {
        if bytes.len() < 16 || bytes.len() > 96 * 1024 * 1024 {
            return Err("Invalid animated bundle size".into());
        }
        let size = u32::from_le_bytes(bytes[8..12].try_into().unwrap()) as usize;
        if size > 16 * 1024 * 1024 {
            return Err("Animated assets exceed budget".into());
        }
        let assets = serde_json::from_slice(
            bytes
                .get(12..12 + size)
                .ok_or("Truncated animated assets")?,
        )
        .map_err(|e| e.to_string())?;
        let base = bytes.get(12 + size..).ok_or("Missing base scene")?;
        if !base.starts_with(MAGIC) {
            return Err("Animated bundle must contain one RCPLAY01 base".into());
        }
        let mut game = decode(base)?;
        game.attach_visuals(assets)?;
        return Ok(game);
    }
    if bytes.len() < 12 || bytes.len() > 96 * 1024 * 1024 || &bytes[..8] != MAGIC {
        return Err("Unsupported game bundle".into());
    }
    let size = u32::from_le_bytes(bytes[8..12].try_into().unwrap()) as usize;
    if size > 32 * 1024 * 1024 {
        return Err("Game metadata exceeds budget".into());
    }
    let metadata = bytes.get(12..12 + size).ok_or("Truncated game metadata")?;
    let manifest: Manifest = serde_json::from_slice(metadata).map_err(|e| e.to_string())?;
    let scene = rc_render::Scene::from_snapshot(&bytes[12 + size..])?;
    manifest.into_game(scene)
}

/// RCPLAY02 wraps an unchanged RCPLAY01 scene with bounded compressed visual assets.
pub fn encode_animated(base: &[u8], assets: &crate::visual::Assets) -> Result<Vec<u8>, String> {
    if !base.starts_with(MAGIC) {
        return Err("Expected legacy base scene".into());
    }
    let data = serde_json::to_vec(assets).map_err(|e| e.to_string())?;
    if data.len() > 16 * 1024 * 1024 || data.len() + base.len() + 12 > 96 * 1024 * 1024 {
        return Err("Animated bundle exceeds budget".into());
    }
    let mut out = b"RCPLAY02".to_vec();
    out.extend_from_slice(&(data.len() as u32).to_le_bytes());
    out.extend(data);
    out.extend(base);
    Ok(out)
}

/// Combat assets wrap an unchanged animated scene; older scenes retain their original rules.
pub fn encode_combat(base: &[u8], assets: &crate::combat::Assets) -> Result<Vec<u8>, String> {
    assets.validate()?;
    if !base.starts_with(b"RCPLAY02") {
        return Err("Expected animated base scene".into());
    }
    let data = serde_json::to_vec(assets).map_err(|e| e.to_string())?;
    if data.len() > 16 * 1024 * 1024 || data.len() + base.len() + 12 > 96 * 1024 * 1024 {
        return Err("Combat bundle exceeds budget".into());
    }
    let mut out = b"RCPLAY03".to_vec();
    out.extend_from_slice(&(data.len() as u32).to_le_bytes());
    out.extend(data);
    out.extend(base);
    Ok(out)
}
