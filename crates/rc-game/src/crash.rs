//! Bounded geo_01a crash-site event host. This does not execute a general script VM,
//! spawn graphics/AI or change collision; the caller must apply the emitted effects.
use crate::Point;
use serde::{Deserialize, Serialize};

#[derive(Clone, Debug, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Trigger {
    pub original: u32,
    pub position: Point,
    pub radius: f64,
    pub height: f64,
}
impl Trigger {
    pub(crate) fn validate(&self) -> bool {
        self.original > 0
            && self.position.iter().all(|v| v.is_finite() && v.abs() < 1e8)
            && self.radius.is_finite()
            && (1. ..=10000.).contains(&self.radius)
            && self.height.is_finite()
            && (1. ..=10000.).contains(&self.height)
    }
    pub(crate) fn contains(&self, p: Point, extent: Point) -> bool {
        (p[0] - self.position[0]).hypot(p[1] - self.position[1])
            < self.radius + extent[0].max(extent[1])
            && (p[2] - self.position[2]).abs() < self.height + extent[2]
    }
}
#[derive(Clone, Debug, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Factory {
    pub original: u32,
    pub position: Point,
    pub rotation: [i32; 3],
    pub prototype: String,
    pub health: u32,
    pub capacity: u32,
}
#[derive(Clone, Debug, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Assets {
    pub provenance: serde_json::Value,
    pub spawn: Trigger,
    pub vulnerability: Trigger,
    pub factories: Vec<Factory>,
    pub counter: u32,
    pub counter_export: u32,
    pub bacta_barrier: u32,
    pub path_barrier: u32,
    pub path_barrier_health: u32,
    pub path_barrier_damage: u32,
    pub exit_barrier: u32,
    pub advisor_duration: f64,
    pub retreat_delay: f64,
}
impl Assets {
    pub fn validate(&self) -> Result<(), String> {
        if !self.spawn.validate()
            || !self.vulnerability.validate()
            || self.spawn.original == self.vulnerability.original
            || self.factories.len() != 5
            || self.counter != 5
            || self.counter_export == 0
            || self.bacta_barrier == 0
            || self.path_barrier == 0
            || self.exit_barrier == 0
            || self.path_barrier_health == 0
            || self.path_barrier_damage < self.path_barrier_health
            || !self.advisor_duration.is_finite()
            || !(0. ..=120.).contains(&self.advisor_duration)
            || !self.retreat_delay.is_finite()
            || !(0. ..=120.).contains(&self.retreat_delay)
        {
            return Err("Invalid crash-site flow assets".into());
        }
        let mut ids = std::collections::BTreeSet::new();
        for f in &self.factories {
            if f.original == 0
                || !ids.insert(f.original)
                || f.capacity != 1
                || f.prototype != "Properties.BattleDroidTM"
                || !(1..=10000).contains(&f.health)
                || f.position.iter().any(|v| !v.is_finite() || v.abs() > 1e8)
            {
                return Err("Unsupported crash-site factory".into());
            }
        }
        Ok(())
    }
}
#[derive(Clone, Debug, PartialEq, Serialize, serde::Deserialize)]
pub enum Effect {
    CollisionDisabled(u32),
    ObjectiveCompleted(u32),
    PlaySound(String),
    Event(String),
    HurtBarrier { original: u32, amount: u32 },
    Tracker(String),
    Spawn(u32),
    Invulnerable { tag: String, enabled: bool },
}
#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, serde::Deserialize)]
pub enum Stage {
    Bacta,
    AdvisorDelay,
    Advisor,
    Retreat,
    Approach,
}
#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, serde::Deserialize)]
pub enum CombatStage {
    Dormant,
    Protected,
    Vulnerable,
    Secured,
}
#[derive(Clone, Debug, PartialEq, Serialize, serde::Deserialize)]
pub struct State {
    pub stage: Stage,
    pub combat: CombatStage,
    pub remaining: f64,
    pub spawned: bool,
    pub vulnerable: bool,
    pub deaths: Vec<u32>,
    pub counter_remaining: u32,
    pub spawn_touching: bool,
    pub vulnerability_touching: bool,
}
impl State {
    pub fn new(a: &Assets) -> Result<Self, String> {
        a.validate()?;
        Ok(Self {
            stage: Stage::Bacta,
            combat: CombatStage::Dormant,
            remaining: 0.,
            spawned: false,
            vulnerable: false,
            deaths: vec![],
            counter_remaining: a.counter,
            spawn_touching: false,
            vulnerability_touching: false,
        })
    }
    /// Exactly one successful BactaUsed notification. Healing/holding is the caller's job.
    pub fn bacta_used(&mut self, a: &Assets) -> Vec<Effect> {
        if self.stage != Stage::Bacta {
            return vec![];
        }
        self.stage = Stage::AdvisorDelay;
        self.remaining = 1.;
        vec![
            Effect::Event("BactaUsed".into()),
            Effect::CollisionDisabled(a.bacta_barrier),
            Effect::ObjectiveCompleted(0),
        ]
    }
    /// Player-cylinder contact adapter. No position/health mutation. Events are edge guarded.
    pub fn advance(
        &mut self,
        a: &Assets,
        seconds: f64,
        position: Point,
        extent: Point,
    ) -> Result<Vec<Effect>, String> {
        if !seconds.is_finite()
            || !(0. ..=1.).contains(&seconds)
            || position.iter().any(|v| !v.is_finite() || v.abs() > 1e8)
            || extent
                .iter()
                .any(|v| !v.is_finite() || !(0. ..=10000.).contains(v))
        {
            return Err("Invalid crash-site advance".into());
        }
        let mut effects = vec![];
        let spawn_touching = a.spawn.contains(position, extent);
        let vulnerability_touching = a.vulnerability.contains(position, extent);
        let spawn_entered = spawn_touching && !self.spawn_touching;
        let vulnerability_entered = vulnerability_touching && !self.vulnerability_touching;
        self.spawn_touching = spawn_touching;
        self.vulnerability_touching = vulnerability_touching;
        let mut delta = seconds;
        while matches!(
            self.stage,
            Stage::AdvisorDelay | Stage::Advisor | Stage::Retreat
        ) {
            if delta + 1e-9 < self.remaining {
                self.remaining -= delta;
                break;
            }
            delta = (delta - self.remaining).max(0.);
            match self.stage {
                Stage::AdvisorDelay => {
                    self.stage = Stage::Advisor;
                    self.remaining = a.advisor_duration;
                    effects.push(Effect::PlaySound("GEO_Voice.GEO_01.CA1G1791".into()));
                }
                Stage::Advisor => {
                    self.stage = Stage::Retreat;
                    self.remaining = a.retreat_delay;
                    effects.extend([
                        Effect::Event("RetreatToGammaSector".into()),
                        Effect::Invulnerable {
                            tag: "TrenchRim01Droids".into(),
                            enabled: false,
                        },
                        Effect::Event("PullBackToGammaSectorExplosion".into()),
                        Effect::PlaySound("GEO_Voice.GEO_01.TPRG1003".into()),
                    ]);
                }
                Stage::Retreat => {
                    self.stage = Stage::Approach;
                    self.remaining = 0.;
                    effects.extend([
                        Effect::Event("TriggerBlownUpGuy".into()),
                        Effect::HurtBarrier {
                            original: a.path_barrier,
                            amount: a.path_barrier_damage,
                        },
                        Effect::Tracker("TrackerTarget01".into()),
                    ]);
                }
                _ => unreachable!(),
            }
        }
        // Script129/factories listen independently of Script739. The physical
        // barricade controls access; there is no invented Bacta prerequisite here.
        if self.combat != CombatStage::Secured {
            if !self.spawned && spawn_entered {
                self.spawned = true;
                self.combat = CombatStage::Protected;
                effects.push(Effect::Event("SpawnGunshipArea".into()));
                effects.extend(a.factories.iter().map(|f| Effect::Spawn(f.original)));
                effects.push(Effect::Invulnerable {
                    tag: "GunshipDroids".into(),
                    enabled: true,
                });
            }
            // A pre-spawn touch is deliberately not queued: ACTION_WaitForEvent is transient.
            if self.spawned && !self.vulnerable && vulnerability_entered {
                self.vulnerable = true;
                self.combat = CombatStage::Vulnerable;
                effects.extend([
                    Effect::Event("DialogThereIsTooMany".into()),
                    Effect::Invulnerable {
                        tag: "GunshipDroids".into(),
                        enabled: false,
                    },
                ]);
            }
        }
        Ok(effects)
    }
    /// Called only after a live spawned pawn dies. Stable factory identity suppresses duplicates.
    /// Damage/death detection and combat AI are outside this selected event host.
    pub fn pawn_died(&mut self, a: &Assets, factory: u32) -> Result<Vec<Effect>, String> {
        if !a.factories.iter().any(|f| f.original == factory) {
            return Err("Death from unknown crash-site factory".into());
        }
        if !self.spawned
            || !self.vulnerable
            || self.deaths.contains(&factory)
            || self.combat == CombatStage::Secured
        {
            return Ok(vec![]);
        }
        self.deaths.push(factory);
        self.counter_remaining -= 1;
        let mut effects = vec![Effect::Event("DeadGunshipDroid".into())];
        if self.counter_remaining == 0 {
            self.combat = CombatStage::Secured;
            effects.extend([
                Effect::Event("CrashSiteSecured".into()),
                Effect::ObjectiveCompleted(1),
                Effect::Tracker("TrackerTarget04".into()),
                Effect::CollisionDisabled(a.exit_barrier),
            ]);
        }
        Ok(effects)
    }
}
