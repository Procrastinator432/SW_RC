//! Bounded ordinary JumpDest host. Native planning/trajectory/landing decisions;
//! static sweeps, body scaling, animation scheduling and ballistic stepping are adapters.
use crate::{lunge, navigation, visual::PreparedMesh, Game, Point, STEP};
use rc_package::{defensive_jump as d, jump_goal, jump_trajectory};
use serde::{Deserialize, Serialize};

#[derive(Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Node {
    pub original: u32,
    pub name: String,
    pub position: [f32; 3],
}
#[derive(Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Assets {
    pub provenance: serde_json::Value,
    pub nodes: Vec<Node>,
    pub air_speed: f32,
    pub collision_radius: f32,
    pub probability: f32,
    pub player_probability: f32,
    pub root_translation: [f32; 3],
}
impl Assets {
    pub fn validate(&self) -> Result<(), String> {
        if self.nodes.is_empty()
            || self.nodes.len() > 64
            || !self.air_speed.is_finite()
            || !(1. ..=10000.).contains(&self.air_speed)
            || !self.collision_radius.is_finite()
            || !(1. ..=1000.).contains(&self.collision_radius)
            || [self.probability, self.player_probability]
                .iter()
                .any(|v| !v.is_finite() || !(0. ..=1.).contains(v))
            || self
                .root_translation
                .iter()
                .any(|v| !v.is_finite() || v.abs() > 10000.)
            || self.nodes.iter().enumerate().any(|(i, n)| {
                n.original == 0
                    || n.position.iter().any(|v| !v.is_finite() || v.abs() > 1e8)
                    || self.nodes[..i].iter().any(|p| p.original == n.original)
            })
        {
            return Err("Invalid defensive jump assets".into());
        }
        Ok(())
    }
    pub fn validate_mesh(&self, mesh: &PreparedMesh) -> Result<(), String> {
        for name in ["TakeOff", "Land", "FallBreathe"] {
            mesh.clip(name)?;
        }
        let c = &mesh.mesh.clips[mesh.clip("TakeOff")?];
        if mesh.mesh.bones[0].name.name != "Move"
            || c.mapping[0] != 0
            || c.sequence.word_50 != 0
            || mesh.mesh.rotation != [0; 3]
            || mesh.mesh.scale != [1f32.to_bits(); 3]
        {
            return Err("Unsupported TakeOff root binding".into());
        }
        if !(2..=300).contains(&c.sequence.frames)
            || !(1. ..=120.).contains(&f32::from_bits(c.sequence.rate_bits))
        {
            return Err("Unsupported TakeOff duration".into());
        }
        let t = c.sequence.tracks.first().ok_or("Missing Move track")?;
        if t.rotations.iter().any(|v| *v != [0; 3]) {
            return Err("TakeOff root rotation unsupported".into());
        }
        let translation = jump_goal::sequence_translation(
            *t.positions.first().ok_or("Missing root keys")?,
            *t.positions.last().unwrap(),
            t.position_scale_bits,
        );
        if translation.map(f32::to_bits) != self.root_translation.map(f32::to_bits) {
            return Err("TakeOff translation binding mismatch".into());
        }
        Ok(())
    }
}
#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, serde::Deserialize)]
pub enum Stage {
    Start,
    Flight,
    Land,
    Sleep,
}
#[derive(Clone, Debug, Serialize, serde::Deserialize)]
pub struct Motion {
    pub node: u32,
    pub stage: Stage,
    pub origin: Point,
    pub home: Point,
    pub returning: bool,
    pub flight_origin: Point,
    pub destination: Point,
    pub yaw: f64,
    pub velocity: [f32; 3],
    pub gravity: f32,
    pub flight_time: f32,
    pub elapsed: f64,
    pub goal: jump_goal::State,
    pub plan: d::Plan,
}
impl Motion {
    pub fn clip(&self) -> Option<&'static str> {
        match self.stage {
            Stage::Start => Some("TakeOff"),
            Stage::Flight => Some("FallBreathe"),
            Stage::Land => Some("Land"),
            Stage::Sleep => None,
        }
    }
    pub fn point(&self, t: f64) -> Point {
        let v = self.velocity.map(f64::from);
        [
            self.flight_origin[0] + v[0] * t,
            self.flight_origin[1] + v[1] * t,
            self.flight_origin[2] + v[2] * t + 0.5 * f64::from(self.gravity) * t * t,
        ]
    }
}
#[derive(Clone, Debug, Default, Serialize, serde::Deserialize)]
pub struct State {
    pub requests: Vec<bool>,
    pub motions: Vec<Option<Motion>>,
    pub attempts: u64,
    pub started: u64,
    pub returns: u64,
    pub landed: u64,
    pub completed: u64,
    pub interrupted: u64,
    pub blocked: u64,
    pub no_destination: u64,
    pub max_height: f64,
}
impl State {
    pub fn observation(&self) -> [u32; 12] {
        let mut words = [0; 12];
        words[..8].copy_from_slice(&[
            self.attempts as u32,
            self.started as u32,
            self.landed as u32,
            self.completed as u32,
            self.returns as u32,
            self.blocked as u32,
            self.interrupted as u32,
            self.no_destination as u32,
        ]);
        for (i, m) in self.motions.iter().take(4).enumerate() {
            words[8 + i] = m.as_ref().map_or(0, |m| match m.stage {
                Stage::Start => 1,
                Stage::Flight => 2,
                Stage::Land => 3,
                Stage::Sleep => 4,
            });
        }
        words
    }
    pub fn new(count: usize) -> Self {
        Self {
            requests: vec![false; count],
            motions: vec![None; count],
            ..Self::default()
        }
    }
}
fn sweep(
    world: &rc_package::world_collision::StaticBodyWorld,
    from: Point,
    to: Point,
) -> Result<bool, String> {
    let s = world.sweep_motion(from, to, navigation::EXTENT)?;
    if !s.complete {
        return Err("Incomplete defensive jump sweep".into());
    }
    Ok(s.contacts.is_empty())
}
pub fn candidate(game: &Game, owner: usize, node: &Node) -> Result<Option<Motion>, String> {
    candidate_from(game, game.state.enemies[owner].position, node)
}
fn candidate_from(game: &Game, origin: Point, node: &Node) -> Result<Option<Motion>, String> {
    let assets = game
        .combat
        .as_ref()
        .and_then(|a| a.jump.as_ref())
        .ok_or("Missing jump assets")?;
    let mesh = &game.visuals.as_ref().ok_or("Missing jump visuals")?.enemy;
    // Existing encounter uses a smaller body than native actors. Resolve a
    // standing center beneath the original navigation actor, without moving it.
    let Some(destination) = navigation::ground(&game.world, node.position.map(f64::from), 512.)?
    else {
        return Ok(None);
    };
    if crate::distance(origin, destination) < f64::from(assets.collision_radius) * 2. {
        return Ok(None);
    }
    let yaw = (destination[1] - origin[1]).atan2(destination[0] - origin[0]);
    let clip = mesh.clip("TakeOff")?;
    let seq = &mesh.mesh.clips[clip].sequence;
    let height = mesh.rest_bounds[1][2] - mesh.rest_bounds[0][2];
    let scale = 120. / f64::from(height);
    let local = assets.root_translation.map(|v| f64::from(v) * scale);
    let (s, c) = yaw.sin_cos();
    let flight_origin = [
        origin[0] + local[0] * c - local[1] * s,
        origin[1] + local[0] * s + local[1] * c,
        origin[2] + local[2],
    ];
    let volume = game.volumes.select(origin.map(|v| v as f32))?;
    volume.settings.apply(game.profile.physics)?;
    if !volume.complete
        || game.volumes.select(destination.map(|v| v as f32))?.object != volume.object
    {
        return Ok(None);
    }
    let gravity = volume.settings.gravity[2];
    if gravity >= 0. {
        return Ok(None);
    }
    let trajectory = jump_trajectory::portable(jump_trajectory::Input {
        origin: flight_origin.map(|v| v as f32),
        target: destination.map(|v| v as f32),
        horizontal_speed: assets.air_speed,
        gravity_z: gravity,
        allow_flat_arcs: false,
    });
    if !trajectory.time.is_finite()
        || trajectory.time > 12.
        || trajectory.velocity.iter().any(|v| !v.is_finite())
    {
        return Ok(None);
    }
    let motion = Motion {
        node: node.original,
        stage: Stage::Start,
        origin,
        home: origin,
        returning: false,
        flight_origin,
        destination,
        yaw,
        velocity: trajectory.velocity,
        gravity,
        flight_time: trajectory.time,
        elapsed: 0.,
        goal: jump_goal::State::default(),
        plan: d::Plan {
            selected: None,
            goals: vec![],
            clear_strafe: false,
        },
    };
    let mut previous = origin;
    for i in 1..seq.frames {
        let delta = lunge::offset(mesh, clip, i as f32 / seq.frames as f32, yaw)?;
        let next = std::array::from_fn(|a| origin[a] + delta[a]);
        if !sweep(&game.world, previous, next)? {
            return Ok(None);
        }
        previous = next;
    }
    if crate::distance(previous, flight_origin) > 0.5
        || !sweep(&game.world, previous, flight_origin)?
    {
        return Ok(None);
    }
    // A parabola deviates from its chord by at most |g|*dt^2/8.
    // Expand the swept body vertically by this bound. The final floor contact
    // is expected; actual landing still uses the normal 60 Hz body sweep.
    let steps = (f64::from(trajectory.time) / (STEP * 4.)).ceil() as usize;
    let interval = f64::from(trajectory.time) / steps as f64;
    let margin = f64::from(gravity.abs()) * interval * interval / 8.;
    let extent = [
        navigation::EXTENT[0],
        navigation::EXTENT[1],
        navigation::EXTENT[2] + margin,
    ];
    previous = flight_origin;
    for i in 1..=steps {
        let next = motion.point(i as f64 * interval);
        let sweep = game.world.sweep_motion(previous, next, extent)?;
        if !sweep.complete {
            return Err("Incomplete conservative arc sweep".into());
        }
        if sweep.contacts.iter().any(|c| {
            i != steps
                || c.hit.normal[2] < 0.7
                || c.hit.start_overlapping
                || trajectory.velocity[2] + gravity * trajectory.time > 0.
        }) {
            return Ok(None);
        }
        // Reject unsupported gravity boundaries; frozen trajectory is a host policy.
        let v = game.volumes.select(next.map(|v| v as f32))?;
        if !v.complete || v.object != volume.object {
            return Ok(None);
        }
        previous = next;
    }
    if crate::distance(previous, destination) > 0.5 {
        return Ok(None);
    }
    Ok(Some(motion))
}
struct PlanHost<'a> {
    nodes: &'a [d::Node],
    origin: [f32; 3],
    destination: u32,
    enemy_reachable: bool,
    anchor: u32,
}
impl d::Host for PlanHost<'_> {
    fn find_nearest(&mut self, kind: d::Kind) -> Option<d::Target> {
        if kind == d::Kind::Perch {
            return None;
        }
        let selected = d::nearest(
            self.nodes,
            d::Query {
                origin: self.origin,
                range: 10000.,
                anchor_present: true,
                require_reachable: true,
                require_same_zone: true,
                require_clear: false,
            },
        )
        .selected?;
        let n = self.nodes.iter().find(|n| n.id == selected)?;
        Some(d::Target {
            id: n.id,
            position: n.position,
        })
    }
    fn reserve(&mut self, node: u32) {
        self.destination = node;
    }
    fn clear_to(&mut self, _: u32) -> bool {
        true
    }
    fn reachable_to_enemy(&mut self, _: u32) -> bool {
        self.enemy_reachable
    }
    fn own_anchor(&mut self) -> Option<u32> {
        Some(self.anchor)
    }
}
impl Game {
    pub(crate) fn defensive_tick(&mut self) -> Result<(), String> {
        if self.combat.as_ref().is_none_or(|a| a.jump.is_none()) {
            return Ok(());
        }
        let count = self.state.enemies.len();
        if self.state.combat.as_ref().unwrap().jump.is_none() {
            self.state.combat.as_mut().unwrap().jump = Some(State::new(count));
        }
        for i in 0..count {
            let combat = self.state.combat.as_ref().unwrap();
            let dead = self.state.enemies[i].health == 0
                || combat.melee.as_ref().unwrap().player.health <= 0.
                || self.state.enemies[i].flash > 0.;
            let request = combat.jump.as_ref().unwrap().requests[i];
            let current = combat.jump.as_ref().unwrap().motions[i].clone();
            if dead {
                let state = self.state.combat.as_mut().unwrap().jump.as_mut().unwrap();
                state.requests[i] = false;
                if state.motions[i].take().is_some() {
                    self.state.enemies[i].position =
                        navigation::ground(&self.world, self.state.enemies[i].position, 4096.)?
                            .ok_or("Interrupted jump has no safe floor")?;
                    state.interrupted += 1;
                }
                continue;
            }
            if let Some(mut motion) = current {
                let before = self.state.enemies[i].position;
                let visual = &self.state.visuals.as_ref().unwrap().enemies[i];
                let mesh = &self.visuals.as_ref().unwrap().enemy;
                let playing = &mesh.mesh.clips[visual.clip].sequence.name.name;
                let mut blocked = false;
                match motion.stage {
                    Stage::Start => {
                        if playing == "TakeOff" {
                            let delta = lunge::offset(mesh, visual.clip, visual.frame, motion.yaw)?;
                            let next = std::array::from_fn(|a| motion.origin[a] + delta[a]);
                            blocked = !sweep(&self.world, before, next)?;
                            if !blocked {
                                self.state.enemies[i].position = next;
                            }
                            if visual.finished && !blocked {
                                blocked = !sweep(&self.world, next, motion.flight_origin)?;
                                if !blocked {
                                    self.state.enemies[i].position = motion.flight_origin;
                                    motion.stage = Stage::Flight;
                                    motion.goal.pre_update(
                                        2,
                                        motion.flight_origin.map(|v| v as f32),
                                        motion.destination.map(|v| v as f32),
                                        0.,
                                    );
                                }
                            }
                        }
                    }
                    Stage::Flight => {
                        motion.elapsed += STEP;
                        let next = motion.point(motion.elapsed);
                        let hit = self.world.sweep_motion(before, next, navigation::EXTENT)?;
                        if !hit.complete {
                            return Err("Incomplete flight sweep".into());
                        }
                        if let Some(contact) = hit.contacts.first() {
                            let t = contact.hit.fraction;
                            let center =
                                std::array::from_fn(|a| before[a] + (next[a] - before[a]) * t);
                            let radius = self
                                .combat
                                .as_ref()
                                .unwrap()
                                .jump
                                .as_ref()
                                .unwrap()
                                .collision_radius;
                            if contact.hit.normal[2] >= 0.7
                                && motion.velocity[2] + motion.gravity * motion.elapsed as f32 <= 0.
                                && motion.goal.pre_update(
                                    1,
                                    center.map(|v| v as f32),
                                    motion.destination.map(|v| v as f32),
                                    radius * radius,
                                ) == jump_goal::Decision::Land
                            {
                                self.state.enemies[i].position =
                                    [center[0], center[1], center[2] + 0.02];
                                motion.stage = Stage::Land;
                                motion.elapsed = 0.;
                                self.state
                                    .combat
                                    .as_mut()
                                    .unwrap()
                                    .jump
                                    .as_mut()
                                    .unwrap()
                                    .landed += 1;
                            } else {
                                blocked = true;
                            }
                        } else {
                            self.state.enemies[i].position = next;
                        }
                        if motion.elapsed > f64::from(motion.flight_time) + 2. {
                            blocked = true;
                        }
                    }
                    Stage::Land => {
                        if playing == "Land" && visual.finished {
                            motion.stage = Stage::Sleep;
                            motion.elapsed = 0.;
                            let player = self.state.pawn.body.position;
                            self.state
                                .combat
                                .as_mut()
                                .unwrap()
                                .melee
                                .as_mut()
                                .unwrap()
                                .attacks[i]
                                .yaw = (player[1] - before[1]).atan2(player[0] - before[0]);
                        }
                    }
                    Stage::Sleep => {
                        motion.elapsed += STEP;
                        if motion.elapsed >= 0.5 {
                            // Bounded Follow/Goto host delegates to the existing walking graph.
                            let player = self.state.pawn.body.position;
                            let target = if motion
                                .plan
                                .goals
                                .iter()
                                .any(|g| matches!(g, d::Goal::FollowEnemy { .. }))
                            {
                                navigation::ground(
                                    &self.world,
                                    [player[0], player[1], player[2] + 80.],
                                    256.,
                                )?
                                .unwrap_or(player)
                            } else {
                                motion.home
                            };
                            let route = self
                                .combat
                                .as_ref()
                                .unwrap()
                                .navigation
                                .as_ref()
                                .unwrap()
                                .plan(&self.world, before, target)?;
                            if route.is_empty() && !motion.returning {
                                // Goto over an unsupported drop delegates to a separately
                                // collision-checked return arc. This is a port host policy.
                                let node = Node {
                                    original: motion.node,
                                    name: "ReturnToAnchor".into(),
                                    position: motion.home.map(|v| v as f32),
                                };
                                if let Some(mut back) = candidate_from(self, before, &node)? {
                                    back.returning = true;
                                    back.home = motion.home;
                                    back.plan = motion.plan;
                                    self.state
                                        .combat
                                        .as_mut()
                                        .unwrap()
                                        .melee
                                        .as_mut()
                                        .unwrap()
                                        .attacks[i]
                                        .yaw = back.yaw;
                                    let state =
                                        self.state.combat.as_mut().unwrap().jump.as_mut().unwrap();
                                    state.returns += 1;
                                    state.motions[i] = Some(back);
                                    continue;
                                }
                            }
                            let state = self.state.combat.as_mut().unwrap().jump.as_mut().unwrap();
                            state.completed += 1;
                            state.motions[i] = None;
                            if let Some(agents) =
                                &mut self.state.combat.as_mut().unwrap().navigation
                            {
                                agents[i].route = route;
                                agents[i].waypoint = 0;
                                agents[i].target = target;
                                agents[i].next_plan =
                                    self.state.statistics.ticks as f64 * STEP + 1.;
                            }
                            continue;
                        }
                    }
                }
                let state = self.state.combat.as_mut().unwrap().jump.as_mut().unwrap();
                state.max_height = state
                    .max_height
                    .max(self.state.enemies[i].position[2] - motion.origin[2]);
                if blocked {
                    self.state.enemies[i].position =
                        navigation::ground(&self.world, self.state.enemies[i].position, 4096.)?
                            .ok_or("Blocked jump has no safe floor")?;
                    state.blocked += 1;
                    state.motions[i] = None;
                } else {
                    state.motions[i] = Some(motion);
                }
            } else if request {
                self.state
                    .combat
                    .as_mut()
                    .unwrap()
                    .jump
                    .as_mut()
                    .unwrap()
                    .requests[i] = false;
                self.state
                    .combat
                    .as_mut()
                    .unwrap()
                    .jump
                    .as_mut()
                    .unwrap()
                    .attempts += 1;
                let assets = self.combat.as_ref().unwrap().jump.as_ref().unwrap();
                let mut choices = vec![];
                let mut nodes = vec![];
                let origin = self.state.enemies[i].position.map(|v| v as f32);
                let mut best = f32::from_bits(0x60ad78ec);
                for node in &assets.nodes {
                    let [x, y, z] = std::array::from_fn(|a| node.position[a] - origin[a]);
                    let squared = (y * y + x * x) + z * z;
                    let closer = squared < 10000f32 * 10000f32 && squared < best;
                    let available = !self
                        .state
                        .combat
                        .as_ref()
                        .unwrap()
                        .jump
                        .as_ref()
                        .unwrap()
                        .motions
                        .iter()
                        .flatten()
                        .any(|m| m.node == node.original);
                    let mut c = if available && closer {
                        candidate(self, i, node)?
                    } else {
                        None
                    };
                    if let Some(outward) = &c {
                        let home = Node {
                            original: node.original,
                            name: "ReturnToAnchor".into(),
                            position: origin,
                        };
                        if candidate_from(self, outward.destination, &home)?.is_none() {
                            c = None;
                        }
                    }
                    nodes.push(d::Node {
                        id: node.original,
                        position: node.position,
                        enabled: true,
                        available,
                        same_zone: c.is_some(),
                        class_matches: true,
                        reachable: c.is_some(),
                        clear: c.is_some(),
                    });
                    if let Some(c) = c {
                        best = squared;
                        choices.push(c);
                    }
                }
                let anchor = self
                    .combat
                    .as_ref()
                    .unwrap()
                    .navigation
                    .as_ref()
                    .unwrap()
                    .nodes
                    .iter()
                    .min_by(|a, b| {
                        crate::distance(a.position, self.state.enemies[i].position)
                            .total_cmp(&crate::distance(b.position, self.state.enemies[i].position))
                    })
                    .map_or(0, |n| n.original as u32);
                let selected = d::nearest(
                    &nodes,
                    d::Query {
                        origin,
                        range: 10000.,
                        anchor_present: true,
                        require_reachable: true,
                        require_same_zone: true,
                        require_clear: false,
                    },
                )
                .selected;
                let enemy_reachable =
                    if let Some(c) = choices.iter().find(|c| Some(c.node) == selected) {
                        let player = self.state.pawn.body.position;
                        if let Some(target) = navigation::ground(
                            &self.world,
                            [player[0], player[1], player[2] + 80.],
                            256.,
                        )? {
                            !self
                                .combat
                                .as_ref()
                                .unwrap()
                                .navigation
                                .as_ref()
                                .unwrap()
                                .plan(&self.world, c.destination, target)?
                                .is_empty()
                        } else {
                            false
                        }
                    } else {
                        false
                    };
                let mut host = PlanHost {
                    nodes: &nodes,
                    origin,
                    destination: 0,
                    enemy_reachable,
                    anchor,
                };
                let plan = d::plan(
                    &mut d::Flags {
                        must_jump: true,
                        is_melee_attacking: true,
                    },
                    origin,
                    &mut host,
                );
                if let Some((_, id)) = plan.selected {
                    let mut c = choices
                        .into_iter()
                        .find(|c| c.node == id)
                        .ok_or("Selected unsupported jump")?;
                    c.plan = plan;
                    let combat = self.state.combat.as_mut().unwrap();
                    combat.melee.as_mut().unwrap().attacks[i].yaw = c.yaw;
                    combat.melee.as_mut().unwrap().attacks[i].active = None;
                    if let Some(agents) = &mut combat.navigation {
                        agents[i].goal = None;
                        agents[i].route.clear();
                    }
                    let state = combat.jump.as_mut().unwrap();
                    state.started += 1;
                    state.motions[i] = Some(c);
                } else {
                    self.state
                        .combat
                        .as_mut()
                        .unwrap()
                        .jump
                        .as_mut()
                        .unwrap()
                        .no_destination += 1;
                }
            }
        }
        Ok(())
    }
}
