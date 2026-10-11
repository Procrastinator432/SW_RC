//! Selected CTProjExplosive script rules. World movement, touch dispatch, native
//! falling physics, animation release and visible-actor selection belong to the host.
use serde::{Deserialize, Serialize};
#[derive(Clone, Debug, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Assets {
    pub speed: f32,
    pub max_speed: f32,
    pub toss_z: f32,
    pub fuse_time: f64,
    pub post_impact_fuse: f64,
    pub life_span: f64,
    pub world_damping: f32,
    pub actor_damping: f32,
    pub rest_speed: f32,
    pub min_bounce_sound_time: f64,
    pub damage: f32,
    pub damage_radius: f32,
}
impl Assets {
    pub fn validate(&self) -> Result<(), String> {
        if [
            self.speed,
            self.max_speed,
            self.toss_z,
            self.rest_speed,
            self.damage,
            self.damage_radius,
        ]
        .iter()
        .any(|v| !v.is_finite() || !(0. ..=1e6).contains(v))
            || self.speed == 0.
            || self.damage_radius == 0.
            || [
                self.fuse_time,
                self.post_impact_fuse,
                self.life_span,
                self.min_bounce_sound_time,
            ]
            .iter()
            .any(|v| !v.is_finite() || !(0. ..=3600.).contains(v))
            || [self.world_damping, self.actor_damping]
                .iter()
                .any(|v| !v.is_finite() || !(0. ..=1.).contains(v))
        {
            return Err("Invalid detonator script assets".into());
        }
        Ok(())
    }
}
#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize)]
pub enum End {
    Detonated,
    Expired,
}
#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize)]
pub enum Effect {
    BounceSound,
    DisableActorCollision,
    Explode,
    Destroy,
}
#[derive(Clone, Debug, PartialEq, Serialize)]
pub struct State {
    pub velocity: [f32; 3],
    pub spawn_time: f64,
    pub age: f64,
    pub timer_remaining: Option<f64>,
    pub last_bounce_sound: f64,
    pub bouncing: bool,
    pub actor_collision: bool,
    pub ended: Option<End>,
}
impl State {
    pub fn new(
        a: &Assets,
        direction: [f32; 3],
        pawn_velocity: [f32; 3],
        level_seconds: f64,
    ) -> Result<Self, String> {
        a.validate()?;
        if direction
            .iter()
            .chain(&pawn_velocity)
            .any(|v| !v.is_finite() || v.abs() > 1e6)
            || (direction.iter().map(|v| v * v).sum::<f32>() - 1.).abs() > 1e-4
            || !level_seconds.is_finite()
            || !(0. ..=1e8).contains(&level_seconds)
        {
            return Err("Invalid prepared grenade release".into());
        }
        let velocity = rc_package::projectile_motion::toss_velocity(
            direction,
            pawn_velocity,
            a.speed,
            a.toss_z,
        );
        Ok(Self {
            velocity,
            spawn_time: level_seconds,
            age: 0.,
            timer_remaining: (a.fuse_time > 0.).then_some(a.fuse_time),
            last_bounce_sound: 0.,
            bouncing: true,
            actor_collision: true,
            ended: None,
        })
    }
    /// Advance script clocks only; a lifetime expiry destroys without calling Explode.
    /// Exact timer/lifetime ties use lifetime first as an explicit host ordering policy.
    pub fn advance(&mut self, a: &Assets, dt: f64) -> Result<Vec<Effect>, String> {
        a.validate()?;
        if !dt.is_finite() || !(0. ..=1.).contains(&dt) {
            return Err("Invalid detonator time step".into());
        }
        if self.ended.is_some() {
            return Ok(vec![]);
        }
        let life = if a.life_span > 0. {
            (a.life_span - self.age).max(0.)
        } else {
            f64::INFINITY
        };
        let fuse = self.timer_remaining.unwrap_or(f64::INFINITY);
        if life <= dt + 1e-9 && life <= fuse {
            self.age += life;
            self.ended = Some(End::Expired);
            return Ok(vec![Effect::Destroy]);
        }
        if fuse <= dt + 1e-9 {
            self.age += fuse;
            self.timer_remaining = Some(0.);
            self.ended = Some(End::Detonated);
            return Ok(vec![Effect::Explode, Effect::Destroy]);
        }
        self.age += dt;
        if let Some(timer) = &mut self.timer_remaining {
            *timer -= dt;
        }
        Ok(vec![])
    }
    /// Contact is dispatched after movement by the host. Ignoring the instigator
    /// occurs before fuse shortening. With an initial fuse, repeated contacts never
    /// extend a short fuse; the original FuseTime==0 branch rearms on each contact.
    pub fn impact(
        &mut self,
        a: &Assets,
        normal: [f32; 3],
        actor: bool,
        instigator: bool,
    ) -> Result<Vec<Effect>, String> {
        a.validate()?;
        if normal.iter().any(|v| !v.is_finite())
            || (normal.iter().map(|v| v * v).sum::<f32>() - 1.).abs() > 1e-4
        {
            return Err("Invalid grenade contact normal".into());
        }
        if self.ended.is_some() || instigator || (actor && !self.actor_collision) {
            return Ok(vec![]);
        }
        if a.post_impact_fuse > 0.
            && (a.fuse_time == 0. || self.timer_remaining.is_some_and(|v| a.post_impact_fuse < v))
        {
            self.timer_remaining = Some(a.post_impact_fuse);
        }
        if !self.bouncing {
            return Ok(vec![]);
        }
        let damping = if actor {
            a.actor_damping
        } else {
            a.world_damping
        };
        let dot = (self.velocity[0] * normal[0] + self.velocity[1] * normal[1])
            + self.velocity[2] * normal[2];
        self.velocity =
            std::array::from_fn(|i| (self.velocity[i] - 2. * dot * normal[i]) * damping);
        let mut effects = vec![];
        let now = self.spawn_time + self.age;
        if now - self.last_bounce_sound > a.min_bounce_sound_time {
            self.last_bounce_sound = now;
            effects.push(Effect::BounceSound);
        }
        if self.velocity.iter().map(|v| v * v).sum::<f32>().sqrt() < a.rest_speed {
            self.bouncing = false;
            self.actor_collision = false;
            effects.push(Effect::DisableActorCollision);
        }
        Ok(effects)
    }
}
/// Actor.HurtRadius damage arithmetic for a victim already selected by
/// VisibleCollidingActors, authority/exclusion filters and line of sight.
/// Iterator selection, impulse and victim-specific damage filters are not implied.
pub fn radius_damage(
    amount: f32,
    radius: f32,
    distance: f32,
    victim_radius: f32,
) -> Result<f32, String> {
    if [amount, radius, distance, victim_radius]
        .iter()
        .any(|v| !v.is_finite() || !(0. ..=1e8).contains(v))
        || radius == 0.
    {
        return Err("Invalid explosion damage query".into());
    }
    let scale = if distance < victim_radius {
        1.
    } else {
        1. - ((distance - victim_radius) * (1. / radius))
    };
    Ok(scale * amount)
}
#[cfg(test)]
mod tests {
    use super::*;
    fn assets() -> Assets {
        Assets {
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
    #[test]
    fn throw_inherits_projected_motion_only() {
        let a = assets();
        let s = State::new(&a, [1., 0., 0.], [300., 800., 50.], 0.).unwrap();
        assert_eq!(s.velocity, [2320., 0., 200.]);
        assert_eq!(
            State::new(&a, [-1., 0., 0.], [300., 800., 50.], 0.)
                .unwrap()
                .velocity,
            [-2080., 0., 200.]
        );
    }
    #[test]
    fn original_lifetime_precedes_unhit_fuse() {
        let a = assets();
        let mut s = State::new(&a, [1., 0., 0.], [0.; 3], 0.).unwrap();
        for _ in 0..7 {
            assert!(s.advance(&a, 1.).unwrap().is_empty());
        }
        assert_eq!(s.advance(&a, 1.).unwrap(), vec![Effect::Destroy]);
        assert_eq!(s.ended, Some(End::Expired));
        assert!(s.advance(&a, 1.).unwrap().is_empty());
    }
    #[test]
    fn contact_shortens_once_and_explodes_once() {
        let a = assets();
        let mut s = State::new(&a, [1., 0., 0.], [0.; 3], 0.).unwrap();
        s.advance(&a, 0.2).unwrap();
        assert_eq!(
            s.impact(&a, [-1., 0., 0.], false, false).unwrap(),
            vec![Effect::BounceSound]
        );
        assert_eq!(s.velocity, [-660., 0., 60.000004]);
        assert_eq!(s.timer_remaining, Some(1.));
        s.advance(&a, 0.5).unwrap();
        s.impact(&a, [0., 0., 1.], false, false).unwrap();
        assert_eq!(s.timer_remaining, Some(0.5));
        assert_eq!(
            s.advance(&a, 0.5).unwrap(),
            vec![Effect::Explode, Effect::Destroy]
        );
        assert!(s.impact(&a, [0., 0., 1.], false, false).unwrap().is_empty());
    }
    #[test]
    fn actor_bounce_rest_and_instigator_are_distinct() {
        let a = assets();
        let mut s = State::new(&a, [1., 0., 0.], [0.; 3], 0.).unwrap();
        let before = s.clone();
        assert!(s.impact(&a, [-1., 0., 0.], true, true).unwrap().is_empty());
        assert_eq!(s, before);
        s.impact(&a, [-1., 0., 0.], true, false).unwrap();
        assert_eq!(s.velocity, [-220., 0., 20.]);
        s.impact(&a, [0., 0., 1.], true, false).unwrap();
        assert_eq!(
            s.impact(&a, [0., 0., 1.], true, false).unwrap(),
            vec![Effect::DisableActorCollision]
        );
        assert!(!s.bouncing && !s.actor_collision);
        assert!(s.impact(&a, [0., 0., 1.], true, false).unwrap().is_empty());
    }
    #[test]
    fn bounce_sound_uses_strict_original_interval() {
        let a = assets();
        let mut s = State::new(&a, [1., 0., 0.], [0.; 3], 0.).unwrap();
        s.advance(&a, a.min_bounce_sound_time).unwrap();
        assert!(!s
            .impact(&a, [-1., 0., 0.], false, false)
            .unwrap()
            .contains(&Effect::BounceSound));
        s.advance(&a, 0.001).unwrap();
        assert!(s
            .impact(&a, [-1., 0., 0.], false, false)
            .unwrap()
            .contains(&Effect::BounceSound));
    }
    #[test]
    fn damage_respects_victim_radius_and_invalid_queries() {
        assert_eq!(radius_damage(400., 600., 0., 22.).unwrap(), 400.);
        assert_eq!(radius_damage(400., 600., 322., 22.).unwrap(), 200.);
        assert_eq!(radius_damage(400., 600., 622., 22.).unwrap(), 0.);
        assert!(radius_damage(400., 0., 1., 22.).is_err());
    }
    #[test]
    fn first_bounce_uses_level_time_not_projectile_age() {
        let a = assets();
        let mut s = State::new(&a, [1., 0., 0.], [0.; 3], 100.).unwrap();
        assert!(s
            .impact(&a, [-1., 0., 0.], false, false)
            .unwrap()
            .contains(&Effect::BounceSound));
        assert_eq!(s.last_bounce_sound, 100.);
        assert!(!s
            .impact(&a, [0., 0., 1.], false, false)
            .unwrap()
            .contains(&Effect::BounceSound));
    }
    #[test]
    fn zero_initial_fuse_rearms_and_late_impact_cannot_extend_lifetime() {
        let mut a = assets();
        a.fuse_time = 0.;
        let mut s = State::new(&a, [1., 0., 0.], [0.; 3], 0.).unwrap();
        assert_eq!(s.timer_remaining, None);
        s.impact(&a, [-1., 0., 0.], false, false).unwrap();
        s.advance(&a, 0.5).unwrap();
        s.impact(&a, [0., 0., 1.], false, false).unwrap();
        assert_eq!(s.timer_remaining, Some(1.));
        let a = assets();
        let mut s = State::new(&a, [1., 0., 0.], [0.; 3], 0.).unwrap();
        for _ in 0..7 {
            s.advance(&a, 1.).unwrap();
        }
        s.advance(&a, 0.5).unwrap();
        s.impact(&a, [-1., 0., 0.], false, false).unwrap();
        assert_eq!(s.advance(&a, 0.5).unwrap(), vec![Effect::Destroy]);
        assert_eq!(s.ended, Some(End::Expired));
    }
    #[test]
    fn invalid_input_does_not_mutate_projectile() {
        let a = assets();
        let mut s = State::new(&a, [1., 0., 0.], [0.; 3], 0.).unwrap();
        let before = s.clone();
        for dt in [-1., f64::NAN, 2.] {
            assert!(s.advance(&a, dt).is_err());
            assert_eq!(s, before);
        }
        assert!(s.impact(&a, [0.; 3], false, false).is_err());
        assert_eq!(s, before);
        let mut bad = a.clone();
        bad.post_impact_fuse = f64::NAN;
        assert!(s.impact(&bad, [1., 0., 0.], false, false).is_err());
        assert_eq!(s, before);
        assert!(State::new(&a, [1.; 3], [0.; 3], 0.).is_err());
    }
}
