//! Versioned port campaign receipts for original A→B→C and C→factory exits.
//! Scene replacement is transactional; this is not native TravelInfo parity.
use crate::{Game, Phase};
use serde::{Deserialize, Serialize};
#[derive(Clone, Debug, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Handoff {
    pub schema: u32,
    pub source: String,
    pub destination: String,
    pub trigger: u32,
    pub tick: u64,
    pub objectives: [u8; 7],
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub map2: Option<Map2Carry>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub map3: Option<Map3Carry>,
    pub player: serde_json::Value,
    pub squad: serde_json::Value,
}
#[derive(Clone, Debug, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Map2Carry {
    pub objectives: [bool; 8],
    pub fixer: serde_json::Value,
}
#[derive(Clone, Debug, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Map3Carry {
    pub objectives: [bool; 7],
    pub fixer: serde_json::Value,
    pub sev: serde_json::Value,
}
impl Game {
    /// Repeatable immutable receipt for the completed original campaign exit.
    pub fn campaign_handoff(&self) -> Result<Option<Handoff>, String> {
        if let Some(h) = self.state.spire.as_ref().and_then(|s| s.handoff.as_ref()) {
            return Ok(Some(h.clone()));
        }
        let Some(m) = self.state.map.as_ref() else {
            return Ok(None);
        };
        if self.state.phase == Phase::Won
            && self
                .elevator_state()
                .is_some_and(|s| s.completed_tick.is_some())
        {
            let o = m.opening.as_ref().ok_or("Missing third-map objectives")?;
            if self.state.health == 0
                || !o.objectives.iter().all(|v| *v)
                || !o.events.contains("ConditionSunFacDead")
            {
                return Err("Incomplete third-map exit prerequisites".into());
            }
            return Ok(Some(Handoff {
                schema: 3,
                source: "GEO_01C.ctm".into(),
                destination: "GEO_03A.ctm#playerstart".into(),
                trigger: 39,
                tick: self.state.statistics.ticks,
                objectives: [0, 1, 2, 3, 4, 5, 6],
                map2: None,
                map3: Some(Map3Carry {
                    objectives: o.objectives,
                    fixer: serde_json::to_value(&m.fixer).map_err(|e| e.to_string())?,
                    sev: serde_json::to_value(&o.sev).map_err(|e| e.to_string())?,
                }),
                player: self.dc15_player(serde_json::json!({"health":self.state.health,"ammo":self.state.ammo,"reserve":self.state.reserve,
                    "grenades":{"flow":{"ammunition":m.grenades}},"position":self.state.pawn.body.position,"yaw":self.state.yaw,"pitch":self.state.pitch})),
                squad: serde_json::to_value(&m.companion).map_err(|e| e.to_string())?,
            }));
        }
        let Some(g) = m.gameplay.as_ref() else {
            return Ok(None);
        };
        if self.state.phase != Phase::Won
            || g.transition.as_deref() != Some("GEO_01C.ctm#playerstart")
        {
            return Ok(None);
        }
        if self.state.health == 0 || !g.objectives.iter().all(|x| *x) {
            return Err("Incomplete second-map exit prerequisites".into());
        }
        Ok(Some(Handoff {
            schema: 2,
            source: "GEO_01B.ctm".into(),
            destination: "GEO_01C.ctm#playerstart".into(),
            trigger: 567,
            tick: self.state.statistics.ticks,
            objectives: [0, 1, 2, 3, 4, 5, 6],
            map3: None,
            map2: Some(Map2Carry {
                objectives: g.objectives,
                fixer: serde_json::to_value(&g.fixer).map_err(|e| e.to_string())?,
            }),
            player: self.dc15_player(serde_json::json!({"health":self.state.health,"ammo":self.state.ammo,"reserve":self.state.reserve,"grenades":{"flow":{"ammunition":m.grenades}},"position":self.state.pawn.body.position,"yaw":self.state.yaw,"pitch":self.state.pitch})),
            squad: serde_json::to_value(&m.companion).map_err(|e| e.to_string())?,
        }))
    }

    /// Build the destination completely before the caller replaces the session.
    pub fn travel_to(&self, bytes: &[u8]) -> Result<Game, String> {
        if self.state.phase != Phase::Won {
            return Err("Map transition requires the completed original exit".into());
        }
        let handoff = self
            .campaign_handoff()?
            .ok_or("No campaign handoff is ready")?;
        let mut next = crate::bundle::decode(bytes)?;
        next.restore_handoff(&handoff)?;
        Ok(next)
    }
    pub(crate) fn capture_map_handoff(&mut self, url: &str) -> Result<(), String> {
        if !self
            .opening
            .as_ref()
            .is_some_and(|o| o.assets.stop_at_transition)
        {
            return Ok(());
        }
        let spire = self.state.spire.as_ref().ok_or("Missing exit state")?;
        if spire.handoff.is_some() {
            return Ok(());
        }
        let m = self.state.mission.as_ref().ok_or("Missing mission state")?;
        if url != "GEO_01B.ctm#playerstart"
            || self.state.phase != Phase::Won
            || self.state.health == 0
            || !m.completed
            || !spire.flow.completed
            || !spire.flow.barrier_open
            || !spire.flow.door_open
            || self
                .state
                .demolition
                .as_ref()
                .is_none_or(|d| d.complete_tick.is_none() || d.explosion_tick.is_none())
            || self
                .state
                .rendezvous
                .as_ref()
                .is_none_or(|r| !r.flow.objective_completed)
            || self
                .state
                .thermal
                .as_ref()
                .is_none_or(|t| !t.gun.objective_completed)
            || m.crash
                .as_ref()
                .is_none_or(|c| c.flow.counter_remaining != 0)
        {
            return Err("Mission prerequisites missing at map transition".into());
        }
        let handoff = Handoff {
            map3: None,
            map2: None,
            schema: 1,
            source: "GEO_01A.ctm".into(),
            destination: url.into(),
            trigger: 9,
            tick: self.state.statistics.ticks,
            objectives: [0, 1, 2, 3, 4, 5, 6],
            player: self.dc15_player(serde_json::json!({"health":self.state.health,"ammo":self.state.ammo,"reserve":self.state.reserve,
                "grenades":m.grenades,"combat":self.state.combat,"position":self.state.pawn.body.position,
                "yaw":self.state.yaw,"pitch":self.state.pitch})),
            squad: serde_json::to_value(&self.state.squad).map_err(|e| e.to_string())?,
        };
        self.state.spire.as_mut().unwrap().handoff = Some(handoff);
        Ok(())
    }
}

