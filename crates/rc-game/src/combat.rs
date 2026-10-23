//! Selected original weapon data with an explicit fixed-step projectile adapter.
//! Laser-bolt native integration, bone damage, AI and sound spatialization are not yet equivalent.
use crate::{direction, ray_box, Game, Point, STEP};
use serde::{Deserialize, Serialize};

#[derive(Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Ballistics {
    pub provenance: serde_json::Value,
    pub fire_offset: [f32; 3],
    pub proximity_damage: u32,
}
#[derive(Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Assets {
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub jump: Option<crate::jump::Assets>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub bot: Option<crate::bot::Assets>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub game_rules: Option<crate::game_rules::Assets>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub lunge: Option<crate::lunge::Assets>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub navigation: Option<crate::navigation::Assets>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub ballistics: Option<Ballistics>,
    pub provenance: serde_json::Value,
    pub magazine: u32,
    pub total_ammo: u32,
    pub shot_delay: f64,
    pub damage: u32,
    pub enemy_health: u32,
    pub speed: f64,
    pub acceleration: f64,
    pub max_speed: f64,
    pub laser_bolt: bool,
    pub lifespan: f64,
    pub reload_duration: f64,
    pub sounds: Vec<Sound>,
    pub fire_sound: usize,
    pub notifies: Vec<Notify>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub melee: Option<crate::melee::Rules>,
}
#[derive(Clone, Debug, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Sound {
    pub source: String,
    #[serde(with = "crate::wave_bytes")]
    pub wave: Vec<u8>,
}
#[derive(Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Notify {
    pub weapon: bool,
    pub clip: String,
    pub time: f32,
    pub sound: usize,
}
#[derive(Debug, Clone, Serialize, serde::Deserialize)]
pub struct Projectile {
    #[serde(skip_serializing_if = "Option::is_none")]
    pub velocity: Option<[f32; 3]>,
    pub position: Point,
    pub direction: Point,
    pub speed: f64,
    pub age: f64,
}
#[derive(Debug, Clone, Serialize, serde::Deserialize)]
pub struct Event {
    pub sequence: u64,
    pub tick: u64,
    pub sound: usize,
    pub owner: i32,
}
#[derive(Debug, Clone, Default, Serialize, serde::Deserialize)]
pub struct State {
    #[serde(skip_serializing_if = "Option::is_none")]
    pub jump: Option<crate::jump::State>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub lunge: Option<crate::lunge::State>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub navigation: Option<Vec<crate::navigation::Agent>>,
    pub projectiles: Vec<Projectile>,
    pub events: Vec<Event>,
    pub sequence: u64,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub melee: Option<crate::melee::State>,
}
impl State {
    pub fn emit(&mut self, sound: usize, owner: i32, tick: u64) {
        self.sequence += 1;
        if self.events.len() == 256 {
            self.events.remove(0);
        }
        self.events.push(Event {
            sequence: self.sequence,
            tick,
            sound,
            owner,
        });
    }
}
impl Assets {
    pub fn validate(&self) -> Result<(), String> {
        if let Some(jump) = &self.jump {
            jump.validate()?;
            if self.bot.is_none()
                || self.lunge.is_none()
                || self.navigation.is_none()
                || self.melee.is_none()
            {
                return Err("Defensive jump requires bot, lunge, navigation and melee".into());
            }
        }
        if let Some(bot) = &self.bot {
            bot.validate()?;
            if self.navigation.is_none() {
                return Err("Bot goal profile requires navigation".into());
            }
        }
        if let Some(rules) = &self.game_rules {
            rules.validate()?;
            if self.melee.is_none() {
                return Err("GameRules require prepared damage".into());
            }
        }
        if self.lunge.is_some() && (self.navigation.is_none() || self.melee.is_none()) {
            return Err("Lunge requires navigation and melee".into());
        }
        if let Some(nav) = &self.navigation {
            nav.validate()?;
            if self.melee.is_none() {
                return Err("Navigation requires melee rules".into());
            }
        }
        if self.ballistics.as_ref().is_some_and(|b| {
            b.fire_offset
                .iter()
                .any(|v| !v.is_finite() || v.abs() > 1000.)
                || b.proximity_damage > 10000
        }) {
            return Err("Invalid ballistics profile".into());
        }
        if let Some(melee) = &self.melee {
            melee.validate()?;
        }
        if self.magazine == 0
            || self.magazine > 1000
            || self.total_ammo < self.magazine
            || self.total_ammo > 10000
            || self.damage == 0
            || self.damage > 10000
            || self.enemy_health == 0
            || self.enemy_health > 10000
            || [
                self.shot_delay,
                self.speed,
                self.max_speed,
                self.lifespan,
                self.reload_duration,
            ]
            .iter()
            .any(|v| !v.is_finite() || *v <= 0. || *v > 100000.)
            || self.shot_delay < STEP
            || self.lifespan > 60.
            || !self.acceleration.is_finite()
            || !(0. ..=1e6).contains(&self.acceleration)
            || self.sounds.is_empty()
            || self.sounds.len() > 64
            || self.fire_sound >= self.sounds.len()
            || self.notifies.len() > 256
            || self.sounds.iter().map(|s| s.wave.len()).sum::<usize>() > 8 * 1024 * 1024
        {
            return Err("Invalid combat asset budget/rules".into());
        }
        for sound in &self.sounds {
            validate_wave(&sound.wave)?;
        }
        for n in &self.notifies {
            if n.sound >= self.sounds.len() || !n.time.is_finite() || !(0. ..1.).contains(&n.time) {
                return Err("Invalid combat notify".into());
            }
        }
        Ok(())
    }
    pub fn sound_packet(&self) -> Vec<u8> {
        let mut out = b"RCSND001".to_vec();
        out.extend_from_slice(&(self.sounds.len() as u32).to_le_bytes());
        for s in &self.sounds {
            out.extend_from_slice(&(s.wave.len() as u32).to_le_bytes());
            out.extend_from_slice(&s.wave);
        }
        out
    }
}
/// The selected assets are PCM WAV. Reject malformed sizes/chunks before they reach a host decoder.
pub fn validate_wave(b: &[u8]) -> Result<(), String> {
    if b.len() < 44
        || b.len() > 2 * 1024 * 1024
        || &b[..4] != b"RIFF"
        || &b[8..12] != b"WAVE"
        || u32::from_le_bytes(b[4..8].try_into().unwrap()) as usize + 8 != b.len()
    {
        return Err("Invalid original WAV".into());
    }
    let mut pos = 12;
    let (mut format, mut data) = (false, false);
    while pos < b.len() {
        let header = b.get(pos..pos + 8).ok_or("Truncated WAV chunk")?;
        let len = u32::from_le_bytes(header[4..8].try_into().unwrap()) as usize;
        let chunk = b.get(pos + 8..pos + 8 + len).ok_or("Truncated WAV data")?;
        if &header[..4] == b"fmt " {
            // WAVEFORMATEX PCM may carry cbSize=0 (18 bytes). The dispenser's
            // original mechanical cues use this form; samples remain unchanged.
            if format || !(chunk.len() == 16 || chunk.len() == 18 && chunk[16..18] == [0, 0]) {
                return Err("Unsupported WAV format".into());
            }
            let u16at = |i| u16::from_le_bytes(chunk[i..i + 2].try_into().unwrap());
            let rate = u32::from_le_bytes(chunk[4..8].try_into().unwrap());
            if u16at(0) != 1
                || u16at(2) != 1
                || u16at(14) != 16
                || u16at(12) != 2
                || !(8000..=48000).contains(&rate)
                || u32::from_le_bytes(chunk[8..12].try_into().unwrap()) != rate * 2
            {
                return Err("Unsupported PCM WAV".into());
            }
            format = true;
        } else if &header[..4] == b"data" {
            if data || chunk.is_empty() || chunk.len() % 2 != 0 {
                return Err("Invalid WAV samples".into());
            }
            data = true;
        }
        pos = pos.checked_add(8 + len + (len & 1)).ok_or("WAV overflow")?;
    }
    if pos != b.len() || !format || !data {
        return Err("Incomplete WAV".into());
    }
    Ok(())
}

