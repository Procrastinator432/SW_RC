//! Scene integration of the selected original crash-site flow. Weapon damage and
//! enemy shooting/approach still use the explicitly provisional encounter host.
use crate::{crash, dynamic, Enemy, Game, Phase, Point, STEP};
use serde::{Deserialize, Serialize};
#[derive(Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Assets {
    pub flow: crash::Assets,
    pub objects: Vec<dynamic::Object>,
    pub route: Vec<Point>,
    pub spawns: Vec<Point>,
}
#[derive(Clone, Debug, Serialize, serde::Deserialize)]
pub struct State {
    pub flow: crash::State,
    pub waypoint: usize,
    pub tracker: String,
}
impl Assets {
    pub fn validate(&self) -> Result<(), String> {
        self.flow.validate()?;
        let ids: std::collections::BTreeSet<_> = self.objects.iter().map(|o| o.original).collect();
        let required = [
            self.flow.bacta_barrier,
            self.flow.path_barrier,
            self.flow.exit_barrier,
        ]
        .into_iter()
        .collect();
        if ids != required
            || self.objects.len() != 3
            || self.route.is_empty()
            || self.route.len() > 256
            || self.spawns.len() != 5
            || self
                .route
                .iter()
                .chain(&self.spawns)
                .flatten()
                .any(|v| !v.is_finite() || v.abs() > 1e8)
        {
            return Err("Invalid crash-site scene integration".into());
        }
        for (spawn, factory) in self.spawns.iter().zip(&self.flow.factories) {
            if (spawn[0] - factory.position[0]).abs() > 1.
                || (spawn[1] - factory.position[1]).abs() > 1.
                || (spawn[2] - factory.position[2]).abs() > 512.
            {
                return Err("Factory spawn moved outside original anchor".into());
            }
        }
        Ok(())
    }
}
impl Game {
    pub fn crash_target(&self) -> Option<Point> {
        let m = self.state.mission.as_ref()?;
        if !m.completed {
            return None;
        }
        let a = self.mission.as_ref()?.crash.as_ref()?;
        let s = m.crash.as_ref()?;
        a.route
            .get(s.waypoint)
            .copied()
            .or_else(|| a.route.last().copied())
    }
    pub fn crash_status(&self) -> Option<String> {
        let a = self.mission.as_ref()?.crash.as_ref()?;
        let m = self.state.mission.as_ref()?;
        if !m.completed {
            return None;
        }
        let s = m.crash.as_ref()?;
        if s.flow.combat == crash::CombatStage::Secured {
            return Some("Originalziele abgeschlossen: Bacta und Absturzstelle gesichert".into());
        }
        let p = self.state.pawn.body.position;
        let target = self.crash_target()?;
        let turn = ((target[1] - p[1]).atan2(target[0] - p[0]) - self.state.yaw
            + std::f64::consts::PI)
            .rem_euclid(std::f64::consts::TAU)
            - std::f64::consts::PI;
        Some(format!(
            "Absturzstelle sichern · Droiden {} · Wegpunkt {}/{} · {:.0} m · {} {:.0}°{}",
            s.flow.counter_remaining,
            s.waypoint + 1,
            a.route.len(),
            crate::distance(p, target) / 100.,
            if turn >= 0. { "links" } else { "rechts" },
            turn.abs().to_degrees(),
            if s.flow.spawned && !s.flow.vulnerable {
                " · weiter vorrücken"
            } else {
                ""
            }
        ))
    }
    pub(crate) fn crash_tick(&mut self) -> Result<(), String> {
        let Some(a) = self.mission.as_ref().and_then(|m| m.crash.as_ref()) else {
            return Ok(());
        };
        let m = self.state.mission.as_mut().ok_or("Missing mission state")?;
        let s = m.crash.as_mut().ok_or("Missing crash-site state")?;
        let mut effects = if m.completed {
            s.flow.bacta_used(&a.flow)
        } else {
            vec![]
        };
        effects.extend(s.flow.advance(
            &a.flow,
            STEP,
            self.state.pawn.body.position,
            self.profile.physics.extent,
        )?);
        for (enemy, factory) in self.state.enemies.iter().zip(&a.flow.factories) {
            if enemy.health == 0 {
                effects.extend(s.flow.pawn_died(&a.flow, factory.original)?);
            }
        }
        if m.completed {
            while s.waypoint + 1 < a.route.len()
                && crate::distance(self.state.pawn.body.position, a.route[s.waypoint]) < 100.
            {
                s.waypoint += 1;
            }
        }
        for effect in effects {
            self.apply_crash_effect(effect)?;
        }
        Ok(())
    }
    fn apply_crash_effect(&mut self, effect: crash::Effect) -> Result<(), String> {
        match effect {
            crash::Effect::CollisionDisabled(id) => self.disable_dynamic(id)?,
            crash::Effect::HurtBarrier { original, .. } => self.disable_dynamic(original)?,
            crash::Effect::Spawn(id) => {
                let a = self.mission.as_ref().unwrap().crash.as_ref().unwrap();
                let i = a
                    .flow
                    .factories
                    .iter()
                    .position(|f| f.original == id)
                    .ok_or("Unknown factory")?;
                if self.state.enemies.len() != i {
                    return Err("Out-of-order factory spawn".into());
                }
                self.state.enemies.push(Enemy {
                    position: a.spawns[i],
                    health: a.flow.factories[i].health,
                    cooldown: 1.,
                    flash: 0.,
                });
                if let Some(v) = &self.visuals {
                    let mut new = v.state(self.state.enemies.len())?;
                    if let Some(old) = &self.state.visuals {
                        new.weapon = old.weapon.clone();
                        new.weapon_animation = old.weapon_animation.clone();
                    }
                    self.state.visuals = Some(new);
                }
            }
            crash::Effect::ObjectiveCompleted(1) => {
                if self.mission.as_ref().unwrap().grenades.is_none() {
                    self.state.phase = Phase::Won;
                }
            }
            crash::Effect::PlaySound(source) => {
                let a = self.mission.as_ref().unwrap();
                let sound = a
                    .sounds
                    .iter()
                    .position(|s| s.source == source)
                    .ok_or("Missing crash-site sound")?;
                let m = self.state.mission.as_mut().unwrap();
                m.audio_sequence += 1;
                m.events.push(crate::combat::Event {
                    sequence: m.audio_sequence,
                    tick: self.state.statistics.ticks,
                    sound,
                    owner: -1,
                });
            }
            crash::Effect::Tracker(tag) => {
                self.state
                    .mission
                    .as_mut()
                    .unwrap()
                    .crash
                    .as_mut()
                    .unwrap()
                    .tracker = tag;
            }
            // Events are consumed by the selected flow itself. Trench-rim actors
            // and decorative retreat/explosion scripts are outside this host.
            crash::Effect::Event(tag) => self.queue_world_event(&tag),
            crash::Effect::Invulnerable { .. } | crash::Effect::ObjectiveCompleted(_) => {}
        }
        Ok(())
    }
    pub(crate) fn crash_invulnerable(&self) -> bool {
        self.state
            .mission
            .as_ref()
            .and_then(|m| m.crash.as_ref())
            .is_some_and(|s| s.flow.spawned && !s.flow.vulnerable)
    }
}
