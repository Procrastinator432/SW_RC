//! First playable vertical slice. Combat/encounter rules are explicit port policies,
//! not a claim of reconstructed CTGame combat or campaign-script equivalence.
pub mod bot;
pub mod bundle;
pub mod combat;
pub mod crash;
pub mod crash_runtime;
pub mod detonator;
pub mod detonator_flight;
pub mod dynamic;
pub mod game_rules;
pub mod grenade_pickup;
pub mod grenade_runtime;
pub mod grenade_throw;
pub mod gunner;
pub mod jump;
pub mod lunge;
pub mod melee;
pub mod mission;
pub mod navigation;
pub mod presentation;
pub mod replay;
pub mod visual;
use rc_package::{
    controller::{ControlledBody, ViewInput},
    movement::PlacementState,
    movement_profile::MovementProfile,
    pawn_motion::PawnRuntimeOptions,
    physics::BodyState,
    volumes::VolumeWorld,
    world_collision::StaticBodyWorld,
};
use serde::{Deserialize, Serialize};

pub const STEP: f64 = 1.0 / 60.0;
pub const MAGAZINE: u32 = 20;
pub type Point = [f64; 3];
#[derive(Default, Debug, Clone, Copy, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Input {
    pub forward: f64,
    /// Positive is screen right.
    pub strafe: f64,
    /// Radians of screen-space look motion since the last submitted frame.
    pub look: [f64; 2],
    pub jump: bool,
    pub fire: bool,
    pub reload: bool,
    pub walking: bool,
    #[serde(default, skip_serializing_if = "std::ops::Not::not")]
    pub use_action: bool,
}
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize)]
pub enum Phase {
    Playing,
    Won,
    Lost,
}
#[derive(Debug, Clone, Serialize)]
pub struct Enemy {
    pub position: Point,
    pub health: u32,
    pub cooldown: f64,
    pub flash: f64,
}
#[derive(Debug, Clone, Default, Serialize)]
pub struct Statistics {
    pub ticks: u64,
    pub shots: u32,
    pub hits: u32,
    pub kills: u32,
    pub jumps: u32,
    pub enemy_shots: u32,
    pub reloads: u32,
    pub discarded_seconds: f64,
}
#[derive(Debug, Clone, Serialize)]
pub struct State {
    pub pawn: ControlledBody,
    pub yaw: f64,
    pub pitch: f64,
    pub health: u32,
    pub ammo: u32,
    pub reserve: u32,
    pub reload_remaining: f64,
    pub fire_remaining: f64,
    pub phase: Phase,
    pub enemies: Vec<Enemy>,
    pub hit_flash: f64,
    pub damage_flash: f64,
    pub muzzle_flash: f64,
    pub impact: Option<Point>,
    pub statistics: Statistics,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub visuals: Option<visual::State>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub combat: Option<combat::State>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub mission: Option<mission::State>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub dynamic: Option<Vec<bool>>,
}
pub struct Game {
    pub scene: rc_render::Scene,
    pub world: StaticBodyWorld,
    pub volumes: VolumeWorld,
    pub profile: MovementProfile,
    pub state: State,
    pub exit: Point,
    pub visuals: Option<visual::PreparedAssets>,
    pub combat: Option<combat::Assets>,
    pub mission: Option<mission::Assets>,
    pub dynamic: Option<dynamic::Prepared>,
    pub pickup: Option<grenade_runtime::Prepared>,
    initial: State,
    accumulator: f64,
    pending_jump: bool,
    pending_fire: bool,
    pending_reload: bool,
}
fn distance(a: Point, b: Point) -> f64 {
    (0..3).map(|i| (a[i] - b[i]).powi(2)).sum::<f64>().sqrt()
}
pub fn direction(yaw: f64, pitch: f64) -> Point {
    [
        yaw.cos() * pitch.cos(),
        yaw.sin() * pitch.cos(),
        pitch.sin(),
    ]
}
impl Game {
    pub fn new(
        scene: rc_render::Scene,
        world: StaticBodyWorld,
        volumes: VolumeWorld,
        profile: MovementProfile,
        spawn: Point,
        yaw: f64,
    ) -> Result<Self, String> {
        if spawn
            .iter()
            .chain([yaw].iter())
            .any(|v| !v.is_finite() || v.abs() > 1e8)
        {
            return Err("Invalid game spawn".into());
        }
        if world.body_placement(spawn, profile.physics.extent)?.state == PlacementState::Penetrating
        {
            return Err("Player spawn penetrates collision".into());
        }
        let volume = volumes.select(spawn.map(|v| v as f32))?;
        if !volume.complete {
            return Err("Incomplete spawn volume".into());
        }
        volume.settings.apply(profile.physics)?;
        let grounded = world
            .floor_contact(
                spawn,
                profile.physics.extent,
                profile.physics.support_distance,
                profile.physics.minimum_up,
            )?
            .is_some();
        let state = State {
            pawn: ControlledBody {
                body: BodyState {
                    position: spawn,
                    velocity: [0.; 3],
                    grounded,
                },
                jump_held: false,
            },
            yaw,
            pitch: 0.,
            health: 100,
            ammo: MAGAZINE,
            reserve: 200,
            reload_remaining: 0.,
            fire_remaining: 0.,
            phase: Phase::Playing,
            enemies: vec![],
            hit_flash: 0.,
            damage_flash: 0.,
            muzzle_flash: 0.,
            impact: None,
            statistics: Statistics::default(),
            visuals: None,
            combat: None,
            mission: None,
            dynamic: None,
        };
        Ok(Self {
            scene,
            world,
            volumes,
            profile,
            state: state.clone(),
            exit: spawn,
            visuals: None,
            combat: None,
            mission: None,
            dynamic: None,
            pickup: None,
            initial: state,
            accumulator: 0.,
            pending_jump: false,
            pending_fire: false,
            pending_reload: false,
        })
    }
    pub fn attach_visuals(&mut self, assets: visual::Assets) -> Result<(), String> {
        if self.visuals.is_some() {
            return Err("Visual assets are already attached".into());
        }
        let enemy = assets.enemy.prepare()?;
        let weapon = assets.weapon.prepare()?;
        let hit = if enemy.mesh.source.eq_ignore_ascii_case("droid.BattleDroid") {
            "Impact"
        } else {
            "hit"
        };
        for name in ["ActionBreathe", "RunForward", "Fire", hit, "Death"] {
            enemy.clip(name)?;
        }
        for name in ["ActionBreathe", "Walk", "Fire", "Reload"] {
            weapon.clip(name)?;
        }
        let prepared = visual::PreparedAssets {
            enemy,
            weapon,
            texture_base: self.scene.textures.len(),
        };
        let state = prepared.state(self.state.enemies.len())?;
        for mesh in [&prepared.enemy, &prepared.weapon] {
            self.scene
                .textures
                .extend(mesh.mesh.materials.iter().map(|m| rc_render::Texture {
                    width: m.width,
                    height: m.height,
                    pixels: m.pixels.clone(),
                }));
        }
        self.state.visuals = Some(state);
        self.initial = self.state.clone();
        self.visuals = Some(prepared);
        Ok(())
    }
    /// Select an explicit bounded encounter, keeping its reset state separate from live state.
    pub fn set_encounter(&mut self, positions: &[Point], exit: Point) -> Result<(), String> {
        if positions.is_empty() || positions.len() > 16 || exit.iter().any(|v| !v.is_finite()) {
            return Err("Invalid encounter".into());
        }
        let mut enemies = vec![];
        for (i, p) in positions.iter().enumerate() {
            if p.iter().any(|v| !v.is_finite() || v.abs() > 1e8)
                || self.world.body_placement(*p, [22., 22., 60.])?.state
                    == PlacementState::Penetrating
            {
                return Err("Enemy spawn penetrates collision".into());
            }
            enemies.push(Enemy {
                position: *p,
                health: self.combat.as_ref().map_or(60, |a| a.enemy_health),
                cooldown: 2. + i as f64 * 0.7,
                flash: 0.,
            });
        }
        if self
            .world
            .body_placement(exit, self.profile.physics.extent)?
            .state
            == PlacementState::Penetrating
        {
            return Err("Exit penetrates collision".into());
        }
        self.state.enemies = enemies;
        if self.combat.is_some() {
            self.state.combat = Some(combat::State::default());
            if self.combat.as_ref().is_some_and(|a| a.jump.is_some()) {
                self.state.combat.as_mut().unwrap().jump =
                    Some(jump::State::new(self.state.enemies.len()));
            }
            if let Some(rules) = self.combat.as_ref().and_then(|a| a.melee.as_ref()) {
                self.state.combat.as_mut().unwrap().melee = Some(rules.state(
                    self.state.enemies.len(),
                    self.combat.as_ref().unwrap().enemy_health,
                ));
                self.state.health = melee::display_health(rules.player_health);
            }
        }
        if let Some(assets) = &self.visuals {
            self.state.visuals = Some(assets.state(self.state.enemies.len())?);
        }
        self.initial = self.state.clone();
        self.exit = exit;
        Ok(())
    }
    pub fn restart(&mut self) {
        self.state = self.initial.clone();
        self.sync_dynamic();
        self.accumulator = 0.;
        self.pending_jump = false;
        self.pending_fire = false;
        self.pending_reload = false;
    }
    /// Clear queued input/time on focus loss or pause; no background catch-up on resume.
    pub fn suspend(&mut self) {
        self.accumulator = 0.;
        self.pending_jump = false;
        self.pending_fire = false;
        self.pending_reload = false;
        self.state.pawn.jump_held = false;
        if let Some(c) = &mut self.state.combat {
            c.events.clear();
        }
        if let Some(m) = &mut self.state.mission {
            m.events.clear();
            m.using = false;
        }
    }
    pub fn eye(&self) -> Point {
        let mut p = self.state.pawn.body.position;
        p[2] += self.profile.physics.extent[2] * 0.65;
        p
    }
    pub fn aim_at(&mut self, point: Point) {
        let eye = self.eye();
        let d: Point = std::array::from_fn(|i| point[i] - eye[i]);
        self.state.yaw = d[1].atan2(d[0]);
        self.state.pitch = d[2].atan2(d[0].hypot(d[1])).clamp(-1.4, 1.4);
    }
    /// All input and fixed ticks commit together; a collision error cannot leave a partial frame.
    pub fn advance(&mut self, elapsed: f64, input: Input) -> Result<u32, String> {
        let _timing =
            rc_package::performance::Span::new(rc_package::performance::Category::Simulation);
        if !elapsed.is_finite()
            || !(0.0..=0.25).contains(&elapsed)
            || [input.forward, input.strafe]
                .iter()
                .any(|v| !v.is_finite() || v.abs() > 1.)
            || input.look.iter().any(|v| !v.is_finite() || v.abs() > 6.)
        {
            return Err("Invalid game input/time".into());
        }
        if self.state.phase != Phase::Playing {
            return Ok(0);
        }
        let old = self.state.clone();
        let mut accumulator = self.accumulator + elapsed;
        let jump = self.pending_jump || input.jump;
        let fire = self.pending_fire || input.fire;
        let reload = self.pending_reload || input.reload;
        self.state.yaw = (self.state.yaw - input.look[0]).rem_euclid(std::f64::consts::TAU);
        self.state.pitch = (self.state.pitch - input.look[1]).clamp(-1.4, 1.4);
        let mut ticks = 0;
        while accumulator + 1e-12 >= STEP && ticks < 8 && self.state.phase == Phase::Playing {
            let tick_input = Input {
                jump: if ticks == 0 { jump } else { input.jump },
                fire: if ticks == 0 { fire } else { input.fire },
                reload: if ticks == 0 { reload } else { false },
                ..input
            };
            if let Err(e) = self.tick(tick_input) {
                self.state = old;
                self.sync_dynamic();
                return Err(e);
            }
            ticks += 1;
            accumulator = (accumulator - STEP).max(0.);
        }
        if ticks == 8 && accumulator >= STEP {
            let retained = accumulator % STEP;
            self.state.statistics.discarded_seconds += accumulator - retained;
            accumulator = retained;
        }
        self.accumulator = accumulator;
        self.pending_jump = ticks == 0 && jump;
        self.pending_fire = ticks == 0 && fire;
        self.pending_reload = ticks == 0 && reload;
        Ok(ticks)
    }
    fn tick(&mut self, input: Input) -> Result<(), String> {
        let previous = self.visuals.as_ref().map(|_| self.state.clone());
        let runtime = PawnRuntimeOptions {
            maximum_desired_speed: self.profile.controller.speed,
            weapon_modifier: 1.,
            crouch_ratio: 1.,
            wounded_ratio: 1.,
            crouched: false,
            wounded: false,
            wants_to_crouch: false,
            current_jump_z: self.profile.controller.jump_speed,
        };
        let moved = self.volumes.pawn_diagnostic_tick(
            &self.world,
            &self.state.pawn,
            ViewInput {
                forward: input.forward,
                strafe: -input.strafe,
                yaw: (self.state.yaw * 65536. / std::f64::consts::TAU) as i32,
                walking: input.walking,
                jump: input.jump,
            },
            STEP,
            &self.profile,
            runtime,
        )?;
        self.state.pawn = moved.state;
        self.state.statistics.ticks += 1;
        self.state.statistics.jumps += u32::from(moved.jump_started);
        for timer in [
            &mut self.state.hit_flash,
            &mut self.state.damage_flash,
            &mut self.state.muzzle_flash,
            &mut self.state.fire_remaining,
        ] {
            *timer = (*timer - STEP).max(0.);
        }
        let magazine = self.combat.as_ref().map_or(MAGAZINE, |a| a.magazine);
        if self.combat.is_some() && self.state.fire_remaining < 1e-8 {
            self.state.fire_remaining = 0.;
        }
        if self.state.reload_remaining > 0. {
            self.state.reload_remaining = (self.state.reload_remaining - STEP).max(0.);
            if self.state.reload_remaining == 0. {
                let count = (magazine - self.state.ammo).min(self.state.reserve);
                self.state.ammo += count;
                self.state.reserve -= count;
            }
        }
        if (input.reload || ((input.fire || self.combat.is_some()) && self.state.ammo == 0))
            && self.state.ammo < magazine
            && self.state.reserve > 0
            && self.state.reload_remaining == 0.
        {
            self.state.reload_remaining = self.combat.as_ref().map_or(1.2, |a| a.reload_duration);
            self.state.statistics.reloads += 1;
        }
        if input.fire
            && self.state.fire_remaining == 0.
            && self.state.reload_remaining == 0.
            && self.state.ammo > 0
        {
            self.fire()?;
        }
        self.projectile_tick()?;
        self.defensive_tick()?;
        self.enemy_tick()?;
        if let (Some(assets), Some(previous)) = (&self.visuals, previous) {
            let mut visual = self
                .state
                .visuals
                .take()
                .ok_or("Missing instance visual state")?;
            let events = assets.tick(&self.state, &previous, &mut visual, self.combat.as_ref())?;
            if let Some(c) = &mut self.state.combat {
                for (sound, owner) in events {
                    c.emit(sound, owner, self.state.statistics.ticks);
                }
            }
            self.state.visuals = Some(visual);
            self.lunge_motion_tick()?;
            if let Some(old_visual) = &previous.visuals {
                self.melee_notify_tick(old_visual)?;
            }
        }
        if self.state.health == 0
            || self.state.pawn.body.position[2]
                < f64::from(self.scene.center[2]) - f64::from(self.scene.radius) * 2.
        {
            if let Some(jump) = self.state.combat.as_mut().and_then(|c| c.jump.as_mut()) {
                jump.requests.fill(false);
                for (i, motion) in jump.motions.iter_mut().enumerate() {
                    if motion.take().is_some() {
                        self.state.enemies[i].position =
                            navigation::ground(&self.world, self.state.enemies[i].position, 4096.)?
                                .ok_or("Player defeat leaves unsupported jump")?;
                        jump.interrupted += 1;
                    }
                }
            }
            self.state.phase = Phase::Lost;
        } else if self.mission.is_none()
            && !self.state.enemies.is_empty()
            && self.state.enemies.iter().all(|e| e.health == 0)
            && distance(self.state.pawn.body.position, self.exit) < 140.
        {
            self.state.phase = Phase::Won;
        }
        if self.state.phase == Phase::Playing {
            self.mission_tick(input.use_action);
            self.crash_tick()?;
            self.grenade_tick()?;
        }
        Ok(())
    }
    /// Static body hulls are authoritative for combat occlusion in this first encounter.
    pub fn wall_fraction(&self, from: Point, to: Point) -> Result<f64, String> {
        let hit = self.world.sweep_motion(from, to, [0.; 3])?;
        if !hit.complete {
            return Err("Incomplete combat trace".into());
        }
        Ok(hit.contacts.first().map_or(1., |h| h.hit.fraction))
    }
    fn fire(&mut self) -> Result<(), String> {
        if self.combat.is_some() {
            return self.projectile_fire();
        }
        let eye = self.eye();
        let d = direction(self.state.yaw, self.state.pitch);
        let end = std::array::from_fn(|i| eye[i] + d[i] * 8000.);
        let mut fraction = self.wall_fraction(eye, end)?;
        let mut target = None;
        for (i, e) in self
            .state
            .enemies
            .iter()
            .enumerate()
            .filter(|(_, e)| e.health > 0)
        {
            if let Some(f) = ray_box(eye, end, e.position, [22., 22., 60.]) {
                if f < fraction {
                    fraction = f;
                    target = Some(i);
                }
            }
        }
        self.state.ammo -= 1;
        self.state.fire_remaining = 0.15;
        self.state.muzzle_flash = 0.08;
        self.state.statistics.shots += 1;
        if let Some(sound) = self.mission.as_ref().and_then(|a| a.weapon_fire_sound) {
            let m = self.state.mission.as_mut().ok_or("Missing mission state")?;
            m.audio_sequence += 1;
            if m.events.len() == 256 {
                m.events.remove(0);
            }
            m.events.push(combat::Event {
                sequence: m.audio_sequence,
                tick: self.state.statistics.ticks,
                sound,
                owner: -1,
            });
        }
        self.state.impact = Some(std::array::from_fn(|i| {
            eye[i] + (end[i] - eye[i]) * fraction
        }));
        if let Some(i) = target.filter(|_| !self.crash_invulnerable()) {
            let e = &mut self.state.enemies[i];
            e.health = e.health.saturating_sub(30);
            e.flash = 0.12;
            self.state.hit_flash = 0.12;
            self.state.statistics.hits += 1;
            self.state.statistics.kills += u32::from(e.health == 0);
        }
        Ok(())
    }
    fn enemy_tick(&mut self) -> Result<(), String> {
        if self.combat.as_ref().is_some_and(|a| a.melee.is_some()) {
            return self.melee_enemy_tick();
        }
        let player = self.state.pawn.body.position;
        let eye = self.eye();
        for i in 0..self.state.enemies.len() {
            let mut e = self.state.enemies[i].clone();
            e.flash = (e.flash - STEP).max(0.);
            if e.health == 0 {
                self.state.enemies[i] = e;
                continue;
            }
            e.cooldown = (e.cooldown - STEP).max(0.);
            let d = distance(e.position, player);
            let muzzle = [e.position[0], e.position[1], e.position[2] + 35.];
            let visible = d < 1600. && self.wall_fraction(muzzle, eye)? >= 1.;
            // A short, collision-tested approach; no pathfinding or original AI claim.
            if visible
                && d > 500.
                && (player[0] - e.position[0]).hypot(player[1] - e.position[1]) > 1.
            {
                let length = (player[0] - e.position[0]).hypot(player[1] - e.position[1]);
                let end = [
                    e.position[0] + (player[0] - e.position[0]) / length * 65. * STEP,
                    e.position[1] + (player[1] - e.position[1]) / length * 65. * STEP,
                    e.position[2],
                ];
                let sweep = self.world.sweep_motion(e.position, end, [22., 22., 60.])?;
                if !sweep.complete {
                    return Err("Incomplete enemy movement".into());
                }
                if sweep.contacts.is_empty()
                    && self
                        .world
                        .floor_contact(end, [22., 22., 60.], 4., 0.7)?
                        .is_some()
                {
                    e.position = end;
                }
            }
            if visible && e.cooldown == 0. {
                self.state.health = self.state.health.saturating_sub(5);
                self.state.damage_flash = 0.2;
                self.state.statistics.enemy_shots += 1;
                e.cooldown = 1.4;
                e.flash = 0.12;
            }
            self.state.enemies[i] = e;
        }
        Ok(())
    }
}
fn ray_box(from: Point, to: Point, center: Point, extent: Point) -> Option<f64> {
    let (mut lo, mut hi) = (0.0_f64, 1.0_f64);
    for i in 0..3 {
        let d = to[i] - from[i];
        if d.abs() < 1e-12 {
            if (from[i] - center[i]).abs() > extent[i] {
                return None;
            }
        } else {
            let a = (center[i] - extent[i] - from[i]) / d;
            let b = (center[i] + extent[i] - from[i]) / d;
            lo = lo.max(a.min(b));
            hi = hi.min(a.max(b));
            if lo > hi {
                return None;
            }
        }
    }
    Some(lo)
}

#[cfg(test)]
mod tests;
