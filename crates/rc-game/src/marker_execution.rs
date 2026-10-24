//! Selected B marker execution, enabled only by audited work-site metadata.
//! Work duration and arbitration remain port rules, not native GoalQueue parity.
use crate::{map_gameplay::Interaction, marker_orders::owner, Point};
use serde::{Deserialize, Serialize};
#[derive(Clone, Debug, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Site {
    pub marker: u32,
    pub anchor: u32,
    pub position: Point,
    pub yaw: f64,
}
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Task {
    pub interaction: usize,
    pub paused: bool,
}
#[derive(Clone, Debug, Default, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Execution {
    pub held: bool,
    pub tasks: [Option<Task>; 2],
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub yield_to: Option<Point>,
}
impl Execution {
    pub fn command(
        &mut self,
        jobs: &[Interaction],
        finished: &[bool],
        ready: [bool; 2],
        selected: Option<usize>,
        use_action: bool,
    ) -> [bool; 2] {
        let pressed = use_action && !self.held;
        self.held = use_action;
        let mut changed = [false; 2];
        for (p, changed) in changed.iter_mut().enumerate() {
            if let Some(t) = &mut self.tasks[p] {
                if finished[t.interaction] {
                    self.tasks[p] = None;
                    *changed = true;
                } else if !ready[p] && !t.paused {
                    t.paused = true;
                    *changed = true;
                }
            }
        }
        if pressed {
            if let Some(i) = selected.filter(|i| !finished[*i] && ready[owner(&jobs[*i])]) {
                let p = owner(&jobs[i]);
                let paused = self.tasks[p]
                    .as_ref()
                    .is_some_and(|t| t.interaction == i && !t.paused);
                self.tasks[p] = Some(Task {
                    interaction: i,
                    paused,
                });
                changed[p] = true;
            }
        }
        if self.target(1).is_none() || self.target(0).is_some() {
            self.yield_to = None;
        }
        changed
    }
    pub fn target(&self, p: usize) -> Option<usize> {
        self.tasks[p]
            .as_ref()
            .filter(|t| !t.paused)
            .map(|t| t.interaction)
    }
    pub fn validate(&self, jobs: &[Interaction], finished: &[bool]) -> Result<(), String> {
        if self.yield_to.is_some_and(|p| {
            self.target(1).is_none()
                || self.target(0).is_some()
                || p.iter().any(|v| !v.is_finite() || v.abs() > 1e7)
        }) {
            return Err("Invalid marker yielding state".into());
        }
        for (p, t) in self.tasks.iter().enumerate() {
            if let Some(t) = t {
                if t.interaction >= jobs.len()
                    || owner(&jobs[t.interaction]) != p
                    || finished[t.interaction]
                {
                    return Err("Invalid autonomous marker task".into());
                }
            }
        }
        Ok(())
    }
}
pub(crate) fn validate_state(game: &crate::Game, state: &crate::State) -> Result<(), String> {
    if let Some(s) = state.map.as_ref().and_then(|m| m.gameplay.as_ref()) {
        if let Some(e) = &s.execution {
            let a = game
                .map
                .as_ref()
                .and_then(|m| m.gameplay.as_ref())
                .ok_or("Missing marker execution assets")?;
            if a.work_sites.is_none() {
                return Err("Autonomous task without work sites".into());
            }
            let finished: Vec<_> = s.armed.iter().zip(&s.used).map(|(a, u)| *a || *u).collect();
            e.validate(&a.assets.interactions, &finished)?;
        }
    }
    Ok(())
}
#[cfg(test)]
mod tests {
    use super::*;
    fn jobs() -> Vec<Interaction> {
        [true, false, true]
            .into_iter()
            .enumerate()
            .map(|(i, d)| Interaction {
                original: i as u32,
                actor: String::new(),
                label: String::new(),
                position: [0.; 3],
                prerequisite: String::new(),
                underway: String::new(),
                event: String::new(),
                demolition: d,
                seconds: 3.,
            })
            .collect()
    }
    #[test]
    fn one_press_assigns_and_release_keeps_running() {
        let j = jobs();
        let mut e = Execution::default();
        e.command(&j, &[false; 3], [true; 2], Some(0), true);
        e.command(&j, &[false; 3], [true; 2], None, false);
        assert_eq!(e.target(0), Some(0));
    }
    #[test]
    fn held_input_never_repeats_or_replaces_command() {
        let j = jobs();
        let mut e = Execution::default();
        e.command(&j, &[false; 3], [true; 2], Some(0), true);
        e.command(&j, &[false; 3], [true; 2], Some(2), true);
        assert_eq!(e.target(0), Some(0));
    }
    #[test]
    fn repeated_press_toggles_pause_and_resume() {
        let j = jobs();
        let mut e = Execution::default();
        for paused in [false, true, false] {
            e.command(&j, &[false; 3], [true; 2], None, false);
            e.command(&j, &[false; 3], [true; 2], Some(0), true);
            assert_eq!(e.tasks[0].as_ref().unwrap().paused, paused);
        }
    }
    #[test]
    fn each_member_has_one_replaceable_task() {
        let j = jobs();
        let mut e = Execution::default();
        for i in [0, 1, 2] {
            e.command(&j, &[false; 3], [true; 2], None, false);
            e.command(&j, &[false; 3], [true; 2], Some(i), true);
        }
        assert_eq!(e.target(0), Some(2));
        assert_eq!(e.target(1), Some(1));
    }
    #[test]
    fn unavailability_pauses_and_does_not_auto_resume() {
        let j = jobs();
        let mut e = Execution::default();
        e.command(&j, &[false; 3], [true; 2], Some(0), true);
        e.command(&j, &[false; 3], [false, true], None, false);
        e.command(&j, &[false; 3], [true; 2], None, false);
        assert_eq!(e.target(0), None);
        e.command(&j, &[false; 3], [true; 2], Some(0), true);
        assert_eq!(e.target(0), Some(0));
    }
    #[test]
    fn finished_tasks_release_without_reclaiming_old_progress() {
        let j = jobs();
        let mut e = Execution::default();
        e.command(&j, &[false; 3], [true; 2], Some(0), true);
        e.command(&j, &[true, false, false], [true; 2], None, false);
        assert_eq!(e.tasks, [None, None]);
    }
    #[test]
    fn invalid_member_and_finished_reservation_rejected() {
        let j = jobs();
        let mut e = Execution::default();
        e.tasks[1] = Some(Task {
            interaction: 0,
            paused: false,
        });
        assert!(e.validate(&j, &[false; 3]).is_err());
        e.tasks[1] = None;
        e.tasks[0] = Some(Task {
            interaction: 0,
            paused: false,
        });
        assert!(e.validate(&j, &[true, false, false]).is_err());
    }
    #[test]
    fn saved_yield_survives_restore_and_clears_on_pause() {
        let j = jobs();
        let mut e = Execution::default();
        e.command(&j, &[false; 3], [true; 2], Some(1), true);
        e.yield_to = Some([10., 20., 30.]);
        e.validate(&j, &[false; 3]).unwrap();
        let cold: Execution = serde_json::from_slice(&serde_json::to_vec(&e).unwrap()).unwrap();
        assert_eq!(cold, e);
        e.command(&j, &[false; 3], [true; 2], None, false);
        e.command(&j, &[false; 3], [true; 2], Some(1), true);
        assert_eq!(e.yield_to, None);
    }
    #[test]
    fn yield_requires_active_fixer_and_finite_target() {
        let j = jobs();
        let mut e = Execution {
            yield_to: Some([1., 2., 3.]),
            ..Default::default()
        };
        assert!(e.validate(&j, &[false; 3]).is_err());
        e.command(&j, &[false; 3], [true; 2], Some(1), true);
        e.yield_to = Some([f64::NAN, 2., 3.]);
        assert!(e.validate(&j, &[false; 3]).is_err());
    }
    #[test]
    fn cold_approach_and_pause_roundtrip() {
        let j = jobs();
        let mut e = Execution::default();
        e.command(&j, &[false; 3], [true; 2], Some(0), true);
        let mut cold: Execution = serde_json::from_slice(&serde_json::to_vec(&e).unwrap()).unwrap();
        assert_eq!(cold, e);
        cold.command(&j, &[false; 3], [true; 2], None, false);
        assert_eq!(cold.target(0), Some(0));
        cold.validate(&j, &[false; 3]).unwrap();
    }
}
