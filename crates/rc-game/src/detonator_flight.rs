//! Zero-extent grenade flight through the supplied static collision world.
//! Native bouncing arithmetic/budget with mathematical sweeps and an explicit
//! bounded rest adapter; not native MoveActor, water, sliding or actor-touch parity.
use crate::{detonator, Point};
use rc_package::{actor_falling, volumes::VolumeWorld, world_collision::StaticBodyWorld};
use serde::{Deserialize, Serialize};

#[derive(Clone, Debug, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Volume {
    pub name: String,
    pub fluid_friction: f32,
}
#[derive(Clone, Debug, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Assets {
    pub mass: f32,
    pub buoyancy: f32,
    pub volumes: Vec<Volume>,
}
impl Assets {
    pub fn validate(&self) -> Result<(), String> {
        let mut names = std::collections::BTreeSet::new();
        if !self.mass.is_finite()
            || !(0.001..=1e6).contains(&self.mass)
            || !self.buoyancy.is_finite()
            || !(0. ..=1e6).contains(&self.buoyancy)
            || self.volumes.len() > 1024
            || self.volumes.iter().any(|v| {
                v.name.is_empty()
                    || !names.insert(&v.name)
                    || !v.fluid_friction.is_finite()
                    || !(0. ..=1000.).contains(&v.fluid_friction)
            })
            || !names.contains(&"DefaultPhysicsVolume".to_owned())
        {
            return Err("Invalid grenade flight assets".into());
        }
        Ok(())
    }
    pub fn environment(
        &self,
        volumes: &VolumeWorld,
        position: Point,
    ) -> Result<(String, actor_falling::Environment), String> {
        let selection = volumes.select(position.map(|v| v as f32))?;
        if !selection.complete {
            return Err("Incomplete grenade volume selection".into());
        }
        let s = &selection.settings;
        if s.water || s.zone_velocity != [0.; 3] {
            return Err("Grenade water/moving-volume physics is not supported".into());
        }
        let fluid = self
            .volumes
            .iter()
            .find(|v| v.name == selection.object)
            .ok_or("Missing grenade volume properties")?
            .fluid_friction;
        // GetNetBuoyancy(1048dd40), dry branch. The outputs were initialized to
        // zero by physFalling; only unequal vertical gravity writes half values.
        let (buoyancy, fluid_friction) = actor_falling::dry_buoyancy(
            s.gravity[2],
            volumes.default.gravity[2],
            self.buoyancy,
            fluid,
        );
        if s.gravity.iter().any(|v| !v.is_finite()) {
            return Err("Invalid grenade gravity".into());
        }
        Ok((
            selection.object,
            actor_falling::Environment {
                gravity: s.gravity,
                zone_velocity: s.zone_velocity,
                buoyancy,
                mass: self.mass,
                fluid_friction,
            },
        ))
    }
}
#[derive(Clone, Debug, PartialEq, Serialize, serde::Deserialize)]
pub struct State {
    pub projectile: detonator::State,
    pub position: Point,
    pub settled: bool,
}
#[derive(Debug, Serialize)]
pub struct Segment {
    pub from: Point,
    pub to: Point,
}
#[derive(Debug, Serialize)]
pub struct Contact {
    pub object: String,
    pub position: Point,
    pub normal: [f32; 3],
}
#[derive(Debug, Default, Serialize)]
pub struct Frame {
    /// Only actually traversed paths, clipped at the first blocking contact.
    pub segments: Vec<Segment>,
    pub contacts: Vec<Contact>,
    pub effects: Vec<detonator::Effect>,
    pub volumes: Vec<String>,
    pub landed: bool,
}
impl State {
    pub fn new(projectile: detonator::State, position: Point) -> Result<Self, String> {
        if position.iter().any(|v| !v.is_finite() || v.abs() > 1e8) {
            return Err("Invalid grenade flight origin".into());
        }
        Ok(Self {
            projectile,
            position,
            settled: false,
        })
    }
    /// Atomic on incomplete queries. Host policy: script clocks run before this
    /// frame's motion; an expired/detonated projectile cannot move or touch again.
    pub fn tick(
        &mut self,
        rules: &detonator::Assets,
        assets: &Assets,
        world: &StaticBodyWorld,
        volumes: &VolumeWorld,
        dt: f64,
    ) -> Result<Frame, String> {
        rules.validate()?;
        assets.validate()?;
        if !dt.is_finite() || !(0. ..=0.25).contains(&dt) {
            return Err("Invalid grenade flight time step".into());
        }
        let mut next = self.clone();
        let frame = next.move_frame(rules, assets, world, volumes, dt)?;
        *self = next;
        Ok(frame)
    }
    fn move_frame(
        &mut self,
        rules: &detonator::Assets,
        assets: &Assets,
        world: &StaticBodyWorld,
        volumes: &VolumeWorld,
        dt: f64,
    ) -> Result<Frame, String> {
        let mut frame = Frame {
            effects: self.projectile.advance(rules, dt)?,
            ..Frame::default()
        };
        if self.projectile.ended.is_some() || self.settled || dt == 0. {
            return Ok(frame);
        }
        let mut budget = actor_falling::Budget::new(dt as f32, 0);
        while let Some(step) = budget.next_step() {
            let (volume, environment) = assets.environment(volumes, self.position)?;
            if !frame.volumes.contains(&volume) {
                frame.volumes.push(volume);
            }
            let (velocity, displacement) =
                actor_falling::step(self.projectile.velocity, [0.; 3], environment, step);
            if velocity.iter().chain(&displacement).any(|v| !v.is_finite()) {
                return Err("Grenade motion overflow".into());
            }
            self.projectile.velocity = velocity;
            let from = self.position;
            let end = std::array::from_fn(|i| from[i] + f64::from(displacement[i]));
            let sweep = world.sweep_motion(from, end, [0.; 3])?;
            if !sweep.complete {
                return Err("Incomplete grenade world collision".into());
            }
            let Some(contact) = sweep.contacts.into_iter().next() else {
                self.position = end;
                frame.segments.push(Segment { from, to: end });
                continue;
            };
            if contact.hit.start_overlapping {
                return Err(format!("Grenade penetrates {}", contact.object));
            }
            let fraction = contact.hit.fraction;
            let to = std::array::from_fn(|i| from[i] + (end[i] - from[i]) * fraction);
            let length = contact.hit.normal.iter().map(|v| v * v).sum::<f64>().sqrt();
            if length < 1e-9 || !length.is_finite() {
                return Err("Invalid grenade world normal".into());
            }
            let normal = contact.hit.normal.map(|v| (v / length) as f32);
            frame.segments.push(Segment { from, to });
            frame.contacts.push(Contact {
                object: contact.object,
                position: to,
                normal,
            });
            // Own 0.01-unit separation, not a recovered MoveActor backoff. Query
            // paths end at the true contact and never include this correction.
            self.position = std::array::from_fn(|i| to[i] + f64::from(normal[i]) * 0.01);
            if self.projectile.bouncing {
                frame
                    .effects
                    .extend(self.projectile.impact(rules, normal, false, false)?);
                budget.bounce(step, fraction as f32);
            } else if f64::from(normal[2]) >= 0.7 {
                // Selected processLanded dry/zero-ZoneVelocity transition. No
                // script override, base attachment or rotation adjustment.
                self.projectile.velocity = [0.; 3];
                self.settled = true;
                frame.landed = true;
                break;
            } else {
                // Bounded low-speed rest adapter: remove inward velocity at a
                // wall. Native nonbouncing TwoWallAdjust/reconstruction absent.
                let dot = self
                    .projectile
                    .velocity
                    .iter()
                    .zip(normal)
                    .map(|(a, b)| a * b)
                    .sum::<f32>();
                if dot < 0. {
                    self.projectile.velocity =
                        std::array::from_fn(|i| self.projectile.velocity[i] - normal[i] * dot);
                }
            }
        }
        Ok(frame)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use rc_package::{
        hull::{Hull, HullSet},
        volumes::{PhysicsVolume, VolumeSettings, VolumeShape},
        world_collision::{BodyActor, BodyShape},
    };
    use std::sync::Arc;
    fn rules() -> detonator::Assets {
        detonator::Assets {
            speed: 2200.,
            max_speed: 3000.,
            toss_z: 200.,
            fuse_time: 12.,
            post_impact_fuse: 1.,
            life_span: 8.,
            world_damping: 0.3,
            actor_damping: 0.1,
            rest_speed: 20.,
            min_bounce_sound_time: 0.1,
            damage: 400.,
            damage_radius: 600.,
        }
    }
    fn assets() -> Assets {
        Assets {
            mass: 100.,
            buoyancy: 50.,
            volumes: vec![Volume {
                name: "DefaultPhysicsVolume".into(),
                fluid_friction: 0.3,
            }],
        }
    }
    fn volumes() -> VolumeWorld {
        VolumeWorld {
            default: VolumeSettings {
                gravity: [0., 0., -950.],
                terminal_speed: 4000.,
                ground_friction: 8.,
                water: false,
                zone_velocity: [0.; 3],
            },
            volumes: vec![],
        }
    }
    fn world(bounds: Vec<[Point; 2]>) -> StaticBodyWorld {
        StaticBodyWorld {
            world: Ok(Arc::new(HullSet {
                hulls: bounds
                    .into_iter()
                    .enumerate()
                    .map(|(offset, bounds)| Hull {
                        offset,
                        planes: vec![],
                        bounds,
                    })
                    .collect(),
                missing_solid_leaves: 0,
                empty_root_solid: false,
            })),
            actors: vec![],
        }
    }
    fn grenade(position: Point) -> State {
        State::new(
            detonator::State::new(&rules(), [1., 0., 0.], [0.; 3], 100.).unwrap(),
            position,
        )
        .unwrap()
    }
    #[test]
    fn wall_clips_path_and_shortens_fuse_before_reflected_move() {
        let w = world(vec![[[10., -100., -100.], [11., 100., 100.]]]);
        let mut s = grenade([0.; 3]);
        let f = s
            .tick(&rules(), &assets(), &w, &volumes(), 1. / 60.)
            .unwrap();
        assert_eq!(f.contacts.len(), 1);
        assert_eq!(f.contacts[0].position[0], 10.);
        assert!(f.segments.iter().all(|p| p.to[0] <= 10.));
        assert!(s.position[0] < 10. && s.projectile.velocity[0] < 0.);
        assert_eq!(s.projectile.timer_remaining, Some(1.));
    }
    #[test]
    fn gunner_contact_cannot_cross_a_blocking_wall() {
        let target = crate::gunner::Assets {
            contact: crate::crash::Trigger {
                original: 621,
                position: [20., 0., 0.],
                radius: 1.,
                height: 10.,
            },
            destruction_delay: 1.,
            confirmation_delay: 2.,
            confirmation_seconds: 1.,
            after_confirmation_delay: 2.,
            bunker_health: 49000,
            bunker_script_damage: 50000,
        };
        for blocked in [false, true] {
            let w = world(if blocked {
                vec![[[10., -100., -100.], [11., 100., 100.]]]
            } else {
                vec![]
            });
            let mut s = grenade([0.; 3]);
            let frame = s
                .tick(&rules(), &assets(), &w, &volumes(), 1. / 60.)
                .unwrap();
            let mut gunner = crate::gunner::State::default();
            for p in frame.segments {
                gunner
                    .observe(
                        &target,
                        p.from,
                        p.to,
                        crate::gunner::ContactClass::GrenadeProjectile,
                    )
                    .unwrap();
            }
            assert_eq!(gunner.objective_completed, !blocked);
        }
    }
    #[test]
    fn incomplete_sweep_rolls_back_clocks_and_motion() {
        let mut w = world(vec![]);
        w.actors.push(BodyActor {
            name: "missing".into(),
            shape: BodyShape::Unsupported("unknown".into()),
        });
        let mut s = grenade([0.; 3]);
        let before = s.clone();
        assert!(s
            .tick(&rules(), &assets(), &w, &volumes(), 1. / 60.)
            .is_err());
        assert_eq!(s, before);
    }
    #[test]
    fn repeated_floor_bounces_land_then_explode_once_at_rest() {
        let w = world(vec![[[-10000., -10000., -100.], [10000., 10000., 0.]]]);
        let mut s = grenade([0., 0., 5.]);
        s.projectile.velocity = [10., 0., -30.];
        let mut landed = false;
        let mut explosions = 0;
        for _ in 0..180 {
            let f = s
                .tick(&rules(), &assets(), &w, &volumes(), 1. / 60.)
                .unwrap();
            landed |= f.landed;
            explosions += f
                .effects
                .iter()
                .filter(|&&e| e == detonator::Effect::Explode)
                .count();
        }
        assert!(landed && s.settled);
        assert_eq!(s.projectile.velocity, [0.; 3]);
        assert_eq!(explosions, 1);
        assert!(s.position[2] >= 0.);
    }
    #[test]
    fn expired_projectile_has_no_path_or_contact() {
        let mut s = grenade([0.; 3]);
        s.projectile.age = 7.999;
        let f = s
            .tick(&rules(), &assets(), &world(vec![]), &volumes(), 1. / 60.)
            .unwrap();
        assert!(f.segments.is_empty() && f.contacts.is_empty());
        assert_eq!(f.effects, [detonator::Effect::Destroy]);
        assert_eq!(s.position, [0.; 3]);
    }
    #[test]
    fn dry_default_gravity_zeroes_buoyancy_and_water_is_rejected() {
        let mut v = volumes();
        let (_, e) = assets().environment(&v, [0.; 3]).unwrap();
        assert_eq!((e.buoyancy, e.fluid_friction), (0., 0.));
        v.default.water = true;
        let mut s = grenade([0.; 3]);
        let before = s.clone();
        assert!(s
            .tick(&rules(), &assets(), &world(vec![]), &v, 1. / 60.)
            .is_err());
        assert_eq!(s, before);
    }
    #[test]
    fn unknown_volume_and_invalid_time_are_atomic() {
        let mut v = volumes();
        v.volumes.push(PhysicsVolume {
            name: "unknown".into(),
            priority: 1,
            settings: v.default.clone(),
            shape: VolumeShape::Unsupported("missing brush".into()),
        });
        let mut s = grenade([0.; 3]);
        let before = s.clone();
        assert!(s
            .tick(&rules(), &assets(), &world(vec![]), &v, 0.02)
            .is_err());
        assert!(s
            .tick(&rules(), &assets(), &world(vec![]), &volumes(), f64::NAN)
            .is_err());
        assert_eq!(s, before);
    }
}
