//! Selected original KarmaProp.AffectKarma and Actor.HurtRadius bindings.
//! Pawn contact response uses the port's finite-mass contact adapter.
use crate::{rigid, Game, Point};
use serde::{Deserialize, Serialize};
#[derive(Clone, Debug, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Prop {
    pub original: u32,
    pub location: Point,
    pub collision_radius: f32,
    pub initial_take_shot: bool,
    pub damage_threshold: i32,
}
#[derive(Clone, Debug, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Assets {
    pub props: Vec<Prop>,
    pub shot_momentum: f32,
    pub blast_momentum: f32,
    pub blast_up_kick: f32,
    pub pawn_mass: f64,
}
impl Assets {
    pub fn validate(&self, bodies: &[rigid::Binding]) -> Result<(), String> {
        if self.props.is_empty()
            || self.props.len() != bodies.len()
            || self.props.iter().zip(bodies).any(|(p, b)| {
                p.original != b.original
                    || p.location.iter().any(|v| !v.is_finite() || v.abs() > 1e8)
                    || !p.collision_radius.is_finite()
                    || !(0. ..=1e6).contains(&p.collision_radius)
            })
            || [self.shot_momentum, self.blast_momentum, self.blast_up_kick]
                .iter()
                .any(|v| !v.is_finite() || !(0. ..=1e6).contains(v))
            || !self.pawn_mass.is_finite()
            || !(0.001..=10000.).contains(&self.pawn_mass)
        {
            return Err("Invalid selected Karma interaction bindings".into());
        }
        Ok(())
    }
    pub fn initial(&self) -> Vec<i32> {
        self.props.iter().map(|p| p.damage_threshold).collect()
    }
}
fn dot(a: Point, b: Point) -> f64 {
    (0..3).map(|i| a[i] * b[i]).sum()
}
fn threshold(remaining: &mut i32, damage: f64, take_shot: bool) -> bool {
    *remaining = remaining.wrapping_sub(damage as i32);
    take_shot && *remaining <= 0
}
/// Actor.HurtRadius: radius compensation affects damage and momentum equally;
/// the upward kick is added before scaling. A coincident center has no direction.
pub fn radial(
    source: Point,
    target: Point,
    collision_radius: f32,
    radius: f32,
    damage: f32,
    momentum: f32,
    up: f32,
) -> Result<Option<(f64, Point)>, String> {
    if source
        .iter()
        .chain(&target)
        .any(|v| !v.is_finite() || v.abs() > 1e8)
        || [collision_radius, radius, damage, momentum, up]
            .iter()
            .any(|v| !v.is_finite() || !(0. ..=1e6).contains(v))
        || radius == 0.
    {
        return Err("Invalid Karma radius damage".into());
    }
    let dir: [f32; 3] = std::array::from_fn(|i| target[i] as f32 - source[i] as f32);
    let dist = ((dir[0] * dir[0] + dir[1] * dir[1]) + dir[2] * dir[2]).sqrt();
    let scale = if dist < collision_radius {
        1.
    } else {
        1. - ((dist - collision_radius) * (1. / radius))
    };
    if scale <= 0. {
        return Ok(None);
    }
    let direction = if dist > 0. {
        dir.map(|v| v / dist)
    } else {
        [0.; 3]
    };
    let impulse = std::array::from_fn(|i| {
        f64::from((momentum * direction[i] + if i == 2 { up } else { 0. }) * scale)
    });
    Ok(Some((f64::from(scale * damage), impulse)))
}
impl Game {
    fn rigid_actor_id(&self, name: &str) -> Option<u32> {
        let dynamic = self.dynamic.as_ref()?;
        let original = dynamic
            .objects
            .iter()
            .find(|o| self.world.actors[o.actor].name == name)?
            .original;
        self.opening
            .as_ref()?
            .assets
            .rigid_interactions
            .as_ref()?
            .props
            .iter()
            .any(|p| p.original == original)
            .then_some(original)
    }
    fn rigid_damage(
        &mut self,
        original: u32,
        damage: f64,
        at: Point,
        momentum: Point,
    ) -> Result<bool, String> {
        if !damage.is_finite() || !(0. ..=1e6).contains(&damage) {
            return Err("Invalid Karma damage".into());
        }
        let Some(prepared) = self.opening.as_ref() else {
            return Ok(false);
        };
        let Some(a) = prepared.assets.rigid_interactions.as_ref() else {
            return Ok(false);
        };
        let Some(i) = a.props.iter().position(|p| p.original == original) else {
            return Ok(false);
        };
        let state = self.state.opening.as_mut().ok_or("Missing opening state")?;
        let bodies = state.rigid.as_mut().ok_or("Missing Karma bodies")?;
        let remaining = state
            .rigid_interactions
            .as_mut()
            .ok_or("Missing Karma thresholds")?;
        let mut candidate = bodies[i].clone();
        let mut next = remaining[i];
        if threshold(
            &mut next,
            damage,
            candidate.active || a.props[i].initial_take_shot,
        ) {
            candidate.add_world_impulse(&prepared.assets.rigid[i], at, momentum)?;
        }
        remaining[i] = next;
        bodies[i] = candidate;
        self.sync_dynamic();
        Ok(true)
    }
    /// Shared incoming projectile-impact dispatcher. The nearest world hit and
    /// supplied fraction must agree; this never traces through intervening walls.
    pub fn rigid_projectile_hit(
        &mut self,
        from: Point,
        to: Point,
        fraction: f64,
        damage: u32,
        direction: Point,
    ) -> Result<bool, String> {
        let Some(momentum) = self
            .opening
            .as_ref()
            .and_then(|p| p.assets.rigid_interactions.as_ref())
            .map(|a| a.shot_momentum)
        else {
            return Ok(false);
        };
        let query = self.world.sweep_motion(from, to, [0.; 3])?;
        if !query.complete {
            return Err("Incomplete Karma projectile trace".into());
        }
        let Some(hit) = query.contacts.first() else {
            return Ok(false);
        };
        if (hit.hit.fraction - fraction).abs() > 1e-8 {
            return Ok(false);
        }
        let Some(id) = self.rigid_actor_id(&hit.object) else {
            return Ok(false);
        };
        let at = std::array::from_fn(|i| from[i] + (to[i] - from[i]) * fraction);
        self.rigid_damage(
            id,
            f64::from(damage),
            at,
            direction.map(|v| v * f64::from(momentum)),
        )
    }
    pub(crate) fn rigid_blast(
        &mut self,
        source: Point,
        damage: f32,
        radius: f32,
    ) -> Result<(), String> {
        // CTProjExplosive.Explode only calls HurtRadius for positive damage/radius.
        if damage == 0. || radius == 0. {
            return Ok(());
        }
        let Some(p) = self.opening.as_ref() else {
            return Ok(());
        };
        let Some(a) = p.assets.rigid_interactions.as_ref() else {
            return Ok(());
        };
        let bodies = self
            .state
            .opening
            .as_ref()
            .and_then(|s| s.rigid.as_ref())
            .ok_or("Missing Karma blast bodies")?;
        let mut hits = vec![];
        for ((prop, b), s) in a.props.iter().zip(&p.assets.rigid).zip(bodies) {
            let at = s.point(b, prop.location);
            let Some((amount, j)) = radial(
                source,
                at,
                prop.collision_radius,
                radius,
                damage,
                a.blast_momentum,
                a.blast_up_kick,
            )?
            else {
                continue;
            };
            let query = self.world.sweep_motion(source, at, [0.; 3])?;
            if !query.complete {
                return Err("Incomplete Karma explosion visibility".into());
            }
            if query.contacts.first().is_some_and(|h| {
                h.hit.fraction < 1. - 1e-8 && self.rigid_actor_id(&h.object) != Some(prop.original)
            }) {
                continue;
            }
            hits.push((prop.original, amount, at, j));
        }
        for (id, amount, at, j) in hits {
            self.rigid_damage(id, amount, at, j)?;
        }
        Ok(())
    }
    pub(crate) fn rigid_pawn_contacts(&mut self, motion: &rc_package::pawn_motion::PawnMotion) {
        let Some(a) = self
            .opening
            .as_ref()
            .and_then(|p| p.assets.rigid_interactions.as_ref())
        else {
            return;
        };
        let mass = a.pawn_mass;
        let (velocity, frame) = match motion {
            rc_package::pawn_motion::PawnMotion::Walking(f) => (f.planned_velocity, &f.motion),
            rc_package::pawn_motion::PawnMotion::Falling(f) => (
                f.integration_plan.substeps[0].velocity_before_terminal,
                &f.motion,
            ),
        };
        let mut pushed = std::collections::BTreeSet::new();
        let contacts: Vec<_> = frame
            .contacts
            .iter()
            .filter(|c| c.hit.normal[2].abs() < 0.7)
            .filter_map(|c| {
                let id = self.rigid_actor_id(&c.object)?;
                pushed.insert(id).then_some((id, c.hit.normal))
            })
            .collect();
        let p = self.opening.as_ref().unwrap();
        let bodies = self
            .state
            .opening
            .as_mut()
            .and_then(|s| s.rigid.as_mut())
            .unwrap();
        for (id, n) in contacts {
            let i = p
                .assets
                .rigid
                .iter()
                .position(|b| b.original == id)
                .unwrap();
            let mut extent = self.profile.physics.extent;
            if self.state.crouched {
                extent[2] = 56.;
            }
            let support = (0..3).map(|k| extent[k] * n[k].abs()).sum::<f64>();
            let at = std::array::from_fn(|k| frame.body.position[k] - n[k] * support);
            pawn_impulse(&p.assets.rigid[i], &mut bodies[i], at, n, velocity, mass);
        }
    }
}
fn pawn_impulse(
    b: &rigid::Binding,
    s: &mut rigid::Body,
    at: Point,
    n: Point,
    velocity: Point,
    mass: f64,
) {
    if !s.active {
        return;
    }
    let relative = std::array::from_fn(|i| velocity[i] - s.velocity_at(at)[i]);
    let closing = -dot(relative, n);
    if closing <= 0.05 {
        return;
    }
    let j = closing / (1. / mass + s.denominator(b, at, n));
    s.wake();
    s.impulse(b, at, n.map(|v| -v * j));
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn original_threshold_decrements_while_disabled_and_trigger_enables_shot_response() {
        let mut n = 10;
        assert!(!threshold(&mut n, 6.9, false));
        assert_eq!(n, 4);
        assert!(!threshold(&mut n, 3., true));
        assert_eq!(n, 1);
        assert!(threshold(&mut n, 1., true));
        assert_eq!(n, 0);
        assert!(threshold(&mut n, 2., true));
        assert_eq!(n, -2);
    }
    #[test]
    fn original_blast_falloff_scales_upkick_and_damage_and_handles_coincident_center() {
        let (damage, j) = radial([0.; 3], [60., 0., 0.], 10., 100., 200., 800., 700.)
            .unwrap()
            .unwrap();
        assert_eq!(damage, 100.);
        assert_eq!(j, [400., 0., 350.]);
        assert!(radial([0.; 3], [110., 0., 0.], 10., 100., 200., 800., 700.)
            .unwrap()
            .is_none());
        assert_eq!(
            radial([0.; 3], [0.; 3], 1., 100., 200., 800., 700.)
                .unwrap()
                .unwrap()
                .1,
            [0., 0., 700.]
        );
        assert!(radial([0.; 3], [0.; 3], 1., 0., 200., 800., 700.).is_err());
    }
    #[test]
    fn pawn_contact_wakes_sleeping_body_with_finite_mass_and_off_center_torque() {
        let b = rigid::Binding {
            original: 1,
            center: [0.; 3],
            vertices: [
                [-1., -1., -1.],
                [-1., 1., -1.],
                [-1., 1., 1.],
                [-1., -1., 1.],
                [1., -1., -1.],
                [1., 1., -1.],
                [1., 1., 1.],
                [1., -1., 1.],
            ],
            inertia: [[1., 0., 0.], [0., 1., 0.], [0., 0., 1.]],
            mass: 1.,
            friction: 1.,
            restitution: 0.,
            linear_damping: 0.,
            angular_damping: 0.,
            gravity: [0.; 3],
            initial_velocity: [0.; 3],
            initial_angular_velocity: [0.; 3],
        };
        b.validate().unwrap();
        let mut s = b.initial();
        let original = s.clone();
        pawn_impulse(&b, &mut s, [0., 1., 0.], [-1., 0., 0.], [10., 0., 0.], 100.);
        assert_eq!(s, original, "Inactive tutorial barrier must remain fixed");
        s.active = true;
        s.sleeping = true;
        s.quiet_steps = 480;
        let sleeping = s.clone();
        pawn_impulse(
            &b,
            &mut s,
            [0., 1., 0.],
            [-1., 0., 0.],
            [-10., 0., 0.],
            100.,
        );
        assert_eq!(s, sleeping, "Receding contact must not wake");
        pawn_impulse(&b, &mut s, [0., 1., 0.], [-1., 0., 0.], [10., 0., 0.], 100.);
        assert!(!s.sleeping && s.quiet_steps == 0);
        assert!((s.velocity[0] - 10. / 2.01).abs() < 1e-10);
        assert!((s.angular_velocity[2] + s.velocity[0]).abs() < 1e-10);
        s.add_world_impulse(&b, s.center, [200., 0., 0.]).unwrap();
        assert!((s.velocity[0] - 200. - 10. / 2.01).abs() < 1e-10);
        assert!(s.valid(&b));
    }
}
