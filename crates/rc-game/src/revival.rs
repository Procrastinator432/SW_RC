//! Pawn.Reviving health arithmetic from the original Engine.Pawn source.
//! The integer HUD and synchronization of external damage are port boundaries.
use serde::Serialize;

#[derive(Clone, Debug, Serialize, serde::Deserialize)]
pub struct Recovery {
    pub health: f32,
}
impl Default for Recovery {
    fn default() -> Self {
        Self { health: 1. }
    }
}
impl Recovery {
    /// Retain fractional HP across ticks; only real changes to the displayed
    /// health are synchronized. Rounding each tick would greatly accelerate healing.
    pub fn tick(&mut self, observed: u32, maximum: f32, delta: f32) -> u32 {
        let displayed = crate::melee::display_health(self.health);
        self.health += observed as f32 - displayed as f32;
        self.health += maximum / 12.0_f32 * delta;
        crate::melee::display_health(self.health)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn small_fractional_steps_do_not_round_into_whole_hp_each_tick() {
        let mut recovery = Recovery::default();
        let mut health = 1;
        for _ in 0..60 {
            health = recovery.tick(health, 400., 1. / 60.);
        }
        assert_eq!(health, 35);
        assert!((recovery.health - (1. + 400. / 12.)).abs() < 0.0001);
    }
    #[test]
    fn external_damage_is_not_undone_by_the_fractional_accumulator() {
        let mut recovery = Recovery { health: 20.25 };
        assert_eq!(recovery.tick(16, 400., 1. / 60.), 16);
        assert!((recovery.health - (15.25 + 400. / 12. / 60.)).abs() < 0.00001);
    }
    #[test]
    fn mission_player_maximum_and_external_healing_remain_distinct() {
        let mut recovery = Recovery::default();
        let mut health = 1;
        for _ in 0..60 {
            health = recovery.tick(health, 100., 1. / 60.);
        }
        assert_eq!(health, 10);
        let previous = recovery.health;
        recovery.tick(health + 5, 100., 0.);
        assert_eq!(recovery.health, previous + 5.);
    }
}
