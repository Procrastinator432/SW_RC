//! Selected original artillery scripts/factories. Movement, effects and Warrior
//! melee use explicit bounded adapters; no general UnrealScript VM or CTBot claim.
use crate::{Game, Point, STEP};
use serde::{Deserialize, Serialize};
#[derive(Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Actor {
    pub original: u32,
    pub mesh: Option<crate::visual::Mesh>,
    pub location: [f32; 3],
    pub rotation: [i32; 3],
}
#[derive(Serialize, Deserialize)]
pub struct Notify {
    pub actor: usize,
    pub time: f32,
    pub sound: Option<usize>,
    pub bone: Option<String>,
    pub offset: [f32; 3],
    pub effect: Option<String>,
}
#[derive(Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Texture {
    pub source: String,
    pub width: usize,
    pub height: usize,
    pub pixels: Vec<u32>,
}
#[derive(Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Assets {
    pub actors: Vec<Actor>,
    pub warrior: Option<crate::visual::Mesh>,
    pub notifies: Vec<Notify>,
    pub sounds: Vec<crate::combat::Sound>,
    pub warning: usize,
    pub warning_seconds: f64,
    pub tutorial_source: String,
    pub tutorial_delay: f64,
    pub spawns: Vec<Point>,
    pub spawn_interval: f64,
    pub warrior_health: u32,
    pub warrior_speed: f64,
    pub missile_snapshot: Vec<u8>,
    pub projectile_starts: Vec<Point>,
    pub projectile_velocity: Point,
    pub projectile_interval: f64,
    pub projectile_lifespan: f64,
    pub damage: f32,
    pub radius: f32,
    pub whistle: usize,
    pub explosion: usize,
    pub beam_texture: Texture,
    pub ribbon_texture: Texture,
    pub provenance: serde_json::Value,
}
pub struct Prepared {
    pub assets: Assets,
    pub actors: Vec<crate::visual::PreparedMesh>,
    pub texture_bases: Vec<usize>,
    pub warrior: crate::visual::PreparedMesh,
    pub warrior_texture: usize,
    pub missile: rc_render::Scene,
    pub missile_texture: usize,
    pub sound_base: usize,
    pub beam_texture: usize,
    pub ribbon_texture: usize,
}
#[derive(Clone, Debug, Default, Serialize, serde::Deserialize)]
pub struct Factory {
    pub enabled: bool,
    pub spawned: usize,
    pub remaining: f64,
}
impl Factory {
    pub fn advance(
        &mut self,
        dt: f64,
        alive: usize,
        capacity: usize,
        max_alive: usize,
        interval: f64,
    ) -> bool {
        if !self.enabled || self.spawned >= capacity {
            return false;
        }
        self.remaining = (self.remaining - dt).max(0.);
        if alive >= max_alive || self.remaining > 1e-9 {
            return false;
        }
        self.spawned += 1;
        self.remaining = interval;
        true
    }
}
#[derive(Clone, Debug, Serialize, serde::Deserialize)]
pub struct Missile {
    pub position: Point,
    pub age: f64,
    pub original: u32,
}
#[derive(Clone, Debug, Serialize, serde::Deserialize)]
pub struct Warrior {
    #[serde(skip_serializing_if = "Option::is_none")]
    pub ai: Option<crate::warrior_ai::Agent>,
    pub original: u32,
    pub pawn: rc_package::controller::ControlledBody,
    pub health: u32,
    pub cooldown: f64,
    pub frame: f32,
    pub clip: String,
    pub yaw: f64,
}
#[derive(Clone, Debug, Default, Serialize, serde::Deserialize)]
pub struct Beam {
    pub notify: usize,
    pub tick: u64,
}
#[derive(Clone, Debug, Default, Serialize, serde::Deserialize)]
pub struct State {
    pub attack_tick: Option<u64>,
    pub destroyed_tick: Option<u64>,
    pub tutorial_tick: Option<u64>,
    pub warned: bool,
    pub warrior_factory: Factory,
    pub projectile_factory: Factory,
    pub warriors: Vec<Warrior>,
    pub missiles: Vec<Missile>,
    pub beams: Vec<Beam>,
    pub impacts: Vec<(u64, Point)>,
    pub effects: Vec<(u64, String)>,
    pub damage: Vec<crate::gunner_runtime::Damage>,
}
fn textures(scene: &mut rc_render::Scene, mesh: &crate::visual::PreparedMesh) -> usize {
    let base = scene.textures.len();
    scene
        .textures
        .extend(mesh.mesh.materials.iter().map(|m| rc_render::Texture {
            width: m.width,
            height: m.height,
            pixels: m.pixels.clone(),
        }));
    base
}
impl Game {
    pub fn attach_artillery(&mut self, mut a: Assets) -> Result<(), String> {
        if self.artillery.is_some()
            || self.spire.is_none()
            || self.combat.is_some()
            || a.actors.len() != 2
            || a.spawns.len() != 3
            || a.projectile_starts.len() != 4
            || a.notifies.len() > 64
            || a.sounds.len() > 24
            || a.warrior.is_none()
        {
            return Err("Invalid artillery extension".into());
        }
        for point in a
            .spawns
            .iter()
            .chain(&a.projectile_starts)
            .chain([&a.projectile_velocity])
        {
            if point.iter().any(|v| !v.is_finite() || v.abs() > 1e6) {
                return Err("Invalid artillery point".into());
            }
        }
        for value in [
            a.warning_seconds,
            a.tutorial_delay,
            a.spawn_interval,
            a.projectile_interval,
            a.projectile_lifespan,
            a.warrior_speed,
            f64::from(a.damage),
            f64::from(a.radius),
        ] {
            if !value.is_finite() || value <= 0. || value > 30000. {
                return Err("Invalid artillery timing/damage".into());
            }
        }
        if a.warrior_health == 0
            || a.warrior_health > 10000
            || [a.warning, a.whistle, a.explosion]
                .iter()
                .any(|i| *i >= a.sounds.len())
        {
            return Err("Invalid artillery roles".into());
        }
        for t in [&a.beam_texture, &a.ribbon_texture] {
            if t.width == 0
                || t.height == 0
                || t.width > 256
                || t.height > 256
                || t.pixels.len() != t.width * t.height
            {
                return Err("Invalid artillery effect texture".into());
            }
        }
        for s in &a.sounds {
            crate::combat::validate_wave(&s.wave)?;
        }
        for n in &a.notifies {
            if n.actor >= 2
                || !n.time.is_finite()
                || !(0. ..1.).contains(&n.time)
                || n.sound.is_some_and(|i| i >= a.sounds.len())
                || n.offset.iter().any(|v| !v.is_finite())
            {
                return Err("Invalid artillery notify".into());
            }
        }
        let mut actors = vec![];
        for (i, actor) in a.actors.iter_mut().enumerate() {
            if actor.original != [580, 31][i] || actor.location.iter().any(|v| !v.is_finite()) {
                return Err("Invalid original artillery actor".into());
            }
            let mesh = actor
                .mesh
                .take()
                .ok_or("Missing artillery mesh")?
                .prepare_reference()?;
            mesh.clip("GunshipAmbushSpiderdroid")?;
            mesh.clip(if i == 0 {
                "GunshipAmbushSpiderdroidLoop"
            } else {
                "DeathBreathe"
            })?;
            actors.push(mesh);
        }
        let warrior = a.warrior.take().unwrap().prepare()?;
        for name in ["ActionBreathe", "RunForward", "Fire", "hit", "Death"] {
            warrior.clip(name)?;
        }
        let mut missile = rc_render::Scene::from_snapshot(&a.missile_snapshot)?;
        if missile.triangles.len() > 1000 || missile.textures.len() > 8 {
            return Err("Invalid artillery missile geometry".into());
        }
        let texture_bases = actors
            .iter()
            .map(|m| textures(&mut self.scene, m))
            .collect();
        let warrior_texture = textures(&mut self.scene, &warrior);
        let missile_texture = self.scene.textures.len();
        self.scene.textures.append(&mut missile.textures);
        let beam_texture = self.scene.textures.len();
        self.scene.textures.push(rc_render::Texture {
            width: a.beam_texture.width,
            height: a.beam_texture.height,
            pixels: a.beam_texture.pixels.clone(),
        });
        let ribbon_texture = self.scene.textures.len();
        self.scene.textures.push(rc_render::Texture {
            width: a.ribbon_texture.width,
            height: a.ribbon_texture.height,
            pixels: a.ribbon_texture.pixels.clone(),
        });
        let sound_base = self.mission.as_ref().unwrap().sounds.len();
        self.mission.as_mut().unwrap().sounds.append(&mut a.sounds);
        self.artillery = Some(Prepared {
            assets: a,
            actors,
            texture_bases,
            warrior,
            warrior_texture,
            missile,
            missile_texture,
            sound_base,
            beam_texture,
            ribbon_texture,
        });
        self.state.artillery = Some(State::default());
        self.initial = self.state.clone();
        Ok(())
    }
    fn artillery_sound(&mut self, role: usize) {
        let base = self.artillery.as_ref().unwrap().sound_base;
        let m = self.state.mission.as_mut().unwrap();
        m.audio_sequence += 1;
        if m.events.len() >= 256 {
            m.events.remove(0);
        }
        m.events.push(crate::combat::Event {
            sequence: m.audio_sequence,
            tick: self.state.statistics.ticks,
            sound: base + role,
            owner: -1,
        });
    }
    pub(crate) fn artillery_tick(&mut self) -> Result<(), String> {
        let Some(p) = &self.artillery else {
            return Ok(());
        };
        let tick = self.state.statistics.ticks;
        let start = self
            .state
            .spire
            .as_ref()
            .unwrap()
            .effects
            .iter()
            .find_map(|(tick, e)| {
                matches!(e,crate::spire::Effect::Event(tag) if tag=="GunshipAttack")
                    .then_some(*tick)
            });
        let tutorial = self
            .state
            .mission
            .as_ref()
            .unwrap()
            .events
            .iter()
            .find_map(|e| {
                (self
                    .mission
                    .as_ref()
                    .unwrap()
                    .sounds
                    .get(e.sound)
                    .is_some_and(|sound| sound.source == p.assets.tutorial_source))
                .then_some(e.tick)
            });
        let mut sounds = vec![];
        let mut fx = vec![];
        let mut effect_labels = vec![];
        let s = self.state.artillery.as_mut().unwrap();
        if s.tutorial_tick.is_none() {
            s.tutorial_tick = tutorial;
        }
        if let Some(begin) = s.tutorial_tick {
            let age = (tick - begin) as f64 * STEP;
            if age >= p.assets.tutorial_delay && !s.warned {
                s.warned = true;
                sounds.push(p.assets.warning);
                effect_labels.push("Tutorial3554".into());
            }
            if age >= p.assets.tutorial_delay + p.assets.warning_seconds
                && !s.warrior_factory.enabled
            {
                s.warrior_factory.enabled = true;
                effect_labels.push("SpawnWarriorAttack".into());
            }
        }
        if let Some(begin) = start {
            if s.attack_tick.is_none() {
                s.attack_tick = Some(begin);
                s.projectile_factory.enabled = true;
                effect_labels.extend([
                    "REP_Gunship_GroundDust".into(),
                    "SpiderDroid.Team5.Invulnerable".into(),
                ]);
            }
            let old = if tick == begin {
                -f32::EPSILON
            } else {
                (tick - begin - 1) as f32 * STEP as f32
            };
            let now = (tick - begin) as f32 * STEP as f32;
            s.beams.retain(|b| tick - b.tick < 181);
            for (index, n) in p.assets.notifies.iter().enumerate() {
                let mesh = &p.actors[n.actor];
                let clip = mesh.clip("GunshipAmbushSpiderdroid")?;
                let seq = &mesh.mesh.clips[clip].sequence;
                let seconds = seq.frames as f32 / f32::from_bits(seq.rate_bits);
                if notify_crossed(old, now, seconds, n.time) {
                    if let Some(sound) = n.sound {
                        sounds.push(sound);
                    }
                    if let Some(effect) = &n.effect {
                        let position = actor_bone(
                            p,
                            n.actor,
                            now / seconds,
                            n.bone.as_deref().unwrap_or("root"),
                            n.offset,
                        )?;
                        if effect == "CTEffects.SpiderDroid_BeamAutoDestroy" {
                            s.beams.push(Beam {
                                notify: index,
                                tick,
                            });
                        } else {
                            fx.push(position);
                        }
                        effect_labels.push(effect.clone());
                    }
                }
            }
            let spider = &p.actors[1];
            let seq = &spider.mesh.clips[spider.clip("GunshipAmbushSpiderdroid")?].sequence;
            let end = (seq.frames - 1) as f32 / f32::from_bits(seq.rate_bits);
            if now >= end && s.destroyed_tick.is_none() {
                s.destroyed_tick = Some(tick);
                effect_labels.extend([
                    "SpiderDroidDestroyed".into(),
                    "SpiderDroid.Team0.DeathBreathe".into(),
                ]);
            }
            if s.projectile_factory.advance(
                STEP,
                s.missiles.len(),
                4,
                4,
                p.assets.projectile_interval,
            ) {
                let i = s.projectile_factory.spawned - 1;
                s.missiles.push(Missile {
                    position: p.assets.projectile_starts[i],
                    age: 0.,
                    original: 1013 + i as u32,
                });
                sounds.push(p.assets.whistle);
            }
        }
        let alive = s.warriors.iter().filter(|w| w.health > 0).count();
        if s.warrior_factory
            .advance(STEP, alive, 3, 1, p.assets.spawn_interval)
        {
            let i = s.warrior_factory.spawned - 1;
            s.warriors.push(Warrior {
                ai: None,
                original: [1018, 1028, 1029][i],
                pawn: rc_package::controller::ControlledBody {
                    body: rc_package::physics::BodyState {
                        position: p.assets.spawns[i],
                        velocity: [0.; 3],
                        grounded: true,
                    },
                    jump_held: false,
                },
                health: p.assets.warrior_health,
                cooldown: 1.4,
                frame: 0.,
                clip: "ActionBreathe".into(),
                yaw: 0.,
            });
            effect_labels.push(format!("WarriorSpawn{}", s.warriors[i].original));
        }
        let mut impacts = vec![];
        let mut missiles = vec![];
        for mut m in std::mem::take(&mut s.missiles) {
            let next =
                std::array::from_fn(|i| m.position[i] + p.assets.projectile_velocity[i] * STEP);
            let h = self.world.sweep_motion(m.position, next, [0.; 3])?;
            if !h.complete {
                return Err("Incomplete artillery collision".into());
            }
            m.age += STEP;
            if let Some(hit) = h.contacts.first() {
                let at = std::array::from_fn(|i| {
                    m.position[i] + (next[i] - m.position[i]) * hit.hit.fraction
                });
                impacts.push(at);
            } else if m.age < p.assets.projectile_lifespan {
                m.position = next;
                missiles.push(m);
            } else {
                effect_labels.push("ProjectileExpired".into());
            }
        }
        s.missiles = missiles;
        for label in effect_labels {
            s.effects.push((tick, label));
        }
        for at in impacts {
            let damage = self.blast_damage(at, p.assets.damage, p.assets.radius)?;
            for d in &damage {
                if d.victim < 0 {
                    self.state.health = self.state.health.saturating_sub(d.amount);
                    self.state.damage_flash = 0.2;
                }
            }
            self.state.artillery.as_mut().unwrap().damage.extend(damage);
            self.state
                .artillery
                .as_mut()
                .unwrap()
                .impacts
                .push((tick, at));
            fx.push(at);
            sounds.push(p.assets.explosion);
        }
        let blast_points = self
            .state
            .artillery
            .as_ref()
            .unwrap()
            .impacts
            .iter()
            .filter(|(t, _)| *t == tick)
            .map(|(_, at)| *at)
            .collect::<Vec<_>>();
        for at in blast_points {
            let p = self.artillery.as_ref().unwrap();
            let (damage, radius) = (p.assets.damage, p.assets.radius);
            self.warrior_blast(at, damage, radius)?;
            self.squad_blast(at, damage, radius)?;
        }
        for at in fx {
            self.state
                .bunker
                .as_mut()
                .unwrap()
                .explosions
                .push(crate::gunner_runtime::Explosion {
                    position: at,
                    age: 0.,
                    bunker: false,
                });
        }
        for sound in sounds {
            self.artillery_sound(sound);
        }
        self.warrior_tick()?;
        Ok(())
    }
    fn warrior_tick(&mut self) -> Result<(), String> {
        if self.warrior_ai.is_some() {
            return self.warrior_ai_tick();
        }
        let p = self.artillery.as_ref().unwrap();
        let player = self.state.pawn.body.position;
        let mut profile = rc_package::movement_profile::MovementProfile {
            controller: self.profile.controller,
            ratios: self.profile.ratios,
            physics: self.profile.physics,
            properties: std::collections::BTreeMap::new(),
        };
        profile.physics.extent = [50., 50., 115.];
        profile.controller.speed = p.assets.warrior_speed;
        let state = self.state.artillery.as_mut().unwrap();
        for w in &mut state.warriors {
            let previous_clip = w.clip.clone();
            if w.health == 0 {
                w.clip = "Death".into();
            } else {
                w.cooldown = (w.cooldown - STEP).max(0.);
                let from = w.pawn.body.position;
                let d = crate::distance(from, player);
                w.yaw = (player[1] - from[1]).atan2(player[0] - from[0]);
                if d < 2600. && d > 140. {
                    let next = self.volumes.pawn_diagnostic_tick(
                        &self.world,
                        &w.pawn,
                        rc_package::controller::ViewInput {
                            forward: 1.,
                            strafe: 0.,
                            yaw: (w.yaw * 65536. / std::f64::consts::TAU) as i32,
                            walking: false,
                            jump: false,
                        },
                        STEP,
                        &profile,
                        rc_package::pawn_motion::PawnRuntimeOptions {
                            maximum_desired_speed: p.assets.warrior_speed,
                            weapon_modifier: 1.,
                            crouch_ratio: 1.,
                            wounded_ratio: 1.,
                            crouched: false,
                            wounded: false,
                            wants_to_crouch: false,
                            current_jump_z: profile.controller.jump_speed,
                        },
                    )?;
                    w.pawn = next.state;
                    w.clip = if crate::distance(from, w.pawn.body.position) > 0.01 {
                        "RunForward"
                    } else {
                        "ActionBreathe"
                    }
                    .into();
                } else {
                    w.clip = "ActionBreathe".into();
                }
                if d < 170.
                    && w.cooldown == 0.
                    && self
                        .world
                        .sweep_motion(from, player, [0.; 3])?
                        .contacts
                        .is_empty()
                {
                    self.state.health = self.state.health.saturating_sub(5);
                    self.state.damage_flash = 0.2;
                    self.state.statistics.enemy_shots += 1;
                    w.cooldown = 1.4;
                    w.clip = "Fire".into();
                    state
                        .effects
                        .push((self.state.statistics.ticks, "WarriorMelee".into()));
                }
            }
            if previous_clip != w.clip {
                w.frame = 0.;
            }
            let seq = &p.warrior.mesh.clips[p.warrior.clip(&w.clip)?].sequence;
            let next = w.frame + f32::from_bits(seq.rate_bits) * STEP as f32 / seq.frames as f32;
            w.frame = if w.health == 0 {
                next.min(1. - 1. / seq.frames as f32)
            } else {
                next.rem_euclid(1.)
            };
        }
        Ok(())
    }
    pub(crate) fn warrior_blast(
        &mut self,
        position: Point,
        damage: f32,
        radius: f32,
    ) -> Result<(), String> {
        let Some(state) = &self.state.artillery else {
            return Ok(());
        };
        let mut hits = vec![];
        for (i, w) in state.warriors.iter().enumerate() {
            if w.health == 0 {
                continue;
            }
            let at = w.pawn.body.position;
            let d = crate::distance(at, position) as f32;
            if d < radius + 50. && self.wall_fraction(position, at)? >= 1. {
                let amount = crate::detonator::radius_damage(damage, radius, d, 50.)?
                    .max(0.)
                    .ceil() as u32;
                if amount > 0 {
                    hits.push((i, at, amount));
                }
            }
        }
        let state = self.state.artillery.as_mut().unwrap();
        for (i, at, amount) in hits {
            let w = &mut state.warriors[i];
            if let Some(ai) = &mut w.ai {
                w.frame = 0.;
                ai.interrupt();
            }
            let old = w.health;
            w.health = w.health.saturating_sub(amount);
            state.damage.push(crate::gunner_runtime::Damage {
                tick: self.state.statistics.ticks,
                victim: 10000 + i as i32,
                amount,
                position: at,
            });
            if old > 0 && w.health == 0 {
                w.frame = 0.;
                self.state.statistics.kills += 1;
                state
                    .effects
                    .push((self.state.statistics.ticks, "AttackWarriorDead".into()));
            }
        }
        Ok(())
    }
    pub(crate) fn warrior_hit(&mut self, index: usize, damage: u32) {
        let w = &mut self.state.artillery.as_mut().unwrap().warriors[index];
        if let Some(ai) = &mut w.ai {
            w.frame = 0.;
            ai.interrupt();
        }
        let before = w.health;
        w.health = w.health.saturating_sub(damage);
        self.state.hit_flash = 0.12;
        self.state.statistics.hits += 1;
        if before > 0 && w.health == 0 {
            w.frame = 0.;
            self.state.statistics.kills += 1;
            self.state
                .artillery
                .as_mut()
                .unwrap()
                .effects
                .push((self.state.statistics.ticks, "AttackWarriorDead".into()));
        }
    }
    pub(crate) fn artillery_geometry(
        &self,
    ) -> Result<
        (
            Vec<rc_package::geometry::Triangle>,
            Vec<Option<rc_render::FaceTexture>>,
        ),
        String,
    > {
        let (Some(p), Some(s)) = (&self.artillery, &self.state.artillery) else {
            return Ok((vec![], vec![]));
        };
        let mut triangles = vec![];
        let mut faces = vec![];
        if let Some(tick) = s.attack_tick {
            let elapsed = (self.state.statistics.ticks - tick) as f32 * STEP as f32;
            for i in 0..2 {
                let mesh = &p.actors[i];
                let attack = mesh.clip("GunshipAmbushSpiderdroid")?;
                let seq = &mesh.mesh.clips[attack].sequence;
                let seconds = (seq.frames - 1) as f32 / f32::from_bits(seq.rate_bits);
                let (clip, frame) = if elapsed < seconds {
                    (
                        attack,
                        elapsed * f32::from_bits(seq.rate_bits) / seq.frames as f32,
                    )
                } else {
                    let c = mesh.clip(if i == 0 {
                        "GunshipAmbushSpiderdroidLoop"
                    } else {
                        "DeathBreathe"
                    })?;
                    let seq = &mesh.mesh.clips[c].sequence;
                    (
                        c,
                        ((elapsed - seconds) * f32::from_bits(seq.rate_bits) / seq.frames as f32)
                            .rem_euclid(1.),
                    )
                };
                let mut scene = mesh.geometry_with_root(&mesh.channel(clip, frame)?, false)?;
                let delta = if clip == attack {
                    mesh.cinematic_root_delta(clip, frame)?
                } else {
                    let end = mesh.cinematic_root_delta(attack, 1. - 1. / seq.frames as f32)?;
                    let current = mesh.cinematic_root_delta(clip, frame)?;
                    std::array::from_fn(|i| end[i] + current[i])
                };
                let actor = &p.assets.actors[i];
                for t in &mut scene.triangles {
                    t.points = t.points.map(|v| {
                        rc_package::mesh::transform_point(
                            std::array::from_fn(|i| v[i] + delta[i]),
                            actor.location,
                            actor.rotation,
                            [1.; 3],
                            [0.; 3],
                        )
                    });
                }
                append(scene, p.texture_bases[i], &mut triangles, &mut faces);
            }
        }
        for w in &s.warriors {
            let clip = p.warrior.clip(&w.clip)?;
            let mut scene = p
                .warrior
                .geometry_with_root(&p.warrior.channel(clip, w.frame)?, true)?;
            let (sin, cos) = (w.yaw as f32).sin_cos();
            for t in &mut scene.triangles {
                t.points = t.points.map(|v| {
                    [
                        w.pawn.body.position[0] as f32 + v[0] * cos - v[1] * sin,
                        w.pawn.body.position[1] as f32 + v[0] * sin + v[1] * cos,
                        w.pawn.body.position[2] as f32 - 115. + v[2] - p.warrior.rest_bounds[0][2],
                    ]
                });
            }
            append(scene, p.warrior_texture, &mut triangles, &mut faces);
        }
        for m in &s.missiles {
            let mut scene = rc_render::Scene::new(p.missile.triangles.clone())?;
            scene.appearance = p.missile.appearance.clone();
            for t in &mut scene.triangles {
                t.points = t
                    .points
                    .map(|v| std::array::from_fn(|i| v[i] + m.position[i] as f32));
            }
            append(scene, p.missile_texture, &mut triangles, &mut faces);
        }
        let camera = self.eye().map(|v| v as f32);
        if let Some(begin) = s.attack_tick {
            let elapsed = (self.state.statistics.ticks - begin) as f32 * STEP as f32;
            for beam in &s.beams {
                let n = &p.assets.notifies[beam.notify];
                let mesh = &p.actors[n.actor];
                let clip = mesh.clip("GunshipAmbushSpiderdroid")?;
                let seq = &mesh.mesh.clips[clip].sequence;
                let frame = elapsed * f32::from_bits(seq.rate_bits) / seq.frames as f32;
                let start = actor_bone(
                    p,
                    n.actor,
                    frame,
                    n.bone.as_deref().unwrap_or("root"),
                    n.offset,
                )?
                .map(|v| v as f32);
                let local = mesh.attachment_transform(
                    &mesh.channel(clip, frame.min(0.99))?,
                    n.bone.as_deref().unwrap_or("root"),
                    false,
                )?;
                let actor = &p.assets.actors[n.actor];
                let origin = rc_package::mesh::transform_point(
                    [0.; 3],
                    [0.; 3],
                    actor.rotation,
                    [1.; 3],
                    [0.; 3],
                );
                let axis = rc_package::mesh::transform_point(
                    local[0][..3].try_into().unwrap(),
                    [0.; 3],
                    actor.rotation,
                    [1.; 3],
                    [0.; 3],
                );
                let end = std::array::from_fn(|i| start[i] + (axis[i] - origin[i]) * 6000.);
                ribbon(
                    start,
                    end,
                    camera,
                    50.,
                    p.beam_texture,
                    &mut triangles,
                    &mut faces,
                );
            }
        }
        for m in &s.missiles {
            let start = m.position.map(|v| v as f32);
            let length = p
                .assets
                .projectile_velocity
                .iter()
                .map(|v| v * v)
                .sum::<f64>()
                .sqrt();
            let end = std::array::from_fn(|i| {
                start[i] - (p.assets.projectile_velocity[i] / length * 600.) as f32
            });
            ribbon(
                start,
                end,
                camera,
                15.,
                p.ribbon_texture,
                &mut triangles,
                &mut faces,
            );
        }
        Ok((triangles, faces))
    }
}
fn notify_crossed(old: f32, now: f32, seconds: f32, time: f32) -> bool {
    old < seconds && old / seconds < time && time <= (now / seconds).min(1.)
}
fn ribbon(
    start: [f32; 3],
    end: [f32; 3],
    eye: [f32; 3],
    width: f32,
    texture: usize,
    triangles: &mut Vec<rc_package::geometry::Triangle>,
    faces: &mut Vec<Option<rc_render::FaceTexture>>,
) {
    let d: [f32; 3] = std::array::from_fn(|i| end[i] - start[i]);
    let v: [f32; 3] = std::array::from_fn(|i| eye[i] - start[i]);
    let side = [
        d[1] * v[2] - d[2] * v[1],
        d[2] * v[0] - d[0] * v[2],
        d[0] * v[1] - d[1] * v[0],
    ];
    let norm = side.iter().map(|x| x * x).sum::<f32>().sqrt().max(1.);
    let points: [[f32; 3]; 4] = std::array::from_fn(|n| {
        std::array::from_fn(|i|if n<2{start[i]}else{end[i]}+side[i]/norm*width*if n%2==0{-1.}else{1.})
    });
    let uv = [[0., 0.], [1., 0.], [0., 1.], [1., 1.]];
    for ids in [[0, 1, 3], [0, 3, 2]] {
        triangles.push(rc_package::geometry::Triangle {
            points: ids.map(|i| points[i]),
            surface: 0xffffff,
        });
        faces.push(Some(rc_render::FaceTexture {
            texture,
            uv: ids.map(|i| uv[i]),
        }));
    }
}
fn append(
    scene: rc_render::Scene,
    base: usize,
    triangles: &mut Vec<rc_package::geometry::Triangle>,
    faces: &mut Vec<Option<rc_render::FaceTexture>>,
) {
    triangles.extend(scene.triangles);
    faces.extend(scene.appearance.into_iter().map(|f| {
        f.map(|mut f| {
            f.texture += base;
            f
        })
    }));
}
fn actor_bone(
    p: &Prepared,
    index: usize,
    frame: f32,
    bone: &str,
    offset: [f32; 3],
) -> Result<Point, String> {
    let mesh = &p.actors[index];
    let clip = mesh.clip("GunshipAmbushSpiderdroid")?;
    let matrix = mesh.attachment_transform(&mesh.channel(clip, frame.min(0.99))?, bone, false)?;
    let delta = mesh.cinematic_root_delta(clip, frame.min(0.99))?;
    let point = std::array::from_fn(|i| {
        matrix[3][i]
            + offset[0] * matrix[0][i]
            + offset[1] * matrix[1][i]
            + offset[2] * matrix[2][i]
            + delta[i]
    });
    let actor = &p.assets.actors[index];
    Ok(
        rc_package::mesh::transform_point(point, actor.location, actor.rotation, [1.; 3], [0.; 3])
            .map(f64::from),
    )
}
#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn factory_capacity_and_max_alive() {
        let mut f = Factory {
            enabled: true,
            ..Factory::default()
        };
        assert!(f.advance(STEP, 0, 3, 1, 0.5));
        for _ in 0..100 {
            assert!(!f.advance(STEP, 1, 3, 1, 0.5));
        }
        assert!(f.advance(STEP, 0, 3, 1, 0.5));
        assert!(!f.advance(STEP, 0, 3, 1, 0.5));
        assert!(f.advance(0.5, 0, 3, 1, 0.5));
        assert!(!f.advance(10., 0, 3, 1, 0.5));
        assert_eq!(f.spawned, 3);
    }
    #[test]
    fn nonlooping_attack_notifies_never_repeat_in_idle_loop() {
        for time in [0., 0.25, 0.52] {
            let mut count = 0;
            for tick in 0..3000 {
                let old = if tick == 0 {
                    -f32::EPSILON
                } else {
                    (tick - 1) as f32 / 60.
                };
                let now = tick as f32 / 60.;
                count += usize::from(notify_crossed(old, now, 475. / 30., time));
            }
            assert_eq!(count, 1);
        }
    }
    #[test]
    fn disabled_factory_does_not_consume_capacity() {
        let mut f = Factory::default();
        assert!(!f.advance(10., 0, 4, 4, 0.3));
        assert_eq!(f.spawned, 0);
    }
}
