//! Prepared arithmetic of Pawn.TakeDamage (original Pawn.uc 3268 onwards).
//! This is source-based, not a bytecode/whole-function parity claim. Hit metadata,
//! callbacks, animation, death, physics and material changes remain external.
//! Finite arithmetic inputs only; VM Max/Name semantics for arbitrary malformed
//! or non-ASCII inputs are not reconstructed here.
use crate::damage_variance::{calculate, Binding};

#[derive(Debug, Clone, Copy, PartialEq, serde::Serialize, serde::Deserialize)]
pub struct State {
    pub health: f32,
    pub shields: f32,
    pub next_shield_recharge: f32,
}
pub struct BoneMultiplier<'a> {
    pub bone: &'a str,
    pub multiplier: f32,
}
pub struct Rules<'a> {
    pub authority: bool,
    pub armor: f32,
    pub bone_multipliers: &'a [BoneMultiplier<'a>],
    pub variance: Binding<'a>,
    /// None is no class reference; Some(Invalid) is an explicit unusable CDO.
    pub shield_variance: Option<Binding<'a>>,
    pub shield_recharge_delay: f32,
}
pub struct Hit<'a> {
    pub damage: f32,
    pub bone: &'a str,
    pub damage_type: Option<&'a [u32]>,
    pub bypass_shields: bool,
    pub time: f32,
}

/// `reduce_damage` is the real Game.ReduceDamage boundary after bone, armor and
/// DamageVariance. No assumed difficulty, team, god-mode or neutral-zone factor.
/// Returns arithmetic health damage; already-dead callback/return exceptions
/// of the complete script are outside this block. A shield-only hit returns zero. State is fractional
/// f32, matching the script. The recharge deadline is updated on non-bypass hits
/// even when there were no shields. No automatic health lower clamp is applied.
pub fn apply(
    state: &mut State,
    rules: &Rules<'_>,
    hit: &Hit<'_>,
    reduce_damage: impl FnOnce(f32) -> f32,
) -> f32 {
    if !rules.authority {
        return 0.;
    }
    let mut damage = hit.damage;
    if let Some(bone) = rules
        .bone_multipliers
        .iter()
        .find(|b| !b.bone.is_empty() && b.bone.eq_ignore_ascii_case(hit.bone))
    {
        damage *= bone.multiplier;
    }
    damage *= 1. - rules.armor;
    let explicit = if state.shields > 0. {
        rules.shield_variance
    } else {
        None
    };
    let mut actual = reduce_damage(calculate(damage, hit.damage_type, rules.variance, explicit));
    if actual <= 0. {
        return 0.;
    }
    if !hit.bypass_shields {
        state.shields -= actual;
        state.next_shield_recharge =
            (hit.time + rules.shield_recharge_delay).max(state.next_shield_recharge);
        if state.shields < 0. {
            actual = -state.shields;
            state.shields = 0.;
        } else {
            return 0.;
        }
    }
    state.health -= actual;
    actual
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::damage_variance::{Include, Variance};
    fn state() -> State {
        State {
            health: 100.,
            shields: 100.,
            next_shield_recharge: 0.,
        }
    }
    fn rules() -> Rules<'static> {
        Rules {
            authority: true,
            armor: 0.,
            bone_multipliers: &[],
            variance: Binding::Invalid,
            shield_variance: None,
            shield_recharge_delay: 5.,
        }
    }
    fn hit() -> Hit<'static> {
        Hit {
            damage: 125.,
            bone: "",
            damage_type: Some(&[1, 2]),
            bypass_shields: false,
            time: 10.,
        }
    }
    #[test]
    fn selected_melee_modifier_shields_and_fractional_overflow() {
        let v = Variance {
            include: &[Include {
                class: Some(2),
                multiplier: 0.5,
            }],
            exclude: &[],
        };
        let r = Rules {
            variance: Binding::Valid(&v),
            ..rules()
        };
        let mut s = state();
        assert_eq!(apply(&mut s, &r, &hit(), |d| d), 0.);
        assert_eq!(
            s,
            State {
                health: 100.,
                shields: 37.5,
                next_shield_recharge: 15.
            }
        );
        assert_eq!(apply(&mut s, &r, &Hit { time: 11., ..hit() }, |d| d), 25.);
        assert_eq!(
            s,
            State {
                health: 75.,
                shields: 0.,
                next_shield_recharge: 16.
            }
        );
    }
    #[test]
    fn bone_armor_variance_and_game_reduction_order() {
        let v = Variance {
            include: &[Include {
                class: None,
                multiplier: 0.5,
            }],
            exclude: &[],
        };
        let r = Rules {
            armor: 0.25,
            bone_multipliers: &[
                BoneMultiplier {
                    bone: "",
                    multiplier: 999.,
                },
                BoneMultiplier {
                    bone: "head",
                    multiplier: 2.,
                },
                BoneMultiplier {
                    bone: "head",
                    multiplier: 9.,
                },
            ],
            variance: Binding::Valid(&v),
            ..rules()
        };
        let mut s = state();
        assert_eq!(
            apply(
                &mut s,
                &r,
                &Hit {
                    damage: 20.,
                    bone: "HEAD",
                    bypass_shields: true,
                    ..hit()
                },
                |d| {
                    assert_eq!(d, 15.);
                    d / 3.
                }
            ),
            5.
        );
        assert_eq!(s.health, 95.);
        assert_eq!(s.shields, 100.);
        assert_eq!(s.next_shield_recharge, 0.);
    }
    #[test]
    fn shield_variance_is_selected_before_bypass_and_zero_shields_use_actor() {
        let immune = Variance {
            include: &[],
            exclude: &[None],
        };
        let r = Rules {
            shield_variance: Some(Binding::Valid(&immune)),
            ..rules()
        };
        let mut s = state();
        assert_eq!(
            apply(
                &mut s,
                &r,
                &Hit {
                    bypass_shields: true,
                    ..hit()
                },
                |d| d
            ),
            0.
        );
        s.shields = 0.;
        assert_eq!(apply(&mut s, &r, &hit(), |d| d), 125.);
        assert_eq!(s.health, -25.);
        assert_eq!(s.next_shield_recharge, 15.);
    }
    #[test]
    fn authority_gate_and_game_immunity_do_not_change_state() {
        let mut s = state();
        let before = s;
        assert_eq!(
            apply(
                &mut s,
                &Rules {
                    authority: false,
                    ..rules()
                },
                &hit(),
                |_| panic!("not authoritative")
            ),
            0.
        );
        assert_eq!(apply(&mut s, &rules(), &hit(), |_| 0.), 0.);
        assert_eq!(s, before);
    }
    #[test]
    fn exact_shield_exhaustion_returns_zero_and_does_not_shorten_deadline() {
        let mut s = State {
            next_shield_recharge: 99.,
            ..state()
        };
        assert_eq!(
            apply(
                &mut s,
                &rules(),
                &Hit {
                    damage: 100.,
                    ..hit()
                },
                |d| d
            ),
            0.
        );
        assert_eq!(
            s,
            State {
                health: 100.,
                shields: 0.,
                next_shield_recharge: 99.
            }
        );
    }
}
