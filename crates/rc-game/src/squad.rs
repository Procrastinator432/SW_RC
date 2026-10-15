//! Selected Delta62 follow, hold, focus fire and mutual revival host.
use crate::{mission_motion, Game, Point, STEP};
use serde::{Deserialize, Serialize};
pub const EXTENT: Point = [40., 40., 84.];
/// Floor-following static corridor query for the selected companion host.
pub fn corridor(
    world: &rc_package::world_collision::StaticBodyWorld,
    from: Point,
    to: Point,
    extent: Point,
) -> Result<Option<Vec<Point>>, String> {
    corridor_with_tolerance(world, from, to, extent, 1.)
}
/// Allows the small shortfall produced by projection onto a sloping floor.
/// The movement solver and floor support remain authoritative; walls/drops still reject.
pub fn corridor_with_tolerance(
    world: &rc_package::world_collision::StaticBodyWorld,
    from: Point,
    to: Point,
    extent: Point,
    tolerance: f64,
) -> Result<Option<Vec<Point>>, String> {
    if !tolerance.is_finite() || !(0. ..=4.).contains(&tolerance) {
        return Err("Invalid corridor tolerance".into());
    }
    let steps = (crate::distance(from, to) / 16.).ceil().max(1.) as usize;
    if steps > 768 {
        return Ok(None);
    }
    let mut previous = from;
    let mut points = vec![];
    for i in 1..=steps {
        let p: Point =
            std::array::from_fn(|j| from[j] + (to[j] - from[j]) * i as f64 / steps as f64);
        let body = rc_package::physics::BodyState {
            position: previous,
            velocity: [(p[0] - previous[0]) / STEP, (p[1] - previous[1]) / STEP, 0.],
            grounded: true,
        };
        let frame = match world.body_tick(
            &body,
            STEP,
            rc_package::physics::PhysicsOptions {
                extent,
                gravity: 950.,
                terminal_speed: 4000.,
                skin: 0.5,
                support_distance: 35.,
                minimum_up: 0.7,
                max_iterations: 8,
                step_height: 35.,
            },
        ) {
            Ok(f) => f,
            Err(e)
                if e == "body tick starts penetrating geometry"
                    || e == "horizontal body tick could not finish safely" =>
            {
                return Ok(None)
            }
            Err(e) => return Err(e),
        };
        if !frame.body.grounded
            || (frame.body.position[0] - p[0]).hypot(frame.body.position[1] - p[1]) > tolerance
        {
            return Ok(None);
        }
        previous = frame.body.position;
        points.push(previous);
    }
    if crate::distance(previous, to) > 4. {
        return Ok(None);
    }
    Ok(Some(points))
}
pub fn plan(
    nav: &crate::navigation::Assets,
    world: &rc_package::world_collision::StaticBodyWorld,
    from: Point,
    to: Point,
    extent: Point,
) -> Result<Vec<Point>, String> {
    plan_with_tolerance(nav, world, from, to, extent, 1.)
}
pub fn plan_with_tolerance(
    nav: &crate::navigation::Assets,
    world: &rc_package::world_collision::StaticBodyWorld,
    from: Point,
    to: Point,
    extent: Point,
    tolerance: f64,
) -> Result<Vec<Point>, String> {
    if let Some(path) = corridor_with_tolerance(world, from, to, extent, tolerance)? {
        return Ok(path);
    }
    let connect = |p| -> Result<Vec<(usize, f64)>, String> {
        let mut nodes: Vec<_> = nav
            .nodes
            .iter()
            .enumerate()
            .map(|(i, n)| (i, crate::distance(p, n.position)))
            .filter(|(_, d)| *d < 1500.)
            .collect();
        nodes.sort_by(|a, b| a.1.total_cmp(&b.1));
        let mut valid = vec![];
        for (i, d) in nodes.into_iter().take(12) {
            if corridor_with_tolerance(world, p, nav.nodes[i].position, extent, tolerance)?
                .is_some()
            {
                valid.push((i, d));
            }
        }
        Ok(valid)
    };
    let Some(path) = nav.shortest(&connect(from)?, &connect(to)?) else {
        return Ok(vec![]);
    };
    let mut result = vec![];
    let mut previous = from;
    for next in path
        .into_iter()
        .map(|i| nav.nodes[i].position)
        .chain(std::iter::once(to))
    {
        let Some(points) = corridor_with_tolerance(world, previous, next, extent, tolerance)?
        else {
            return Ok(vec![]);
        };
        result.extend(points);
        previous = next;
    }
    Ok(result)
}
#[derive(Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Assets {
    pub defense: crate::melee::Defense,
    pub mesh: Option<crate::visual::Mesh>,
    pub navigation: crate::navigation::Assets,
    pub crouch_navigation: crate::navigation::Assets,
    pub crouch_ratio: f64,
    pub weapon_snapshot: Vec<u8>,
    pub sounds: Vec<crate::combat::Sound>,
    pub fire_sound: usize,
    pub health: u32,
    pub max_health: u32,
    pub shields: f32,
    pub max_shields: f32,
    pub shield_delay: f64,
    pub shield_rate: f64,
    pub ground_speed: f64,
    pub damage: u32,
    pub shot_delay: f64,
    pub projectile_speed: f64,
    pub projectile_life: f64,
    pub revive_duration: f64,
    pub provenance: serde_json::Value,
}
impl Assets {
    pub fn validate(&self) -> Result<(), String> {
        if !self.defense.armor.is_finite()
            || !(0. ..=1.).contains(&self.defense.armor)
            || self.defense.include.len() > 128
            || self.defense.exclude.len() > 128
            || self.defense.include.iter().any(|r| {
                !r.multiplier.is_finite()
                    || !(0. ..=100.).contains(&r.multiplier)
                    || r.class.is_some_and(|id| !(1..=65535).contains(&id))
            })
            || self
                .defense
                .exclude
                .iter()
                .flatten()
                .any(|id| !(1..=65535).contains(id))
        {
            return Err("Invalid squad defense".into());
        }
        if !(1. / 120. ..=30.).contains(&self.shot_delay)
            || !(0.01..=30.).contains(&self.projectile_life)
            || !(0.01..=1.).contains(&self.crouch_ratio)
        {
            return Err("Invalid squad timing".into());
        }
        self.navigation.validate()?;
        self.crouch_navigation.validate()?;
        if self.health == 0
            || self.max_health == 0
            || self.health > self.max_health
            || self.max_health > 10000
            || self.damage == 0
            || self.damage > 1000
            || self.sounds.is_empty()
            || self.sounds.len() > 16
            || self.fire_sound >= self.sounds.len()
        {
            return Err("Invalid squad health/audio/damage".into());
        }
        for v in [
            self.shield_delay,
            self.shield_rate,
            self.crouch_ratio,
            self.ground_speed,
            self.shot_delay,
            self.projectile_speed,
            self.projectile_life,
            self.revive_duration,
            self.shields as f64,
            self.max_shields as f64,
        ] {
            if !v.is_finite() || !(0. ..=30000.).contains(&v) {
                return Err("Invalid squad scalar".into());
            }
        }
        if self.ground_speed == 0.
            || self.shot_delay == 0.
            || self.projectile_speed == 0.
            || self.projectile_life == 0.
            || self.revive_duration == 0.
        {
            return Err("Invalid squad positive scalar".into());
        }
        for s in &self.sounds {
            crate::combat::validate_wave(&s.wave)?;
        }
        Ok(())
    }
}
#[derive(Clone, Copy, Debug, Serialize, PartialEq, Eq, serde::Deserialize)]
pub enum Order {
    Follow,
    Hold,
    Focus(u32),
}
#[derive(Clone, Debug, Serialize, serde::Deserialize)]
pub struct Bullet {
    pub position: Point,
    pub velocity: Point,
    pub age: f64,
}
#[derive(Clone, Debug, Serialize, serde::Deserialize)]
pub struct State {
    pub quiet_for: f64,
    pub crouched: bool,
    pub joined: bool,
    pub health: u32,
    pub shields: f32,
    pub down: bool,
    pub player_down: bool,
    pub player_requested_revive: bool,
    pub progress: f64,
    pub player_recovery: Option<f64>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub player_recovery_health: Option<crate::revival::Recovery>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub scorch_recovery_health: Option<crate::revival::Recovery>,
    pub scorch_recovery: Option<f64>,
    pub focus_point: Option<Point>,
    pub focus_until: f64,
    pub trail: Vec<Point>,
    pub trail_cursor: usize,
    pub order: Order,
    pub use_held: bool,
    pub route: Vec<Point>,
    pub waypoint: usize,
    pub next_plan: f64,
    pub clip: String,
    pub frame: f32,
    pub cooldown: f64,
    pub flash: f64,
    pub bullets: Vec<Bullet>,
    pub shots: u64,
    pub hits: u64,
    pub kills: u32,
    pub player_revives: u32,
    pub scorch_revives: u32,
    pub blocked: u64,
    pub plans: u64,
    pub orders: u64,
    pub follow_distance: f64,
    pub events: Vec<(u64, String)>,
}
impl State {
    pub fn new(a: &Assets) -> Self {
        Self {
            quiet_for: 0.,
            crouched: false,
            joined: false,
            health: a.health,
            shields: a.shields,
            down: false,
            player_down: false,
            player_requested_revive: false,
            progress: 0.,
            player_recovery: None,
            player_recovery_health: None,
            scorch_recovery_health: None,
            scorch_recovery: None,
            focus_point: None,
            focus_until: 0.,
            trail: vec![],
            trail_cursor: 0,
            order: Order::Follow,
            use_held: false,
            route: vec![],
            waypoint: 0,
            next_plan: 0.,
            clip: "ActionBreathe".into(),
            frame: 0.,
            cooldown: 0.,
            flash: 0.,
            bullets: vec![],
            shots: 0,
            hits: 0,
            kills: 0,
            player_revives: 0,
            scorch_revives: 0,
            blocked: 0,
            plans: 0,
            orders: 0,
            follow_distance: 0.,
            events: vec![],
        }
    }
    pub fn damage(&mut self, amount: u32) {
        if self.down {
            return;
        }
        self.quiet_for = 0.;
        let blocked = (amount as f32).min(self.shields);
        self.shields -= blocked;
        self.health = self
            .health
            .saturating_sub((amount as f32 - blocked).ceil() as u32);
        self.flash = 0.2;
    }
}
fn sight(
    world: &rc_package::world_collision::StaticBodyWorld,
    from: Point,
    to: Point,
) -> Result<bool, String> {
    Ok(world.sweep_motion(from, to, [0.; 3])?.contacts.is_empty())
}
impl Game {
    pub(crate) fn try_incapacitate_player(&mut self) -> bool {
        let Some(s) = &mut self.state.squad else {
            return false;
        };
        if !s.joined || s.health == 0 || s.down {
            return false;
        }
        if !s.player_down {
            s.player_down = true;
            s.progress = 0.;
            s.player_recovery = None;
            s.player_recovery_health = None;
            s.player_requested_revive = false;
            s.events
                .push((self.state.statistics.ticks, "PlayerIncapacitated".into()));
        }
        true
    }
    pub(crate) fn squad_tick(&mut self, use_action: bool) -> Result<(), String> {
        let Some(p) = self.tactics.take() else {
            return Ok(());
        };
        let a = &p.assets.squad;
        let mut s = self.state.squad.take().unwrap();
        let result = (|| -> Result<(), String> {
            let tick = self.state.statistics.ticks;
            let time = tick as f64 * STEP;
            let joined = self.state.rendezvous.as_ref().unwrap().flow.squad_joined;
            s.joined = joined;
            if !joined {
                s.use_held = use_action;
                return Ok(());
            }
            let player = self.state.pawn.body.position;
            let eye = self.eye();
            let mut pawn = self.state.rendezvous.as_ref().unwrap().pawn.body.position;
            s.quiet_for += STEP;
            if !s.down && s.quiet_for > a.shield_delay && s.shields < a.max_shields {
                s.shields = (s.shields + a.shield_rate as f32 * STEP as f32).min(a.max_shields);
            }
            if !s.player_down
                && self
                    .state
                    .demolition
                    .as_ref()
                    .unwrap()
                    .flow
                    .objective_completed
            {
                let mut trace = player;
                if self.state.crouched {
                    trace[2] += 28.;
                }
                if s.trail
                    .last()
                    .is_none_or(|p| crate::distance(*p, trace) > 8.)
                {
                    if s.trail.len() >= 2048 {
                        s.trail.remove(0);
                        s.trail_cursor = s.trail_cursor.saturating_sub(1);
                    }
                    s.trail.push(trace);
                }
                let old_cursor = s.trail_cursor;
                while s.trail_cursor + 1 < s.trail.len()
                    && (s.trail[s.trail_cursor][0] - pawn[0])
                        .hypot(s.trail[s.trail_cursor][1] - pawn[1])
                        < 8.
                {
                    s.trail_cursor += 1;
                }
                if s.trail_cursor != old_cursor {
                    s.next_plan = 0.;
                    s.route.clear();
                }
            }
            let old_clip = s.clip.clone();
            s.flash = (s.flash - STEP).max(0.);
            s.cooldown = (s.cooldown - STEP).max(0.);
            if s.health == 0 && !s.down {
                s.down = true;
                s.progress = 0.;
                s.frame = 0.;
                s.clip = "ImmobileBreathe".into();
                s.events.push((tick, "ScorchIncapacitated".into()));
            }
            if s.player_down && s.down {
                self.state.phase = crate::Phase::Lost;
                s.events.push((tick, "SquadIncapacitated".into()));
                return Ok(());
            }
            if s.player_down && use_action {
                s.player_requested_revive = true;
            }
            if let Some(elapsed) = &mut s.player_recovery {
                self.state.health = s
                    .player_recovery_health
                    .get_or_insert_with(Default::default)
                    .tick(
                        self.state.health,
                        self.mission.as_ref().unwrap().max_health,
                        STEP as f32,
                    );
                *elapsed += STEP;
                let mesh = &self.rendezvous.as_ref().unwrap().mesh;
                let seq = &mesh.mesh.clips[mesh.clip("PlayerImmobileGetUp")?].sequence;
                if *elapsed >= seq.frames as f64 / f32::from_bits(seq.rate_bits) as f64 {
                    self.state.health = (self.mission.as_ref().unwrap().max_health / 2.) as u32;
                    s.player_down = false;
                    s.player_requested_revive = false;
                    s.player_recovery = None;
                    s.player_recovery_health = None;
                    s.progress = 0.;
                    s.player_revives += 1;
                    s.events.push((tick, "PlayerRevived".into()));
                }
                s.clip = if s.player_recovery.is_some() {
                    "HealImmobileStop"
                } else {
                    "ActionBreathe"
                }
                .into();
            } else if let Some(elapsed) = &mut s.scorch_recovery {
                s.health = s
                    .scorch_recovery_health
                    .get_or_insert_with(Default::default)
                    .tick(s.health, a.max_health as f32, STEP as f32);
                *elapsed += STEP;
                let mesh = &self.rendezvous.as_ref().unwrap().mesh;
                let seq = &mesh.mesh.clips[mesh.clip("ImmobileGetUp")?].sequence;
                s.clip = "ImmobileGetUp".into();
                s.frame =
                    (*elapsed * f32::from_bits(seq.rate_bits) as f64 / seq.frames as f64) as f32;
                if *elapsed >= seq.frames as f64 / f32::from_bits(seq.rate_bits) as f64 {
                    s.health = a.max_health / 2;
                    s.down = false;
                    s.scorch_recovery = None;
                    s.scorch_recovery_health = None;
                    s.clip = "ActionBreathe".into();
                    s.frame = 0.;
                    s.progress = 0.;
                    s.scorch_revives += 1;
                    s.events.push((tick, "ScorchRevived".into()));
                }
            } else if s.down {
                s.clip = "ImmobileBreathe".into();
                let close = crate::distance(eye, pawn) < 200.
                    && sight(&self.world, eye, [pawn[0], pawn[1], pawn[2] + 20.])?;
                if use_action && close && self.state.health > 0 {
                    s.progress += STEP;
                } else {
                    s.progress = 0.;
                }
                if s.progress + 1e-9 >= a.revive_duration {
                    s.scorch_recovery = Some(0.);
                    s.scorch_recovery_health = Some(Default::default());
                    s.clip = "ImmobileGetUp".into();
                    s.health = 1;
                    s.frame = 0.;
                    s.events.push((tick, "ScorchReviveStarted".into()));
                }
            } else {
                let can_move = self
                    .state
                    .demolition
                    .as_ref()
                    .unwrap()
                    .flow
                    .objective_completed;
                let aim = crate::direction(self.state.yaw, self.state.pitch);
                let facing_scorch = {
                    let d = crate::distance(eye, pawn);
                    let dot = (0..3).map(|j| (pawn[j] - eye[j]) * aim[j]).sum::<f64>();
                    d < 240. && dot / d.max(1.) > 0.85 && sight(&self.world, eye, pawn)?
                };
                if use_action && !s.use_held && can_move && !s.player_down {
                    let focus = self
                        .state
                        .artillery
                        .as_ref()
                        .unwrap()
                        .warriors
                        .iter()
                        .filter(|w| w.health > 0)
                        .find(|w| {
                            let d = crate::distance(eye, w.pawn.body.position);
                            let dot = (0..3)
                                .map(|j| (w.pawn.body.position[j] - eye[j]) * aim[j])
                                .sum::<f64>();
                            d < 2600. && dot / d.max(1.) > 0.95
                        })
                        .map(|w| w.original);
                    if let Some(id) = focus {
                        s.order = Order::Focus(id);
                        s.focus_point = self
                            .state
                            .artillery
                            .as_ref()
                            .unwrap()
                            .warriors
                            .iter()
                            .find(|w| w.original == id)
                            .map(|w| w.pawn.body.position);
                        s.focus_until = time + 3.;
                        s.route.clear();
                        s.next_plan = 0.;
                        s.orders += 1;
                        s.events.push((tick, format!("Focus{id}")));
                    } else if facing_scorch {
                        s.order = if s.order == Order::Hold {
                            Order::Follow
                        } else {
                            Order::Hold
                        };
                        s.orders += 1;
                        s.route.clear();
                        s.next_plan = 0.;
                        s.events.push((tick, format!("Order{:?}", s.order)));
                    }
                }
                let muzzle = [pawn[0], pawn[1], pawn[2] + 44.];
                let target = self
                    .state
                    .artillery
                    .as_ref()
                    .unwrap()
                    .warriors
                    .iter()
                    .filter(|w| w.health > 0)
                    .filter(|w| crate::distance(muzzle, w.pawn.body.position) < 2600.)
                    .filter(|w| sight(&self.world, muzzle, w.pawn.body.position).unwrap_or(false))
                    .min_by(|x, y| {
                        let priority = |w: &crate::artillery::Warrior| {
                            if s.order == Order::Focus(w.original) {
                                0
                            } else {
                                1
                            }
                        };
                        priority(x).cmp(&priority(y)).then_with(|| {
                            crate::distance(pawn, x.pawn.body.position)
                                .total_cmp(&crate::distance(pawn, y.pawn.body.position))
                        })
                    })
                    .map(|w| w.pawn.body.position);
                if matches!(s.order,Order::Focus(id) if !self.state.artillery.as_ref().unwrap().warriors.iter().any(|w|w.original==id && w.health>0))
                {
                    s.order = Order::Follow;
                }
                s.clip = if s.crouched {
                    "CrouchBreathe"
                } else {
                    "ActionBreathe"
                }
                .into();
                let focus_goal = if let Order::Focus(id) = s.order {
                    let at = self
                        .state
                        .artillery
                        .as_ref()
                        .unwrap()
                        .warriors
                        .iter()
                        .find(|w| w.original == id && w.health > 0)
                        .map(|w| w.pawn.body.position);
                    let seen = if let Some(at) = at {
                        sight(&self.world, muzzle, at)?
                    } else {
                        false
                    };
                    if seen {
                        s.focus_point = at;
                        s.focus_until = time + 3.;
                    }
                    if time <= s.focus_until
                        && (!seen || at.is_some_and(|at| crate::distance(pawn, at) > 1200.))
                    {
                        s.focus_point
                    } else {
                        None
                    }
                } else {
                    None
                };
                let moving_target = if s.player_down && s.player_requested_revive {
                    Some(player)
                } else if can_move && focus_goal.is_some() {
                    focus_goal
                } else if can_move && s.order != Order::Hold && crate::distance(pawn, player) > 400.
                {
                    Some(s.trail.get(s.trail_cursor).copied().unwrap_or(player))
                } else {
                    None
                };
                if let Some(to) = moving_target {
                    let near = crate::distance(pawn, to) < 160.
                        && sight(&self.world, muzzle, [to[0], to[1], to[2] + 40.])?;
                    let immediate_threat = target.is_some_and(|t| crate::distance(pawn, t) < 900.);
                    if s.player_down && near && !immediate_threat {
                        s.progress += STEP;
                        s.clip = if s.progress < 0.5 {
                            "HealImmobileStart"
                        } else {
                            "HealImmobileLoop"
                        }
                        .into();
                        self.state.rendezvous.as_mut().unwrap().yaw =
                            (player[1] - pawn[1]).atan2(player[0] - pawn[0]);
                        if s.progress + 1e-9 >= a.revive_duration {
                            s.player_recovery = Some(0.);
                            s.player_recovery_health = Some(Default::default());
                            s.clip = "HealImmobileStop".into();
                            self.state.health = 1;
                            s.frame = 0.;
                            s.events.push((tick, "PlayerReviveStarted".into()));
                        }
                    } else {
                        s.progress = 0.;
                        if time >= s.next_plan
                            || (s.waypoint >= s.route.len() && !s.route.is_empty())
                        {
                            let shape = rc_package::crouch::CrouchProfile {
                                standing: EXTENT,
                                crouching: [40., 40., 56.],
                                properties: std::collections::BTreeMap::new(),
                            };
                            let body = self.state.rendezvous.as_ref().unwrap().pawn.body;
                            let stand = self.world.crouch_diagnostic(
                                &rc_package::crouch::CrouchBody {
                                    body,
                                    crouched: s.crouched,
                                },
                                false,
                                &shape,
                            )?;
                            let supported = mission_motion::ground(
                                &self.world,
                                [to[0], to[1], to[2] + 120.],
                                EXTENT,
                                300.,
                            )?;
                            s.route = if !stand.state.crouched {
                                if let Some(at) = supported {
                                    plan(
                                        &a.navigation,
                                        &self.world,
                                        stand.state.body.position,
                                        at,
                                        EXTENT,
                                    )?
                                } else {
                                    vec![]
                                }
                            } else {
                                vec![]
                            };
                            let mut chosen = if s.route.is_empty() {
                                let crouch = self.world.crouch_diagnostic(
                                    &rc_package::crouch::CrouchBody {
                                        body,
                                        crouched: s.crouched,
                                    },
                                    true,
                                    &shape,
                                )?;
                                let at = mission_motion::ground(
                                    &self.world,
                                    [to[0], to[1], to[2] + 120.],
                                    [40., 40., 56.],
                                    300.,
                                )?;
                                s.route = if let Some(at) = at {
                                    plan(
                                        &a.crouch_navigation,
                                        &self.world,
                                        crouch.state.body.position,
                                        at,
                                        [40., 40., 56.],
                                    )?
                                } else {
                                    vec![]
                                };
                                crouch.state
                            } else {
                                stand.state
                            };
                            if s.route.is_empty() && !s.player_down && s.order == Order::Follow {
                                // Verified player breadcrumbs bridge links omitted by the bounded PathList graph.
                                // Each candidate still needs a supported body corridor; never relocate through geometry.
                                for wants_crouch in [false, true] {
                                    let trial = self.world.crouch_diagnostic(
                                        &rc_package::crouch::CrouchBody {
                                            body,
                                            crouched: s.crouched,
                                        },
                                        wants_crouch,
                                        &shape,
                                    )?;
                                    if trial.state.crouched != wants_crouch {
                                        continue;
                                    }
                                    let extent = if wants_crouch {
                                        [40., 40., 56.]
                                    } else {
                                        EXTENT
                                    };
                                    for index in
                                        s.trail_cursor..(s.trail_cursor + 64).min(s.trail.len())
                                    {
                                        let to = s.trail[index];
                                        let Some(to) = mission_motion::ground(
                                            &self.world,
                                            [to[0], to[1], to[2] + 120.],
                                            extent,
                                            300.,
                                        )?
                                        else {
                                            continue;
                                        };
                                        if let Some(points) = corridor(
                                            &self.world,
                                            trial.state.body.position,
                                            to,
                                            extent,
                                        )? {
                                            s.route = points;
                                            chosen = trial.state;
                                            s.trail_cursor = index;
                                        }
                                    }
                                    if !s.route.is_empty() {
                                        break;
                                    }
                                }
                            }
                            s.crouched = chosen.crouched;
                            self.state.rendezvous.as_mut().unwrap().pawn.body = chosen.body;
                            pawn = chosen.body.position;
                            s.waypoint = 0;
                            s.plans += 1;
                            s.next_plan = time
                                + if s.route.is_empty() {
                                    2.
                                } else if s.player_down || matches!(s.order, Order::Focus(_)) {
                                    1.
                                } else {
                                    60.
                                };
                            if s.route.is_empty() {
                                s.blocked += 1;
                                s.events.push((
                                    tick,
                                    format!(
                                        "PlanBlocked cursor{} trail{}",
                                        s.trail_cursor,
                                        s.trail.len()
                                    ),
                                ));
                            }
                        }
                        while s.waypoint < s.route.len()
                            && crate::distance(pawn, s.route[s.waypoint]) < 2.
                        {
                            s.waypoint += 1;
                        }
                        if let Some(to) = s.route.get(s.waypoint) {
                            let d = crate::distance(pawn, *to);
                            let next = std::array::from_fn(|j| {
                                pawn[j]
                                    + (to[j] - pawn[j])
                                        * (a.ground_speed
                                            * if s.crouched { a.crouch_ratio } else { 1. }
                                            * STEP)
                                            .min(d)
                                        / d
                            });
                            if let Some(points) = corridor(
                                &self.world,
                                pawn,
                                next,
                                if s.crouched { [40., 40., 56.] } else { EXTENT },
                            )? {
                                let next = *points.last().unwrap();
                                let r = self.state.rendezvous.as_mut().unwrap();
                                r.pawn.body.position = next;
                                r.yaw = (to[1] - pawn[1]).atan2(to[0] - pawn[0]);
                                s.clip = if s.crouched {
                                    "CrouchForward"
                                } else {
                                    "RunForward"
                                }
                                .into();
                            } else {
                                s.events
                                    .push((tick, format!("MoveBlocked {pawn:?} -> {next:?}")));
                                s.route.clear();
                                s.next_plan = time + 2.;
                                s.blocked += 1;
                            }
                        }
                    }
                }
                if let Some(target) = target.filter(|_| !s.clip.starts_with("Heal")) {
                    let d: Point = std::array::from_fn(|j| target[j] - muzzle[j]);
                    let length = crate::distance(target, muzzle);
                    self.state.rendezvous.as_mut().unwrap().yaw = d[1].atan2(d[0]);
                    s.clip = "Fire".into();
                    if s.cooldown < 1e-8 {
                        let friendly = self
                            .world
                            .sweep_motion(muzzle, target, [0.; 3])?
                            .contacts
                            .is_empty()
                            && crate::ray_box(muzzle, target, player, [40., 40., 84.]).is_none();
                        if friendly && length > 1. && s.bullets.len() < 128 {
                            s.bullets.push(Bullet {
                                position: muzzle,
                                velocity: d.map(|v| v / length * a.projectile_speed),
                                age: 0.,
                            });
                            s.cooldown = a.shot_delay;
                            s.shots += 1;
                            let m = self.state.mission.as_mut().unwrap();
                            m.audio_sequence += 1;
                            if m.events.len() >= 256 {
                                m.events.remove(0);
                            }
                            m.events.push(crate::combat::Event {
                                sequence: m.audio_sequence,
                                tick,
                                sound: p.sound_base + a.fire_sound,
                                owner: 62,
                            });
                        }
                    }
                }
            }
            // Swept original-speed DC17 projectiles. World and friendly body occlude hits.
            let mut survivors = vec![];
            for mut b in s.bullets.drain(..) {
                let end = std::array::from_fn(|j| b.position[j] + b.velocity[j] * STEP);
                let mut fraction = self.wall_fraction(b.position, end)?;
                let mut victim = None;
                if let Some(f) = crate::ray_box(b.position, end, player, [40., 40., 84.]) {
                    fraction = fraction.min(f);
                }
                for (i, w) in self
                    .state
                    .artillery
                    .as_ref()
                    .unwrap()
                    .warriors
                    .iter()
                    .enumerate()
                    .filter(|(_, w)| w.health > 0)
                {
                    if let Some(f) =
                        crate::ray_box(b.position, end, w.pawn.body.position, [50., 50., 115.])
                    {
                        if f < fraction {
                            fraction = f;
                            victim = Some(i);
                        }
                    }
                }
                if let Some(i) = victim {
                    let a = self.state.artillery.as_mut().unwrap();
                    let w = &mut a.warriors[i];
                    let old = w.health;
                    w.health = w.health.saturating_sub(p.assets.squad.damage);
                    w.frame = 0.;
                    if let Some(ai) = &mut w.ai {
                        ai.interrupt();
                    }
                    s.hits += 1;
                    if old > 0 && w.health == 0 {
                        s.kills += 1;
                        a.effects.push((tick, "AttackWarriorDead".into()));
                    }
                } else if fraction >= 1. {
                    b.age += STEP;
                    b.position = end;
                    if b.age < a.projectile_life {
                        survivors.push(b);
                    }
                }
            }
            s.bullets = survivors;
            s.follow_distance = crate::distance(
                self.state.rendezvous.as_ref().unwrap().pawn.body.position,
                player,
            );
            s.use_held = use_action;
            if old_clip != s.clip {
                s.frame = 0.;
            }
            if s.scorch_recovery.is_none() {
                let mesh = &self.rendezvous.as_ref().unwrap().mesh;
                let seq = &mesh.mesh.clips[mesh.clip(&s.clip)?].sequence;
                let next =
                    s.frame + f32::from_bits(seq.rate_bits) * STEP as f32 / seq.frames as f32;
                s.frame = if s.clip == "HealImmobileStop" {
                    next.min(1. - 1. / seq.frames as f32)
                } else {
                    next.rem_euclid(1.)
                };
            }
            Ok(())
        })();
        if s.events.len() > 256 {
            s.events.drain(..s.events.len() - 256);
        }
        self.state.squad = Some(s);
        self.tactics = Some(p);
        result
    }
    pub(crate) fn squad_blast(
        &mut self,
        at: Point,
        damage: f32,
        radius: f32,
    ) -> Result<(), String> {
        let Some(s) = self.state.squad.as_ref() else {
            return Ok(());
        };
        if !s.joined || s.down {
            return Ok(());
        }
        let pos = self.state.rendezvous.as_ref().unwrap().pawn.body.position;
        let distance = crate::distance(pos, at) as f32;
        if distance < radius + 40. && self.wall_fraction(at, pos)? >= 1. {
            let amount = crate::detonator::radius_damage(damage, radius, distance, 40.)?
                .max(0.)
                .ceil() as u32;
            self.state.squad.as_mut().unwrap().damage(amount);
        }
        Ok(())
    }
    pub(crate) fn squad_weapon_geometry(
        &self,
    ) -> Result<
        (
            Vec<rc_package::geometry::Triangle>,
            Vec<Option<rc_render::FaceTexture>>,
        ),
        String,
    > {
        let (Some(p), Some(s)) = (&self.tactics, &self.state.squad) else {
            return Ok((vec![], vec![]));
        };
        if !s.joined
            || s.down
            || s.clip.starts_with("Heal")
            || s.scorch_recovery.is_some()
            || !self
                .state
                .demolition
                .as_ref()
                .is_some_and(|d| d.flow.objective_completed)
        {
            return Ok((vec![], vec![]));
        }
        let r = self.state.rendezvous.as_ref().unwrap();
        let mesh = &self.rendezvous.as_ref().unwrap().mesh;
        let transform = mesh.attachment_transform(
            &mesh.channel(mesh.clip(&s.clip)?, s.frame)?,
            "weaponAttach_R",
            true,
        )?;
        let (sin, cos) = (r.yaw as f32).sin_cos();
        let mut triangles = p.weapon.triangles.clone();
        for t in &mut triangles {
            t.points = t.points.map(|v| {
                let v: [f32; 3] = std::array::from_fn(|i| {
                    v[0] * transform[0][i]
                        + v[1] * transform[1][i]
                        + v[2] * transform[2][i]
                        + transform[3][i]
                });
                [
                    r.pawn.body.position[0] as f32 + v[0] * cos - v[1] * sin,
                    r.pawn.body.position[1] as f32 + v[0] * sin + v[1] * cos,
                    r.pawn.body.position[2] as f32 - if s.crouched { 56. } else { 84. } + v[2]
                        - mesh.rest_bounds[0][2],
                ]
            });
        }
        let faces = p
            .weapon
            .appearance
            .iter()
            .map(|f| {
                (*f).map(|mut f| {
                    f.texture += p.weapon_texture;
                    f
                })
            })
            .collect();
        Ok((triangles, faces))
    }
}

