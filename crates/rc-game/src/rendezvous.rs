//! Selected geo_01a script lanes after the small bunker, ending at objective4.
//! This bounded event/timer host emits effects; it does not spawn AI, move Scorch,
//! change collision, play audio or execute arbitrary Unreal bytecode.
use crate::{crash::Trigger, Point};
use serde::{Deserialize, Serialize};
#[derive(Clone, Debug, Serialize, Deserialize, PartialEq)]
#[serde(deny_unknown_fields)]
pub struct Action {
    pub original: u32,
    pub operation: Operation,
}
#[derive(Clone, Debug, Serialize, Deserialize, PartialEq)]
#[serde(tag = "kind", deny_unknown_fields)]
pub enum Operation {
    Wait {
        seconds: f64,
    },
    Sound {
        source: String,
        seconds: f64,
        owner: String,
    },
    Emit {
        effect: Effect,
    },
}
#[derive(Clone, Debug, Serialize, Deserialize, PartialEq)]
#[serde(tag = "kind", deny_unknown_fields)]
pub enum Effect {
    Event {
        tag: String,
    },
    PlaySound {
        source: String,
        owner: String,
    },
    AddSquadMember {
        member: String,
        squad: String,
    },
    BlinkSquad {
        seconds: f64,
    },
    StopFactory {
        tag: String,
        permanent: bool,
        destroy_offspring: bool,
    },
    HurtActor {
        tag: String,
        damage: f64,
        damage_type: String,
    },
    StatusEffect {
        tag: String,
        class: String,
        seconds: f64,
        damage_per_second: f64,
    },
    ObjectiveCompleted {
        objective: u32,
    },
    Tracker {
        tag: String,
    },
    GotoState {
        tag: String,
        state: String,
    },
    ShowMarker {
        tag: String,
    },
}
#[derive(Clone, Debug, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Program {
    pub script: u32,
    pub trigger: String,
    pub actions: Vec<Action>,
}
#[derive(Clone, Debug, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Assets {
    pub join: Trigger,
    pub programs: Vec<Program>,
    pub provenance: serde_json::Value,
}
fn text(s: &str) -> bool {
    !s.is_empty() && s.len() <= 256 && !s.contains(['\0', '\n', '\r'])
}
fn seconds(v: f64) -> bool {
    v.is_finite() && (0. ..=120.).contains(&v)
}
impl Effect {
    fn validate(&self) -> bool {
        match self {
            Self::Event { tag } | Self::Tracker { tag } | Self::ShowMarker { tag } => text(tag),
            Self::PlaySound { source, owner } => text(source) && text(owner),
            Self::AddSquadMember { member, squad } => text(member) && text(squad),
            Self::BlinkSquad { seconds: v } => seconds(*v),
            Self::StopFactory { tag, .. } => text(tag),
            Self::HurtActor {
                tag,
                damage,
                damage_type,
            } => {
                text(tag)
                    && text(damage_type)
                    && damage.is_finite()
                    && (0. ..=100000.).contains(damage)
            }
            Self::StatusEffect {
                tag,
                class,
                seconds: v,
                damage_per_second,
            } => {
                text(tag)
                    && text(class)
                    && seconds(*v)
                    && damage_per_second.is_finite()
                    && (0. ..=10000.).contains(damage_per_second)
            }
            Self::ObjectiveCompleted { objective } => *objective == 4,
            Self::GotoState { tag, state } => text(tag) && text(state),
        }
    }
}
impl Assets {
    pub fn validate(&self) -> Result<(), String> {
        if self.join.original != 458
            || !self.join.validate()
            || self.programs.is_empty()
            || self.programs.len() > 16
        {
            return Err("Invalid rendezvous assets".into());
        }
        let mut ids = std::collections::BTreeSet::new();
        let mut triggers = std::collections::BTreeSet::new();
        for p in &self.programs {
            if ![195, 325, 511, 531, 847, 1120, 1128, 1134].contains(&p.script)
                || !text(&p.trigger)
                || !triggers.insert((p.script, &p.trigger))
                || p.actions.is_empty()
                || p.actions.len() > 32
            {
                return Err("Invalid selected rendezvous program".into());
            }
            for a in &p.actions {
                if a.original == 0 || !ids.insert(a.original) {
                    return Err("Duplicate original rendezvous action".into());
                }
                let valid = match &a.operation {
                    Operation::Wait { seconds: v } => seconds(*v),
                    Operation::Sound {
                        source,
                        seconds: v,
                        owner,
                    } => text(source) && text(owner) && seconds(*v),
                    Operation::Emit { effect } => {
                        effect.validate() && !matches!(effect, Effect::PlaySound { .. })
                    }
                };
                if !valid {
                    return Err("Invalid rendezvous operation".into());
                }
            }
        }
        Ok(())
    }
}
#[derive(Clone, Debug, Default, Serialize, PartialEq, serde::Deserialize)]
pub struct Lane {
    pub started: bool,
    pub finished: bool,
    pub next: usize,
    pub remaining: f64,
}
#[derive(Clone, Debug, Serialize, PartialEq, serde::Deserialize)]
pub struct State {
    pub lanes: Vec<Lane>,
    pub objective_completed: bool,
    pub squad_joined: bool,
    pub charge_enabled: bool,
    pub join_touching: bool,
}
impl State {
    pub fn new(a: &Assets) -> Result<Self, String> {
        a.validate()?;
        Ok(Self {
            lanes: vec![Lane::default(); a.programs.len()],
            objective_completed: false,
            squad_joined: false,
            charge_enabled: false,
            join_touching: false,
        })
    }
    fn validate(&self, a: &Assets) -> Result<(), String> {
        a.validate()?;
        if self.lanes.len() != a.programs.len()
            || self.lanes.iter().zip(&a.programs).any(|(s, p)| {
                s.next > p.actions.len()
                    || !seconds(s.remaining)
                    || s.finished && (!s.started || s.next != p.actions.len() || s.remaining != 0.)
                    || !s.started && (s.next != 0 || s.remaining != 0.)
            })
        {
            return Err("Invalid rendezvous state".into());
        }
        Ok(())
    }
    fn signal(&mut self, a: &Assets, event: &str) {
        for (lane, p) in self.lanes.iter_mut().zip(&a.programs) {
            if !lane.started && p.trigger == event {
                lane.started = true;
            }
        }
    }
    fn emit(&mut self, a: &Assets, e: Effect, out: &mut Vec<Effect>) {
        match &e {
            Effect::Event { tag } => self.signal(a, tag),
            Effect::ObjectiveCompleted { objective: 4 } => self.objective_completed = true,
            Effect::AddSquadMember { member, squad }
                if member == "Commando62Delta" && squad == "CTSquad" =>
            {
                self.squad_joined = true
            }
            Effect::GotoState { tag, state } if tag == "DemolitionCharge" => {
                self.charge_enabled = state == "Auto"
            }
            _ => {}
        }
        out.push(e);
    }
    // At a shared deadline, finish every ready lane before moving the common clock.
    // Events can start another lane in this same instant, even an earlier lane.
    fn flush(&mut self, a: &Assets, out: &mut Vec<Effect>) -> Result<(), String> {
        for _ in 0..=a.programs.len() {
            let mut progressed = false;
            for i in 0..self.lanes.len() {
                while self.lanes[i].started
                    && !self.lanes[i].finished
                    && self.lanes[i].remaining <= 1e-10
                {
                    self.lanes[i].remaining = 0.;
                    progressed = true;
                    if self.lanes[i].next == a.programs[i].actions.len() {
                        self.lanes[i].finished = true;
                        break;
                    }
                    let op = a.programs[i].actions[self.lanes[i].next].operation.clone();
                    self.lanes[i].next += 1;
                    match op {
                        Operation::Wait { seconds } => self.lanes[i].remaining = seconds,
                        Operation::Sound {
                            source,
                            seconds,
                            owner,
                        } => {
                            self.lanes[i].remaining = seconds;
                            self.emit(a, Effect::PlaySound { source, owner }, out);
                        }
                        Operation::Emit { effect } => self.emit(a, effect, out),
                    }
                }
            }
            if !progressed {
                return Ok(());
            }
        }
        Err("Rendezvous event cascade exceeded budget".into())
    }
    /// Original external event, latched once per selected script lane.
    pub fn event(&mut self, a: &Assets, event: &str) -> Result<Vec<Effect>, String> {
        self.validate(a)?;
        if !text(event) {
            return Err("Invalid rendezvous event".into());
        }
        let mut next = self.clone();
        next.signal(a, event);
        let mut effects = vec![];
        next.flush(a, &mut effects)?;
        *self = next;
        Ok(effects)
    }
    /// Player-cylinder observation is separate from time advancement. Repeated
    /// observations or re-entry cannot replay the one-shot squad script.
    pub fn observe(
        &mut self,
        a: &Assets,
        position: Point,
        extent: Point,
    ) -> Result<Vec<Effect>, String> {
        self.validate(a)?;
        if position.iter().any(|v| !v.is_finite() || v.abs() > 1e8)
            || extent
                .iter()
                .any(|v| !v.is_finite() || !(0. ..=1000.).contains(v))
        {
            return Err("Invalid rendezvous player cylinder".into());
        }
        let mut next = self.clone();
        let touching = a.join.contains(position, extent);
        let mut effects = vec![];
        if touching && !next.join_touching {
            next.signal(a, "Add62ToSquad");
            next.flush(a, &mut effects)?;
        }
        next.join_touching = touching;
        *self = next;
        Ok(effects)
    }
    pub fn advance(&mut self, a: &Assets, dt: f64) -> Result<Vec<Effect>, String> {
        self.validate(a)?;
        if !dt.is_finite() || !(0. ..=1.).contains(&dt) {
            return Err("Invalid rendezvous time step".into());
        }
        let mut next = self.clone();
        let mut effects = vec![];
        next.flush(a, &mut effects)?;
        let mut left = dt;
        while left > 0. {
            let deadline = next
                .lanes
                .iter()
                .filter(|s| s.started && !s.finished && s.remaining > 0.)
                .map(|s| s.remaining)
                .reduce(f64::min);
            let Some(deadline) = deadline else {
                break;
            };
            let step = left.min(deadline);
            for lane in &mut next.lanes {
                if lane.started && !lane.finished {
                    lane.remaining = (lane.remaining - step).max(0.);
                }
            }
            left = (left - step).max(0.);
            next.flush(a, &mut effects)?;
        }
        *self = next;
        Ok(effects)
    }
}
#[cfg(test)]
mod tests {
    use super::*;
    fn assets() -> Assets {
        let emit = |id, e| Action {
            original: id,
            operation: Operation::Emit { effect: e },
        };
        Assets {
            join: Trigger {
                original: 458,
                position: [0.; 3],
                radius: 500.,
                height: 40.,
            },
            provenance: serde_json::Value::Null,
            programs: vec![
                Program {
                    script: 1128,
                    trigger: "DialogDemolition".into(),
                    actions: vec![
                        Action {
                            original: 3541,
                            operation: Operation::Sound {
                                source: "GEO_Voice.Intro".into(),
                                seconds: 0.5,
                                owner: "PlayerCommando".into(),
                            },
                        },
                        emit(3547, Effect::ObjectiveCompleted { objective: 4 }),
                        emit(
                            3548,
                            Effect::Event {
                                tag: "ActivateDemoCharge".into(),
                            },
                        ),
                    ],
                },
                Program {
                    script: 195,
                    trigger: "ActivateDemoCharge".into(),
                    actions: vec![emit(
                        1749,
                        Effect::GotoState {
                            tag: "DemolitionCharge".into(),
                            state: "Auto".into(),
                        },
                    )],
                },
                Program {
                    script: 531,
                    trigger: "Add62ToSquad".into(),
                    actions: vec![
                        Action {
                            original: 2518,
                            operation: Operation::Sound {
                                source: "Character_Voice.D62".into(),
                                seconds: 1.,
                                owner: "PlayerCommando".into(),
                            },
                        },
                        emit(
                            2521,
                            Effect::AddSquadMember {
                                member: "Commando62Delta".into(),
                                squad: "CTSquad".into(),
                            },
                        ),
                        emit(
                            2526,
                            Effect::Event {
                                tag: "DialogDemolition".into(),
                            },
                        ),
                    ],
                },
                Program {
                    script: 847,
                    trigger: "Conversation04".into(),
                    actions: vec![
                        Action {
                            original: 3178,
                            operation: Operation::Sound {
                                source: "GEO_Sound.Radio".into(),
                                seconds: 2.,
                                owner: "PlayerCommando".into(),
                            },
                        },
                        emit(
                            3181,
                            Effect::Event {
                                tag: "RadioDone".into(),
                            },
                        ),
                    ],
                },
            ],
        }
    }
    #[test]
    fn no_time_or_radio_can_complete_rendezvous_without_player_contact() {
        let a = assets();
        let mut s = State::new(&a).unwrap();
        s.event(&a, "Conversation04").unwrap();
        for _ in 0..20 {
            s.advance(&a, 1.).unwrap();
        }
        assert!(!s.squad_joined && !s.objective_completed && !s.charge_enabled);
    }
    #[test]
    fn squad_before_objective_and_charge_after_intro() {
        let a = assets();
        let mut s = State::new(&a).unwrap();
        assert_eq!(s.observe(&a, [0.; 3], [40., 40., 84.]).unwrap().len(), 1);
        s.advance(&a, 0.999).unwrap();
        assert!(!s.squad_joined);
        s.advance(&a, 0.001).unwrap();
        assert!(s.squad_joined && !s.objective_completed && !s.charge_enabled);
        s.advance(&a, 0.5).unwrap();
        assert!(s.objective_completed && s.charge_enabled);
    }
    #[test]
    fn event_cascade_carries_unused_time_to_an_earlier_lane() {
        let a = assets();
        let mut s = State::new(&a).unwrap();
        s.event(&a, "Add62ToSquad").unwrap();
        let e = s.advance(&a, 1.).unwrap();
        assert!(e
            .iter()
            .any(|e| matches!(e,Effect::PlaySound{source,..}if source=="GEO_Voice.Intro")));
        assert_eq!(s.lanes[0].remaining, 0.5);
        s.advance(&a, 1.).unwrap();
        assert!(s.objective_completed && s.charge_enabled);
    }
    #[test]
    fn parallel_radio_has_its_own_clock_and_repeated_events_do_not_replay() {
        let a = assets();
        let mut s = State::new(&a).unwrap();
        s.event(&a, "Conversation04").unwrap();
        s.event(&a, "Add62ToSquad").unwrap();
        s.advance(&a, 1.).unwrap();
        assert_eq!(s.lanes[3].remaining, 1.);
        assert!(s.event(&a, "Conversation04").unwrap().is_empty());
        s.advance(&a, 1.).unwrap();
        assert!(s.lanes[3].finished);
        assert!(s.event(&a, "Add62ToSquad").unwrap().is_empty());
    }
    #[test]
    fn partitioned_time_has_identical_effect_order_and_flags() {
        let a = assets();
        let mut x = State::new(&a).unwrap();
        x.event(&a, "Add62ToSquad").unwrap();
        let mut y = x.clone();
        let mut left = vec![];
        let mut right = vec![];
        for _ in 0..2 {
            left.extend(x.advance(&a, 1.).unwrap());
        }
        for _ in 0..240 {
            right.extend(y.advance(&a, 1. / 120.).unwrap());
        }
        assert_eq!(left, right);
        assert_eq!(x, y);
    }
    #[test]
    fn cylinder_requires_both_axes_and_edge_reentry_is_once() {
        let a = assets();
        let mut s = State::new(&a).unwrap();
        for p in [[600., 0., 0.], [0., 0., 125.]] {
            assert!(s.observe(&a, p, [40., 40., 84.]).unwrap().is_empty());
        }
        assert!(!s.observe(&a, [0.; 3], [40., 40., 84.]).unwrap().is_empty());
        s.observe(&a, [1000., 0., 0.], [40., 40., 84.]).unwrap();
        assert!(s.observe(&a, [0.; 3], [40., 40., 84.]).unwrap().is_empty());
    }
    #[test]
    fn bad_time_cylinder_assets_and_state_are_atomic() {
        let a = assets();
        let mut s = State::new(&a).unwrap();
        let before = s.clone();
        assert!(s.advance(&a, f64::NAN).is_err());
        assert!(s.observe(&a, [f64::INFINITY; 3], [1.; 3]).is_err());
        assert_eq!(s, before);
        let mut bad = a.clone();
        bad.programs[0].actions[0].original = 2518;
        assert!(s.event(&bad, "Add62ToSquad").is_err());
        assert_eq!(s, before);
        s.lanes[0].next = 999;
        let before = s.clone();
        assert!(s.advance(&a, 0.).is_err());
        assert_eq!(s, before);
    }
    #[test]
    fn one_event_starts_two_original_scripts_and_nonblocking_sound_does_not_wait() {
        let mut a = assets();
        a.programs.push(Program {
            script: 1120,
            trigger: "Conversation04".into(),
            actions: vec![
                Action {
                    original: 3520,
                    operation: Operation::Sound {
                        source: "DroidVoice".into(),
                        seconds: 0.,
                        owner: "None".into(),
                    },
                },
                Action {
                    original: 3521,
                    operation: Operation::Wait { seconds: 1. },
                },
                Action {
                    original: 3525,
                    operation: Operation::Emit {
                        effect: Effect::Event {
                            tag: "62WalkToIntro".into(),
                        },
                    },
                },
            ],
        });
        let mut s = State::new(&a).unwrap();
        let effects = s.event(&a, "Conversation04").unwrap();
        assert_eq!(
            effects
                .iter()
                .filter(|e| matches!(e, Effect::PlaySound { .. }))
                .count(),
            2
        );
        let effects = s.advance(&a, 1.).unwrap();
        assert!(effects.contains(&Effect::Event {
            tag: "62WalkToIntro".into()
        }));
        assert!(s.lanes.last().unwrap().finished);
        assert!(!s.lanes[3].finished);
    }
}