/// Exact supported campaign links; a saved receipt cannot skip a map.
pub(crate) fn valid_route(h: &Handoff) -> bool {
    h.objectives == [0, 1, 2, 3, 4, 5, 6]
        && match (
            h.schema,
            h.source.as_str(),
            h.destination.as_str(),
            h.trigger,
        ) {
            (1, "GEO_01A.ctm", "GEO_01B.ctm#playerstart", 9) => {
                h.map2.is_none() && h.map3.is_none()
            }
            (2, "GEO_01B.ctm", "GEO_01C.ctm#playerstart", 567) => {
                h.map3.is_none()
                    && h.map2.as_ref().is_some_and(|m| {
                        m.objectives.iter().all(|v| *v) && m.fixer["joined"] == true
                    })
            }
            (3, "GEO_01C.ctm", "GEO_03A.ctm#playerstart", 39) => {
                h.map2.is_none()
                    && h.squad["joined"] == true
                    && h.map3.as_ref().is_some_and(|m| {
                        m.objectives.iter().all(|v| *v)
                            && m.fixer["joined"] == true
                            && m.sev["joined"] == true
                    })
            }
            _ => false,
        }
}

#[cfg(test)]
mod tests {
    use super::*;
    fn a() -> Handoff {
        Handoff {
            schema: 1,
            source: "GEO_01A.ctm".into(),
            destination: "GEO_01B.ctm#playerstart".into(),
            trigger: 9,
            tick: 1,
            objectives: [0, 1, 2, 3, 4, 5, 6],
            map3: None,
            map2: None,
            player: serde_json::Value::Null,
            squad: serde_json::Value::Null,
        }
    }
    fn b() -> Handoff {
        let mut h = a();
        h.schema = 2;
        h.source = "GEO_01B.ctm".into();
        h.destination = "GEO_01C.ctm#playerstart".into();
        h.trigger = 567;
        h.map2 = Some(Map2Carry {
            objectives: [true; 8],
            fixer: serde_json::json!({"joined":true}),
        });
        h
    }
    #[test]
    fn original_schema_remains_compatible() {
        let h = a();
        assert!(valid_route(&h));
        let v = serde_json::to_value(&h).unwrap();
        assert!(v.get("map2").is_none());
        assert!(valid_route(&serde_json::from_value(v).unwrap()));
    }
    #[test]
    fn original_receipt_cannot_skip_second_map() {
        let mut h = a();
        h.destination = "GEO_01C.ctm#playerstart".into();
        assert!(!valid_route(&h));
    }
    #[test]
    fn second_map_requires_all_eight_objectives() {
        let mut h = b();
        assert!(valid_route(&h));
        h.map2.as_mut().unwrap().objectives[7] = false;
        assert!(!valid_route(&h));
    }
    #[test]
    fn second_map_requires_fixer_and_exact_exit() {
        let mut h = b();
        h.map2.as_mut().unwrap().fixer = serde_json::Value::Null;
        assert!(!valid_route(&h));
        let mut h = b();
        h.trigger = 9;
        assert!(!valid_route(&h));
    }
    #[test]
    fn second_map_receipt_cannot_be_replayed_into_map_two() {
        let mut h = b();
        h.destination = "GEO_01B.ctm#playerstart".into();
        assert!(!valid_route(&h));
    }
    #[test]
    fn mixed_schema_payload_is_rejected() {
        let mut h = a();
        h.map2 = b().map2;
        assert!(!valid_route(&h));
        let mut h = b();
        h.map2 = None;
        assert!(!valid_route(&h));
    }
    #[test]
    fn third_map_requires_exact_exit_all_goals_and_three_members() {
        let mut h = b();
        h.schema = 3;
        h.source = "GEO_01C.ctm".into();
        h.destination = "GEO_03A.ctm#playerstart".into();
        h.trigger = 39;
        h.map2 = None;
        h.squad = serde_json::json!({"joined":true});
        h.map3 = Some(Map3Carry {
            objectives: [true; 7],
            fixer: serde_json::json!({"joined":true}),
            sev: serde_json::json!({"joined":true}),
        });
        assert!(valid_route(&h));
        h.map3.as_mut().unwrap().objectives[6] = false;
        assert!(!valid_route(&h));
        h.map3.as_mut().unwrap().objectives[6] = true;
        h.map3.as_mut().unwrap().sev = serde_json::Value::Null;
        assert!(!valid_route(&h));
        h.map3.as_mut().unwrap().sev = serde_json::json!({"joined":true});
        h.trigger = 567;
        assert!(!valid_route(&h));
        h.trigger = 39;
        h.map2 = b().map2;
        assert!(!valid_route(&h));
    }
}