#[cfg(test)]
mod corridor_tests {
    use super::*;
    use rc_package::{
        hull::{Hull, HullSet},
        world_collision::{BodyActor, BodyShape, StaticBodyWorld},
    };
    use std::sync::Arc;
    fn ramp() -> StaticBodyWorld {
        let k = 10_f64.sqrt();
        let h = Hull {
            offset: 0,
            bounds: [[-1000., -1000., -500.], [1000., 1000., 500.]],
            planes: vec![
                [1., 0., 0., 1000.],
                [-1., 0., 0., 1000.],
                [0., 1., 0., 1000.],
                [0., -1., 0., 1000.],
                [0., 0., -1., 500.],
                [0., -1. / k, 3. / k, 0.],
            ],
        };
        StaticBodyWorld {
            world: Ok(Arc::new(HullSet {
                hulls: vec![h],
                missing_solid_leaves: 0,
                empty_root_solid: false,
            })),
            actors: vec![],
        }
    }
    #[test]
    fn projected_ramp_motion_is_supported_without_changing_legacy_tolerance() {
        let world = ramp();
        let a = [0., 0., 84. + 40. / 3. + 0.02];
        let b = [200., 340., 84. + 40. / 3. + 340. / 3. + 0.02];
        assert!(corridor(&world, a, b, EXTENT).unwrap().is_none());
        let path = corridor_with_tolerance(&world, a, b, EXTENT, 4.)
            .unwrap()
            .unwrap();
        assert!(crate::distance(*path.last().unwrap(), b) < 4.);
        for p in path {
            assert_ne!(
                world.body_placement(p, EXTENT).unwrap().state,
                rc_package::movement::PlacementState::Penetrating
            );
        }
    }
    #[test]
    fn ramp_tolerance_does_not_admit_crossing_a_wall() {
        let mut world = ramp();
        world.actors.push(BodyActor {
            name: "wall".into(),
            shape: BodyShape::Hulls(Arc::new(HullSet {
                hulls: vec![Hull {
                    offset: 0,
                    bounds: [[-1000., 165., -500.], [1000., 175., 500.]],
                    planes: vec![
                        [1., 0., 0., 1000.],
                        [-1., 0., 0., 1000.],
                        [0., 1., 0., 175.],
                        [0., -1., 0., -165.],
                        [0., 0., 1., 500.],
                        [0., 0., -1., 500.],
                    ],
                }],
                missing_solid_leaves: 0,
                empty_root_solid: false,
            })),
        });
        assert!(corridor_with_tolerance(
            &world,
            [0., 0., 97.3533333333],
            [0., 340., 210.6866666667],
            EXTENT,
            4.
        )
        .unwrap()
        .is_none());
    }
    #[test]
    fn ramp_tolerance_requires_floor_support_and_bounded_finite_tolerance() {
        let world = StaticBodyWorld {
            world: Ok(Arc::new(HullSet {
                hulls: vec![],
                missing_solid_leaves: 0,
                empty_root_solid: false,
            })),
            actors: vec![],
        };
        assert!(
            corridor_with_tolerance(&world, [0., 0., 100.], [100., 0., 100.], EXTENT, 4.)
                .unwrap()
                .is_none()
        );
        assert!(
            corridor_with_tolerance(&world, [0.; 3], [100., 0., 0.], EXTENT, f64::NAN).is_err()
        );
        assert!(corridor_with_tolerance(&world, [0.; 3], [100., 0., 0.], EXTENT, 5.).is_err());
    }
}
