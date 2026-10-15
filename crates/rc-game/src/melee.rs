//! Selected ForcePike integration. Native damage arithmetic and notify times;
//! optional original difficulty/neutral-volume reduction; steering and turning remain adapters.
use crate::{distance, visual, Game, STEP};
use rc_package::{
    damage_variance::{Binding, Include, Variance},
    pawn_damage,
};
use serde::{Deserialize, Serialize};

#[derive(Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct DamageRule {
    pub class: Option<u32>,
    pub multiplier: f32,
}
#[derive(Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Defense {
    pub armor: f32,
    pub include: Vec<DamageRule>,
    pub exclude: Vec<Option<u32>>,
}
#[derive(Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct AttackClip {
    pub name: String,
    pub hit_time: f32,
    pub notify: i32,
}
#[derive(Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Rules {
    pub provenance: serde_json::Value,
    pub player_health: f32,
    pub player_shields: f32,
    pub shield_delay: f32,
    pub shield_rate: f32,
    pub player: Defense,
    pub enemy: Defense,
    pub projectile_type: Vec<u32>,
    pub projectile_bypass: bool,
    pub melee_type: Vec<u32>,
    pub melee_bypass: bool,
    pub melee_damage: f32,
    pub melee_range: f64,
    pub clips: Vec<AttackClip>,
}
#[derive(Debug, Clone, Serialize, serde::Deserialize)]
pub struct Attack {
    pub active: Option<usize>,
    pub serial: u64,
    pub yaw: f64,
}
#[derive(Debug, Clone, Serialize, serde::Deserialize)]
pub struct State {
    pub player: pawn_damage::State,
    pub enemies: Vec<pawn_damage::State>,
    pub attacks: Vec<Attack>,
}
pub fn display_health(value: f32) -> u32 {
    value.max(0.).ceil() as u32
}
impl Rules {
    pub fn validate(&self) -> Result<(), String> {
        let id = |id: u32| (1..=65535).contains(&id);
        for d in [&self.player, &self.enemy] {
            if !d.armor.is_finite()
                || !(0. ..=1.).contains(&d.armor)
                || d.include.len() > 128
                || d.exclude.len() > 128
                || d.include.iter().any(|r| {
                    !r.multiplier.is_finite()
                        || !(0. ..=100.).contains(&r.multiplier)
                        || r.class.is_some_and(|c| !id(c))
                })
                || d.exclude.iter().flatten().any(|&c| !id(c))
            {
                return Err("Invalid melee defense".into());
            }
        }
        if [
            self.player_health,
            self.player_shields,
            self.shield_delay,
            self.shield_rate,
            self.melee_damage,
        ]
        .iter()
        .any(|v| !v.is_finite() || !(0. ..=10000.).contains(v))
            || self.player_health == 0.
            || self.melee_damage == 0.
            || !self.melee_range.is_finite()
            || !(1. ..=1000.).contains(&self.melee_range)
            || [&self.projectile_type, &self.melee_type]
                .iter()
                .any(|a| a.is_empty() || a.len() > 64 || a.iter().any(|&c| !id(c)))
            || self.clips.len() != 4
        {
            return Err("Invalid melee rules".into());
        }
        for (clip, name) in
            self.clips
                .iter()
                .zip(["MeleeAttack", "MeleeAttack2", "Lunge", "Lunge2"])
        {
            if clip.name != name
                || !clip.hit_time.is_finite()
                || !(0. ..1.).contains(&clip.hit_time)
                || clip.notify <= 0
            {
                return Err("Invalid melee clip".into());
            }
        }
        Ok(())
    }
    pub fn validate_clips(&self, mesh: &visual::PreparedMesh) -> Result<(), String> {
        for clip in &self.clips {
            let seq = &mesh.mesh.clips[mesh.clip(&clip.name)?].sequence;
            if clip.hit_time > 1. - 1. / seq.frames as f32
                || !seq.notifies.iter().any(|n| {
                    n.object_index == clip.notify && n.time_bits == clip.hit_time.to_bits()
                })
            {
                return Err("Melee hit does not match original clip notify".into());
            }
        }
        Ok(())
    }
    pub fn state(&self, count: usize, enemy_health: u32) -> State {
        State {
            player: pawn_damage::State {
                health: self.player_health,
                shields: self.player_shields,
                next_shield_recharge: 0.,
            },
            enemies: vec![
                pawn_damage::State {
                    health: enemy_health as f32,
                    shields: 0.,
                    next_shield_recharge: 0.
                };
                count
            ],
            attacks: vec![
                Attack {
                    active: None,
                    serial: 0,
                    yaw: 0.
                };
                count
            ],
        }
    }
}
pub fn damage(
    state: &mut pawn_damage::State,
    defense: &Defense,
    ancestry: &[u32],
    hit: pawn_damage::Hit<'_>,
    recharge_delay: f32,
    context: (f32, bool),
) {
    let include: Vec<_> = defense
        .include
        .iter()
        .map(|r| Include {
            class: r.class,
            multiplier: r.multiplier,
        })
        .collect();
    let variance = Variance {
        include: &include,
        exclude: &defense.exclude,
    };
    let rules = pawn_damage::Rules {
        authority: true,
        armor: defense.armor,
        bone_multipliers: &[],
        variance: Binding::Valid(&variance),
        shield_variance: None,
        shield_recharge_delay: recharge_delay,
    };
    // CalculateDamageFrom override precedes GameInfo.ReduceDamage.
    pawn_damage::apply(
        state,
        &rules,
        &pawn_damage::Hit {
            damage_type: Some(ancestry),
            ..hit
        },
        |d| crate::game_rules::reduce(d, context.0, context.1, false, |_, damage| damage),
    );
}
impl Game {
    pub(crate) fn melee_enemy_tick(&mut self) -> Result<(), String> {
        if self.combat.as_ref().is_some_and(|a| a.navigation.is_some()) {
            return self.navigation_enemy_tick();
        }
        let rules = self
            .combat
            .as_ref()
            .and_then(|a| a.melee.as_ref())
            .ok_or("Missing melee rules")?;
        let time = self.state.statistics.ticks as f32 * STEP as f32;
        let player = self.state.pawn.body.position;
        {
            let state = self
                .state
                .combat
                .as_mut()
                .unwrap()
                .melee
                .as_mut()
                .ok_or("Missing melee state")?;
            if time > state.player.next_shield_recharge
                && state.player.shields < rules.player_shields
                && state.player.health > 0.
            {
                state.player.shields = (state.player.shields + rules.shield_rate * STEP as f32)
                    .min(rules.player_shields);
            }
        }
        for i in 0..self.state.enemies.len() {
            let mut e = self.state.enemies[i].clone();
            e.flash = (e.flash - STEP).max(0.);
            e.cooldown = (e.cooldown - STEP).max(0.);
            let state = self.state.combat.as_ref().unwrap().melee.as_ref().unwrap();
            let mut attack = state.attacks[i].clone();
            if e.health == 0 || e.flash > 0. {
                attack.active = None;
            }
            if e.health > 0 && attack.active.is_none() && e.flash == 0. {
                let d = distance(e.position, player);
                let visible = d < 1600. && self.wall_fraction(e.position, player)? >= 1.;
                if visible {
                    attack.yaw = (player[1] - e.position[1]).atan2(player[0] - e.position[0]);
                    if d > rules.melee_range * 0.9 {
                        let length = (player[0] - e.position[0]).hypot(player[1] - e.position[1]);
                        if length > 1. {
                            let end = [
                                e.position[0] + (player[0] - e.position[0]) / length * 65. * STEP,
                                e.position[1] + (player[1] - e.position[1]) / length * 65. * STEP,
                                e.position[2],
                            ];
                            let sweep =
                                self.world.sweep_motion(e.position, end, [22., 22., 60.])?;
                            if !sweep.complete {
                                return Err("Incomplete melee approach".into());
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
                    } else if e.cooldown == 0. {
                        // Only the two stationary melee clips are activated. Native lunges
                        // and jumps require unported navigation/root-motion goals.
                        attack.active = Some((attack.serial % 2) as usize);
                        attack.serial += 1;
                        e.cooldown = 2.;
                        self.state.statistics.enemy_shots += 1;
                    }
                }
            }
            self.state
                .combat
                .as_mut()
                .unwrap()
                .melee
                .as_mut()
                .unwrap()
                .attacks[i] = attack;
            self.state.enemies[i] = e;
        }
        Ok(())
    }
    pub(crate) fn melee_notify_tick(&mut self, previous: &visual::State) -> Result<(), String> {
        let Some(rules) = self.combat.as_ref().and_then(|a| a.melee.as_ref()) else {
            return Ok(());
        };
        let assets = self.visuals.as_ref().ok_or("Missing melee visuals")?;
        let visual = self
            .state
            .visuals
            .as_ref()
            .ok_or("Missing melee visual state")?;
        let state = self
            .state
            .combat
            .as_ref()
            .unwrap()
            .melee
            .as_ref()
            .ok_or("Missing melee state")?;
        let mut hits = vec![];
        let mut finished = vec![];
        for (i, attack) in state.attacks.iter().enumerate() {
            if let Some(index) = attack.active {
                let clip = &rules.clips[index];
                let now = &visual.enemies[i];
                let old = &previous.enemies[i];
                if assets.enemy.mesh.clips[now.clip].sequence.name.name != clip.name {
                    continue;
                }
                let from = if old.clip == now.clip && old.started {
                    old.frame
                } else {
                    -f32::EPSILON
                };
                if visual::crossed_notify(from, now.frame, clip.hit_time) {
                    hits.push(i);
                }
                if now.finished {
                    finished.push(i);
                }
            }
        }
        let player = self.state.pawn.body.position;
        for i in hits {
            let e = &self.state.enemies[i];
            let yaw = self
                .state
                .combat
                .as_ref()
                .unwrap()
                .melee
                .as_ref()
                .unwrap()
                .attacks[i]
                .yaw;
            let delta = [
                player[0] - e.position[0],
                player[1] - e.position[1],
                player[2] - e.position[2],
            ];
            let horizontal = delta[0].hypot(delta[1]);
            let dot = if horizontal > 0. {
                (delta[0] * yaw.cos() + delta[1] * yaw.sin()) / horizontal
            } else {
                0.
            };
            if e.health > 0
                && distance(e.position, player) <= rules.melee_range
                && delta[2].abs() <= self.profile.physics.extent[2].max(60.) * 1.5
                && dot > 0.8
                && self.wall_fraction(e.position, player)? >= 1.
            {
                let context = self.damage_context(player, true)?;
                let state = self.state.combat.as_mut().unwrap().melee.as_mut().unwrap();
                let before = state.player;
                let time = self.state.statistics.ticks as f32 * STEP as f32;
                damage(
                    &mut state.player,
                    &rules.player,
                    &rules.melee_type,
                    pawn_damage::Hit {
                        damage: rules.melee_damage,
                        bone: "",
                        damage_type: None,
                        bypass_shields: rules.melee_bypass,
                        time,
                    },
                    rules.shield_delay,
                    context,
                );
                if before.health != state.player.health || before.shields != state.player.shields {
                    self.state.damage_flash = 0.2;
                }
                self.state.health = display_health(state.player.health);
            }
        }
        let state = self.state.combat.as_mut().unwrap().melee.as_mut().unwrap();
        for i in finished {
            state.attacks[i].active = None;
        }
        Ok(())
    }
}
