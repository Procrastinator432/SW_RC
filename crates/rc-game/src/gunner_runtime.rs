//! Original bunker transition and delayed gun-position completion. Explosion
//! sprites and visible-candidate damage remain bounded presentation/combat adapters.
use crate::{gunner, Game, Phase, Point, STEP};
use serde::{Deserialize, Serialize};
#[derive(Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Sprite {
    pub source: String,
    pub width: usize,
    pub height: usize,
    pub pixels: Vec<u32>,
    pub size: f32,
    pub seconds: f64,
    pub grid: [u32; 2],
}
#[derive(Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Assets {
    pub objects: Vec<crate::dynamic::Object>,
    pub bunker_position: Point,
    pub sounds: Vec<crate::combat::Sound>,
    pub sprites: Vec<Sprite>,
    pub provenance: serde_json::Value,
}
#[derive(Clone, Debug, Serialize, serde::Deserialize)]
pub struct Explosion {
    pub position: Point,
    pub age: f64,
    pub bunker: bool,
}
#[derive(Clone, Debug, Serialize, serde::Deserialize)]
pub struct State {
    pub contact_tick: Option<u64>,
    pub destruction_tick: Option<u64>,
    pub confirmation_tick: Option<u64>,
    pub complete_tick: Option<u64>,
    pub health: u32,
    pub explosions: Vec<Explosion>,
    pub events: Vec<gunner::Effect>,
    pub damage: Vec<Damage>,
}
#[derive(Clone, Debug, Serialize, serde::Deserialize)]
pub struct Damage {
    pub tick: u64,
    pub victim: i32,
    pub amount: u32,
    pub position: Point,
}
pub struct Prepared {
    pub assets: Assets,
    pub texture_base: usize,
    pub sound_base: usize,
}
impl Game {
    pub fn attach_gunner(&mut self, mut a: Assets) -> Result<(), String> {
        if self.bunker.is_some()
            || self.thermal.is_none()
            || a.objects.len() != 2
            || a.objects
                .iter()
                .map(|o| o.original)
                .collect::<std::collections::BTreeSet<_>>()
                != [850, 1093].into_iter().collect()
            || a.bunker_position
                .iter()
                .any(|v| !v.is_finite() || v.abs() > 1e8)
            || a.sounds.len() != 2
            || a.sprites.is_empty()
            || a.sprites.len() > 8
        {
            return Err("Invalid gun-position extension".into());
        }
        for s in &a.sounds {
            crate::combat::validate_wave(&s.wave)?;
        }
        if a.sounds[0].source != "GEN_Sound.Impacts_Explos.exp_GEN_MedWhistle_01"
            || a.sounds[1].source != "GEN_Sound.Impacts_Explos.exp_GEN_sw_large_02"
        {
            return Err("Incorrect original explosion sound roles".into());
        }
        for s in &a.sprites {
            if s.width == 0
                || s.height == 0
                || s.width > 512
                || s.height > 512
                || s.pixels.len() != s.width * s.height
                || !s.size.is_finite()
                || !(1. ..=2000.).contains(&s.size)
                || !s.seconds.is_finite()
                || !(0.01..=10.).contains(&s.seconds)
                || s.grid.iter().any(|n| !(1..=16).contains(n))
                || s.source.is_empty()
            {
                return Err("Invalid original explosion sprite".into());
            }
        }
        let objects = std::mem::take(&mut a.objects);
        self.append_dynamic(objects)?;
        let texture_base = self.scene.textures.len();
        self.scene
            .textures
            .extend(a.sprites.iter().map(|s| rc_render::Texture {
                width: s.width,
                height: s.height,
                pixels: s.pixels.clone(),
            }));
        let sound_base = self.mission.as_ref().unwrap().sounds.len();
        self.mission.as_mut().unwrap().sounds.append(&mut a.sounds);
        self.state.bunker = Some(State {
            contact_tick: None,
            destruction_tick: None,
            confirmation_tick: None,
            complete_tick: None,
            health: self.thermal.as_ref().unwrap().assets.gun.bunker_health,
            explosions: vec![],
            events: vec![],
            damage: vec![],
        });
        self.bunker = Some(Prepared {
            assets: a,
            texture_base,
            sound_base,
        });
        self.initial = self.state.clone();
        Ok(())
    }
    fn explosion_sound(&mut self, role: usize) {
        let sound = self.bunker.as_ref().unwrap().sound_base + role;
        let m = self.state.mission.as_mut().unwrap();
        m.audio_sequence += 1;
        if m.events.len() >= 256 {
            m.events.remove(0);
        }
        m.events.push(crate::combat::Event {
            sequence: m.audio_sequence,
            tick: self.state.statistics.ticks,
            sound,
            owner: -1,
        });
    }
    pub(crate) fn blast_damage(
        &self,
        position: Point,
        damage: f32,
        damage_radius: f32,
    ) -> Result<Vec<Damage>, String> {
        let mut damages = vec![];
        let mut candidates = vec![(
            -1,
            self.state.pawn.body.position,
            self.profile.physics.extent[0] as f32,
            self.state.health,
        )];
        candidates.extend(
            self.state
                .enemies
                .iter()
                .enumerate()
                .map(|(i, e)| (i as i32, e.position, 22., e.health)),
        );
        for (victim, at, radius, health) in candidates {
            let distance = crate::distance(position, at) as f32;
            if health == 0
                || distance > damage_radius + radius
                || self.wall_fraction(position, at)? < 1.
            {
                continue;
            }
            let amount = crate::detonator::radius_damage(damage, damage_radius, distance, radius)?
                .max(0.)
                .ceil() as u32;
            if amount > 0 {
                damages.push(Damage {
                    tick: self.state.statistics.ticks,
                    victim,
                    amount,
                    position: at,
                });
            }
        }
        Ok(damages)
    }
    pub(crate) fn thermal_explosion(&mut self, position: Point) -> Result<(), String> {
        if self.bunker.is_none() {
            return Ok(());
        }
        let rules = &self.thermal.as_ref().unwrap().assets.projectile;
        let damages = self.blast_damage(position, rules.damage, rules.damage_radius)?;
        let (damage, radius) = (rules.damage, rules.damage_radius);
        self.warrior_blast(position, damage, radius)?;
        self.squad_blast(position, damage, radius)?;
        for d in &damages {
            if d.victim < 0 {
                self.state.health = self.state.health.saturating_sub(d.amount);
                self.state.damage_flash = 0.2;
            } else {
                let e = &mut self.state.enemies[d.victim as usize];
                let old = e.health;
                e.health = e.health.saturating_sub(d.amount);
                e.flash = 0.12;
                self.state.statistics.kills += u32::from(old > 0 && e.health == 0);
            }
        }
        let state = self.state.bunker.as_mut().unwrap();
        state.damage.extend(damages);
        if state.damage.len() > 64 {
            state.damage.drain(..state.damage.len() - 64);
        }
        if state.explosions.len() >= 32 {
            state.explosions.remove(0);
        }
        state.explosions.push(Explosion {
            position,
            age: 0.,
            bunker: false,
        });
        self.explosion_sound(0);
        Ok(())
    }
    pub(crate) fn gunner_tick(&mut self) -> Result<(), String> {
        let Some(prepared) = &self.bunker else {
            return Ok(());
        };
        let max_age = prepared
            .assets
            .sprites
            .iter()
            .map(|s| s.seconds)
            .fold(0., f64::max);
        let state = self.state.bunker.as_mut().unwrap();
        for e in &mut state.explosions {
            e.age += STEP;
        }
        state.explosions.retain(|e| e.age < max_age);
        let gun = &mut self.state.thermal.as_mut().unwrap().gun;
        if gun.elapsed.is_none() {
            return Ok(());
        }
        let tick = self.state.statistics.ticks;
        if state.contact_tick.is_none() {
            state.contact_tick = Some(tick);
            return Ok(());
        }
        let effects = gun.advance(&self.thermal.as_ref().unwrap().assets.gun, STEP)?;
        let destroy = effects
            .iter()
            .any(|e| matches!(e,gunner::Effect::HurtActor {tag,..} if tag=="SmallBunker"));
        let confirm = effects.contains(&gunner::Effect::ConfirmationSound);
        state.events.extend(effects);
        if destroy {
            state.health = 0;
            state.destruction_tick = Some(tick);
            state.explosions.push(Explosion {
                position: prepared.assets.bunker_position,
                age: 0.,
                bunker: true,
            });
            self.disable_dynamic(850)?;
            self.disable_dynamic(1093)?;
            self.explosion_sound(1);
        }
        if confirm {
            self.state.bunker.as_mut().unwrap().confirmation_tick = Some(tick);
            self.throw_sound(4);
        }
        if self.state.health == 0 && !self.try_incapacitate_player() {
            self.state.phase = Phase::Lost;
        } else if self.state.thermal.as_ref().unwrap().gun.completed {
            self.state
                .bunker
                .as_mut()
                .unwrap()
                .complete_tick
                .get_or_insert(tick);
            if self.rendezvous.is_none() {
                self.state.phase = Phase::Won;
            }
        }
        Ok(())
    }
    pub(crate) fn explosion_geometry(
        &self,
        right: [f32; 3],
        up: [f32; 3],
    ) -> (
        Vec<rc_package::geometry::Triangle>,
        Vec<Option<rc_render::FaceTexture>>,
    ) {
        let mut triangles = vec![];
        let mut faces = vec![];
        if let (Some(prepared), Some(state)) = (&self.bunker, &self.state.bunker) {
            for e in &state.explosions {
                for (index, s) in prepared.assets.sprites.iter().enumerate() {
                    if e.age >= s.seconds {
                        continue;
                    }
                    let fraction = (e.age / s.seconds) as f32;
                    let size = s.size * (1. + fraction) * if e.bunker { 1.6 } else { 1. };
                    let points: [[f32; 3]; 4] = std::array::from_fn(|n| {
                        std::array::from_fn(|i| {
                            e.position[i] as f32
                                + right[i] * size * if n & 1 == 0 { -1. } else { 1. }
                                + up[i] * size * if n & 2 == 0 { -1. } else { 1. }
                        })
                    });
                    let count = s.grid[0] * s.grid[1];
                    let cell = ((fraction * count as f32) as u32).min(count - 1);
                    let u = (cell % s.grid[0]) as f32 / s.grid[0] as f32;
                    let v = (cell / s.grid[0]) as f32 / s.grid[1] as f32;
                    let uv = [
                        [u, v + 1. / s.grid[1] as f32],
                        [u + 1. / s.grid[0] as f32, v + 1. / s.grid[1] as f32],
                        [u, v],
                        [u + 1. / s.grid[0] as f32, v],
                    ];
                    for ids in [[0, 1, 3], [0, 3, 2]] {
                        triangles.push(rc_package::geometry::Triangle {
                            points: ids.map(|i| points[i]),
                            surface: 0xffffff,
                        });
                        faces.push(Some(rc_render::FaceTexture {
                            texture: prepared.texture_base + index,
                            uv: ids.map(|i| uv[i]),
                        }));
                    }
                }
            }
        }
        (triangles, faces)
    }
}