impl Game {
    pub fn attach_combat(&mut self, assets: Assets) -> Result<(), String> {
        assets.validate()?;
        if self.combat.is_some() || self.dc15.is_some() {
            return Err("Combat already attached".into());
        }
        let visual = self
            .visuals
            .as_ref()
            .ok_or("Combat needs animated assets")?;
        for n in &assets.notifies {
            let mesh = if n.weapon {
                &visual.weapon
            } else {
                &visual.enemy
            };
            mesh.clip(&n.clip)?;
        }
        self.state.ammo = assets.magazine;
        self.state.reserve = assets.total_ammo - assets.magazine;
        for e in &mut self.state.enemies {
            e.health = assets.enemy_health;
        }
        if let Some(melee) = &assets.melee {
            melee.validate_clips(&visual.enemy)?;
        }
        if assets.lunge.is_some() {
            visual.enemy.validate_lunge()?;
        }
        if let Some(jump) = &assets.jump {
            jump.validate_mesh(&visual.enemy)?;
        }
        let mut state = State::default();
        if assets.jump.is_some() {
            state.jump = Some(crate::jump::State::new(self.state.enemies.len()));
        }
        if let Some(melee) = &assets.melee {
            state.melee = Some(melee.state(self.state.enemies.len(), assets.enemy_health));
            self.state.health = crate::melee::display_health(melee.player_health);
        }
        self.state.combat = Some(state);
        self.initial = self.state.clone();
        self.combat = Some(assets);
        Ok(())
    }
    pub(crate) fn projectile_fire(&mut self) -> Result<(), String> {
        let assets = self.combat.as_ref().ok_or("Missing combat assets")?;
        let eye = self.eye();
        let d = direction(self.state.yaw, self.state.pitch);
        let mut origin = eye;
        let mut aim = d;
        let mut blocked = false;
        let mut proximity_target = None;
        if let Some(profile) = &assets.ballistics {
            let rotation = [
                (self.state.pitch * 65536. / std::f64::consts::TAU) as i32,
                (self.state.yaw * 65536. / std::f64::consts::TAU) as i32,
                0,
            ];
            let matrix = rc_package::mesh_transform::rotation_matrix(rotation, [0; 3])?;
            let axes =
                std::array::from_fn(|r| std::array::from_fn(|i| f32::from_bits(matrix[r][i])));
            origin = rc_package::projectile_motion::fire_start(
                eye.map(|v| v as f32),
                axes,
                profile.fire_offset,
            )
            .map(f64::from);
            let view = axes[0].map(f64::from);
            // Selected PlayerController.AdjustAim direct-trace branch. Optional
            // PickTarget/bone assistance and stochastic aim error remain excluded.
            let mut spot = std::array::from_fn(|i| f64::from(eye[i] as f32 + axes[0][i] * 10000.));
            spot[1] = f64::from(spot[1] as f32 - profile.fire_offset[1]);
            let mut hit = self.wall_fraction(eye, spot)?;
            for e in self.state.enemies.iter().filter(|e| e.health > 0) {
                if let Some(f) = ray_box(eye, spot, e.position, [22., 22., 60.]) {
                    hit = hit.min(f);
                }
            }
            let target = std::array::from_fn(|i| eye[i] + (spot[i] - eye[i]) * hit);
            aim = rc_package::projectile_motion::reticle_direction(
                origin.map(|v| v as f32),
                target.map(|v| v as f32),
                view.map(|v| v as f32),
            )
            .map(f64::from);
            let mut fraction = self.wall_fraction(eye, origin)?;
            for (i, e) in self
                .state
                .enemies
                .iter()
                .enumerate()
                .filter(|(_, e)| e.health > 0)
            {
                if let Some(f) = ray_box(eye, origin, e.position, [22., 22., 60.]) {
                    if f < fraction {
                        fraction = f;
                        proximity_target = Some(i);
                    }
                }
            }
            blocked = fraction < 1.;
        }
        let state = self.state.combat.as_mut().ok_or("Missing combat state")?;
        if state.projectiles.len() >= 1024 {
            return Err("Projectile budget exceeded".into());
        }
        if !blocked {
            state.projectiles.push(Projectile {
                velocity: assets.ballistics.as_ref().map(|_| {
                    aim.map(|v| {
                        if assets.laser_bolt {
                            0.
                        } else {
                            v as f32 * assets.speed as f32
                        }
                    })
                }),
                position: origin,
                direction: aim,
                speed: if assets.laser_bolt { 0. } else { assets.speed },
                age: 0.,
            });
        }
        state.emit(assets.fire_sound, -1, self.state.statistics.ticks);
        self.state.ammo -= 1;
        self.state.fire_remaining = assets.shot_delay;
        self.state.muzzle_flash = 0.08;
        self.state.statistics.shots += 1;
        if blocked {
            self.state.impact = Some(std::array::from_fn(|i| eye[i] + d[i] * 50.));
            if let Some(i) = proximity_target {
                let damage = assets.ballistics.as_ref().unwrap().proximity_damage;
                self.projectile_hit(i, damage)?;
            } else {
                let fraction = self.wall_fraction(eye, origin)?;
                let damage = self
                    .combat
                    .as_ref()
                    .unwrap()
                    .ballistics
                    .as_ref()
                    .unwrap()
                    .proximity_damage;
                self.rigid_projectile_hit(eye, origin, fraction, damage, aim)?;
            }
        }
        Ok(())
    }
    fn projectile_hit(&mut self, i: usize, damage: u32) -> Result<(), String> {
        let context = self.damage_context(self.state.enemies[i].position, false)?;
        let health =
            if let Some(rules) = &self.combat.as_ref().ok_or("Missing combat assets")?.melee {
                let state = self
                    .state
                    .combat
                    .as_mut()
                    .unwrap()
                    .melee
                    .as_mut()
                    .ok_or("Missing melee state")?;
                crate::melee::damage(
                    &mut state.enemies[i],
                    &rules.enemy,
                    &rules.projectile_type,
                    rc_package::pawn_damage::Hit {
                        damage: damage as f32,
                        bone: "",
                        damage_type: None,
                        bypass_shields: rules.projectile_bypass,
                        time: self.state.statistics.ticks as f32 * STEP as f32,
                    },
                    0.,
                    context,
                );
                crate::melee::display_health(state.enemies[i].health)
            } else {
                self.state.enemies[i].health.saturating_sub(damage)
            };
        let e = &mut self.state.enemies[i];
        e.health = health;
        // Neutral-zone reduction returns before Pawn's wound/animation path.
        // Geometric hit feedback is separate from an injured actor reaction.
        if !context.1 {
            e.flash = 0.12;
        }
        self.state.hit_flash = 0.12;
        self.state.statistics.hits += 1;
        self.state.statistics.kills += u32::from(e.health == 0);
        Ok(())
    }
    pub(crate) fn projectile_tick(&mut self) -> Result<(), String> {
        if self.combat.is_none() {
            return Ok(());
        }
        let assets = self.combat.as_ref().unwrap();
        let (accel, max_speed, damage, lifespan) = (
            assets.acceleration,
            assets.max_speed,
            assets.damage,
            assets.lifespan,
        );
        let mut projectiles = std::mem::take(
            &mut self
                .state
                .combat
                .as_mut()
                .ok_or("Missing combat state")?
                .projectiles,
        );
        let mut live = vec![];
        for mut p in projectiles.drain(..) {
            p.age = if p.velocity.is_some() {
                f64::from(p.age as f32 + STEP as f32)
            } else {
                p.age + STEP
            };
            if p.age > lifespan {
                continue;
            }
            let end = if let Some(v) = p.velocity {
                let a = p.direction.map(|d| d as f32 * accel as f32);
                let (v, delta) =
                    rc_package::projectile_motion::step(v, a, max_speed as f32, STEP as f32);
                p.velocity = Some(v);
                p.speed = f64::from(((v[0] * v[0] + v[1] * v[1]) + v[2] * v[2]).sqrt());
                std::array::from_fn(|i| f64::from(p.position[i] as f32 + delta[i]))
            } else {
                p.speed = (p.speed + accel * STEP).min(max_speed);
                std::array::from_fn(|i| p.position[i] + p.direction[i] * p.speed * STEP)
            };
            let mut fraction = self.wall_fraction(p.position, end)?;
            let mut target = None;
            for (i, e) in self
                .state
                .enemies
                .iter()
                .enumerate()
                .filter(|(_, e)| e.health > 0)
            {
                if let Some(f) = ray_box(p.position, end, e.position, [22., 22., 60.]) {
                    if f < fraction {
                        fraction = f;
                        target = Some(i);
                    }
                }
            }
            if fraction < 1. || target.is_some() {
                self.state.impact = Some(std::array::from_fn(|i| {
                    p.position[i] + (end[i] - p.position[i]) * fraction
                }));
                if let Some(i) = target {
                    self.projectile_hit(i, damage)?;
                } else {
                    self.rigid_projectile_hit(p.position, end, fraction, damage, p.direction)?;
                }
            } else {
                p.position = end;
                live.push(p);
            }
        }
        self.state.combat.as_mut().unwrap().projectiles = live;
        Ok(())
    }
}

#[cfg(test)]
mod wave_format_tests {
    use super::validate_wave;
    #[test]
    fn pcm_waveformatex_zero_extension_preserves_samples_and_rejects_unknown_extensions() {
        let mut wave = b"RIFF".to_vec();
        wave.extend(40u32.to_le_bytes());
        wave.extend(b"WAVEfmt ");
        wave.extend(18u32.to_le_bytes());
        wave.extend([
            1, 0, 1, 0, 0x22, 0x56, 0, 0, 0x44, 0xac, 0, 0, 2, 0, 16, 0, 0, 0,
        ]);
        wave.extend(b"data");
        wave.extend(2u32.to_le_bytes());
        wave.extend([0x34, 0x12]);
        assert!(validate_wave(&wave).is_ok());
        wave[36] = 1; // cbSize requests an unsupported extension.
        assert!(validate_wave(&wave).is_err());
        wave[36] = 0;
        wave[20] = 2; // ADPCM must not enter the PCM-only host.
        assert!(validate_wave(&wave).is_err());
    }
}
