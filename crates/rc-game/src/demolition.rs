//! Selected objective5 marker/charge host. Movement, rendering, damage and input
//! targeting are caller responsibilities; this is not a general marker GoalQueue.
use serde::{Deserialize, Serialize};

#[derive(Clone, Debug, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Assets {
    pub marker: u32,
    pub charge: u32,
    pub prop: u32,
    pub navigation: u32,
    pub setup_seconds: f64,
    pub start_seconds: f64,
    pub place_fraction: f64,
    pub stop_seconds: f64,
    pub player_range: f64,
    pub damage: f64,
    pub damage_radius: f64,
    pub placed_sound: String,
    pub armed_sound: String,
    pub explosion_sound: String,
    pub programs: Vec<Program>,
}
#[derive(Clone, Debug, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Program {
    pub trigger: String,
    pub actions: Vec<Action>,
}
#[derive(Clone, Debug, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Action {
    pub original: u32,
    pub operation: Operation,
}
#[derive(Clone, Debug, Serialize, Deserialize)]
#[serde(tag = "kind", deny_unknown_fields)]
pub enum Operation {
    Wait { seconds: f64 },
    WaitArmed,
    Sound { source: String, seconds: f64 },
    IfExplodedEnd,
    Emit { effect: Effect },
}
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
#[serde(tag = "kind", deny_unknown_fields)]
pub enum Effect {
    Event {
        tag: String,
    },
    Sound {
        source: String,
    },
    Voice {
        enabled: bool,
    },
    Tracker {
        tag: String,
    },
    StopFactory {
        tag: String,
        permanent: bool,
        destroy_offspring: bool,
    },
    ObjectiveCompleted {
        objective: u32,
    },
    DestroyProp {
        original: u32,
    },
    UnblockNavigation {
        original: u32,
    },
    Blast {
        damage: f64,
        radius: f64,
    },
}
fn text(s: &str) -> bool {
    !s.is_empty() && s.len() <= 256 && !s.contains(['\0', '\n', '\r'])
}
fn seconds(v: f64) -> bool {
    v.is_finite() && (0. ..=120.).contains(&v)
}
impl Assets {
    pub fn validate(&self) -> Result<(), String> {
        if self.marker != 466
            || self.charge != 465
            || self.prop != 851
            || self.navigation == 0
            || !seconds(self.setup_seconds)
            || self.setup_seconds == 0.
            || !seconds(self.start_seconds)
            || self.start_seconds == 0.
            || self.setup_seconds < self.start_seconds
            || !seconds(self.stop_seconds)
            || self.stop_seconds == 0.
            || !self.place_fraction.is_finite()
            || !(0. ..=1.).contains(&self.place_fraction)
            || !self.player_range.is_finite()
            || !(1. ..=1000.).contains(&self.player_range)
            || !self.damage.is_finite()
            || !(0. ..=100000.).contains(&self.damage)
            || !self.damage_radius.is_finite()
            || !(1. ..=2000.).contains(&self.damage_radius)
            || [&self.placed_sound, &self.armed_sound, &self.explosion_sound]
                .into_iter()
                .any(|s| !text(s))
            || self.programs.len() != 2
        {
            return Err("Invalid selected demolition assets".into());
        }
        let mut ids = std::collections::BTreeSet::new();
        let mut triggers = std::collections::BTreeSet::new();
        for p in &self.programs {
            if !["DemoUnderway", "BunkerDoorDestroyed"].contains(&p.trigger.as_str())
                || !triggers.insert(&p.trigger)
                || p.actions.is_empty()
                || p.actions.len() > 32
            {
                return Err("Invalid demolition script fragment".into());
            }
            for a in &p.actions {
                if !ids.insert(a.original)
                    || !((1752..=1760).contains(&a.original) || (2536..=2557).contains(&a.original))
                {
                    return Err("Invalid original demolition action".into());
                }
                let valid = match &a.operation {
                    Operation::Wait { seconds: v } => seconds(*v),
                    Operation::Sound { source, seconds: v } => text(source) && seconds(*v),
                    Operation::WaitArmed | Operation::IfExplodedEnd => true,
                    Operation::Emit { effect } => match effect {
                        Effect::Event { tag } | Effect::Tracker { tag } => text(tag),
                        Effect::Voice { .. } => true,
                        Effect::StopFactory { tag, .. } => text(tag),
                        Effect::ObjectiveCompleted { objective } => *objective == 5,
                        _ => false,
                    },
                };
                if !valid {
                    return Err("Invalid selected demolition operation".into());
                }
            }
            if p.trigger == "DemoUnderway"
                && !matches!(
                    p.actions.last().map(|a| &a.operation),
                    Some(Operation::Emit {
                        effect: Effect::Voice { enabled: true }
                    })
                )
            {
                return Err("Demolition dialogue must restore AI voice".into());
            }
        }
        Ok(())
    }
}
#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, serde::Deserialize)]
pub enum Phase {
    Disabled,
    Available,
    Moving,
    Starting,
    Setting,
    Stopping,
    Armed,
    Exploded,
}
#[derive(Clone, Debug, Default, PartialEq, Serialize, serde::Deserialize)]
pub struct Lane {
    pub started: bool,
    pub next: usize,
    pub remaining: f64,
}
#[derive(Clone, Debug, PartialEq, Serialize, serde::Deserialize)]
pub struct State {
    pub phase: Phase,
    pub participant: Option<u32>,
    pub placed: bool,
    pub elapsed: f64,
    pub setup_elapsed: f64,
    pub objective_completed: bool,
    pub plunger_selected: bool,
    pub lanes: Vec<Lane>,
}
impl State {
    pub fn new(a: &Assets) -> Result<Self, String> {
        a.validate()?;
        Ok(Self {
            phase: Phase::Disabled,
            participant: None,
            placed: false,
            elapsed: 0.,
            setup_elapsed: 0.,
            objective_completed: false,
            plunger_selected: false,
            lanes: vec![Lane::default(); a.programs.len()],
        })
    }
    fn validate(&self, a: &Assets) -> Result<(), String> {
        a.validate()?;
        if !seconds(self.elapsed)
            || !seconds(self.setup_elapsed)
            || self.setup_elapsed > a.setup_seconds + 1e-7
            || self.lanes.len() != a.programs.len()
            || self.lanes.iter().zip(&a.programs).any(|(l, p)| {
                l.next > p.actions.len()
                    || !seconds(l.remaining)
                    || !l.started && (l.next != 0 || l.remaining != 0.)
            })
            || matches!(
                self.phase,
                Phase::Moving
                    | Phase::Starting
                    | Phase::Setting
                    | Phase::Stopping
                    | Phase::Armed
                    | Phase::Exploded
            ) && self.participant != Some(62)
            || matches!(
                self.phase,
                Phase::Setting | Phase::Stopping | Phase::Armed | Phase::Exploded
            ) && !self.placed
            || self.plunger_selected && !matches!(self.phase, Phase::Stopping | Phase::Armed)
            || self.objective_completed && self.phase != Phase::Exploded
        {
            return Err("Invalid demolition state".into());
        }
        Ok(())
    }
    pub fn enable(&mut self, a: &Assets, squad_joined: bool) -> Result<bool, String> {
        self.validate(a)?;
        if !squad_joined || self.phase != Phase::Disabled {
            return Ok(false);
        }
        self.phase = Phase::Available;
        Ok(true)
    }
    /// Caller must establish that the player targets the enabled original marker.
    pub fn command(
        &mut self,
        a: &Assets,
        member: u32,
        targeted: bool,
    ) -> Result<Vec<Effect>, String> {
        self.validate(a)?;
        if self.phase != Phase::Available || member != 62 || !targeted {
            return Ok(vec![]);
        }
        self.phase = Phase::Moving;
        self.participant = Some(member);
        Ok(vec![Effect::Event {
            tag: "SpawnDemoDroids".into(),
        }])
    }
    /// Arrival must come from the collision-checked companion controller.
    pub fn arrived(&mut self, a: &Assets, member: u32) -> Result<bool, String> {
        self.validate(a)?;
        if self.phase != Phase::Moving || self.participant != Some(member) {
            return Ok(false);
        }
        self.phase = Phase::Starting;
        self.elapsed = 0.;
        Ok(true)
    }
    fn place(&mut self, a: &Assets) -> Vec<Effect> {
        self.placed = true;
        let lane = a
            .programs
            .iter()
            .position(|p| p.trigger == "DemoUnderway")
            .unwrap();
        self.lanes[lane].started = true;
        vec![
            Effect::Sound {
                source: a.placed_sound.clone(),
            },
            Effect::Event {
                tag: "DemoUnderway".into(),
            },
        ]
    }
    /// Original MDplaceCharge notify is accepted once, only from the participant.
    pub fn notify(&mut self, a: &Assets, member: u32, event: &str) -> Result<Vec<Effect>, String> {
        self.validate(a)?;
        if self.phase != Phase::Starting
            || self.participant != Some(member)
            || event != "MDplaceCharge"
            || self.placed
        {
            return Ok(vec![]);
        }
        Ok(self.place(a))
    }
    /// Targeting selects the plunger within PlayerRange. HandSignal::FireWeapon
    /// subsequently detonates its retained TargetActivate even after retreating.
    pub fn select_plunger(&mut self, a: &Assets, distance: f64) -> Result<bool, String> {
        self.validate(a)?;
        if !distance.is_finite() || distance < 0. {
            return Err("Invalid charge targeting distance".into());
        }
        if !matches!(self.phase, Phase::Stopping | Phase::Armed) || distance > a.player_range {
            return Ok(false);
        }
        self.plunger_selected = true;
        Ok(true)
    }
    /// No timer or incoming weapon damage detonates this selected original charge.
    pub fn detonate(&mut self, a: &Assets, player_fire: bool) -> Result<Vec<Effect>, String> {
        self.validate(a)?;
        if !matches!(self.phase, Phase::Stopping | Phase::Armed)
            || !self.plunger_selected
            || !player_fire
        {
            return Ok(vec![]);
        }
        self.phase = Phase::Exploded;
        self.plunger_selected = false;
        self.elapsed = 0.;
        let lane = a
            .programs
            .iter()
            .position(|p| p.trigger == "BunkerDoorDestroyed")
            .unwrap();
        self.lanes[lane].started = true;
        let mut effects = vec![
            Effect::Sound {
                source: a.explosion_sound.clone(),
            },
            Effect::Event {
                tag: "BunkerDoorDestroyed".into(),
            },
            Effect::Blast {
                damage: a.damage,
                radius: a.damage_radius,
            },
            Effect::DestroyProp { original: a.prop },
            Effect::UnblockNavigation {
                original: a.navigation,
            },
        ];
        effects.extend(self.scripts(a, 0.)?);
        Ok(effects)
    }
    fn scripts(&mut self, a: &Assets, dt: f64) -> Result<Vec<Effect>, String> {
        let mut out = vec![];
        for (l, p) in self.lanes.iter_mut().zip(&a.programs) {
            if !l.started {
                continue;
            }
            let mut budget = dt;
            while l.next < p.actions.len() {
                if l.remaining > 0. {
                    let consumed = budget.min(l.remaining);
                    budget -= consumed;
                    l.remaining -= consumed;
                    if l.remaining > 1e-9 {
                        break;
                    }
                    l.remaining = 0.;
                }
                let op = &p.actions[l.next].operation;
                if matches!(op, Operation::WaitArmed)
                    && !matches!(self.phase, Phase::Stopping | Phase::Armed | Phase::Exploded)
                {
                    break;
                }
                l.next += 1;
                match op {
                    Operation::Wait { seconds } => l.remaining = *seconds,
                    Operation::Sound { source, seconds } => {
                        out.push(Effect::Sound {
                            source: source.clone(),
                        });
                        l.remaining = *seconds;
                    }
                    Operation::IfExplodedEnd if self.phase == Phase::Exploded => {
                        l.next = p.actions.len() - 1
                    }
                    Operation::Emit { effect } => {
                        if matches!(effect, Effect::ObjectiveCompleted { objective: 5 }) {
                            self.objective_completed = true;
                        }
                        out.push(effect.clone());
                    }
                    _ => {}
                }
                if budget <= 0. && l.remaining > 0. {
                    break;
                }
            }
        }
        Ok(out)
    }
    pub fn advance(&mut self, a: &Assets, dt: f64) -> Result<Vec<Effect>, String> {
        self.validate(a)?;
        if !dt.is_finite() || !(0. ..=1.).contains(&dt) {
            return Err("Invalid demolition delta".into());
        }
        // Split at all phase/notify boundaries so dialogue receives only the time
        // after its event and setup starts at the original notify, not clip end.
        let mut left = dt;
        let mut out = vec![];
        loop {
            let boundary = match self.phase {
                Phase::Starting if !self.placed => {
                    (a.start_seconds * a.place_fraction - self.elapsed).max(0.)
                }
                Phase::Starting => (a.start_seconds - self.elapsed)
                    .max(0.)
                    .min((a.setup_seconds - self.setup_elapsed).max(0.)),
                Phase::Setting => (a.setup_seconds - self.setup_elapsed).max(0.),
                Phase::Stopping => (a.stop_seconds - self.elapsed).max(0.),
                _ => left,
            };
            let part = left.min(boundary);
            if self.phase == Phase::Starting || self.phase == Phase::Stopping {
                self.elapsed += part;
            }
            if self.placed && matches!(self.phase, Phase::Starting | Phase::Setting) {
                self.setup_elapsed += part;
            }
            out.extend(self.scripts(a, part)?);
            left = (left - part).max(0.);
            match self.phase {
                Phase::Starting
                    if !self.placed
                        && self.elapsed + 1e-9 >= a.start_seconds * a.place_fraction =>
                {
                    out.extend(self.place(a))
                }
                Phase::Starting if self.elapsed + 1e-9 >= a.start_seconds => {
                    self.phase = Phase::Setting;
                    self.elapsed = 0.;
                }
                Phase::Setting if self.setup_elapsed + 1e-9 >= a.setup_seconds => {
                    self.setup_elapsed = a.setup_seconds;
                    self.phase = Phase::Stopping;
                    self.elapsed = 0.;
                    out.push(Effect::Sound {
                        source: a.armed_sound.clone(),
                    });
                    out.push(Effect::Event {
                        tag: "DemoChargeArmed".into(),
                    });
                }
                Phase::Stopping if self.elapsed + 1e-9 >= a.stop_seconds => {
                    self.phase = Phase::Armed;
                    self.elapsed = 0.;
                }
                _ => {}
            }
            if left <= 1e-10 {
                out.extend(self.scripts(a, 0.)?);
                break;
            }
        }
        Ok(out)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    fn assets() -> Assets {
        let emit = |original, effect| Action {
            original,
            operation: Operation::Emit { effect },
        };
        Assets {
            marker: 466,
            charge: 465,
            prop: 851,
            navigation: 1094,
            setup_seconds: 10.,
            start_seconds: 29. / 30.,
            place_fraction: 0.6272413730621338,
            stop_seconds: 1.3,
            player_range: 192.,
            damage: 15000.,
            damage_radius: 256.,
            placed_sound: "Placed".into(),
            armed_sound: "Armed".into(),
            explosion_sound: "Explode".into(),
            programs: vec![
                Program {
                    trigger: "DemoUnderway".into(),
                    actions: vec![
                        emit(2536, Effect::Voice { enabled: false }),
                        Action {
                            original: 2537,
                            operation: Operation::Sound {
                                source: "Underway".into(),
                                seconds: 2.,
                            },
                        },
                        Action {
                            original: 2539,
                            operation: Operation::WaitArmed,
                        },
                        Action {
                            original: 2540,
                            operation: Operation::Sound {
                                source: "Ready".into(),
                                seconds: 1.,
                            },
                        },
                        Action {
                            original: 2541,
                            operation: Operation::Wait { seconds: 5. },
                        },
                        Action {
                            original: 2542,
                            operation: Operation::IfExplodedEnd,
                        },
                        Action {
                            original: 2545,
                            operation: Operation::Sound {
                                source: "Reminder".into(),
                                seconds: 1.,
                            },
                        },
                        emit(2557, Effect::Voice { enabled: true }),
                    ],
                },
                Program {
                    trigger: "BunkerDoorDestroyed".into(),
                    actions: vec![
                        emit(
                            1752,
                            Effect::Event {
                                tag: "MegaForceBunkerWall".into(),
                            },
                        ),
                        emit(1755, Effect::ObjectiveCompleted { objective: 5 }),
                        Action {
                            original: 1759,
                            operation: Operation::Wait { seconds: 4. },
                        },
                        emit(
                            1760,
                            Effect::Event {
                                tag: "Conversation05".into(),
                            },
                        ),
                    ],
                },
            ],
        }
    }
    fn starting(a: &Assets) -> State {
        let mut s = State::new(a).unwrap();
        assert!(s.enable(a, true).unwrap());
        assert_eq!(
            s.command(a, 62, true).unwrap(),
            vec![Effect::Event {
                tag: "SpawnDemoDroids".into()
            }]
        );
        assert!(s.arrived(a, 62).unwrap());
        s
    }
    fn armed(a: &Assets) -> State {
        let mut s = starting(a);
        for _ in 0..15 {
            s.advance(a, 1.).unwrap();
        }
        assert_eq!(s.phase, Phase::Armed);
        s
    }
    #[test]
    fn command_needs_enabled_marker_squad_and_correct_participant() {
        let a = assets();
        let mut s = State::new(&a).unwrap();
        let before = s.clone();
        assert!(!s.enable(&a, false).unwrap());
        assert!(s.command(&a, 62, true).unwrap().is_empty());
        assert_eq!(s, before);
        s.enable(&a, true).unwrap();
        let before = s.clone();
        assert!(s.command(&a, 40, true).unwrap().is_empty());
        assert!(s.command(&a, 62, false).unwrap().is_empty());
        assert_eq!(s, before);
        s.command(&a, 62, true).unwrap();
        let before = s.clone();
        assert!(s.command(&a, 62, true).unwrap().is_empty());
        assert!(!s.arrived(&a, 40).unwrap());
        assert_eq!(s, before);
        assert!(s.arrived(&a, 62).unwrap());
    }
    #[test]
    fn travel_never_places_or_arms_before_physical_arrival() {
        let a = assets();
        let mut s = State::new(&a).unwrap();
        s.enable(&a, true).unwrap();
        s.command(&a, 62, true).unwrap();
        for _ in 0..60 {
            assert!(s.advance(&a, 1.).unwrap().is_empty());
        }
        assert_eq!(s.phase, Phase::Moving);
        assert!(!s.placed);
    }
    #[test]
    fn placement_notify_is_participant_only_and_once() {
        let a = assets();
        let mut s = starting(&a);
        let before = s.clone();
        assert!(s.notify(&a, 40, "MDplaceCharge").unwrap().is_empty());
        assert!(s.notify(&a, 62, "other").unwrap().is_empty());
        assert_eq!(s, before);
        assert_eq!(s.notify(&a, 62, "MDplaceCharge").unwrap().len(), 2);
        assert!(s.notify(&a, 62, "MDplaceCharge").unwrap().is_empty());
        let mut effects = vec![];
        for _ in 0..15 {
            effects.extend(s.advance(&a, 1.).unwrap());
        }
        assert!(!effects.contains(&Effect::Sound {
            source: "Placed".into()
        }));
    }
    #[test]
    fn setup_clock_starts_at_notify_and_arms_before_stop_clip_finishes() {
        let a = assets();
        let mut s = starting(&a);
        let notify = a.start_seconds * a.place_fraction;
        s.advance(&a, notify - 0.001).unwrap();
        assert!(!s.placed);
        s.advance(&a, 0.001).unwrap();
        assert!(s.placed);
        assert!(s.setup_elapsed < 1e-8);
        for _ in 0..9 {
            s.advance(&a, 1.).unwrap();
        }
        s.advance(&a, 0.999).unwrap();
        assert_eq!(s.phase, Phase::Setting);
        let e = s.advance(&a, 0.001).unwrap();
        assert_eq!(s.phase, Phase::Stopping);
        assert!(e.contains(&Effect::Event {
            tag: "DemoChargeArmed".into()
        }));
        assert!(e.contains(&Effect::Sound {
            source: "Ready".into()
        }));
    }
    #[test]
    fn armed_charge_waits_for_player_and_detonation_is_once() {
        let a = assets();
        let mut s = armed(&a);
        for _ in 0..120 {
            s.advance(&a, 1.).unwrap();
        }
        assert_eq!(s.phase, Phase::Armed);
        assert!(!s.objective_completed);
        let before = s.clone();
        assert!(s.detonate(&a, false).unwrap().is_empty());
        assert_eq!(s, before);
        assert!(s.detonate(&a, true).unwrap().is_empty());
        assert!(!s.select_plunger(&a, a.player_range + 1.).unwrap());
        assert!(s.select_plunger(&a, a.player_range).unwrap());
        let e = s.detonate(&a, true).unwrap();
        assert!(e.contains(&Effect::Blast {
            damage: 15000.,
            radius: 256.
        }));
        assert!(e.contains(&Effect::DestroyProp { original: 851 }));
        assert!(e.contains(&Effect::UnblockNavigation { original: 1094 }));
        assert!(s.objective_completed);
        assert!(s.detonate(&a, true).unwrap().is_empty());
        assert!(s.command(&a, 62, true).unwrap().is_empty());
    }
    #[test]
    fn detonating_during_stop_is_allowed_but_before_arming_is_not() {
        let a = assets();
        let mut s = starting(&a);
        assert!(s.detonate(&a, true).unwrap().is_empty());
        for _ in 0..10 {
            s.advance(&a, 1.).unwrap();
        }
        s.advance(&a, 0.7).unwrap();
        assert_eq!(s.phase, Phase::Stopping);
        s.select_plunger(&a, 100.).unwrap();
        assert!(!s.detonate(&a, true).unwrap().is_empty());
    }
    #[test]
    fn original_conditional_skips_reminders_after_explosion_and_restores_voice() {
        let a = assets();
        let mut s = starting(&a);
        for _ in 0..11 {
            s.advance(&a, 1.).unwrap();
        }
        s.select_plunger(&a, 100.).unwrap();
        let mut e = s.detonate(&a, true).unwrap();
        for _ in 0..12 {
            e.extend(s.advance(&a, 1.).unwrap());
        }
        assert!(!e.contains(&Effect::Sound {
            source: "Reminder".into()
        }));
        assert_eq!(
            e.iter()
                .filter(|e| **e == Effect::Voice { enabled: true })
                .count(),
            1
        );
        assert_eq!(
            e.iter()
                .filter(|e| **e
                    == Effect::Event {
                        tag: "Conversation05".into()
                    })
                .count(),
            1
        );
    }
    #[test]
    fn phase_and_effects_are_independent_of_frame_partition() {
        let a = assets();
        let mut x = starting(&a);
        let mut y = x.clone();
        let mut xe = vec![];
        let mut ye = vec![];
        for _ in 0..20 {
            xe.extend(x.advance(&a, 1.).unwrap());
        }
        for _ in 0..2880 {
            ye.extend(y.advance(&a, 1. / 144.).unwrap());
        }
        assert_eq!(xe, ye);
        assert_eq!(x.phase, y.phase);
        assert_eq!(x.lanes, y.lanes);
        assert!((x.setup_elapsed - y.setup_elapsed).abs() < 1e-8);
    }
    #[test]
    fn invalid_inputs_assets_and_state_leave_state_unchanged() {
        let a = assets();
        let mut s = starting(&a);
        let before = s.clone();
        for dt in [-1., f64::NAN, f64::INFINITY, 1.01] {
            assert!(s.advance(&a, dt).is_err());
            assert_eq!(s, before);
        }
        assert!(s.select_plunger(&a, f64::NAN).is_err());
        assert_eq!(s, before);
        let mut bad = a.clone();
        bad.programs[0].actions[0].original = 1752;
        assert!(s.advance(&bad, 0.).is_err());
        assert_eq!(s, before);
        s.lanes[0].next = 999;
        let before = s.clone();
        assert!(s.advance(&a, 0.).is_err());
        assert_eq!(s, before);
    }
    #[test]
    fn plunger_selection_does_not_fire_and_survives_retreat() {
        let a = assets();
        let mut s = armed(&a);
        assert!(s.select_plunger(&a, 100.).unwrap());
        for _ in 0..30 {
            s.advance(&a, 1.).unwrap();
        }
        assert_eq!(s.phase, Phase::Armed);
        assert!(!s.select_plunger(&a, 1000.).unwrap());
        assert!(s.plunger_selected);
        assert!(!s.detonate(&a, true).unwrap().is_empty());
    }
}
