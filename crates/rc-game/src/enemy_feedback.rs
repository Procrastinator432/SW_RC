//! Feedback for an accepted bounded hitscan action; no delayed or repeated damage.
use crate::{Point, State};
use serde::{Deserialize, Serialize};
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Trace {
    pub ranged: bool,
    pub tick: u64,
    pub owner: usize,
    pub from: Point,
    pub to: Point,
}
impl State {
    pub(crate) fn accept_enemy_attack(
        &mut self,
        owner: usize,
        from: Point,
        to: Point,
        damage: u32,
        sound: Option<usize>,
        ranged: bool,
    ) -> bool {
        if self.phase != crate::Phase::Playing
            || self.health == 0
            || self.enemies.get(owner).is_none_or(|e| e.health == 0)
        {
            return false;
        }
        self.health = self.health.saturating_sub(damage);
        self.damage_flash = 0.2;
        self.statistics.enemy_shots += 1;
        if self.enemy_traces.len() == 64 {
            self.enemy_traces.remove(0);
        }
        self.enemy_traces.push(Trace {
            tick: self.statistics.ticks,
            owner,
            from,
            to,
            ranged,
        });
        if let Some(sound) = sound {
            let tick = self.statistics.ticks;
            if let Some(m) = self.mission.as_mut() {
                m.audio_sequence += 1;
                if m.events.len() == 256 {
                    m.events.remove(0);
                }
                m.events.push(crate::combat::Event {
                    sequence: m.audio_sequence,
                    tick,
                    sound,
                    owner: owner as i32,
                });
            } else if let Some(m) = self.map.as_mut() {
                m.audio_sequence += 1;
                if m.events.len() == 256 {
                    m.events.remove(0);
                }
                m.events.push(crate::combat::Event {
                    sequence: m.audio_sequence,
                    tick,
                    sound,
                    owner: owner as i32,
                });
            }
        }
        true
    }
}
