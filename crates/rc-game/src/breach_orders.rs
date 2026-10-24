//! Selected C breach command lifecycle; explicit port policy, not native marker GoalQueue.
use serde::{Deserialize, Serialize};
#[derive(Clone, Debug, Default, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Control {
    pub held: bool,
    pub paused: bool,
    pub await_release: bool,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub revival: Option<Revival>,
}
#[derive(Clone, Debug, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Revival {
    pub member: usize,
    pub elapsed: f64,
}
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Action {
    None,
    Start,
    Cancel,
    Pause,
    Resume,
}
impl Control {
    pub fn legacy(held: bool) -> Self {
        Self {
            held,
            ..Default::default()
        }
    }
    pub fn update(
        &mut self,
        stage: u8,
        ready: bool,
        usable: bool,
        use_action: bool,
        inhibited: bool,
    ) -> Action {
        let pressed = use_action && !self.held;
        self.held = use_action;
        if !use_action {
            self.await_release = false;
        }
        if matches!(stage, 1 | 2) && !ready {
            self.paused = true;
        }
        if inhibited || !usable {
            return Action::None;
        }
        if stage == 0 && ready && use_action && !self.await_release {
            return Action::Start;
        }
        if self.paused {
            if pressed && ready {
                self.paused = false;
                return Action::Resume;
            }
            if pressed && stage == 1 {
                self.paused = false;
                self.await_release = true;
                return Action::Cancel;
            }
        } else if pressed {
            if stage == 1 {
                self.await_release = true;
                return Action::Cancel;
            }
            if stage == 2 {
                self.paused = true;
                return Action::Pause;
            }
        }
        Action::None
    }
    pub fn validate(&self, stage: u8) -> Result<(), String> {
        if stage > 3
            || self.paused && !matches!(stage, 1 | 2)
            || self.await_release && (stage != 0 || !self.held || self.paused)
            || self.revival.as_ref().is_some_and(|r| {
                r.member >= 3
                    || !r.elapsed.is_finite()
                    || !(0. ..3.).contains(&r.elapsed)
                    || !self.held
            })
        {
            return Err("Invalid breach command checkpoint".into());
        }
        Ok(())
    }
}
pub fn state(s: &crate::State) -> Option<&crate::map3_breach::State> {
    s.map
        .as_ref()?
        .opening
        .as_ref()?
        .planning
        .as_ref()?
        .guard
        .as_ref()?
        .breach
        .as_deref()
}
pub(crate) fn validate_state(game: &crate::Game, s: &crate::State) -> Result<(), String> {
    if let Some(b) = state(s) {
        if game.breach_prepared().is_none()
            || b.stage > 3
            || b.animations.len() != 3
            || !b.door_fraction.is_finite()
            || !(0. ..=1.).contains(&b.door_fraction)
        {
            return Err("Invalid breach checkpoint state".into());
        }
        if let Some(c) = &b.control {
            c.validate(b.stage)?;
        }
    }
    Ok(())
}
#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn held_original_command_starts_once_without_cancelling_formation() {
        let mut c = Control::default();
        assert_eq!(c.update(0, true, true, true, false), Action::Start);
        assert_eq!(c.update(1, true, true, true, false), Action::None);
        assert_eq!(c.update(2, true, true, true, false), Action::None);
    }
    #[test]
    fn formation_cancel_waits_for_release_before_reassignment() {
        let mut c = Control::default();
        assert_eq!(c.update(1, true, true, true, false), Action::Cancel);
        assert_eq!(c.update(0, true, true, true, false), Action::None);
        c.update(0, true, true, false, false);
        assert_eq!(c.update(0, true, true, true, false), Action::Start);
    }
    #[test]
    fn animation_can_pause_and_resume_but_never_cancel() {
        let mut c = Control::default();
        assert_eq!(c.update(2, true, true, true, false), Action::Pause);
        c.update(2, true, true, false, false);
        assert_eq!(c.update(2, true, true, true, false), Action::Resume);
        assert!(!c.paused);
    }
    #[test]
    fn loss_of_participant_requires_explicit_ready_resume() {
        let mut c = Control::default();
        assert_eq!(c.update(2, false, false, false, false), Action::None);
        assert!(c.paused);
        assert_eq!(c.update(2, true, true, false, false), Action::None);
        assert_eq!(c.update(2, true, true, true, false), Action::Resume);
    }
    #[test]
    fn unavailable_formation_can_be_cancelled_but_started_animation_cannot() {
        let mut c = Control::default();
        assert_eq!(c.update(1, false, true, true, false), Action::Cancel);
        let mut c = Control::default();
        assert_eq!(c.update(2, false, true, true, false), Action::None);
        assert!(c.paused);
    }
    #[test]
    fn healing_priority_and_legacy_held_input_do_not_toggle_commands() {
        let mut c = Control::default();
        assert_eq!(c.update(2, true, true, true, true), Action::None);
        assert_eq!(c.update(2, true, true, true, false), Action::None);
        let mut old = Control::legacy(true);
        assert_eq!(old.update(2, true, true, true, false), Action::None);
    }
    #[test]
    fn cold_control_restore_and_invalid_metadata() {
        let mut c = Control::default();
        c.update(2, false, false, false, false);
        let mut copy: Control = serde_json::from_slice(&serde_json::to_vec(&c).unwrap()).unwrap();
        copy.validate(2).unwrap();
        assert_eq!(copy.update(2, true, true, true, false), Action::Resume);
        copy.paused = true;
        assert!(copy.validate(0).is_err());
        copy.paused = false;
        copy.revival = Some(Revival {
            member: 3,
            elapsed: 1.,
        });
        assert!(copy.validate(2).is_err());
    }
}
