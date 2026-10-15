//! Mission Warrior controller. Original decision/goal/notify rules with bounded
//! perception, walking graph, reaction and scheduling hosts.
use crate::{Game, Point, STEP};
use rc_package::{geonosian_attack as decision, pawn_damage};
use serde::{Deserialize, Serialize};
pub const EXTENT: Point = [50., 50., 115.];
#[derive(Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Assets {
    pub mesh: Option<crate::visual::Mesh>,
    pub navigation: crate::navigation::Assets,
    pub melee: crate::melee::Rules,
    pub burst_count: i32,
    pub provenance: serde_json::Value,
}
#[derive(Clone, Debug, Default, Serialize, serde::Deserialize)]
pub struct Agent {
    #[serde(skip_serializing_if = "Option::is_none")]
    pub advanced: Option<crate::mission_tactics::EnemyAgent>,
    pub navigation: crate::navigation::Agent,
    pub attack: Option<usize>,
    pub hit_emitted: bool,
    pub serial: u64,
    pub last_seen: Option<Point>,
    pub memory_until: f64,
    pub reaction: f64,
    pub interruptions: u64,
    pub attacks: u64,
    pub hits: u64,
    pub misses: u64,
    pub lost_contacts: u64,
}
impl Agent {
    pub fn interrupt(&mut self) {
        if let Some(a) = &mut self.advanced {
            a.abort = true;
        }
        if self.attack.take().is_some() {
            self.interruptions += 1;
        }
        self.navigation.goal = None;
        self.navigation.route.clear();
        self.navigation.next_plan = 0.;
        self.reaction = 0.25;
    }
    pub(crate) fn remember(&mut self, visible: bool, player: Point, time: f64) -> Option<Point> {
        if visible {
            self.last_seen = Some(player);
            self.memory_until = time + 3.;
        }
        if !visible && time > self.memory_until {
            if self.last_seen.take().is_some() {
                self.lost_contacts += 1;
            }
            self.navigation.route.clear();
            self.navigation.goal = None;
        }
        self.last_seen
    }
}
struct Host(bool);
impl decision::Host for Host {
    fn can_path(&mut self) -> bool {
        self.0
    }
    fn can_jump(&mut self) -> bool {
        false
    }
    fn can_move_lunge(&mut self) -> bool {
        false
    }
    fn can_throw_grenade(&mut self) -> bool {
        false
    }
}
impl Game {
    pub fn attach_warrior_ai(&mut self, mut assets: Assets) -> Result<(), String> {
        if self.warrior_ai.is_some() || self.artillery.is_none() {
            return Err("Warrior AI requires artillery".into());
        }
        assets.navigation.validate()?;
        assets.melee.validate()?;
        if !(1..=32).contains(&assets.burst_count) {
            return Err("Invalid Warrior BurstCount".into());
        }
        let mesh = assets
            .mesh
            .take()
            .ok_or("Missing Warrior AI mesh")?
            .prepare()?;
        assets.melee.validate_clips(&mesh)?;
        for name in ["Death", "hit", "RunForward"] {
            mesh.clip(name)?;
        }
        let base = self.scene.textures.len();
        self.scene
            .textures
            .extend(mesh.mesh.materials.iter().map(|m| rc_render::Texture {
                width: m.width,
                height: m.height,
                pixels: m.pixels.clone(),
            }));
        let prepared = self.artillery.as_mut().unwrap();
        prepared.warrior = mesh;
        prepared.warrior_texture = base;
        self.warrior_ai = Some(assets);
        Ok(())
    }
    pub(crate) fn warrior_ai_tick(&mut self) -> Result<(), String> {
        if self.tactics.is_some() {
            return self.tactics_enemy_tick();
        }
        let assets = self.warrior_ai.as_ref().unwrap();
        let prepared = self.artillery.as_ref().unwrap();
        let time = self.state.statistics.ticks as f64 * STEP;
        let player = self.state.pawn.body.position;
        let state = self.state.artillery.as_mut().unwrap();
        for w in &mut state.warriors {
            let ai = w.ai.get_or_insert_with(Agent::default);
            let old_clip = w.clip.clone();
            if w.health == 0 {
                ai.attack = None;
                ai.navigation.goal = None;
                ai.navigation.route.clear();
                w.clip = "Death".into();
            } else if ai.reaction > 0. {
                ai.reaction = (ai.reaction - STEP).max(0.);
                w.clip = "hit".into();
                w.pawn.body.velocity = [0.; 3];
            } else {
                let from = w.pawn.body.position;
                let d = crate::distance(from, player);
                let visible = d < 2600.
                    && self
                        .world
                        .sweep_motion(
                            [from[0], from[1], from[2] + 80.],
                            [player[0], player[1], player[2] + 60.],
                            [0.; 3],
                        )?
                        .contacts
                        .is_empty();
                let target = ai.remember(visible, player, time);
                if let Some(target) =
                    target.filter(|_| ai.attack.is_none() && time >= ai.navigation.next_plan)
                {
                    let probe = [target[0], target[1], target[2] + 160.];
                    let supported = match self.world.floor_contact(probe, EXTENT, 400., 0.7) {
                        Ok(contact) => contact,
                        Err(error) if error == "floor probe starts penetrating geometry" => None,
                        Err(error) => return Err(error),
                    };
                    ai.navigation.route = if let Some(c) = supported {
                        let at = [probe[0], probe[1], probe[2] - 400. * c.hit.fraction + 0.02];
                        assets.navigation.plan_for(&self.world, from, at, EXTENT)?
                    } else {
                        vec![]
                    };
                    ai.navigation.target = target;
                    ai.navigation.waypoint = 0;
                    ai.navigation.plans += 1;
                    if ai.navigation.route.len() > 1 {
                        ai.navigation.detours += 1;
                    }
                    if ai.navigation.route.is_empty() {
                        ai.navigation.blocked += 1;
                    }
                    ai.navigation.next_plan = time
                        + if ai.navigation.route.is_empty() {
                            2.
                        } else {
                            1.
                        };
                }
                if ai.attack.is_none() {
                    if let Some(target) = target {
                        w.yaw = (target[1] - from[1]).atan2(target[0] - from[0]);
                    }
                    if visible && ai.navigation.goal.is_none() {
                        let nav = &assets.navigation;
                        let mask = decision::capabilities(
                            (d - self.profile.physics.extent[0]) as f32,
                            decision::Parameters {
                                lunge_distance: nav.lunge_distance,
                                jump_distance: nav.jump_distance,
                                offensive_jump_probability: nav.offensive_jump_probability,
                            },
                            &mut Host(
                                !ai.navigation.route.is_empty() || d <= assets.melee.melee_range,
                            ),
                        );
                        if decision::choose(mask, nav.offensive_jump_probability, || 0)
                            == decision::Maneuver::Melee
                        {
                            ai.navigation.goal = Some(crate::navigation::Goal::original(
                                time as f32,
                                assets.burst_count,
                            ));
                        }
                    }
                }
                if let Some(goal) = &mut ai.navigation.goal {
                    let facing = if d > 0. {
                        ((player[0] - from[0]) * w.yaw.cos() + (player[1] - from[1]) * w.yaw.sin())
                            / d
                    } else {
                        1.
                    };
                    let (start, finish) = goal.pre_update(
                        time,
                        d,
                        assets.melee.melee_range,
                        facing,
                        ai.attack.is_some(),
                        visible && !ai.navigation.route.is_empty(),
                    );
                    if start && visible {
                        let clip = (ai.serial % 2) as usize;
                        ai.serial += 1;
                        ai.attacks += 1;
                        ai.attack = Some(clip);
                        ai.hit_emitted = false;
                        w.clip = assets.melee.clips[clip].name.clone();
                        w.frame = 0.;
                        self.state.statistics.enemy_shots += 1;
                    }
                    if finish {
                        ai.navigation.goal = None;
                    }
                }
                if let Some(clip) = ai.attack {
                    let rule = &assets.melee.clips[clip];
                    let seq =
                        &prepared.warrior.mesh.clips[prepared.warrior.clip(&rule.name)?].sequence;
                    let next =
                        w.frame + f32::from_bits(seq.rate_bits) * STEP as f32 / seq.frames as f32;
                    if !ai.hit_emitted && w.frame < rule.hit_time && next >= rule.hit_time {
                        ai.hit_emitted = true;
                        let facing = if d > 0. {
                            ((player[0] - from[0]) * w.yaw.cos()
                                + (player[1] - from[1]) * w.yaw.sin())
                                / d
                        } else {
                            1.
                        };
                        if visible
                            && d <= assets.melee.melee_range
                            && facing > 0.5
                            && self.state.health > 0
                        {
                            let mut health = pawn_damage::State {
                                health: self.state.health as f32,
                                shields: 0.,
                                next_shield_recharge: 0.,
                            };
                            crate::melee::damage(
                                &mut health,
                                &assets.melee.player,
                                &assets.melee.melee_type,
                                pawn_damage::Hit {
                                    damage: assets.melee.melee_damage,
                                    bone: "",
                                    damage_type: None,
                                    bypass_shields: assets.melee.melee_bypass,
                                    time: time as f32,
                                },
                                0.,
                                (1., false),
                            );
                            let before = self.state.health;
                            self.state.health = crate::melee::display_health(health.health);
                            self.state.damage_flash = 0.2;
                            ai.hits += 1;
                            state.damage.push(crate::gunner_runtime::Damage {
                                tick: self.state.statistics.ticks,
                                victim: -1,
                                amount: before.saturating_sub(self.state.health),
                                position: player,
                            });
                            state
                                .effects
                                .push((self.state.statistics.ticks, "WarriorMeleeNotify".into()));
                        } else {
                            ai.misses += 1;
                        }
                    }
                    w.frame = next;
                    if next >= 1. - 1. / seq.frames as f32 {
                        ai.attack = None;
                        w.frame = 0.;
                        w.clip = "ActionBreathe".into();
                    }
                    continue;
                }
                w.clip = "ActionBreathe".into();
                if target.is_some_and(|known| {
                    crate::distance(from, known) > assets.melee.melee_range * 0.75
                }) {
                    while ai.navigation.waypoint < ai.navigation.route.len()
                        && crate::distance(from, ai.navigation.route[ai.navigation.waypoint]) < 8.
                    {
                        ai.navigation.waypoint += 1;
                    }
                    if let Some(&to) = ai.navigation.route.get(ai.navigation.waypoint) {
                        let length = crate::distance(from, to);
                        let step = (prepared.assets.warrior_speed * STEP).min(length);
                        let end =
                            std::array::from_fn(|i| from[i] + (to[i] - from[i]) * step / length);
                        if crate::navigation::walkable_for(&self.world, from, end, EXTENT)? {
                            w.pawn.body.position = end;
                            w.yaw = (to[1] - from[1]).atan2(to[0] - from[0]);
                            w.clip = "RunForward".into();
                        } else {
                            ai.navigation.route.clear();
                            ai.navigation.goal = None;
                            ai.navigation.blocked += 1;
                            ai.navigation.next_plan = time + 2.;
                        }
                    }
                }
            }
            if old_clip != w.clip {
                w.frame = 0.;
            }
            let seq = &prepared.warrior.mesh.clips[prepared.warrior.clip(&w.clip)?].sequence;
            let next = w.frame + f32::from_bits(seq.rate_bits) * STEP as f32 / seq.frames as f32;
            w.frame = if w.health == 0 || w.clip == "hit" {
                next.min(1. - 1. / seq.frames as f32)
            } else {
                next.rem_euclid(1.)
            };
        }
        Ok(())
    }
}
#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn lost_sight_keeps_last_seen_position_then_forgets() {
        let mut a = Agent::default();
        let p = [1., 2., 3.];
        assert_eq!(a.remember(true, p, 10.), Some(p));
        assert_eq!(a.remember(false, [99.; 3], 12.9), Some(p));
        assert_eq!(a.remember(false, [99.; 3], 13.01), None);
        assert_eq!(a.lost_contacts, 1);
    }
    #[test]
    fn hit_interrupts_attack_and_clears_route_and_goal() {
        let mut a = Agent {
            attack: Some(0),
            ..Agent::default()
        };
        a.navigation.goal = Some(crate::navigation::Goal::original(1., 1));
        a.navigation.route.push([1.; 3]);
        a.interrupt();
        assert!(a.attack.is_none() && a.navigation.goal.is_none() && a.navigation.route.is_empty());
        assert_eq!(a.interruptions, 1);
        assert_eq!(a.reaction, 0.25);
    }
}
