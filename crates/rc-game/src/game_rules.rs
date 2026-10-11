//! Selected single-player damage boundary from PlayerCommando/GameInfo/GameRules.
//! No dynamic script rule chain or multiplayer GameDifficulty scaling.
use crate::{Game, Point};
use serde::{Deserialize, Serialize};
use std::collections::BTreeMap;

#[derive(Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Assets {
    pub provenance: serde_json::Value,
    pub modifiers: [f32; 3],
    pub selected: usize,
    pub neutral_volumes: BTreeMap<String, bool>,
}
impl Assets {
    pub fn validate(&self) -> Result<(), String> {
        if self.selected > 2
            || self
                .modifiers
                .iter()
                .any(|v| !v.is_finite() || *v <= 0. || *v > 100.)
            || self.neutral_volumes.len() > 4096
            || !self.neutral_volumes.contains_key("DefaultPhysicsVolume")
            || self
                .neutral_volumes
                .keys()
                .any(|n| n.is_empty() || n.len() > 512)
        {
            return Err("Invalid single-player damage rules".into());
        }
        Ok(())
    }
}

/// PlayerCommando.CalculateDamageFrom multiplies the superclass result, then
/// Pawn.TakeDamage calls GameInfo.ReduceDamage. The neutral branch still invokes
/// NetDamage; the non-neutral god-mode branch returns before invoking it.
pub fn reduce(
    damage: f32,
    difficulty: f32,
    neutral: bool,
    god_mode: bool,
    net_damage: impl FnOnce(f32, f32) -> f32,
) -> f32 {
    let original = damage * difficulty;
    let reduced = if neutral {
        0.
    } else if god_mode {
        return 0.;
    } else {
        original
    };
    net_damage(original, reduced)
}

impl Game {
    /// Prepared actors have no god-mode controller. A base/absent GameRules
    /// chain forwards damage unchanged; arbitrary spawned rule subclasses are
    /// deliberately outside this scene profile.
    pub(crate) fn damage_context(&self, point: Point, player: bool) -> Result<(f32, bool), String> {
        let Some(rules) = self.combat.as_ref().and_then(|a| a.game_rules.as_ref()) else {
            return Ok((1., false));
        };
        let selection = self.volumes.select(point.map(|v| v as f32))?;
        if !selection.complete {
            return Err("Incomplete damage volume selection".into());
        }
        let neutral = *rules
            .neutral_volumes
            .get(&selection.object)
            .ok_or("Missing damage volume flag")?;
        Ok((
            if player {
                rules.modifiers[rules.selected]
            } else {
                1.
            },
            neutral,
        ))
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn difficulty_and_neutral_god_chain_order() {
        for (factor, expected) in [(0.25, 15.625), (1., 62.5), (1.75, 109.375)] {
            assert_eq!(
                reduce(62.5, factor, false, false, |original, damage| {
                    assert_eq!(original, expected);
                    damage
                }),
                expected
            );
        }
        assert_eq!(
            reduce(62.5, 1.75, true, true, |original, damage| {
                assert_eq!(original, 109.375);
                assert_eq!(damage, 0.);
                7.
            }),
            7.
        );
        assert_eq!(
            reduce(62.5, 1., false, true, |_, _| panic!(
                "god mode must bypass chain"
            )),
            0.
        );
    }
    #[test]
    fn malformed_profile_rejected() {
        let mut a = Assets {
            provenance: serde_json::Value::Null,
            modifiers: [0.25, 1., 1.75],
            selected: 1,
            neutral_volumes: [("DefaultPhysicsVolume".into(), false)].into(),
        };
        assert!(a.validate().is_ok());
        a.selected = 3;
        assert!(a.validate().is_err());
        a.selected = 1;
        a.modifiers[0] = f32::NAN;
        assert!(a.validate().is_err());
        a.modifiers[0] = 0.25;
        a.neutral_volumes.clear();
        assert!(a.validate().is_err());
    }
    #[test]
    fn difficulty_after_variance_preserves_float_rounding_and_zero_deadline() {
        use crate::melee::{damage, DamageRule, Defense};
        use rc_package::pawn_damage::{Hit, State};
        let defense = Defense {
            armor: 0.,
            include: vec![DamageRule {
                class: None,
                multiplier: 0.7,
            }],
            exclude: vec![],
        };
        let mut state = State {
            health: 4.,
            shields: 0.,
            next_shield_recharge: 0.,
        };
        let hit = || Hit {
            damage: 3.,
            bone: "",
            damage_type: None,
            bypass_shields: false,
            time: 1.,
        };
        damage(&mut state, &defense, &[1], hit(), 5., (1.75, false));
        assert_eq!(state.health, 0.3250003);
        assert_eq!(state.next_shield_recharge, 6.);
        // Observe the exact reduced float directly: pre-multiplying difficulty
        // would yield 3.674999952316284 instead of 3.674999713897705.
        assert_eq!(reduce(2.1, 1.75, false, false, |_, d| d), 3.6749997);
        let before = state;
        damage(&mut state, &defense, &[1], hit(), 50., (1.75, true));
        assert_eq!(state, before);
    }
}
