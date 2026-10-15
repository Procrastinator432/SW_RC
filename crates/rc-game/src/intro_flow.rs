//! Selected Script13/197 lifecycle contract. Animation end, placement and movement
//! completion are real host callbacks; no timer substitutes for those boundaries.
use serde::{Deserialize, Serialize};
#[derive(Clone, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Flow {
    pub first_voice_seconds: f64,
    pub second_voice_seconds: f64,
    pub second_voice_wait: bool,
    pub destroy_delay: f64,
    pub teleport_destination: u32,
    pub move_destination: u32,
    pub teleport_no_check: bool,
    pub world_collision: bool,
}
impl Flow {
    pub fn validate(&self) -> Result<(), String> {
        if [
            self.first_voice_seconds,
            self.second_voice_seconds,
            self.destroy_delay,
        ]
        .iter()
        .any(|v| !v.is_finite() || !(0. ..=120.).contains(v))
            || self.teleport_destination == 0
            || self.move_destination == 0
        {
            return Err("Invalid original intro flow".into());
        }
        Ok(())
    }
}
#[derive(Clone, Debug, PartialEq, Serialize, serde::Deserialize)]
pub enum Effect {
    Animation {
        actor: u32,
        clip: String,
        repeat: bool,
    },
    Sound {
        owner: u32,
    },
    Collision {
        actor: u32,
        actors: bool,
        world: bool,
    },
    Teleport {
        actor: u32,
        destination: u32,
        no_check: bool,
    },
    RotateTo {
        actor: u32,
        destination: u32,
    },
    MoveGoal {
        actor: u32,
        destination: u32,
    },
    Destroy {
        actor: u32,
    },
}
#[derive(Clone, Debug, PartialEq, Serialize, serde::Deserialize)]
pub struct Event {
    pub seconds: f64,
    pub original: u32,
    pub effect: Effect,
}
#[derive(Clone, Debug, Default, PartialEq, Serialize, serde::Deserialize)]
pub enum WarriorPhase {
    #[default]
    Idle,
    Intro,
    WaitingGunship,
    Moving,
    Destroyed,
}
#[derive(Clone, Debug, Default, Serialize, serde::Deserialize)]
pub struct State {
    pub initialized: bool,
    pub clock: f64,
    pub rappel_started: bool,
    pub warrior: WarriorPhase,
    pub trooper_stage: u8,
    pub remaining: f64,
    pub trooper_destroyed: bool,
    pub events: Vec<Event>,
}
impl State {
    fn emit(&mut self, id: u32, effect: Effect) {
        self.events.push(Event {
            seconds: self.clock,
            original: id,
            effect,
        });
    }
    pub fn begin(&mut self, a: &Flow) -> Result<(), String> {
        a.validate()?;
        if self.initialized {
            return Ok(());
        }
        self.initialized = true;
        self.emit(
            1256,
            Effect::Animation {
                actor: 452,
                clip: "WarriorIntroBreathe".into(),
                repeat: true,
            },
        );
        self.emit(
            1764,
            Effect::Collision {
                actor: 530,
                actors: false,
                world: a.world_collision,
            },
        );
        self.emit(
            1765,
            Effect::Animation {
                actor: 530,
                clip: "WarriorIntroBreathe".into(),
                repeat: true,
            },
        );
        Ok(())
    }
    pub fn signal(&mut self, a: &Flow, tag: &str) -> Result<(), String> {
        a.validate()?;
        if !self.initialized {
            return Err("Intro not initialized".into());
        }
        if tag == "Rappel" && !self.rappel_started {
            self.rappel_started = true;
            self.warrior = WarriorPhase::Intro;
            self.emit(
                1258,
                Effect::Animation {
                    actor: 452,
                    clip: "WarriorIntro".into(),
                    repeat: false,
                },
            );
            self.emit(1259, Effect::Sound { owner: 453 });
            self.emit(
                1767,
                Effect::Animation {
                    actor: 530,
                    clip: "WarriorIntro".into(),
                    repeat: false,
                },
            );
            self.trooper_stage = 1;
            self.remaining = a.first_voice_seconds;
        }
        if tag == "GunshipWarrior" && self.warrior == WarriorPhase::WaitingGunship {
            self.warrior = WarriorPhase::Moving;
            self.emit(
                1771,
                Effect::MoveGoal {
                    actor: 530,
                    destination: a.move_destination,
                },
            );
        }
        Ok(())
    }
    pub fn tick(&mut self, a: &Flow, delta: f64) -> Result<(), String> {
        a.validate()?;
        if !delta.is_finite() || !(0. ..=0.25).contains(&delta) {
            return Err("Invalid intro tick".into());
        }
        let mut rest = delta;
        for _ in 0..8 {
            if self.trooper_stage == 0 || self.trooper_stage == 4 {
                self.clock += rest;
                return Ok(());
            }
            let step = rest.min(self.remaining);
            self.clock += step;
            self.remaining = (self.remaining - step).max(0.);
            rest -= step;
            if self.remaining > 1e-10 {
                return Ok(());
            }
            match self.trooper_stage {
                1 => {
                    self.emit(1260, Effect::Sound { owner: 452 });
                    self.trooper_stage = 2;
                    self.remaining = if a.second_voice_wait {
                        a.second_voice_seconds
                    } else {
                        0.
                    };
                }
                2 => {
                    self.trooper_stage = 3;
                    self.remaining = a.destroy_delay;
                }
                3 => {
                    self.trooper_stage = 4;
                    self.trooper_destroyed = true;
                    self.emit(1262, Effect::Destroy { actor: 452 });
                }
                _ => return Err("Invalid trooper lane".into()),
            }
        }
        Err("Intro event budget exceeded".into())
    }
    pub fn animation_ended(&mut self, a: &Flow, actor: u32) -> Result<(), String> {
        a.validate()?;
        if actor != 530 || self.warrior != WarriorPhase::Intro {
            return Ok(());
        }
        self.emit(
            1768,
            Effect::Teleport {
                actor: 530,
                destination: a.teleport_destination,
                no_check: a.teleport_no_check,
            },
        );
        // Original script ignores SetLocation's bool and still sets destination rotation.
        self.emit(
            1768,
            Effect::RotateTo {
                actor: 530,
                destination: a.teleport_destination,
            },
        );
        self.emit(
            1769,
            Effect::Collision {
                actor: 530,
                actors: true,
                world: a.world_collision,
            },
        );
        self.warrior = WarriorPhase::WaitingGunship;
        Ok(())
    }
    pub fn move_completed(&mut self, actor: u32) {
        if actor == 530 && self.warrior == WarriorPhase::Moving {
            self.warrior = WarriorPhase::Destroyed;
            self.emit(1772, Effect::Destroy { actor: 530 });
        }
    }
}
#[cfg(test)]
mod tests {
    use super::*;
    fn flow() -> Flow {
        Flow {
            first_voice_seconds: 0.3777324263038549,
            second_voice_seconds: 0.8943764172335601,
            second_voice_wait: false,
            destroy_delay: 3.,
            teleport_destination: 895,
            move_destination: 413,
            teleport_no_check: false,
            world_collision: true,
        }
    }
    #[test]
    fn trooper_dialogue_does_not_wait_for_its_animation_or_second_voice() {
        let a = flow();
        let mut s = State::default();
        s.begin(&a).unwrap();
        s.signal(&a, "Rappel").unwrap();
        for _ in 0..240 {
            s.tick(&a, 1. / 60.).unwrap();
        }
        let e = s.events.iter().find(|e| e.original == 1262).unwrap();
        assert!((e.seconds - (a.first_voice_seconds + 3.)).abs() < 1e-9);
        assert_eq!(s.warrior, WarriorPhase::Intro);
        assert!(s.trooper_destroyed);
    }
    #[test]
    fn warrior_needs_animation_end_then_event_then_real_movement_completion() {
        let a = flow();
        let mut s = State::default();
        s.begin(&a).unwrap();
        s.signal(&a, "Rappel").unwrap();
        s.signal(&a, "GunshipWarrior").unwrap();
        s.tick(&a, 0.1).unwrap();
        assert_eq!(s.warrior, WarriorPhase::Intro);
        s.animation_ended(&a, 452).unwrap();
        assert_eq!(s.warrior, WarriorPhase::Intro);
        s.animation_ended(&a, 530).unwrap();
        assert_eq!(s.warrior, WarriorPhase::WaitingGunship);
        let tail = &s.events[s.events.len() - 3..];
        assert!(matches!(
            tail[0].effect,
            Effect::Teleport {
                destination: 895,
                no_check: false,
                ..
            }
        ));
        assert!(matches!(
            tail[1].effect,
            Effect::RotateTo {
                destination: 895,
                ..
            }
        ));
        s.move_completed(530);
        assert_eq!(s.warrior, WarriorPhase::WaitingGunship);
        s.signal(&a, "GunshipWarrior").unwrap();
        assert_eq!(s.warrior, WarriorPhase::Moving);
        s.move_completed(452);
        assert_eq!(s.warrior, WarriorPhase::Moving);
        s.move_completed(530);
        s.move_completed(530);
        assert_eq!(s.events.iter().filter(|e| e.original == 1772).count(), 1);
    }
    #[test]
    fn repeated_signals_and_animation_callbacks_do_not_duplicate_actions() {
        let a = flow();
        let mut s = State::default();
        s.begin(&a).unwrap();
        s.begin(&a).unwrap();
        s.signal(&a, "Rappel").unwrap();
        s.signal(&a, "Rappel").unwrap();
        s.animation_ended(&a, 530).unwrap();
        s.animation_ended(&a, 530).unwrap();
        assert_eq!(s.events.iter().filter(|e| e.original == 1259).count(), 1);
        assert_eq!(
            s.events
                .iter()
                .filter(|e| matches!(e.effect, Effect::Teleport { .. }))
                .count(),
            1
        );
    }
    #[test]
    fn voice_timing_is_frame_rate_independent_and_respects_wait_flag() {
        for wait in [false, true] {
            let mut a = flow();
            a.second_voice_wait = wait;
            let mut times = vec![];
            for hz in [20., 60., 144.] {
                let mut s = State::default();
                s.begin(&a).unwrap();
                s.signal(&a, "Rappel").unwrap();
                for _ in 0..(hz * 6.) as usize {
                    s.tick(&a, 1. / hz).unwrap();
                }
                times.push(
                    s.events
                        .iter()
                        .find(|e| e.original == 1262)
                        .unwrap()
                        .seconds,
                );
            }
            let expected = a.first_voice_seconds
                + a.destroy_delay
                + if wait { a.second_voice_seconds } else { 0. };
            assert!(times.iter().all(|v| (v - expected).abs() < 1e-9));
        }
    }
    #[test]
    fn invalid_timing_is_rejected_before_state_changes() {
        let mut a = flow();
        a.destroy_delay = f64::NAN;
        let mut s = State::default();
        assert!(s.begin(&a).is_err());
        assert!(!s.initialized);
        let a = flow();
        let before = serde_json::to_vec(&s).unwrap();
        assert!(s.tick(&a, f64::INFINITY).is_err());
        assert_eq!(serde_json::to_vec(&s).unwrap(), before);
    }
}
