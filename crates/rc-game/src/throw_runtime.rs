//! Optional live thermal throw section. The RCPLAY06 gunner extension applies
//! bunker transitions and blast effects; RCPLAY05 preserves contact-only behavior.
use crate::{detonator, detonator_flight, grenade_throw, gunner, Game, Point, STEP};
use serde::{Deserialize, Serialize};

#[derive(Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Assets {
    pub animation: grenade_throw::Assets,
    pub projectile: detonator::Assets,
    pub flight: detonator_flight::Assets,
    pub gun: gunner::Assets,
    pub offset: [f32; 3],
    pub route: Vec<Point>,
    pub snapshot: Vec<u8>,
    /// Throw Foley, weapon grab Foley, bounce, empty, confirmation.
    pub sounds: Vec<crate::combat::Sound>,
    pub foley_frames: [f32; 2],
}
#[derive(Clone, Debug, Default, Serialize, serde::Deserialize)]
pub struct State {
    pub gate: grenade_throw::State,
    pub held: bool,
    pub projectiles: Vec<detonator_flight::State>,
    pub gun: gunner::State,
    pub waypoint: usize,
    pub released: u32,
    pub detonated: u32,
}
pub struct Prepared {
    pub assets: Assets,
    pub triangles: Vec<rc_package::geometry::Triangle>,
    pub appearance: Vec<Option<rc_render::FaceTexture>>,
    pub sound_base: usize,
}
impl Game {
    pub fn attach_throw(&mut self, mut a: Assets) -> Result<(), String> {
        a.animation.validate()?;
        a.projectile.validate()?;
        a.flight.validate()?;
        a.gun.validate()?;
        if self.thermal.is_some()
            || self
                .mission
                .as_ref()
                .and_then(|m| m.grenades.as_ref())
                .is_none()
            || a.route.is_empty()
            || a.route.len() > 256
            || a.route
                .iter()
                .flatten()
                .any(|v| !v.is_finite() || v.abs() > 1e8)
            || a.offset.iter().any(|v| !v.is_finite() || v.abs() > 1000.)
            || a.sounds.len() != 5
            || a.foley_frames
                .iter()
                .any(|v| !v.is_finite() || !(0. ..=1.).contains(v))
        {
            return Err("Invalid live throw assets".into());
        }
        let mesh = &self
            .visuals
            .as_ref()
            .ok_or("Throw requires original weapon")?
            .weapon;
        let sequence = &mesh.mesh.clips[mesh.clip("ThrowGrenade")?].sequence;
        if sequence.frames != a.animation.frames
            || sequence.rate_bits != a.animation.rate.to_bits()
            || sequence
                .notifies
                .iter()
                .filter(|n| n.time_bits == a.animation.release_frame.to_bits())
                .count()
                != 1
            || a.foley_frames.iter().any(|time| {
                sequence
                    .notifies
                    .iter()
                    .filter(|n| n.time_bits == time.to_bits())
                    .count()
                    != 1
            })
        {
            return Err("Throw release does not match original weapon channel".into());
        }
        let roles = [
            "GEN_Sound.Foley_Movement.fol_GEN_grenadeThrow_01",
            "GEN_Sound.Foley_Movement.fol_GEN_wepGrab_02",
            "GEN_Sound.Impacts_Explos.imp_GEN_grenadeBounce_01",
            "GEN_Sound.Interface.int_GEN_selGrnEmpty_01",
            "GEO_Voice.GEO_01.CA1G1716",
        ];
        if a.sounds
            .iter()
            .zip(roles)
            .any(|(sound, role)| sound.source != role)
        {
            return Err("Incorrect throw sound roles".into());
        }
        for s in &a.sounds {
            crate::combat::validate_wave(&s.wave)?;
        }
        let scene = rc_render::Scene::from_snapshot(&a.snapshot)?;
        if scene.triangles.len() != 352 || scene.appearance.iter().any(Option::is_none) {
            return Err("Incomplete live detonator mesh".into());
        }
        let base = self.scene.textures.len();
        let appearance = scene
            .appearance
            .into_iter()
            .map(|f| {
                f.map(|mut f| {
                    f.texture += base;
                    f
                })
            })
            .collect();
        let sound_base = self.mission.as_ref().unwrap().sounds.len();
        self.mission.as_mut().unwrap().sounds.append(&mut a.sounds);
        self.scene.textures.extend(scene.textures);
        self.thermal = Some(Prepared {
            assets: a,
            triangles: scene.triangles,
            appearance,
            sound_base,
        });
        self.state.thermal = Some(State::default());
        self.initial = self.state.clone();
        Ok(())
    }
    pub(crate) fn throw_begin(&mut self, pressed: bool) -> Result<(), String> {
        let Some(t) = &mut self.state.thermal else {
            return Ok(());
        };
        let edge = pressed && !t.held;
        t.held = pressed;
        if !edge || self.state.reload_remaining > 0. || t.projectiles.len() >= 32 {
            return Ok(());
        }
        let ammo = self
            .state
            .mission
            .as_ref()
            .unwrap()
            .grenades
            .as_ref()
            .unwrap()
            .flow
            .ammunition;
        let effects = t.gate.begin(ammo);
        if effects.contains(&grenade_throw::Effect::PlayThrowAnimation) {
            let mesh = &self.visuals.as_ref().unwrap().weapon;
            self.state
                .visuals
                .as_mut()
                .unwrap()
                .weapon
                .select(mesh, "ThrowGrenade", true)?;
        }
        if effects.contains(&grenade_throw::Effect::EmptySound) {
            self.throw_sound(3);
        }
        Ok(())
    }
    pub(crate) fn throw_sound(&mut self, role: usize) {
        let m = self.state.mission.as_mut().unwrap();
        m.audio_sequence += 1;
        if m.events.len() >= 256 {
            m.events.remove(0);
        }
        m.events.push(crate::combat::Event {
            sequence: m.audio_sequence,
            tick: self.state.statistics.ticks,
            sound: self.thermal.as_ref().unwrap().sound_base + role,
            owner: -1,
        });
    }
    pub(crate) fn throw_tick(&mut self) -> Result<(), String> {
        let Some(prepared) = &self.thermal else {
            return Ok(());
        };
        let a = &prepared.assets;
        let eye = self.eye();
        let t = self.state.thermal.as_mut().unwrap();
        let mut sounds = vec![];
        let mut explosions = vec![];
        let mut gun_events = vec![];
        // Existing actors advance first. A release starts at age zero this tick.
        for p in &mut t.projectiles {
            let frame = p.tick(&a.projectile, &a.flight, &self.world, &self.volumes, STEP)?;
            for segment in frame.segments {
                gun_events.extend(t.gun.observe(
                    &a.gun,
                    segment.from,
                    segment.to,
                    gunner::ContactClass::GrenadeProjectile,
                )?);
            }
            for effect in frame.effects {
                match effect {
                    detonator::Effect::BounceSound => sounds.push(2),
                    detonator::Effect::Explode => {
                        t.detonated += 1;
                        explosions.push(p.position);
                    }
                    _ => {}
                }
            }
        }
        t.projectiles.retain(|p| p.projectile.ended.is_none());
        if t.gate.active {
            let v = &self.state.visuals.as_ref().unwrap().weapon;
            let old = if t.gate.started {
                t.gate.frame
            } else {
                -f32::EPSILON
            };
            for (role, time) in a.foley_frames.iter().enumerate() {
                if crate::visual::crossed_notify(old, v.frame, *time) {
                    sounds.push(role);
                }
            }
            let ammo = &mut self
                .state
                .mission
                .as_mut()
                .unwrap()
                .grenades
                .as_mut()
                .unwrap()
                .flow
                .ammunition;
            let effects = t.gate.animation(&a.animation, v.frame, v.finished, ammo)?;
            if effects.contains(&grenade_throw::Effect::ReleaseProjectile) {
                let matrix = rc_package::mesh_transform::rotation_matrix(
                    [
                        (self.state.pitch * 65536. / std::f64::consts::TAU) as i32,
                        (self.state.yaw * 65536. / std::f64::consts::TAU) as i32,
                        0,
                    ],
                    [0; 3],
                )?;
                let axes =
                    std::array::from_fn(|i| std::array::from_fn(|j| f32::from_bits(matrix[i][j])));
                let origin = rc_package::projectile_motion::fire_start(
                    eye.map(|v| v as f32),
                    axes,
                    a.offset,
                )
                .map(f64::from);
                t.projectiles.push(detonator_flight::State::new(
                    detonator::State::new(
                        &a.projectile,
                        axes[0],
                        self.state.pawn.body.velocity.map(|v| v as f32),
                        self.state.statistics.ticks as f64 * STEP,
                    )?,
                    origin,
                )?);
                t.released += 1;
            }
        }
        // Actual contact is exposed, but destruction script effects are not applied
        // until bunker actors are bound. Never claim mission victory here.
        while t.waypoint + 1 < a.route.len()
            && crate::distance(self.state.pawn.body.position, a.route[t.waypoint]) < 25.
        {
            t.waypoint += 1;
        }
        for role in sounds {
            self.throw_sound(role);
        }
        if let Some(state) = &mut self.state.bunker {
            state.events.extend(gun_events);
        }
        for position in explosions {
            self.thermal_explosion(position)?;
        }
        Ok(())
    }
    pub fn throw_target(&self) -> Option<Point> {
        let picked = self.state.mission.as_ref()?.grenades.as_ref()?;
        if !picked.flow.objective_completed
            || picked.flow.ammunition == 0 && self.state.thermal.as_ref()?.released == 0
        {
            return None;
        }
        let t = self.state.thermal.as_ref()?;
        self.thermal.as_ref()?.assets.route.get(t.waypoint).copied()
    }
}
