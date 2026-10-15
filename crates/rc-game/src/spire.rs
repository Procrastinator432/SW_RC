//! Selected final geo_01a gates; original events/audio with bounded script and door hosts.
use crate::{crash::Trigger, Point};
use serde::{Deserialize, Serialize};
#[derive(Clone, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Flow {
    pub area: Trigger,
    pub door: Trigger,
    pub exit: Trigger,
    pub next_url: String,
    pub door_seconds: f64,
    pub opening: usize,
    pub opened: usize,
    pub conversation: Vec<Action>,
    pub artillery: Vec<Action>,
}
#[derive(Clone, Serialize, Deserialize)]
pub struct Action {
    pub original: u32,
    pub operation: Operation,
}
#[derive(Clone, Serialize, Deserialize)]
#[serde(tag = "kind", deny_unknown_fields)]
pub enum Operation {
    Sound { sound: usize, seconds: f64 },
    Wait { seconds: f64 },
    Event { tag: String },
    Release { original: u32 },
}
#[derive(Clone, Debug, PartialEq, Serialize, serde::Deserialize)]
pub enum Effect {
    Sound(usize),
    Event(String),
    Release(u32),
    Hint,
    Exit(String),
}
#[derive(Clone, Debug, Default, Serialize, serde::Deserialize)]
pub struct Lane {
    pub started: bool,
    pub next: usize,
    pub remaining: f64,
}
#[derive(Clone, Debug, Default, Serialize, serde::Deserialize)]
pub struct State {
    pub enabled: bool,
    pub entered: bool,
    pub conversation: Lane,
    pub artillery: Lane,
    pub barrier_open: bool,
    pub door_started: bool,
    pub door_elapsed: f64,
    pub door_open: bool,
    pub completed: bool,
}
impl Flow {
    pub fn validate(&self, sounds: usize) -> Result<(), String> {
        if !self.area.validate()
            || !self.door.validate()
            || !self.exit.validate()
            || self.area.original != 39
            || self.door.original != 24
            || self.exit.original != 9
            || self.next_url != "GEO_01B.ctm#playerstart"
            || !self.door_seconds.is_finite()
            || !(0. ..=30.).contains(&self.door_seconds)
            || self.opening >= sounds
            || self.opened >= sounds
            || self.conversation.is_empty()
            || self.artillery.is_empty()
        {
            return Err("Invalid spire gates".into());
        }
        for a in self.conversation.iter().chain(&self.artillery) {
            match &a.operation {
                Operation::Sound { sound, seconds }
                    if *sound < sounds && seconds.is_finite() && (0. ..=120.).contains(seconds) => {
                }
                Operation::Wait { seconds }
                    if seconds.is_finite() && (0. ..=120.).contains(seconds) => {}
                Operation::Event { tag } if !tag.is_empty() && tag.len() < 128 => {}
                Operation::Release { original } if [860, 861, 862].contains(original) => {}
                _ => return Err("Invalid spire action".into()),
            }
        }
        if self.conversation.len() + self.artillery.len() > 64 {
            return Err("Spire script exceeds budget".into());
        }
        Ok(())
    }
}
fn lane(l: &mut Lane, actions: &[Action], mut dt: f64, out: &mut Vec<Effect>) {
    if !l.started {
        return;
    }
    loop {
        let consumed = dt.min(l.remaining);
        dt -= consumed;
        l.remaining -= consumed;
        if l.remaining > 1e-9 || l.next == actions.len() {
            break;
        }
        l.remaining = 0.;
        let a = &actions[l.next];
        l.next += 1;
        match &a.operation {
            Operation::Sound { sound, seconds } => {
                out.push(Effect::Sound(*sound));
                l.remaining = *seconds;
            }
            Operation::Wait { seconds } => l.remaining = *seconds,
            Operation::Event { tag } => out.push(Effect::Event(tag.clone())),
            Operation::Release { original } => out.push(Effect::Release(*original)),
        }
    }
}
impl State {
    pub fn advance(
        &mut self,
        a: &Flow,
        dt: f64,
        position: Point,
        extent: Point,
        exploded: bool,
        conversation: bool,
    ) -> Result<Vec<Effect>, String> {
        if !dt.is_finite()
            || !(0. ..=1.).contains(&dt)
            || position.iter().chain(&extent).any(|x| !x.is_finite())
        {
            return Err("Invalid spire input".into());
        }
        let mut out = vec![];
        self.enabled |= exploded;
        if !self.enabled || self.completed {
            return Ok(out);
        }
        if conversation {
            self.conversation.started = true;
        }
        if !self.entered && a.area.contains(position, extent) {
            self.entered = true;
            self.artillery.started = true;
            out.push(Effect::Event("EnteringSpiderDroidArea".into()));
        }
        lane(&mut self.conversation, &a.conversation, dt, &mut out);
        lane(&mut self.artillery, &a.artillery, dt, &mut out);
        if out
            .iter()
            .any(|e| matches!(e,Effect::Event(tag) if tag=="UnblockSpiderDroidBarricade"))
        {
            self.barrier_open = true;
        }
        if !self.door_started && a.door.contains(position, extent) {
            self.door_started = true;
            out.push(Effect::Sound(a.opening));
            out.push(Effect::Event("D_Exit".into()));
        }
        if self.door_started && !self.door_open {
            self.door_elapsed += dt;
            if self.door_elapsed + 1e-9 >= a.door_seconds {
                self.door_open = true;
                out.extend([
                    Effect::Release(34),
                    Effect::Release(35),
                    Effect::Sound(a.opened),
                    Effect::Event("PopUpEndLevel".into()),
                    Effect::Hint,
                ]);
            }
        }
        if self.barrier_open && self.door_open && a.exit.contains(position, extent) {
            self.completed = true;
            out.push(Effect::Exit(a.next_url.clone()));
        }
        Ok(out)
    }
}
#[cfg(test)]
mod tests {
    use super::*;
    fn assets() -> Flow {
        let trigger = |original, x| Trigger {
            original,
            position: [x, 0., 0.],
            radius: 10.,
            height: 10.,
        };
        Flow {
            area: trigger(39, 0.),
            door: trigger(24, 200.),
            exit: trigger(9, 300.),
            next_url: "GEO_01B.ctm#playerstart".into(),
            door_seconds: 0.5,
            opening: 2,
            opened: 3,
            conversation: vec![
                Action {
                    original: 3183,
                    operation: Operation::Sound {
                        sound: 0,
                        seconds: 0.5,
                    },
                },
                Action {
                    original: 3184,
                    operation: Operation::Sound {
                        sound: 1,
                        seconds: 0.5,
                    },
                },
            ],
            artillery: vec![
                Action {
                    original: 1313,
                    operation: Operation::Wait { seconds: 4. },
                },
                Action {
                    original: 1314,
                    operation: Operation::Release { original: 860 },
                },
                Action {
                    original: 1315,
                    operation: Operation::Event {
                        tag: "UnblockSpiderDroidBarricade".into(),
                    },
                },
            ],
        }
    }
    #[test]
    fn gates_require_explosion_then_original_area_and_do_not_autocomplete() {
        let a = assets();
        a.validate(4).unwrap();
        let mut s = State::default();
        assert!(s
            .advance(&a, 1., [0.; 3], [1.; 3], false, false)
            .unwrap()
            .is_empty());
        s.advance(&a, 1., [300., 0., 0.], [1.; 3], true, false)
            .unwrap();
        assert!(!s.entered && !s.completed);
        s.advance(&a, 1., [0.; 3], [1.; 3], true, true).unwrap();
        assert!(s.entered && !s.barrier_open);
        for _ in 0..3 {
            s.advance(&a, 1., [0.; 3], [1.; 3], true, true).unwrap();
        }
        assert!(s.barrier_open && !s.completed);
    }
    #[test]
    fn door_and_exit_are_once_only_and_preserve_destination() {
        let a = assets();
        let mut s = State::default();
        for _ in 0..4 {
            s.advance(&a, 1., [0.; 3], [1.; 3], true, true).unwrap();
        }
        let out = s
            .advance(&a, 0.5, [200., 0., 0.], [1.; 3], true, true)
            .unwrap();
        assert!(s.door_open && out.contains(&Effect::Hint));
        let out = s
            .advance(&a, 0.5, [300., 0., 0.], [1.; 3], true, true)
            .unwrap();
        assert_eq!(out, vec![Effect::Exit(a.next_url.clone())]);
        assert!(s
            .advance(&a, 1., [300., 0., 0.], [1.; 3], true, true)
            .unwrap()
            .is_empty());
    }
    #[test]
    fn lane_waits_conserve_time_across_step_partitions() {
        let a = assets();
        let mut x = State::default();
        let mut y = x.clone();
        let mut ex = vec![];
        let mut ey = vec![];
        for _ in 0..5 {
            ex.extend(x.advance(&a, 1., [0.; 3], [1.; 3], true, true).unwrap());
        }
        for _ in 0..500 {
            ey.extend(y.advance(&a, 0.01, [0.; 3], [1.; 3], true, true).unwrap());
        }
        assert_eq!(ex, ey);
        assert!(x.barrier_open && y.barrier_open);
    }
    #[test]
    fn invalid_delta_is_atomic() {
        let a = assets();
        let mut s = State::default();
        let old = serde_json::to_vec(&s).unwrap();
        assert!(s
            .advance(&a, f64::NAN, [0.; 3], [1.; 3], true, true)
            .is_err());
        assert_eq!(serde_json::to_vec(&s).unwrap(), old);
    }
}
