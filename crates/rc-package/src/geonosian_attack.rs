//! Prepared decision block of ctgame.dll 10039ff6..1003a212.
//!
//! Inputs are AFTER ComputeEssentials: distance is the native computed distance
//! minus the target collision radius, not center-to-center distance. Navigation,
//! animation root motion, grenade feasibility and rand() remain host boundaries.
//! This module neither runs the goal queue nor applies melee damage.

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
#[repr(u32)]
pub enum Maneuver {
    Wait = 0,
    Melee = 8,
    Follow = 0x200,
    Lunge = 0x400,
    Jump = 0x800,
    Grenade = 0x1000,
}

#[derive(Debug, Clone, Copy)]
pub struct Parameters {
    pub lunge_distance: f32,
    pub jump_distance: f32,
    pub offensive_jump_probability: f32,
}

/// Calls must retain native order, including queries whose maneuver is later
/// overridden by a grenade. The host supplies CanMoveTo at the transformed end
/// of the selected lunge animation, not at the enemy's current position.
pub trait Host {
    fn can_path(&mut self) -> bool;
    fn can_jump(&mut self) -> bool;
    fn can_move_lunge(&mut self) -> bool;
    fn can_throw_grenade(&mut self) -> bool;
}

pub fn capabilities(distance: f32, p: Parameters, host: &mut impl Host) -> u32 {
    let mut mask = 0;
    if host.can_path() {
        mask |= Maneuver::Follow as u32;
        if p.jump_distance > distance {
            mask |= Maneuver::Melee as u32;
        }
    }
    if distance > p.lunge_distance && p.jump_distance > distance && host.can_jump() {
        mask |= Maneuver::Jump as u32;
    }
    if p.lunge_distance >= distance && host.can_move_lunge() {
        mask |= Maneuver::Lunge as u32;
    }
    if host.can_throw_grenade() {
        mask |= Maneuver::Grenade as u32;
    }
    mask
}

/// Original MSVCRT rand() result (0..32767) conversion, using constant bits from
/// ctgame.dll 1004e130. No replacement RNG is chosen by this module.
pub fn random_fraction(value: u16) -> f32 {
    f32::from(value) * f32::from_bits(0x3800_0100)
}

/// ctgame.dll 1003a19a..1003a212. Conditional random consumption is observable:
/// a grenade consumes none; a failed jump can consume a second draw for lunge.
pub fn choose(mask: u32, probability: f32, mut random: impl FnMut() -> u16) -> Maneuver {
    if mask & Maneuver::Grenade as u32 != 0 {
        return Maneuver::Grenade;
    }
    if mask & Maneuver::Jump as u32 != 0 && probability > random_fraction(random()) {
        return Maneuver::Jump;
    }
    if mask & Maneuver::Lunge as u32 != 0 && random_fraction(random()) > f32::from_bits(0x3f4c_cccd)
    {
        return Maneuver::Lunge;
    }
    if mask & Maneuver::Melee as u32 != 0 {
        Maneuver::Melee
    } else if mask & Maneuver::Follow as u32 != 0 {
        Maneuver::Follow
    } else {
        Maneuver::Wait
    }
}

/// 1003a313..1003a328, called AFTER adding a melee goal. Equality succeeds,
/// unlike the offensive jump comparison; unordered values fail (COMISS / JC).
pub fn defensive_jump_after_melee(probability: f32, random: u16) -> bool {
    probability >= random_fraction(random)
}

/// 1003a2f1..1003a328: one rand() call on either target-type branch, including
/// zero probabilities. Caller has already enqueued the melee goal.
pub fn request_defensive_jump(
    player_target: bool,
    probability: f32,
    player_probability: f32,
    mut random: impl FnMut() -> u16,
) -> bool {
    let draw = random();
    defensive_jump_after_melee(
        if player_target {
            player_probability
        } else {
            probability
        },
        draw,
    )
}

#[cfg(test)]
mod tests {
    use super::*;

    struct Navigation {
        calls: Vec<&'static str>,
    }
    impl Host for Navigation {
        fn can_path(&mut self) -> bool {
            self.calls.push("path");
            true
        }
        fn can_jump(&mut self) -> bool {
            self.calls.push("jump");
            true
        }
        fn can_move_lunge(&mut self) -> bool {
            self.calls.push("lunge");
            true
        }
        fn can_throw_grenade(&mut self) -> bool {
            self.calls.push("grenade");
            false
        }
    }
    fn parameters() -> Parameters {
        Parameters {
            lunge_distance: 800.,
            jump_distance: 3000.,
            offensive_jump_probability: 0.95,
        }
    }
    #[test]
    fn native_distance_boundaries_and_navigation_call_order() {
        for (distance, expected, calls) in [
            (800., 0x608, vec!["path", "lunge", "grenade"]),
            (800.00006, 0xa08, vec!["path", "jump", "grenade"]),
            (2999.9998, 0xa08, vec!["path", "jump", "grenade"]),
            (3000., 0x200, vec!["path", "grenade"]),
            (f32::NAN, 0x200, vec!["path", "grenade"]),
        ] {
            let mut host = Navigation { calls: vec![] };
            assert_eq!(capabilities(distance, parameters(), &mut host), expected);
            assert_eq!(host.calls, calls);
        }
    }
    #[test]
    fn grenade_has_priority_and_does_not_advance_random() {
        assert_eq!(
            choose(0x1e08, 1., || panic!("unexpected draw")),
            Maneuver::Grenade
        );
    }
    #[test]
    fn failed_jump_draw_precedes_lunge_draw() {
        let mut draws = [32767, 32767].into_iter();
        assert_eq!(
            choose(0xe08, 0.95, || draws.next().unwrap()),
            Maneuver::Lunge
        );
        assert!(draws.next().is_none());
    }
    #[test]
    fn offensive_and_defensive_equality_differ() {
        let threshold = random_fraction(10000);
        assert_eq!(choose(0xa08, threshold, || 10000), Maneuver::Melee);
        assert!(defensive_jump_after_melee(threshold, 10000));
        assert!(!defensive_jump_after_melee(f32::NAN, 0));
        assert_eq!(choose(0xa08, f32::NAN, || 0), Maneuver::Melee);
    }
    #[test]
    fn native_random_endpoints() {
        assert_eq!(random_fraction(0).to_bits(), 0);
        assert_eq!(random_fraction(32767).to_bits(), 1_f32.to_bits());
        assert_eq!(
            choose(0x208, 1., || panic!("unexpected draw")),
            Maneuver::Melee
        );
        assert_eq!(
            choose(0x200, 1., || panic!("unexpected draw")),
            Maneuver::Follow
        );
        assert_eq!(choose(0, 1., || panic!("unexpected draw")), Maneuver::Wait);
    }
}
