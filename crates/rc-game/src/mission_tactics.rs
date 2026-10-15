//! Mission enemy maneuvers and squad integration; prepared original decisions,
//! root curves and values, with explicit bounded scheduling/navigation hosts.
use crate::{mission_motion as motion, warrior_ai::EXTENT, Game, Point, STEP};
use rc_package::{defensive_jump as defensive, geonosian_attack as ai};
use serde::{Deserialize, Serialize};
#[derive(Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Assets {
    pub enemy_mesh: Option<crate::visual::Mesh>,
    pub jump: crate::jump::Assets,
    pub squad: crate::squad::Assets,
    pub provenance: serde_json::Value,
}
pub struct Prepared {
    pub assets: Assets,
    pub weapon: rc_render::Scene,
    pub weapon_texture: usize,
    pub sound_base: usize,
}
#[derive(Clone, Debug, Default, Serialize, serde::Deserialize)]
pub struct EnemyAgent {
    pub motion: Option<motion::Motion>,
    pub random: u32,
    pub draws: u64,
    pub abort: bool,
    pub defensive_pending: bool,
    pub lunges: u64,
    pub jumps: u64,
    pub defensive_jumps: u64,
    pub landed: u64,
    pub blocked: u64,
    pub interrupted: u64,
    pub target_scorch: bool,
}
impl EnemyAgent {
    pub fn draw(&mut self) -> u16 {
        self.random = self.random.wrapping_mul(214013).wrapping_add(2531011);
        self.draws += 1;
        ((self.random >> 16) & 32767) as u16
    }
}
struct Host(bool, bool, bool);
impl ai::Host for Host {
    fn can_path(&mut self) -> bool {
        self.0
    }
    fn can_jump(&mut self) -> bool {
        self.1
    }
    fn can_move_lunge(&mut self) -> bool {
        self.2
    }
    fn can_throw_grenade(&mut self) -> bool {
        false
    }
}
fn visible(
    world: &rc_package::world_collision::StaticBodyWorld,
    from: Point,
    to: Point,
) -> Result<bool, String> {
    Ok(crate::distance(from, to) < 3000.
        && world
            .sweep_motion(
                [from[0], from[1], from[2] + 80.],
                [to[0], to[1], to[2] + 60.],
                [0.; 3],
            )?
            .contacts
            .is_empty())
}
fn jump_choice(
    game: &Game,
    from: Point,
    target: Point,
    reserved: &[u32],
    is_defensive: bool,
) -> Result<Option<motion::Motion>, String> {
    let p = game.tactics.as_ref().unwrap();
    let mesh = &game.artillery.as_ref().unwrap().warrior;
    let mut candidates = vec![];
    let mut nodes = vec![];
    for n in &p.assets.jump.nodes {
        let available = !reserved.contains(&n.original);
        let candidate = if available {
            motion::jump(
                &game.world,
                &game.volumes,
                mesh,
                &p.assets.jump,
                from,
                n,
                is_defensive,
            )?
        } else {
            None
        };
        let good = candidate.as_ref().is_some_and(|m| {
            is_defensive || crate::distance(m.destination, target) < crate::distance(from, target)
        });
        nodes.push(defensive::Node {
            id: n.original,
            position: n.position,
            enabled: true,
            available,
            same_zone: good,
            class_matches: true,
            reachable: good,
            clear: good,
        });
        if good {
            candidates.push(candidate.unwrap());
        }
    }
    let selected = defensive::nearest(
        &nodes,
        defensive::Query {
            origin: from.map(|v| v as f32),
            range: 10000.,
            anchor_present: true,
            require_reachable: true,
            require_same_zone: true,
            require_clear: true,
        },
    )
    .selected;
    Ok(selected.and_then(|id| candidates.into_iter().find(|m| m.node == Some(id))))
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
pub fn health_hit(
    health: &mut u32,
    shields: &mut f32,
    rules: &crate::melee::Rules,
    defense: &crate::melee::Defense,
    time: f32,
) -> u32 {
    let before = *health;
    let mut state = rc_package::pawn_damage::State {
        health: *health as f32,
        shields: *shields,
        next_shield_recharge: 0.,
    };
    crate::melee::damage(
        &mut state,
        defense,
        &rules.melee_type,
        rc_package::pawn_damage::Hit {
            damage: rules.melee_damage,
            bone: "",
            damage_type: None,
            bypass_shields: rules.melee_bypass,
            time,
        },
        0.,
        (1., false),
    );
    *health = crate::melee::display_health(state.health);
    *shields = state.shields;
    before.saturating_sub(*health)
}
impl Game {
    pub fn attach_tactics(&mut self, mut a: Assets) -> Result<(), String> {
        if self.tactics.is_some() || self.warrior_ai.is_none() || self.rendezvous.is_none() {
            return Err("Tactics requires Warrior AI and Scorch".into());
        }
        a.jump.validate()?;
        a.squad.validate()?;
        let enemy = a
            .enemy_mesh
            .take()
            .ok_or("Missing enhanced enemy mesh")?
            .prepare()?;
        a.jump.validate_mesh(&enemy)?;
        self.warrior_ai
            .as_ref()
            .unwrap()
            .melee
            .validate_clips(&enemy)?;
        let scorch = a.squad.mesh.take().ok_or("Missing squad mesh")?.prepare()?;
        for name in [
            "Fire",
            "ImmobileBreathe",
            "ImmobileGetUp",
            "HealImmobileStart",
            "HealImmobileLoop",
            "HealImmobileStop",
            "CrouchBreathe",
            "CrouchForward",
            "DemolitionSetChargeStart",
            "DemolitionSetChargeLoop",
            "DemolitionSetChargeStop",
            "GestureSignalOK",
        ] {
            scorch.clip(name)?;
        }
        let weapon = rc_render::Scene::from_snapshot(&a.squad.weapon_snapshot)?;
        let base = textures(&mut self.scene, &enemy);
        let p = self.artillery.as_mut().unwrap();
        p.warrior = enemy;
        p.warrior_texture = base;
        let base = textures(&mut self.scene, &scorch);
        let p = self.rendezvous.as_mut().unwrap();
        p.mesh = scorch;
        p.texture_base = base;
        let weapon_texture = self.scene.textures.len();
        self.scene.textures.extend(weapon.textures.clone());
        let sound_base = self.mission.as_ref().unwrap().sounds.len();
        self.mission
            .as_mut()
            .unwrap()
            .sounds
            .extend(a.squad.sounds.iter().map(|s| crate::combat::Sound {
                source: s.source.clone(),
                wave: s.wave.clone(),
            }));
        self.state.squad = Some(crate::squad::State::new(&a.squad));
        self.tactics = Some(Prepared {
            assets: a,
            weapon,
            weapon_texture,
            sound_base,
        });
        self.initial = self.state.clone();
        Ok(())
    }
    pub(crate) fn tactics_enemy_tick(&mut self) -> Result<(), String> {
        let mut state = self.state.artillery.take().unwrap();
        let result = (|| -> Result<(), String> {
            let time = self.state.statistics.ticks as f64 * STEP;
            let assets = self.warrior_ai.as_ref().unwrap();
            let mesh = &self.artillery.as_ref().unwrap().warrior;
            for i in 0..state.warriors.len() {
                let reserved: Vec<_> = state
                    .warriors
                    .iter()
                    .filter_map(|w| w.ai.as_ref()?.advanced.as_ref()?.motion.as_ref()?.node)
                    .collect();
                let w = &mut state.warriors[i];
                let agent = w.ai.get_or_insert_with(crate::warrior_ai::Agent::default);
                let adv = agent.advanced.get_or_insert_with(|| EnemyAgent {
                    random: 0x62 + w.original,
                    ..EnemyAgent::default()
                });
                let before = w.pawn.body.position;
                let old_clip = w.clip.clone();
                if adv.abort || w.health == 0 {
                    if adv.motion.take().is_some() {
                        adv.interrupted += 1;
                        w.pawn.body.grounded = false;
                    }
                    adv.abort = false;
                    agent.attack = None;
                    agent.navigation.goal = None;
                    adv.defensive_pending = false;
                }
                if !w.pawn.body.grounded && adv.motion.is_none() {
                    let gravity = self
                        .volumes
                        .select(before.map(|v| v as f32))?
                        .settings
                        .gravity[2] as f64;
                    w.pawn.body.velocity[2] += gravity * STEP;
                    let next = std::array::from_fn(|j| before[j] + w.pawn.body.velocity[j] * STEP);
                    let hit = self.world.sweep_motion(before, next, EXTENT)?;
                    if let Some(c) = hit.contacts.first() {
                        w.pawn.body.position = std::array::from_fn(|j| {
                            before[j] + (next[j] - before[j]) * c.hit.fraction
                        });
                        w.pawn.body.position[2] += 0.02;
                        w.pawn.body.velocity = [0.; 3];
                        w.pawn.body.grounded = c.hit.normal[2] >= 0.7;
                    } else {
                        w.pawn.body.position = next;
                    }
                }
                if w.health == 0 {
                    w.clip = "Death".into();
                    agent.navigation.route.clear();
                } else if agent.reaction > 0. {
                    agent.reaction = (agent.reaction - STEP).max(0.);
                    w.clip = "hit".into();
                } else {
                    let from = w.pawn.body.position;
                    let player = self.state.pawn.body.position;
                    let scorch = self.state.rendezvous.as_ref().unwrap().pawn.body.position;
                    let squad = self.state.squad.as_ref().unwrap();
                    let player_valid = self.state.health > 0 && !squad.player_down;
                    let scorch_valid = squad.joined && squad.health > 0 && !squad.down;
                    let scorch_visible = scorch_valid && visible(&self.world, from, scorch)?;
                    let player_visible = player_valid && visible(&self.world, from, player)?;
                    // Visible nearer threats only; keep a committed swing/motion aimed at its selected victim.
                    if agent.attack.is_none() && adv.motion.is_none() {
                        let target_scorch = scorch_visible
                            && (!player_visible
                                || crate::distance(from, scorch)
                                    < crate::distance(from, player) * 0.8);
                        if adv.target_scorch != target_scorch {
                            agent.last_seen = None;
                            agent.navigation.goal = None;
                            agent.navigation.next_plan = 0.;
                        }
                        adv.target_scorch = target_scorch;
                    }
                    let target = if adv.target_scorch { scorch } else { player };
                    let seen = if adv.target_scorch {
                        scorch_visible
                    } else {
                        player_visible
                    };
                    let d = crate::distance(from, target);
                    let known = if seen {
                        agent.last_seen = Some(target);
                        agent.memory_until = time + 3.;
                        agent.last_seen
                    } else if time <= agent.memory_until {
                        agent.last_seen
                    } else {
                        agent.last_seen = None;
                        agent.navigation.route.clear();
                        None
                    };
                    if let Some(m) = &mut adv.motion {
                        let old = m.frame;
                        let old_stage = m.stage;
                        let (done, blocked) =
                            m.advance(&self.world, mesh, &mut w.pawn.body.position)?;
                        w.yaw = m.yaw;
                        w.clip = m.clip.clone();
                        w.frame = m.frame;
                        w.pawn.body.grounded = m.stage == 2 || m.node.is_none();
                        if old_stage == 1 && m.stage == 2 {
                            adv.landed += 1;
                        }
                        if m.node.is_none() {
                            let attack = agent.attack.ok_or("Lunge missing attack notify")?;
                            let rule = &assets.melee.clips[attack];
                            if !agent.hit_emitted && old < rule.hit_time && m.frame >= rule.hit_time
                            {
                                agent.hit_emitted = true;
                                let at = w.pawn.body.position;
                                let length = crate::distance(at, target);
                                let facing = ((target[0] - at[0]) * w.yaw.cos()
                                    + (target[1] - at[1]) * w.yaw.sin())
                                    / length.max(0.001);
                                if seen && length <= assets.melee.melee_range && facing > 0.5 {
                                    let amount = if adv.target_scorch {
                                        let s = self.state.squad.as_mut().unwrap();
                                        s.quiet_for = 0.;
                                        health_hit(
                                            &mut s.health,
                                            &mut s.shields,
                                            &assets.melee,
                                            &self.tactics.as_ref().unwrap().assets.squad.defense,
                                            time as f32,
                                        )
                                    } else {
                                        health_hit(
                                            &mut self.state.health,
                                            &mut 0.,
                                            &assets.melee,
                                            &assets.melee.player,
                                            time as f32,
                                        )
                                    };
                                    agent.hits += 1;
                                    state.damage.push(crate::gunner_runtime::Damage {
                                        tick: self.state.statistics.ticks,
                                        victim: if adv.target_scorch { -62 } else { -1 },
                                        amount,
                                        position: target,
                                    });
                                } else {
                                    agent.misses += 1;
                                }
                            }
                        }
                        if done || blocked {
                            if blocked {
                                adv.blocked += 1;
                                w.pawn.body.grounded = false;
                            }
                            adv.motion = None;
                            agent.attack = None;
                            agent.navigation.goal = None;
                            agent.navigation.next_plan = 0.;
                            w.frame = 0.;
                            w.clip = "ActionBreathe".into();
                        }
                        continue;
                    }
                    if agent.attack.is_none() && w.pawn.body.grounded {
                        if let Some(known) = known.filter(|_| time >= agent.navigation.next_plan) {
                            let supported = motion::ground(
                                &self.world,
                                [known[0], known[1], known[2] + 160.],
                                EXTENT,
                                400.,
                            )?;
                            agent.navigation.route = if let Some(at) = supported {
                                assets.navigation.plan_for(&self.world, from, at, EXTENT)?
                            } else {
                                vec![]
                            };
                            agent.navigation.waypoint = 0;
                            agent.navigation.target = known;
                            agent.navigation.plans += 1;
                            if agent.navigation.route.len() > 1 {
                                agent.navigation.detours += 1;
                            }
                            if agent.navigation.route.is_empty() {
                                agent.navigation.blocked += 1;
                            }
                            agent.navigation.next_plan = time
                                + if agent.navigation.route.is_empty() {
                                    2.
                                } else {
                                    1.
                                };
                        }
                        if adv.defensive_pending && agent.navigation.goal.is_none() {
                            adv.defensive_pending = false;
                            if let Some(m) = jump_choice(self, from, target, &reserved, true)? {
                                adv.motion = Some(m);
                                adv.jumps += 1;
                                adv.defensive_jumps += 1;
                                w.frame = 0.;
                                w.clip = "TakeOff".into();
                                continue;
                            }
                        }
                        if let Some(known) = known {
                            w.yaw = (known[1] - from[1]).atan2(known[0] - from[0]);
                        }
                        if seen && agent.navigation.goal.is_none() {
                            let clip = 2 + (agent.serial % 2) as usize;
                            let lunge = if d - self.profile.physics.extent[0]
                                <= assets.navigation.lunge_distance as f64
                            {
                                motion::lunge(
                                    &self.world,
                                    mesh,
                                    from,
                                    w.yaw,
                                    &assets.melee.clips[clip].name,
                                )?
                            } else {
                                None
                            };
                            let jump = if d - self.profile.physics.extent[0]
                                > assets.navigation.lunge_distance as f64
                                && d < assets.navigation.jump_distance as f64
                            {
                                jump_choice(self, from, target, &reserved, false)?
                            } else {
                                None
                            };
                            let mask = ai::capabilities(
                                (d - self.profile.physics.extent[0]) as f32,
                                ai::Parameters {
                                    lunge_distance: assets.navigation.lunge_distance,
                                    jump_distance: assets.navigation.jump_distance,
                                    offensive_jump_probability: assets
                                        .navigation
                                        .offensive_jump_probability,
                                },
                                &mut Host(
                                    !agent.navigation.route.is_empty()
                                        || d <= assets.melee.melee_range,
                                    jump.is_some(),
                                    lunge.is_some(),
                                ),
                            );
                            match ai::choose(
                                mask,
                                assets.navigation.offensive_jump_probability,
                                || adv.draw(),
                            ) {
                                ai::Maneuver::Lunge => {
                                    adv.motion = lunge;
                                    adv.lunges += 1;
                                    agent.attack = Some(clip);
                                    agent.serial += 1;
                                    agent.hit_emitted = false;
                                    agent.attacks += 1;
                                    self.state.statistics.enemy_shots += 1;
                                    w.frame = 0.;
                                    w.clip = assets.melee.clips[clip].name.clone();
                                    continue;
                                }
                                ai::Maneuver::Jump => {
                                    adv.motion = jump;
                                    adv.jumps += 1;
                                    w.frame = 0.;
                                    w.clip = "TakeOff".into();
                                    continue;
                                }
                                ai::Maneuver::Melee => {
                                    agent.navigation.goal =
                                        Some(crate::navigation::Goal::original(
                                            time as f32,
                                            assets.burst_count,
                                        ));
                                    let j = &self.tactics.as_ref().unwrap().assets.jump;
                                    adv.defensive_pending = ai::request_defensive_jump(
                                        !adv.target_scorch,
                                        j.probability,
                                        j.player_probability,
                                        || adv.draw(),
                                    );
                                }
                                _ => {}
                            }
                        }
                    }
                    if let Some(goal) = &mut agent.navigation.goal {
                        let facing = ((target[0] - from[0]) * w.yaw.cos()
                            + (target[1] - from[1]) * w.yaw.sin())
                            / d.max(0.001);
                        let (start, finish) = goal.pre_update(
                            time,
                            d,
                            assets.melee.melee_range,
                            facing,
                            agent.attack.is_some(),
                            seen && !agent.navigation.route.is_empty(),
                        );
                        if start && seen {
                            let clip = (agent.serial % 2) as usize;
                            agent.serial += 1;
                            agent.attack = Some(clip);
                            agent.hit_emitted = false;
                            agent.attacks += 1;
                            self.state.statistics.enemy_shots += 1;
                            w.clip = assets.melee.clips[clip].name.clone();
                            w.frame = 0.;
                        }
                        if finish {
                            agent.navigation.goal = None;
                        }
                    }
                    if let Some(attack) = agent.attack {
                        let rule = &assets.melee.clips[attack];
                        let seq = &mesh.mesh.clips[mesh.clip(&rule.name)?].sequence;
                        let next = w.frame
                            + f32::from_bits(seq.rate_bits) * STEP as f32 / seq.frames as f32;
                        if !agent.hit_emitted && w.frame < rule.hit_time && next >= rule.hit_time {
                            agent.hit_emitted = true;
                            let facing = ((target[0] - from[0]) * w.yaw.cos()
                                + (target[1] - from[1]) * w.yaw.sin())
                                / d.max(0.001);
                            if seen && d <= assets.melee.melee_range && facing > 0.5 {
                                let amount = if adv.target_scorch {
                                    let s = self.state.squad.as_mut().unwrap();
                                    s.quiet_for = 0.;
                                    health_hit(
                                        &mut s.health,
                                        &mut s.shields,
                                        &assets.melee,
                                        &self.tactics.as_ref().unwrap().assets.squad.defense,
                                        time as f32,
                                    )
                                } else {
                                    health_hit(
                                        &mut self.state.health,
                                        &mut 0.,
                                        &assets.melee,
                                        &assets.melee.player,
                                        time as f32,
                                    )
                                };
                                agent.hits += 1;
                                state.damage.push(crate::gunner_runtime::Damage {
                                    tick: self.state.statistics.ticks,
                                    victim: if adv.target_scorch { -62 } else { -1 },
                                    amount,
                                    position: target,
                                });
                                state.effects.push((
                                    self.state.statistics.ticks,
                                    "WarriorMeleeNotify".into(),
                                ));
                            } else {
                                agent.misses += 1;
                            }
                        }
                        w.frame = next;
                        if next >= 1. - 1. / seq.frames as f32 {
                            agent.attack = None;
                            w.frame = 0.;
                            w.clip = "ActionBreathe".into();
                        }
                        continue;
                    }
                    w.clip = "ActionBreathe".into();
                    if known
                        .is_some_and(|k| crate::distance(from, k) > assets.melee.melee_range * 0.75)
                        && w.pawn.body.grounded
                    {
                        while agent.navigation.waypoint < agent.navigation.route.len()
                            && crate::distance(
                                from,
                                agent.navigation.route[agent.navigation.waypoint],
                            ) < 8.
                        {
                            agent.navigation.waypoint += 1;
                        }
                        if let Some(to) = agent.navigation.route.get(agent.navigation.waypoint) {
                            let length = crate::distance(from, *to);
                            let step = (self.artillery.as_ref().unwrap().assets.warrior_speed
                                * STEP)
                                .min(length);
                            let end = std::array::from_fn(|j| {
                                from[j] + (to[j] - from[j]) * step / length
                            });
                            if crate::navigation::walkable_for(&self.world, from, end, EXTENT)? {
                                w.pawn.body.position = end;
                                w.yaw = (to[1] - from[1]).atan2(to[0] - from[0]);
                                w.clip = "RunForward".into();
                            } else {
                                agent.navigation.route.clear();
                                agent.navigation.blocked += 1;
                                agent.navigation.next_plan = time + 2.;
                            }
                        }
                    }
                }
                if old_clip != w.clip {
                    w.frame = 0.;
                }
                let seq = &mesh.mesh.clips[mesh.clip(&w.clip)?].sequence;
                let next =
                    w.frame + f32::from_bits(seq.rate_bits) * STEP as f32 / seq.frames as f32;
                w.frame = if w.health == 0 || w.clip == "hit" {
                    next.min(1. - 1. / seq.frames as f32)
                } else {
                    next.rem_euclid(1.)
                };
            }
            Ok(())
        })();
        self.state.artillery = Some(state);
        result
    }
}
