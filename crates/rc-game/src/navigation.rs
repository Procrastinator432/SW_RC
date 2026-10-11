//! Bounded walking host over selected original PathList/ReachSpec connections.
//! Dijkstra, floor sampling, steering and clip selection are port adapters.
use crate::{distance, Game, Point, STEP};
use rc_package::{geonosian_attack as ai, world_collision::StaticBodyWorld};
use serde::{Deserialize, Serialize};

pub const EXTENT: Point = [22., 22., 60.];
#[derive(Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Node {
    pub original: i32,
    pub name: String,
    pub position: Point,
    pub extra_cost: f64,
}
#[derive(Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Edge {
    pub original: i32,
    pub start: usize,
    pub end: usize,
    pub distance: f64,
}
#[derive(Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Assets {
    pub provenance: serde_json::Value,
    pub nodes: Vec<Node>,
    pub edges: Vec<Edge>,
    pub lunge_distance: f32,
    pub jump_distance: f32,
    pub offensive_jump_probability: f32,
}
#[derive(Default, Debug, Clone, Serialize)]
pub struct Agent {
    #[serde(skip_serializing_if = "Option::is_none")]
    pub defensive_after_melee: Option<bool>,
    pub route: Vec<Point>,
    pub waypoint: usize,
    pub target: Point,
    pub next_plan: f64,
    pub plans: u64,
    pub detours: u64,
    pub blocked: u64,
    pub goal: Option<Goal>,
}
#[derive(Debug, Clone, Serialize)]
pub struct Goal {
    #[serde(skip_serializing_if = "Option::is_none")]
    pub native: Option<rc_package::melee_goal::State>,
    pub remaining: u32,
    pub deadline: f64,
    pub next_reachability: f64,
}
impl Goal {
    pub fn new(time: f64, count: u32) -> Self {
        Self {
            native: None,
            remaining: count,
            deadline: time + f64::from(count) * 2.,
            next_reachability: time + 1.,
        }
    }
    pub fn original(time: f32, count: i32) -> Self {
        let native = rc_package::melee_goal::State::activate(
            time,
            count,
            rc_package::melee_goal::attack_time(count),
        );
        Self {
            remaining: count as u32,
            deadline: f64::from(native.completion_time),
            next_reachability: f64::from(native.next_reachability),
            native: Some(native),
        }
    }
    // Native PreUpdate tests reachability before attack and expiry after attack.
    pub fn pre_update(
        &mut self,
        time: f64,
        distance: f64,
        range: f64,
        facing: f64,
        playing: bool,
        reachable: bool,
    ) -> (bool, bool) {
        if let Some(native) = &mut self.native {
            let decision = native.update(
                time as f32,
                rc_package::melee_goal::Query {
                    target_valid: true,
                    distance_squared: (distance * distance) as f32,
                    range_squared: (range as f32) * (range as f32),
                    facing: facing as f32,
                    playing,
                    reachable,
                },
            );
            self.remaining = native.remaining as u32;
            self.deadline = f64::from(native.completion_time);
            self.next_reachability = f64::from(native.next_reachability);
            return decision;
        }
        if time > self.next_reachability {
            self.next_reachability = time + 1.;
            if distance > range && !reachable {
                return (false, true);
            }
        }
        let start = self.remaining > 0 && distance <= range && facing > 0.5 && !playing;
        if start {
            self.remaining -= 1;
        }
        (
            start,
            (time >= self.deadline || self.remaining == 0) && !playing && !start,
        )
    }
}
pub fn ground(
    world: &StaticBodyWorld,
    position: Point,
    depth: f64,
) -> Result<Option<Point>, String> {
    Ok(world.floor_contact(position, EXTENT, depth, 0.7)?.map(|c| {
        [
            position[0],
            position[1],
            position[2] - depth * c.hit.fraction + 0.02,
        ]
    }))
}
/// No unsupported stairs, jumps or drops: every sampled segment needs floor support.
pub fn walkable(world: &StaticBodyWorld, from: Point, to: Point) -> Result<bool, String> {
    let sweep = world.sweep_motion(from, to, EXTENT)?;
    if !sweep.complete {
        return Err("Incomplete navigation sweep".into());
    }
    if !sweep.contacts.is_empty() {
        return Ok(false);
    }
    let steps = (distance(from, to) / 24.).ceil().max(1.) as usize;
    if steps > 256 {
        return Ok(false);
    }
    for i in 0..=steps {
        let p = std::array::from_fn(|a| from[a] + (to[a] - from[a]) * i as f64 / steps as f64);
        if world.floor_contact(p, EXTENT, 4., 0.7)?.is_none() {
            return Ok(false);
        }
    }
    Ok(true)
}
impl Assets {
    pub fn validate(&self) -> Result<(), String> {
        if self.nodes.is_empty()
            || self.nodes.len() > 512
            || self.edges.len() > 4096
            || !self.lunge_distance.is_finite()
            || !(0. ..=10000.).contains(&self.lunge_distance)
            || !self.jump_distance.is_finite()
            || !(0. ..=10000.).contains(&self.jump_distance)
            || !self.offensive_jump_probability.is_finite()
            || !(0. ..=1.).contains(&self.offensive_jump_probability)
            || self.nodes.iter().any(|n| {
                n.original <= 0
                    || n.position.iter().any(|v| !v.is_finite() || v.abs() > 1e8)
                    || !n.extra_cost.is_finite()
                    || !(0. ..=1e6).contains(&n.extra_cost)
            })
            || self.edges.iter().any(|e| {
                e.original <= 0
                    || e.start >= self.nodes.len()
                    || e.end >= self.nodes.len()
                    || !e.distance.is_finite()
                    || !(1. ..=1e6).contains(&e.distance)
            })
        {
            return Err("Invalid walking navigation assets".into());
        }
        Ok(())
    }
    /// Directed graph; equal-cost choices retain stable serialized node order.
    pub fn shortest(&self, starts: &[(usize, f64)], ends: &[(usize, f64)]) -> Option<Vec<usize>> {
        let mut costs = vec![f64::INFINITY; self.nodes.len()];
        let mut previous = vec![None; self.nodes.len()];
        let mut done = vec![false; self.nodes.len()];
        for &(i, c) in starts {
            costs[i] = costs[i].min(c);
        }
        for _ in 0..self.nodes.len() {
            let u = (0..costs.len())
                .filter(|&i| !done[i] && costs[i].is_finite())
                .min_by(|&a, &b| costs[a].total_cmp(&costs[b]).then(a.cmp(&b)));
            let Some(u) = u else {
                break;
            };
            done[u] = true;
            for edge in self.edges.iter().filter(|e| e.start == u) {
                let c = costs[u] + edge.distance + self.nodes[edge.end].extra_cost;
                if c < costs[edge.end] {
                    costs[edge.end] = c;
                    previous[edge.end] = Some(u);
                }
            }
        }
        let &(mut end, _) =
            ends.iter()
                .filter(|(i, _)| costs[*i].is_finite())
                .min_by(|(a, ac), (b, bc)| {
                    (costs[*a] + ac).total_cmp(&(costs[*b] + bc)).then(a.cmp(b))
                })?;
        let mut path = vec![end];
        while let Some(p) = previous[end] {
            path.push(p);
            end = p;
        }
        path.reverse();
        Some(path)
    }
    pub fn plan(
        &self,
        world: &StaticBodyWorld,
        from: Point,
        to: Point,
    ) -> Result<Vec<Point>, String> {
        if walkable(world, from, to)? {
            return Ok(vec![to]);
        }
        let connect = |point| -> Result<Vec<(usize, f64)>, String> {
            let mut nearest: Vec<_> = self
                .nodes
                .iter()
                .enumerate()
                .map(|(i, n)| (i, distance(n.position, point)))
                .filter(|(_, d)| *d < 1500.)
                .collect();
            nearest.sort_by(|a, b| a.1.total_cmp(&b.1).then(a.0.cmp(&b.0)));
            let mut result = vec![];
            for (i, d) in nearest.into_iter().take(8) {
                if walkable(world, point, self.nodes[i].position)? {
                    result.push((i, d));
                }
            }
            Ok(result)
        };
        let starts = connect(from)?;
        let ends = connect(to)?;
        let Some(path) = self.shortest(&starts, &ends) else {
            return Ok(vec![]);
        };
        let mut result: Vec<_> = path.into_iter().map(|i| self.nodes[i].position).collect();
        result.push(to);
        Ok(result)
    }
}
struct WalkingHost(bool, bool);
impl ai::Host for WalkingHost {
    fn can_path(&mut self) -> bool {
        self.0
    }
    fn can_jump(&mut self) -> bool {
        false
    }
    fn can_move_lunge(&mut self) -> bool {
        self.1
    }
    fn can_throw_grenade(&mut self) -> bool {
        false
    }
}
impl Game {
    pub(crate) fn navigation_enemy_tick(&mut self) -> Result<(), String> {
        let assets = self.combat.as_ref().unwrap();
        let nav = assets.navigation.as_ref().unwrap();
        let rules = assets.melee.as_ref().unwrap();
        let time = self.state.statistics.ticks as f64 * STEP;
        let player = self.state.pawn.body.position;
        let target = ground(&self.world, [player[0], player[1], player[2] + 80.], 256.)?;
        let combat = self.state.combat.as_mut().unwrap();
        let agents = combat
            .navigation
            .get_or_insert_with(|| vec![Agent::default(); self.state.enemies.len()]);
        let lunge = if let Some(profile) = &assets.lunge {
            Some(combat.lunge.get_or_insert_with(|| {
                crate::lunge::State::new(profile.seed, self.state.enemies.len())
            }))
        } else {
            None
        };
        let mut lunge = lunge;
        let melee = combat.melee.as_mut().unwrap();
        if time as f32 > melee.player.next_shield_recharge && melee.player.health > 0. {
            melee.player.shields =
                (melee.player.shields + rules.shield_rate * STEP as f32).min(rules.player_shields);
        }
        for (i, e) in self.state.enemies.iter_mut().enumerate() {
            let agent = &mut agents[i];
            let attack = &mut melee.attacks[i];
            e.flash = (e.flash - STEP).max(0.);
            e.cooldown = (e.cooldown - STEP).max(0.);
            if e.health == 0 || melee.player.health <= 0. || e.flash > 0. {
                if let Some(state) = lunge.as_deref_mut() {
                    if state.motions[i].take().is_some() {
                        e.position = crate::lunge::settle(&self.world, e.position)?;
                        state.interrupted += 1;
                    }
                }
                attack.active = None;
                agent.goal = None;
                agent.defensive_after_melee = None;
                agent.route.clear();
                agent.next_plan = time;
                continue;
            }
            if combat
                .jump
                .as_ref()
                .is_some_and(|s| s.requests[i] || s.motions[i].is_some())
            {
                continue;
            }
            if time >= agent.next_plan && lunge.as_ref().is_none_or(|s| s.motions[i].is_none()) {
                agent.route = if let Some(t) = target {
                    nav.plan(&self.world, e.position, t)?
                } else {
                    vec![]
                };
                agent.target = target.unwrap_or(player);
                agent.waypoint = 0;
                agent.plans += 1;
                if agent.route.len() > 1 {
                    agent.detours += 1;
                }
                if agent.route.is_empty() {
                    agent.blocked += 1;
                }
                agent.next_plan = time + if agent.route.is_empty() { 2. } else { 1. };
            }
            let reachable = !agent.route.is_empty();
            let d = distance(e.position, player);
            if agent.goal.is_none() && attack.active.is_none() {
                let yaw = (player[1] - e.position[1]).atan2(player[0] - e.position[0]);
                let index = 2 + (attack.serial % 2) as usize;
                let candidate = if lunge.is_some()
                    && d - self.profile.physics.extent[0] <= f64::from(nav.lunge_distance)
                {
                    let mesh = &self.visuals.as_ref().ok_or("Missing lunge visuals")?.enemy;
                    let motion = crate::lunge::Motion {
                        clip: mesh.clip(&rules.clips[index].name)?,
                        origin: e.position,
                        yaw,
                        frame: 0.,
                    };
                    if crate::lunge::feasible(&self.world, mesh, &motion)? {
                        Some(motion)
                    } else {
                        None
                    }
                } else {
                    None
                };
                let p = ai::Parameters {
                    lunge_distance: nav.lunge_distance,
                    jump_distance: nav.jump_distance,
                    offensive_jump_probability: nav.offensive_jump_probability,
                };
                let mask = ai::capabilities(
                    (d - self.profile.physics.extent[0]) as f32,
                    p,
                    &mut WalkingHost(reachable, candidate.is_some()),
                );
                let chosen = ai::choose(mask, p.offensive_jump_probability, || {
                    lunge.as_deref_mut().map_or(0, |s| s.draw())
                });
                if chosen == ai::Maneuver::Lunge {
                    let state = lunge.as_deref_mut().ok_or("Missing lunge state")?;
                    state.motions[i] = candidate;
                    state.started += 1;
                    attack.yaw = yaw;
                    attack.active = Some(index);
                    attack.serial += 1;
                    e.cooldown = 2.;
                    self.state.statistics.enemy_shots += 1;
                } else if chosen == ai::Maneuver::Melee {
                    if let (Some(profile), Some(rng)) = (&assets.jump, lunge.as_deref_mut()) {
                        agent.defensive_after_melee = Some(ai::request_defensive_jump(
                            true,
                            profile.probability,
                            profile.player_probability,
                            || rng.draw(),
                        ));
                    }
                    agent.goal = Some(if let Some(bot) = &assets.bot {
                        Goal::original(time as f32, bot.burst_count)
                    } else {
                        // Historical scenes retain their original adapter budget/clock.
                        Goal::new(time, 2)
                    });
                }
            }
            if attack.active.is_none() {
                attack.yaw = (player[1] - e.position[1]).atan2(player[0] - e.position[0]);
            }
            if let Some(goal) = &mut agent.goal {
                let facing = if d > 0. {
                    ((player[0] - e.position[0]) * attack.yaw.cos()
                        + (player[1] - e.position[1]) * attack.yaw.sin())
                        / d
                } else {
                    0.
                };
                let (start, finish) = goal.pre_update(
                    time,
                    d,
                    rules.melee_range,
                    facing,
                    attack.active.is_some(),
                    reachable,
                );
                if start {
                    attack.active = Some((attack.serial % 2) as usize);
                    attack.serial += 1;
                    e.cooldown = 2.;
                    self.state.statistics.enemy_shots += 1;
                }
                if finish {
                    if let Some(jump) = &mut combat.jump {
                        jump.requests[i] = agent.defensive_after_melee.take().unwrap_or(false);
                    }
                    agent.goal = None;
                    attack.active = None;
                }
            }
            if combat.jump.as_ref().is_some_and(|s| s.requests[i]) {
                continue;
            }
            if attack.active.is_none() && d > rules.melee_range * 0.75 && reachable {
                while agent.waypoint < agent.route.len()
                    && distance(e.position, agent.route[agent.waypoint]) < 2.
                {
                    agent.waypoint += 1;
                }
                if let Some(&waypoint) = agent.route.get(agent.waypoint) {
                    let length = distance(e.position, waypoint);
                    let step = (65. * STEP).min(length);
                    let end = std::array::from_fn(|a| {
                        e.position[a] + (waypoint[a] - e.position[a]) * step / length
                    });
                    if walkable(&self.world, e.position, end)? {
                        e.position = end;
                    } else {
                        agent.route.clear();
                        agent.goal = None;
                        agent.blocked += 1;
                        agent.next_plan = time + 2.;
                    }
                }
            }
        }
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn native_melee_goal_boundaries_and_order() {
        let mut g = Goal::new(0., 2);
        assert_eq!(
            g.pre_update(1., 351., 350., 1., false, false),
            (false, false)
        );
        assert_eq!(
            g.pre_update(1.0001, 351., 350., 1., false, false),
            (false, true)
        );
        let mut g = Goal::new(0., 2);
        assert_eq!(
            g.pre_update(0., 350., 350., 0.5, false, true),
            (false, false)
        );
        assert_eq!(
            g.pre_update(4., 350., 350., 0.5001, false, true),
            (true, false)
        );
        assert_eq!(g.pre_update(5., 350., 350., 1., true, true), (false, false));
        assert_eq!(g.pre_update(5., 351., 350., 1., false, true), (false, true));
        let mut g = Goal::new(0., 1);
        assert_eq!(g.pre_update(0., 350., 350., 1., false, true), (true, false));
        assert_eq!(
            g.pre_update(0.1, 350., 350., 1., true, true),
            (false, false)
        );
        assert_eq!(
            g.pre_update(0.2, 350., 350., 1., false, true),
            (false, true)
        );
    }
    fn graph() -> Assets {
        Assets {
            provenance: serde_json::Value::Null,
            lunge_distance: 800.,
            jump_distance: 3000.,
            offensive_jump_probability: 0.95,
            nodes: (0..4)
                .map(|i| Node {
                    original: i + 1,
                    name: i.to_string(),
                    position: [0.; 3],
                    extra_cost: 0.,
                })
                .collect(),
            edges: vec![
                Edge {
                    original: 1,
                    start: 0,
                    end: 1,
                    distance: 3.,
                },
                Edge {
                    original: 2,
                    start: 1,
                    end: 3,
                    distance: 3.,
                },
                Edge {
                    original: 3,
                    start: 0,
                    end: 2,
                    distance: 2.,
                },
                Edge {
                    original: 4,
                    start: 2,
                    end: 3,
                    distance: 2.,
                },
            ],
        }
    }
    #[test]
    fn directed_costs_unreachable_and_stable_ties() {
        let mut g = graph();
        g.validate().unwrap();
        assert_eq!(g.shortest(&[(0, 0.)], &[(3, 0.)]), Some(vec![0, 2, 3]));
        assert_eq!(g.shortest(&[(3, 0.)], &[(0, 0.)]), None);
        g.nodes[2].extra_cost = 2.;
        assert_eq!(g.shortest(&[(0, 0.)], &[(3, 0.)]), Some(vec![0, 1, 3]));
        assert_eq!(g.shortest(&[], &[(3, 0.)]), None);
    }
    #[test]
    fn rejects_unbounded_and_invalid_graphs() {
        let mut g = graph();
        g.edges[0].end = 4;
        assert!(g.validate().is_err());
        let mut g = graph();
        g.nodes[0].position[0] = f64::NAN;
        assert!(g.validate().is_err());
    }
}
