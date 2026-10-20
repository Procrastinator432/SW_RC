//! Selected GEO_01B event chains. Serialized actions and actor identities come
//! from the original map; command timing and combat movement are bounded hosts.
use crate::{Game, Point, STEP};
use serde::{Deserialize, Serialize};
use std::{collections::BTreeSet, sync::Arc};
#[derive(Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Assets {
    #[serde(default = "default_follow_speed")]
    pub follow_speed: f64,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub navigation: Option<crate::navigation::Assets>,
    pub objectives: Vec<String>,
    pub targets: std::collections::BTreeMap<String, Point>,
    pub squad_fire_sound: usize,
    pub demolition_sound: usize,
    pub scripts: Vec<Script>,
    pub triggers: Vec<Trigger>,
    pub factories: Vec<Factory>,
    pub counters: Vec<Counter>,
    pub interactions: Vec<Interaction>,
    pub gates: Vec<Gate>,
    pub stations: Vec<Point>,
    pub ammunition: Vec<Point>,
    pub fixer_position: Point,
    pub scorch_position: Point,
    pub exit: Point,
    pub exit_radius: f64,
    pub destination: String,
    pub droid_mesh: Option<crate::visual::Mesh>,
    pub warrior_mesh: Option<crate::visual::Mesh>,
    pub fixer_mesh: Option<crate::visual::Mesh>,
}
#[derive(Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Script {
    pub original: u32,
    pub actions: Vec<Action>,
}
#[derive(Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Action {
    pub original: u32,
    pub op: String,
    pub text: String,
    pub number: usize,
    pub seconds: f64,
}
#[derive(Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Trigger {
    pub original: u32,
    pub event: String,
    pub position: Point,
    pub radius: f64,
    pub height: f64,
}
#[derive(Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Factory {
    pub original: u32,
    pub event: String,
    pub position: Point,
    pub spawn_points: Vec<Point>,
    pub health: u32,
    pub warrior: bool,
    pub capacity: u32,
    pub interval: f64,
    pub death_event: String,
}
#[derive(Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Counter {
    pub original: u32,
    pub tag: String,
    pub event: String,
    pub count: u32,
}
#[derive(Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Interaction {
    pub original: u32,
    pub actor: String,
    pub label: String,
    pub position: Point,
    pub prerequisite: String,
    pub underway: String,
    pub event: String,
    pub demolition: bool,
    pub seconds: f64,
}
#[derive(Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Gate {
    #[serde(default)]
    pub delay: f64,
    pub original: u32,
    pub actor: String,
    pub event: String,
    pub snapshot: Vec<u8>,
    pub offset: Point,
}
pub struct Prepared {
    pub assets: Assets,
    pub meshes: Vec<(crate::visual::PreparedMesh, usize)>,
    pub gates: Vec<PreparedGate>,
}
pub struct PreparedGate {
    pub scene: rc_render::Scene,
    pub texture: usize,
    pub collider: Option<(usize, Arc<rc_package::hull::HullSet>)>,
}
#[derive(Clone, Debug, Serialize, serde::Deserialize)]
pub struct Cursor {
    pub action: usize,
    pub remaining: f64,
}
#[derive(Clone, Debug, Serialize, serde::Deserialize)]
pub struct State {
    #[serde(skip_serializing_if = "Option::is_none")]
    pub followers: Option<[crate::navigation::Agent; 2]>,
    pub events: BTreeSet<String>,
    pub event_counts: std::collections::BTreeMap<String, u32>,
    pub objectives: [bool; 8],
    pub scripts: Vec<Cursor>,
    pub triggered: Vec<bool>,
    pub factories: Vec<u32>,
    pub factory_timers: Vec<f64>,
    pub enemy_factories: Vec<usize>,
    pub enemy_deaths: Vec<bool>,
    pub progress: Vec<f64>,
    pub armed: Vec<bool>,
    pub used: Vec<bool>,
    pub tracker: String,
    pub fixer: crate::map_runtime::Companion,
    pub pickups: Vec<bool>,
    pub assistance_remaining: f64,
    pub revive_progress: f64,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub gate_timers: Option<Vec<f64>>,
    pub transition: Option<String>,
}
impl State {
    fn new(a: &Assets) -> Self {
        Self {
            followers: a
                .navigation
                .as_ref()
                .map(|_| std::array::from_fn(|_| crate::navigation::Agent::default())),
            events: BTreeSet::from(["levelstart".into()]),
            event_counts: std::collections::BTreeMap::new(),
            objectives: [false; 8],
            scripts: a
                .scripts
                .iter()
                .map(|_| Cursor {
                    action: 0,
                    remaining: 0.,
                })
                .collect(),
            triggered: vec![false; a.triggers.len()],
            factories: vec![0; a.factories.len()],
            factory_timers: vec![0.; a.factories.len()],
            enemy_factories: vec![],
            enemy_deaths: vec![],
            progress: vec![0.; a.interactions.len()],
            armed: vec![false; a.interactions.len()],
            used: vec![false; a.interactions.len()],
            tracker: String::new(),
            fixer: crate::map_runtime::Companion {
                joined: false,
                health: 100,
                shields: 100.,
                down: false,
                position: a.fixer_position,
                yaw: 0.,
                moving: false,
            },
            pickups: vec![false; a.ammunition.len()],
            assistance_remaining: 0.,
            revive_progress: 0.,
            gate_timers: a
                .gates
                .iter()
                .any(|g| g.delay > 0.)
                .then(|| vec![-1.; a.gates.len()]),
            transition: None,
        }
    }
}
impl Game {
    pub(crate) fn prepare_map_gameplay(&mut self, mut a: Assets) -> Result<Prepared, String> {
        if a.objectives.len() != 8
            || a.scripts.len() > 64
            || a.triggers.len() > 128
            || a.factories.len() > 64
            || a.counters.len() > 64
            || a.counters.iter().any(|c| c.count == 0 || c.count > 1000)
            || a.interactions.len() != 5
            || a.gates.len() > 64
            || a.demolition_sound >= self.map.as_ref().map_or(0, |m| m.assets.sounds.len())
            || a.squad_fire_sound >= self.map.as_ref().map_or(0, |m| m.assets.sounds.len())
            || a.destination != "GEO_01C.ctm#playerstart"
            || !a.exit_radius.is_finite()
            || !(1. ..=1000.).contains(&a.exit_radius)
        {
            return Err("Invalid GEO_01B gameplay bindings".into());
        }
        if !a.follow_speed.is_finite() || !(1. ..=1500.).contains(&a.follow_speed) {
            return Err("Invalid companion speed".into());
        }
        if let Some(nav) = &a.navigation {
            nav.validate()?;
        }
        let finite = |p: Point| p.iter().all(|v| v.is_finite() && v.abs() < 1e8);
        if !finite(a.exit)
            || !finite(a.fixer_position)
            || !finite(a.scorch_position)
            || a.targets.values().any(|p| !finite(*p))
            || a.gates.iter().any(|g| {
                !finite(g.offset)
                    || !g.delay.is_finite()
                    || !(0. ..=120.).contains(&g.delay)
                    || g.snapshot.len() > 8 * 1024 * 1024
            })
            || a.stations.iter().chain(&a.ammunition).any(|p| !finite(*p))
            || a.triggers.iter().any(|t| {
                !finite(t.position)
                    || !t.radius.is_finite()
                    || !(1. ..=2000.).contains(&t.radius)
                    || !t.height.is_finite()
                    || !(1. ..=2000.).contains(&t.height)
            })
            || a.factories.iter().any(|f| {
                f.spawn_points.len() > 128
                    || f.spawn_points.iter().any(|p| !finite(*p))
                    || !finite(f.position)
                    || f.capacity == 0
                    || f.capacity > 32
                    || f.health == 0
                    || f.health > 10000
                    || !f.interval.is_finite()
                    || !(0. ..=120.).contains(&f.interval)
            })
            || a.interactions.iter().any(|i| {
                !finite(i.position) || !i.seconds.is_finite() || !(0.1..=120.).contains(&i.seconds)
            })
        {
            return Err("Invalid map2 actor coordinates/rules".into());
        }
        for s in &a.scripts {
            if s.actions.len() > 128 {
                return Err("Map2 script budget exceeded".into());
            }
            for x in &s.actions {
                if ![
                    "wait",
                    "emit",
                    "objective",
                    "tracker",
                    "sound",
                    "timer",
                    "join",
                ]
                .contains(&x.op.as_str())
                    || !x.seconds.is_finite()
                    || !(0. ..=120.).contains(&x.seconds)
                    || x.op == "objective" && x.number >= 8
                    || x.op == "sound"
                        && x.number >= self.map.as_ref().map_or(0, |m| m.assets.sounds.len())
                {
                    return Err("Unsupported map2 action".into());
                }
            }
        }
        let mut meshes = vec![];
        for mesh in [
            a.droid_mesh.take(),
            a.warrior_mesh.take(),
            a.fixer_mesh.take(),
        ] {
            let mesh = mesh
                .ok_or("Missing map2 character mesh")?
                .prepare_reference()?;
            for clip in ["ActionBreathe", "RunForward", "Death"] {
                mesh.clip(clip)?;
            }
            let base = self.scene.textures.len();
            self.scene
                .textures
                .extend(mesh.mesh.materials.iter().map(|m| rc_render::Texture {
                    width: m.width,
                    height: m.height,
                    pixels: m.pixels.clone(),
                }));
            meshes.push((mesh, base));
        }
        let mut gates = vec![];
        for gate in &a.gates {
            let scene = if gate.snapshot.is_empty() {
                rc_render::Scene {
                    triangles: vec![],
                    appearance: vec![],
                    textures: vec![],
                    center: [0.; 3],
                    radius: 1.,
                    starts: vec![],
                    solid: None,
                }
            } else {
                rc_render::Scene::from_snapshot(&gate.snapshot)?
            };
            let texture = self.scene.textures.len();
            self.scene.textures.extend(scene.textures.iter().cloned());
            let collider = self.world.actors.iter().enumerate().find_map(|(i, actor)| {
                if actor.name != gate.actor {
                    return None;
                }
                match &actor.shape {
                    rc_package::world_collision::BodyShape::Hulls(h) => Some((i, h.clone())),
                    _ => None,
                }
            });
            gates.push(PreparedGate {
                scene,
                texture,
                collider,
            });
        }
        let m = self.state.map.as_mut().unwrap();
        if m.companion.is_none() {
            m.companion = Some(crate::map_runtime::Companion {
                joined: true,
                health: 100,
                shields: 100.,
                down: false,
                position: a.scorch_position,
                yaw: 0.,
                moving: false,
            });
        }
        m.gameplay = Some(Box::new(State::new(&a)));
        Ok(Prepared {
            assets: a,
            meshes,
            gates,
        })
    }
    pub(crate) fn map_gameplay_tick(&mut self, input: crate::Input) -> Result<(), String> {
        let Some(p) = self.map.as_ref().and_then(|m| m.gameplay.as_ref()) else {
            return Ok(());
        };
        let a = &p.assets;
        let position = self.state.pawn.body.position;
        let eye = self.eye();
        let facing = crate::direction(self.state.yaw, self.state.pitch);
        // Trace before borrowing state: walls prevent using through a closed door.
        let usable: Vec<bool> = a
            .interactions
            .iter()
            .map(|i| {
                let d = crate::distance(eye, i.position);
                Ok(d < 240.
                    && d > 1.
                    && (0..3)
                        .map(|k| (i.position[k] - eye[k]) / d * facing[k])
                        .sum::<f64>()
                        > 0.5
                    && {
                        let trace = self.world.sweep_motion(eye, i.position, [0.; 3])?;
                        trace.complete
                            && trace
                                .contacts
                                .iter()
                                .all(|c| c.object == i.actor || c.hit.fraction > 0.96)
                    })
            })
            .collect::<Result<_, String>>()?;
        let heal = a
            .stations
            .iter()
            .any(|p| crate::distance(position, *p) < 180.);
        // A carried incapacitated Scorch must not permanently block the first charge.
        // Local 3s revival is a bounded map host, not native revival animation parity.
        let m = self.state.map.as_mut().unwrap();
        let g = m.gameplay.as_mut().unwrap();
        if let Some(c) = &mut m.companion {
            if c.down && input.use_action && crate::distance(position, c.position) < 180. {
                g.revive_progress += STEP;
                if g.revive_progress + 1e-8 >= 3. {
                    c.health = 100;
                    c.down = false;
                    g.revive_progress = 0.;
                }
            } else {
                g.revive_progress = 0.;
            }
        }
        let can_demolish = self
            .state
            .map
            .as_ref()
            .unwrap()
            .companion
            .as_ref()
            .is_some_and(|c| c.joined && !c.down && c.health > 0);
        let s = self.state.map.as_mut().unwrap().gameplay.as_mut().unwrap();
        for (i, t) in a.triggers.iter().enumerate() {
            if !s.triggered[i]
                && (position[0] - t.position[0]).hypot(position[1] - t.position[1]) < t.radius + 40.
                && (position[2] - t.position[2]).abs() < t.height + 84.
            {
                s.triggered[i] = true;
                s.events.insert(t.event.clone());
            }
        }
        if input.use_action && heal {
            self.state.health = (self.state.health + 1).min(100);
        }
        for (i, pickup) in a.ammunition.iter().enumerate() {
            if !s.pickups[i] && crate::distance(position, *pickup) < 150. {
                s.pickups[i] = true;
                if self.dc15.is_none() {
                    self.state.reserve = (self.state.reserve + 40).min(400);
                }
            }
        }
        let mut sounds = vec![];
        for (i, interaction) in a.interactions.iter().enumerate() {
            if s.used[i] || !s.events.contains(&interaction.prerequisite) {
                continue;
            }
            if interaction.demolition && s.armed[i] {
                if input.fire && crate::distance(position, interaction.position) > 300. {
                    s.used[i] = true;
                    s.events.insert(interaction.event.clone());
                    sounds.push(a.demolition_sound);
                }
                continue;
            }
            if input.use_action
                && usable[i]
                && (if interaction.demolition {
                    can_demolish
                } else {
                    s.fixer.joined && !s.fixer.down && s.fixer.health > 0
                })
            {
                if s.progress[i] == 0. {
                    s.events.insert(interaction.underway.clone());
                }
                s.progress[i] += STEP;
                if s.progress[i] + 1e-8 >= interaction.seconds {
                    if interaction.demolition {
                        s.armed[i] = true;
                    } else {
                        s.used[i] = true;
                        s.events.insert(interaction.event.clone());
                    }
                }
            } else {
                s.progress[i] = 0.;
            }
        }
        // Each selected original one-shot script has its own waiting/timer cursor.
        for (script, cursor) in a.scripts.iter().zip(&mut s.scripts) {
            cursor.remaining = (cursor.remaining - STEP).max(0.);
            if cursor.remaining > 1e-8 {
                continue;
            }
            for _ in 0..64 {
                let Some(x) = script.actions.get(cursor.action) else {
                    break;
                };
                match x.op.as_str() {
                    "wait" if !s.events.contains(&x.text) => break,
                    "wait" => {}
                    "emit" => {
                        s.events.insert(x.text.clone());
                    }
                    "objective" => s.objectives[x.number] = true,
                    "tracker" => s.tracker = x.text.clone(),
                    "join" => s.fixer.joined = true,
                    "sound" => sounds.push(x.number),
                    "timer" => {}
                    _ => return Err("Unknown map2 action".into()),
                }
                cursor.action += 1;
                cursor.remaining = x.seconds;
                if cursor.remaining > 0. {
                    break;
                }
            }
        }
        for sound in sounds {
            let m = self.state.map.as_mut().unwrap();
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
        // Original capacity/interval and actor identity; bounded clear-floor spawn host.
        let s = self.state.map.as_mut().unwrap().gameplay.as_mut().unwrap();
        for (i, f) in a.factories.iter().enumerate() {
            s.factory_timers[i] = (s.factory_timers[i] - STEP).max(0.);
            if !s.events.contains(&f.event)
                || s.factories[i] >= f.capacity
                || s.factory_timers[i] > 0.
            {
                continue;
            }
            // Do not stack replacement waves on a living pawn from the same factory.
            if s.enemy_factories
                .iter()
                .enumerate()
                .any(|(e, owner)| *owner == i && self.state.enemies[e].health > 0)
            {
                continue;
            }
            let extent = if f.warrior {
                [35., 35., 80.]
            } else {
                [22., 22., 60.]
            };
            let mut spawn = None;
            for original in f.spawn_points.iter().chain(std::iter::once(&f.position)) {
                for radius in [0., 40., 80., 120.] {
                    for angle in 0..8 {
                        let angle = angle as f64 * std::f64::consts::TAU / 8.;
                        let candidate = [
                            original[0] + radius * angle.cos(),
                            original[1] + radius * angle.sin(),
                            original[2] + 120.,
                        ];
                        if let Some(p) =
                            crate::mission_motion::ground(&self.world, candidate, extent, 2048.)?
                        {
                            if self.state.enemies.iter().any(|e| {
                                e.health > 0 && crate::distance(e.position, p) < extent[0] * 2. + 5.
                            }) {
                                continue;
                            }
                            spawn = Some(p);
                            break;
                        }
                    }
                    if spawn.is_some() {
                        break;
                    }
                }
                if spawn.is_some() {
                    break;
                }
            }
            // A blocked original spawn is retried; it never aborts the player's tick.
            let Some(spawn) = spawn else {
                s.factory_timers[i] = 0.5;
                continue;
            };
            self.state.enemies.push(crate::Enemy {
                position: spawn,
                health: f.health,
                cooldown: 1.4,
                flash: 0.,
            });
            s.enemy_factories.push(i);
            s.enemy_deaths.push(false);
            s.factories[i] += 1;
            s.factory_timers[i] = f.interval;
        }
        for (i, enemy) in self.state.enemies.iter().enumerate() {
            if enemy.health == 0 && !s.enemy_deaths[i] {
                s.enemy_deaths[i] = true;
                let f = &a.factories[s.enemy_factories[i]];
                if !f.death_event.is_empty() {
                    s.events.insert(f.death_event.clone());
                    let count = s.event_counts.entry(f.death_event.clone()).or_default();
                    *count += 1;
                    for counter in &a.counters {
                        if counter.tag == f.death_event && *count >= counter.count {
                            s.events.insert(counter.event.clone());
                        }
                    }
                }
            }
        }
        if s.objectives[..7].iter().all(|v| *v) && crate::distance(position, a.exit) < a.exit_radius
        {
            s.objectives[7] = true;
            s.transition = Some(a.destination.clone());
            self.state.phase = crate::Phase::Won;
        }
        if let Some(timers) = &mut s.gate_timers {
            for (timer, gate) in timers.iter_mut().zip(&a.gates) {
                advance_gate_timer(timer, gate.delay, s.events.contains(&gate.event));
            }
        }
        self.sync_map_gates();
        self.map_fixer_tick()?;
        self.map_assist_tick()?;
        Ok(())
    }
    fn map_fixer_tick(&mut self) -> Result<(), String> {
        if self
            .map
            .as_ref()
            .and_then(|m| m.gameplay.as_ref())
            .is_some_and(|p| p.assets.navigation.is_some())
        {
            return Ok(());
        }
        let s = self.state.map.as_mut().unwrap().gameplay.as_mut().unwrap();
        let c = &mut s.fixer;
        c.moving = false;
        let target = self.state.pawn.body.position;
        let d = crate::distance(c.position, target);
        if c.joined && !c.down && d > 200. && d < 1800. {
            let from = c.position;
            let next = std::array::from_fn(|i| {
                from[i] + (target[i] - from[i]) * (300. * STEP / d).min(1.)
            });
            if let Some(path) =
                crate::squad::corridor(&self.world, from, next, crate::squad::EXTENT)?
            {
                if let Some(next) = path.last() {
                    c.position = *next;
                    c.yaw = (target[1] - from[1]).atan2(target[0] - from[0]);
                    c.moving = true;
                }
            }
        }
        Ok(())
    }
    pub(crate) fn map_follow_tick(&mut self) -> Result<bool, String> {
        let Some(prepared) = self.map.as_ref().and_then(|m| m.gameplay.as_ref()) else {
            return Ok(false);
        };
        let Some(nav) = prepared.assets.navigation.as_ref() else {
            return Ok(false);
        };
        let speed = prepared.assets.follow_speed;
        let target = self.state.pawn.body.position;
        let now = self.state.statistics.ticks as f64 * STEP;
        let m = self.state.map.as_mut().unwrap();
        let s = m.gameplay.as_mut().unwrap();
        let agents = s.followers.as_mut().ok_or("Missing map2 follower state")?;
        if let Some(c) = &mut m.companion {
            follow(&self.world, nav, c, &mut agents[0], target, now, speed)?;
        }
        let fixer_from = s.fixer.position;
        follow(
            &self.world,
            nav,
            &mut s.fixer,
            &mut agents[1],
            target,
            now,
            speed,
        )?;
        if let Some(c) = &m.companion {
            if c.joined
                && s.fixer.joined
                && (c.position[2] - s.fixer.position[2]).abs() < 140.
                && (c.position[0] - s.fixer.position[0]).hypot(c.position[1] - s.fixer.position[1])
                    < 80.
            {
                s.fixer.position = fixer_from;
                s.fixer.moving = false;
            }
        }
        Ok(true)
    }
    fn map_assist_tick(&mut self) -> Result<(), String> {
        let m = self.state.map.as_ref().unwrap();
        let s = m.gameplay.as_ref().unwrap();
        if s.assistance_remaining > 0. {
            self.state
                .map
                .as_mut()
                .unwrap()
                .gameplay
                .as_mut()
                .unwrap()
                .assistance_remaining = (s.assistance_remaining - STEP).max(0.);
            return Ok(());
        }
        let companions = m.companion.iter().chain(std::iter::once(&s.fixer));
        let mut target = None;
        for c in companions.filter(|c| c.joined && !c.down && c.health > 0) {
            let from = [c.position[0], c.position[1], c.position[2] + 40.];
            for (i, e) in self
                .state
                .enemies
                .iter()
                .enumerate()
                .filter(|(_, e)| e.health > 0)
            {
                if crate::distance(from, e.position) < 1600.
                    && self.wall_fraction(from, e.position)? > 0.999
                {
                    target = Some(i);
                    break;
                }
            }
            if target.is_some() {
                break;
            }
        }
        if let Some(i) = target {
            let e = &mut self.state.enemies[i];
            e.health = e.health.saturating_sub(18);
            e.flash = 0.12;
            self.state.statistics.kills += u32::from(e.health == 0);
            let sound = self
                .map
                .as_ref()
                .unwrap()
                .gameplay
                .as_ref()
                .unwrap()
                .assets
                .squad_fire_sound;
            let m = self.state.map.as_mut().unwrap();
            m.gameplay.as_mut().unwrap().assistance_remaining = 0.6;
            m.audio_sequence += 1;
            if m.events.len() >= 256 {
                m.events.remove(0);
            }
            m.events.push(crate::combat::Event {
                sequence: m.audio_sequence,
                tick: self.state.statistics.ticks,
                sound,
                owner: -2,
            });
        }
        Ok(())
    }
    pub(crate) fn map_target(&self) -> Option<Point> {
        let p = self.map.as_ref()?.gameplay.as_ref()?;
        let s = self.state.map.as_ref()?.gameplay.as_ref()?;
        p.assets
            .targets
            .get(&s.tracker)
            .copied()
            .or(Some(p.assets.exit))
    }
    pub(crate) fn map_blocks_weapon(&self) -> bool {
        self.state
            .map
            .as_ref()
            .and_then(|m| m.gameplay.as_ref())
            .is_some_and(|s| {
                s.armed
                    .iter()
                    .zip(&s.used)
                    .any(|(armed, used)| *armed && !*used)
            })
    }
    pub(crate) fn map_enemy_tick(&mut self) -> Result<bool, String> {
        let Some(p) = self.map.as_ref().and_then(|m| m.gameplay.as_ref()) else {
            return Ok(false);
        };
        let s = self.state.map.as_ref().unwrap().gameplay.as_ref().unwrap();
        let warrior: Vec<bool> = s
            .enemy_factories
            .iter()
            .map(|i| p.assets.factories[*i].warrior)
            .collect();
        let player = self.state.pawn.body.position;
        let eye = self.eye();
        for (i, is_warrior) in warrior.into_iter().enumerate() {
            let mut e = self.state.enemies[i].clone();
            e.flash = (e.flash - STEP).max(0.);
            e.cooldown = (e.cooldown - STEP).max(0.);
            if e.health == 0 {
                self.state.enemies[i] = e;
                continue;
            }
            let d = crate::distance(player, e.position);
            let muzzle = [e.position[0], e.position[1], e.position[2] + 35.];
            let visible = d < 1800. && self.wall_fraction(muzzle, eye)? > 0.999;
            if visible && d > if is_warrior { 100. } else { 500. } {
                let speed = if is_warrior { 220. } else { 65. };
                let next = std::array::from_fn(|k| {
                    e.position[k] + (player[k] - e.position[k]) * (speed * STEP / d).min(1.)
                });
                let extent = if is_warrior {
                    [35., 35., 80.]
                } else {
                    [22., 22., 60.]
                };
                if let Some(path) = crate::squad::corridor(&self.world, e.position, next, extent)? {
                    if let Some(p) = path.last() {
                        e.position = *p;
                    }
                }
            }
            if visible && e.cooldown == 0. && (!is_warrior || d < 130.) {
                let sound = (!is_warrior)
                    .then(|| self.map.as_ref().unwrap().assets.enemy_fire_sound)
                    .flatten();
                if self.state.accept_enemy_attack(
                    i,
                    muzzle,
                    eye,
                    if is_warrior { 10 } else { 5 },
                    sound,
                    !is_warrior,
                ) {
                    e.cooldown = if is_warrior { 1.2 } else { 1.4 };
                    e.flash = 0.12;
                }
            }
            self.state.enemies[i] = e;
        }
        Ok(true)
    }
    pub(crate) fn sync_map_gates(&mut self) {
        if let Some(p) = self.map.as_ref().and_then(|m| m.gameplay.as_ref()) {
            let s = self.state.map.as_ref().unwrap().gameplay.as_ref().unwrap();
            for (index, (g, a)) in p.gates.iter().zip(&p.assets.gates).enumerate() {
                if let Some((i, h)) = &g.collider {
                    self.world.actors[*i].shape = if gate_released(a, s, index) {
                        rc_package::world_collision::BodyShape::Clear
                    } else {
                        rc_package::world_collision::BodyShape::Hulls(h.clone())
                    };
                }
            }
        }
    }
    pub(crate) fn map_gameplay_status(&self) -> Option<String> {
        let p = self.map.as_ref()?.gameplay.as_ref()?;
        let s = self.state.map.as_ref()?.gameplay.as_ref()?;
        if self
            .state
            .map
            .as_ref()?
            .companion
            .as_ref()
            .is_some_and(|c| c.down)
        {
            return Some(format!(
                "GEO_01B · Scorch kampfunfaehig · BENUTZEN 3s halten · {:.0}%",
                100. * s.revive_progress / 3.
            ));
        }
        if s.transition.is_some() {
            return Some("GEO_01B abgeschlossen · Uebergang zu GEO_01C bereit".into());
        }
        for (i, x) in p.assets.interactions.iter().enumerate() {
            if s.armed[i] && !s.used[i] {
                return Some("Ladung bereit · 300 Einheiten Abstand, dann FEUER".into());
            }
            if s.progress[i] > 0. && !s.used[i] {
                return Some(format!(
                    "{} · {:.0}%",
                    x.label,
                    100. * s.progress[i] / x.seconds
                ));
            }
        }
        let objective = s.objectives.iter().position(|v| !*v).unwrap_or(7);
        Some(format!(
            "GEO_01B · {} · BENUTZEN am Ziel",
            p.assets.objectives[objective]
        ))
    }
    pub(crate) fn map_encounter_geometry(
        &self,
    ) -> Result<
        (
            Vec<rc_package::geometry::Triangle>,
            Vec<Option<rc_render::FaceTexture>>,
        ),
        String,
    > {
        let Some(p) = self.map.as_ref().and_then(|m| m.gameplay.as_ref()) else {
            return Ok((vec![], vec![]));
        };
        let s = self.state.map.as_ref().unwrap().gameplay.as_ref().unwrap();
        let mut triangles = vec![];
        let mut faces = vec![];
        for (index, (gate, asset)) in p.gates.iter().zip(&p.assets.gates).enumerate() {
            let distance = crate::distance(self.eye(), gate.scene.center.map(f64::from));
            if distance > 5000. + gate.scene.radius as f64 {
                continue;
            }
            if !gate_released(asset, s, index) || asset.offset != [0.; 3] {
                let offset = if gate_released(asset, s, index) {
                    asset.offset
                } else {
                    [0.; 3]
                };
                triangles.extend(gate.scene.triangles.iter().cloned().map(|mut t| {
                    t.points = t
                        .points
                        .map(|v| std::array::from_fn(|i| v[i] + offset[i] as f32));
                    t
                }));
                faces.extend(gate.scene.appearance.iter().map(|f| {
                    f.map(|mut f| {
                        f.texture += gate.texture;
                        f
                    })
                }));
            }
        }
        for (position, yaw, kind, alive, moving) in self
            .state
            .enemies
            .iter()
            .enumerate()
            .filter(|(_, e)| e.health > 0)
            .map(|(i, e)| {
                (
                    e.position,
                    (self.state.pawn.body.position[1] - e.position[1])
                        .atan2(self.state.pawn.body.position[0] - e.position[0]),
                    usize::from(p.assets.factories[s.enemy_factories[i]].warrior),
                    true,
                    e.cooldown < 1.,
                )
            })
            .chain(std::iter::once((
                s.fixer.position,
                s.fixer.yaw,
                2,
                s.fixer.health > 0,
                s.fixer.moving,
            )))
        {
            if !alive {
                continue;
            }
            let (mesh, base) = &p.meshes[kind];
            let clip = mesh.clip(if moving {
                "RunForward"
            } else {
                "ActionBreathe"
            })?;
            let sequence = &mesh.mesh.clips[clip].sequence;
            let frame = (self.state.statistics.ticks as f64
                * STEP
                * f32::from_bits(sequence.rate_bits) as f64
                / sequence.frames as f64)
                .rem_euclid(1.) as f32;
            let channel = mesh.channel(clip, frame)?;
            let mut geometry = mesh.geometry_with_root(&channel, true)?;
            let height = if kind == 1 {
                160.
            } else if kind == 2 {
                168.
            } else {
                120.
            };
            let scale = height / (mesh.rest_bounds[1][2] - mesh.rest_bounds[0][2]);
            let (sin, cos) = (yaw as f32).sin_cos();
            for t in &mut geometry.triangles {
                t.points = t.points.map(|v| {
                    [
                        position[0] as f32 + (v[0] * cos - v[1] * sin) * scale,
                        position[1] as f32 + (v[0] * sin + v[1] * cos) * scale,
                        position[2] as f32 - height / 2. + (v[2] - mesh.rest_bounds[0][2]) * scale,
                    ]
                });
            }
            if kind == 2 {
                let map = self.map.as_ref().unwrap();
                let transform = mesh.attachment_transform(&channel, "weaponAttach_R", true)?;
                for triangle in &map.weapon.triangles {
                    let mut t = triangle.clone();
                    t.points = t.points.map(|v| {
                        let v: [f32; 3] = std::array::from_fn(|i| {
                            v[0] * transform[0][i]
                                + v[1] * transform[1][i]
                                + v[2] * transform[2][i]
                                + transform[3][i]
                        });
                        [
                            position[0] as f32 + (v[0] * cos - v[1] * sin) * scale,
                            position[1] as f32 + (v[0] * sin + v[1] * cos) * scale,
                            position[2] as f32 - height / 2.
                                + (v[2] - mesh.rest_bounds[0][2]) * scale,
                        ]
                    });
                    geometry.triangles.push(t);
                }
                geometry
                    .appearance
                    .extend(map.weapon.appearance.iter().map(|f| {
                        f.map(|mut f| {
                            f.texture += map.weapon_texture;
                            f
                        })
                    }));
            }
            triangles.extend(geometry.triangles);
            faces.extend(geometry.appearance.into_iter().map(|f| {
                f.map(|mut f| {
                    if f.texture < mesh.mesh.materials.len() {
                        f.texture += base;
                    }
                    f
                })
            }));
        }
        Ok((triangles, faces))
    }
}

// Physical graph following shared by Scorch and Fixer. The player is never moved by this host.
pub(crate) fn follow(
    world: &rc_package::world_collision::StaticBodyWorld,
    nav: &crate::navigation::Assets,
    c: &mut crate::map_runtime::Companion,
    agent: &mut crate::navigation::Agent,
    target: Point,
    now: f64,
    speed: f64,
) -> Result<(), String> {
    follow_to(world, nav, c, agent, target, now, speed, 180.)
}
#[allow(clippy::too_many_arguments)]
pub(crate) fn follow_to(
    world: &rc_package::world_collision::StaticBodyWorld,
    nav: &crate::navigation::Assets,
    c: &mut crate::map_runtime::Companion,
    agent: &mut crate::navigation::Agent,
    target: Point,
    now: f64,
    speed: f64,
    stop: f64,
) -> Result<(), String> {
    c.moving = false;
    if !c.joined || c.down || c.health == 0 {
        agent.route.clear();
        return Ok(());
    }
    if crate::distance(c.position, target) < stop {
        agent.route.clear();
        agent.waypoint = 0;
        return Ok(());
    }
    if now >= agent.next_plan
        && (agent.waypoint >= agent.route.len() || crate::distance(agent.target, target) > 140.)
    {
        agent.route = crate::squad::plan_with_tolerance(
            nav,
            world,
            c.position,
            target,
            crate::squad::EXTENT,
            4.,
        )?;
        agent.waypoint = 0;
        agent.target = target;
        agent.next_plan = now + 1.;
        agent.plans += 1;
        if agent.route.is_empty() {
            agent.blocked += 1;
        }
    }
    while let Some(next) = agent.route.get(agent.waypoint) {
        if (c.position[0] - next[0]).hypot(c.position[1] - next[1]) > 5.
            || (c.position[2] - next[2]).abs() > 35.
        {
            break;
        }
        agent.waypoint += 1;
    }
    let next = agent.route.get(agent.waypoint).copied().unwrap_or(target);
    let from = c.position;
    if let Some(position) = follow_step(world, from, next, speed)? {
        c.position = position;
        c.moving = crate::distance(from, position) > 0.01;
        c.yaw = (next[1] - from[1]).atan2(next[0] - from[0]);
        if !c.moving {
            agent.blocked += 1;
            agent.route.clear();
            agent.waypoint = 0;
            agent.next_plan = now + 0.25;
        }
    } else {
        agent.blocked += 1;
        agent.route.clear();
        agent.waypoint = 0;
        agent.next_plan = now + 0.25;
    }

    Ok(())
}

/// Commit the grounded body solver result, including supported floor snaps at ramp ends.
pub(crate) fn follow_step(
    world: &rc_package::world_collision::StaticBodyWorld,
    from: Point,
    to: Point,
    speed: f64,
) -> Result<Option<Point>, String> {
    let d = (to[0] - from[0]).hypot(to[1] - from[1]);
    let fraction = (speed * STEP / d.max(0.001)).min(1.);
    let body = rc_package::physics::BodyState {
        position: from,
        velocity: [
            (to[0] - from[0]) * fraction / STEP,
            (to[1] - from[1]) * fraction / STEP,
            0.,
        ],
        grounded: true,
    };
    let frame = match world.body_tick(
        &body,
        STEP,
        rc_package::physics::PhysicsOptions {
            extent: crate::squad::EXTENT,
            gravity: 950.,
            terminal_speed: 4000.,
            skin: 0.5,
            support_distance: 35.,
            minimum_up: 0.7,
            max_iterations: 8,
            step_height: 35.,
        },
    ) {
        Ok(frame) => frame,
        Err(e)
            if e == "body tick starts penetrating geometry"
                || e == "horizontal body tick could not finish safely" =>
        {
            return Ok(None)
        }
        Err(e) => return Err(e),
    };
    if !frame.body.grounded || (frame.body.position[2] - from[2]).abs() > 35. {
        return Ok(None);
    }
    Ok(Some(frame.body.position))
}

fn default_follow_speed() -> f64 {
    300.
}

fn advance_gate_timer(timer: &mut f64, delay: f64, triggered: bool) {
    if delay == 0. {
        return;
    }
    if *timer < 0. && triggered {
        *timer = delay;
    } else if *timer > 0. {
        *timer = (*timer - STEP).max(0.);
        if *timer < 1e-8 {
            *timer = 0.;
        }
    }
}
fn gate_released(gate: &Gate, state: &State, index: usize) -> bool {
    if gate.delay > 0. {
        state
            .gate_timers
            .as_ref()
            .is_some_and(|timers| timers[index] == 0.)
    } else {
        state.events.contains(&gate.event)
    }
}
#[cfg(test)]
mod retired_collision_tests {
    use super::*;
    fn flat_world() -> rc_package::world_collision::StaticBodyWorld {
        use rc_package::hull::{Hull, HullSet};
        use std::sync::Arc;
        rc_package::world_collision::StaticBodyWorld {
            world: Ok(Arc::new(HullSet {
                hulls: vec![Hull {
                    offset: 0,
                    bounds: [[-1000., -1000., -100.], [1000., 1000., 0.]],
                    planes: vec![
                        [1., 0., 0., 1000.],
                        [-1., 0., 0., 1000.],
                        [0., 1., 0., 1000.],
                        [0., -1., 0., 1000.],
                        [0., 0., 1., 0.],
                        [0., 0., -1., 100.],
                    ],
                }],
                missing_solid_leaves: 0,
                empty_root_solid: false,
            })),
            actors: vec![],
        }
    }
    #[test]
    fn follower_accepts_solver_floor_snap_without_teleporting_horizontally() {
        let world = flat_world();
        let p = follow_step(&world, [0., 0., 113.], [100., 0., 84.5], 450.)
            .unwrap()
            .unwrap();
        assert!(p[0] > 0. && p[0] <= 7.500001);
        assert!((p[2] - 84.5).abs() < 0.01);
        assert_ne!(
            world.body_placement(p, crate::squad::EXTENT).unwrap().state,
            rc_package::movement::PlacementState::Penetrating
        );
    }
    #[test]
    fn follower_cannot_snap_across_an_unsupported_drop() {
        let world = flat_world();
        assert!(follow_step(&world, [0., 0., 184.], [100., 0., 84.5], 450.)
            .unwrap()
            .is_none());
    }
    #[test]
    fn wreck_collision_timer_waits_for_original_event_and_serialized_lifetime() {
        let mut timer = -1.;
        for _ in 0..300 {
            advance_gate_timer(&mut timer, 1., false);
        }
        assert_eq!(timer, -1.);
        advance_gate_timer(&mut timer, 1., true);
        assert_eq!(timer, 1.);
        for _ in 0..59 {
            advance_gate_timer(&mut timer, 1., true);
        }
        assert!(timer > 0.);
        advance_gate_timer(&mut timer, 1., true);
        assert_eq!(timer, 0.);
        for _ in 0..120 {
            advance_gate_timer(&mut timer, 1., true);
        }
        assert_eq!(timer, 0.);
    }
    #[test]
    fn wreck_timer_continues_after_one_shot_event_and_can_reset_for_restart() {
        let mut timer = -1.;
        advance_gate_timer(&mut timer, 0.5, true);
        for _ in 0..30 {
            advance_gate_timer(&mut timer, 0.5, false);
        }
        assert_eq!(timer, 0.);
        timer = -1.;
        advance_gate_timer(&mut timer, 0.5, false);
        assert_eq!(timer, -1.);
    }
}
