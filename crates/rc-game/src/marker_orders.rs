//! Bounded marker reservation and retained work for the selected map host.
//! Holding Use still drives work; this is not native GoalQueue/marker execution.
use crate::map_gameplay::Interaction;
use serde::{Deserialize, Serialize};
#[derive(Clone, Debug, Default, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Orders {
    /// Scorch demolition, Fixer slice. At most one reservation per participant.
    pub reserved: [Option<usize>; 2],
    pub working: Option<usize>,
}
pub fn owner(job: &Interaction) -> usize {
    usize::from(!job.demolition)
}
fn validate_work(jobs: &[Interaction], progress: &[f64], finished: &[bool]) -> Result<(), String> {
    if jobs.len() != progress.len()
        || jobs.len() != finished.len()
        || jobs.iter().zip(progress).any(|(j, p)| {
            !j.seconds.is_finite()
                || j.seconds <= 0.
                || !p.is_finite()
                || *p < 0.
                || *p > j.seconds + 1e-7
        })
    {
        return Err("Invalid marker work progress".into());
    }
    Ok(())
}
impl Orders {
    pub fn validate(
        &self,
        jobs: &[Interaction],
        progress: &[f64],
        finished: &[bool],
    ) -> Result<(), String> {
        validate_work(jobs, progress, finished)?;
        for (participant, index) in self.reserved.iter().enumerate() {
            if let Some(i) = index {
                if *i >= jobs.len()
                    || owner(&jobs[*i]) != participant
                    || finished[*i]
                    || progress[*i] <= 0.
                {
                    return Err("Invalid marker reservation".into());
                }
            }
        }
        if self
            .working
            .is_some_and(|i| i >= jobs.len() || self.reserved[owner(&jobs[i])] != Some(i))
        {
            return Err("Invalid working marker".into());
        }
        Ok(())
    }
}
#[derive(Debug, PartialEq)]
pub struct Update {
    pub interaction: usize,
    pub started: bool,
    pub completed: bool,
}
/// Only the selected site can receive work. Switching a reservation preserves
/// the old site's partial progress; readiness/release pause without resetting it.
pub fn step(
    state: &mut Option<Orders>,
    jobs: &[Interaction],
    progress: &mut [f64],
    finished: &[bool],
    ready: [bool; 2],
    selected: Option<usize>,
    dt: f64,
) -> Result<Option<Update>, String> {
    validate_work(jobs, progress, finished)?;
    if !dt.is_finite() || !(0. ..=1.).contains(&dt) || selected.is_some_and(|i| i >= jobs.len()) {
        return Err("Invalid marker input".into());
    }
    if let Some(s) = state.as_ref() {
        s.validate(jobs, progress, finished)?;
    }
    if dt == 0. {
        return Ok(None);
    }
    let mut next = state.clone().unwrap_or_default();
    next.working = None;
    // Adopt old partial checkpoints lazily, without rewriting them while loading.
    if state.is_none() {
        for (i, j) in jobs.iter().enumerate() {
            if progress[i] > 0. && !finished[i] && next.reserved[owner(j)].is_none() {
                next.reserved[owner(j)] = Some(i);
            }
        }
    }
    let mut result = None;
    if let Some(i) = selected.filter(|i| !finished[*i] && ready[owner(&jobs[*i])]) {
        let participant = owner(&jobs[i]);
        let started = progress[i] == 0.;
        progress[i] = (progress[i] + dt).min(jobs[i].seconds);
        let completed = progress[i] + 1e-8 >= jobs[i].seconds;
        if completed {
            progress[i] = jobs[i].seconds;
            next.reserved[participant] = None;
        } else {
            next.reserved[participant] = Some(i);
            next.working = Some(i);
        }
        result = Some(Update {
            interaction: i,
            started,
            completed,
        });
    }
    *state = (next.reserved.iter().any(Option::is_some)
        || progress.iter().zip(finished).any(|(p, f)| *p > 0. && !*f))
    .then_some(next);
    Ok(result)
}
pub(crate) fn validate_state(game: &crate::Game, state: &crate::State) -> Result<(), String> {
    if let Some(s) = state.map.as_ref().and_then(|m| m.gameplay.as_ref()) {
        let a = &game
            .map
            .as_ref()
            .and_then(|m| m.gameplay.as_ref())
            .ok_or("Missing marker checkpoint assets")?
            .assets;
        let finished: Vec<_> = s.armed.iter().zip(&s.used).map(|(a, u)| *a || *u).collect();
        validate_work(&a.interactions, &s.progress, &finished)?;
        if let Some(orders) = &s.orders {
            orders.validate(&a.interactions, &s.progress, &finished)?;
        }
    }
    Ok(())
}
#[cfg(test)]
mod tests {
    use super::*;
    fn jobs() -> Vec<Interaction> {
        [true, false, true]
            .iter()
            .enumerate()
            .map(|(i, d)| Interaction {
                original: i as u32 + 1,
                actor: i.to_string(),
                label: i.to_string(),
                position: [0.; 3],
                prerequisite: "Ready".into(),
                underway: "Start".into(),
                event: "Done".into(),
                demolition: *d,
                seconds: 3.,
            })
            .collect()
    }
    #[test]
    fn releasing_or_unavailable_participant_retains_work_and_reservation() {
        let j = jobs();
        let mut p = vec![0.; 3];
        let f = vec![false; 3];
        let mut s = None;
        step(&mut s, &j, &mut p, &f, [true; 2], Some(0), 1.).unwrap();
        step(&mut s, &j, &mut p, &f, [true; 2], None, 1.).unwrap();
        assert_eq!(p[0], 1.);
        assert_eq!(s.as_ref().unwrap().working, None);
        step(&mut s, &j, &mut p, &f, [false, true], Some(0), 1.).unwrap();
        assert_eq!(p[0], 1.);
        assert_eq!(s.as_ref().unwrap().reserved[0], Some(0));
        step(&mut s, &j, &mut p, &f, [true; 2], Some(0), 1.).unwrap();
        assert_eq!(p[0], 2.);
    }
    #[test]
    fn replacing_same_participant_reservation_keeps_old_work_and_other_member() {
        let j = jobs();
        let mut p = vec![0.; 3];
        let f = vec![false; 3];
        let mut s = None;
        for i in [0, 1, 2] {
            step(&mut s, &j, &mut p, &f, [true; 2], Some(i), 1.).unwrap();
        }
        assert_eq!(p, vec![1.; 3]);
        assert_eq!(s.as_ref().unwrap().reserved, [Some(2), Some(1)]);
        assert_eq!(s.as_ref().unwrap().working, Some(2));
        step(&mut s, &j, &mut p, &f, [true; 2], Some(0), 1.).unwrap();
        assert_eq!(p, vec![2., 1., 1.]);
        assert_eq!(s.as_ref().unwrap().reserved, [Some(0), Some(1)]);
    }
    #[test]
    fn completion_once_releases_reservation_and_does_not_advance_finished_job() {
        let j = jobs();
        let mut p = vec![0.; 3];
        let mut f = vec![false; 3];
        let mut s = None;
        let mut starts = 0;
        let mut completions = 0;
        for _ in 0..6 {
            if let Some(u) = step(&mut s, &j, &mut p, &f, [true; 2], Some(1), 1.).unwrap() {
                starts += usize::from(u.started);
                completions += usize::from(u.completed);
                f[u.interaction] = u.completed;
            }
        }
        assert_eq!((starts, completions), (1, 1));
        assert_eq!(p[1], 3.);
        assert!(s.is_none());
    }
    #[test]
    fn selected_job_is_the_only_one_that_can_receive_work() {
        let j = jobs();
        let mut p = vec![0.; 3];
        let f = vec![false; 3];
        let mut s = None;
        step(&mut s, &j, &mut p, &f, [true; 2], Some(2), 1.).unwrap();
        assert_eq!(p, vec![0., 0., 1.]);
    }
    #[test]
    fn finishing_replacement_does_not_silently_reassign_old_paused_work() {
        let j = jobs();
        let mut p = vec![0.; 3];
        let mut f = vec![false; 3];
        let mut s = None;
        step(&mut s, &j, &mut p, &f, [true; 2], Some(0), 1.).unwrap();
        for _ in 0..3 {
            if let Some(u) = step(&mut s, &j, &mut p, &f, [true; 2], Some(2), 1.).unwrap() {
                f[2] = u.completed;
            }
        }
        step(&mut s, &j, &mut p, &f, [true; 2], None, 1.).unwrap();
        assert_eq!(s.as_ref().unwrap().reserved, [None, None]);
        assert_eq!(p[0], 1.);
        let u = step(&mut s, &j, &mut p, &f, [true; 2], Some(0), 1.)
            .unwrap()
            .unwrap();
        assert!(!u.started);
        assert_eq!(p[0], 2.);
    }
    #[test]
    fn lazy_old_progress_adoption_and_cold_reservation_resume_are_exact() {
        let j = jobs();
        let mut p = vec![1., 0., 0.];
        let f = vec![false; 3];
        let mut s = None;
        step(&mut s, &j, &mut p, &f, [false; 2], None, 1.).unwrap();
        let mut copy: Option<Orders> =
            serde_json::from_slice(&serde_json::to_vec(&s).unwrap()).unwrap();
        assert_eq!(s, copy);
        let mut q = p.clone();
        let a = step(&mut s, &j, &mut p, &f, [true; 2], Some(0), 1.).unwrap();
        let b = step(&mut copy, &j, &mut q, &f, [true; 2], Some(0), 1.).unwrap();
        assert_eq!(a, b);
        assert_eq!((s, p), (copy, q));
    }
    #[test]
    fn invalid_owner_finished_reservation_and_bad_input_leave_work_unchanged() {
        let j = jobs();
        let mut p = vec![1., 0., 0.];
        let f = vec![false; 3];
        let bad = Orders {
            reserved: [None, Some(0)],
            working: None,
        };
        assert!(bad.validate(&j, &p, &f).is_err());
        let bad = Orders {
            reserved: [Some(0), None],
            working: Some(1),
        };
        assert!(bad.validate(&j, &p, &f).is_err());
        let mut s = Some(Orders {
            reserved: [Some(0), None],
            working: None,
        });
        assert!(s
            .as_ref()
            .unwrap()
            .validate(&j, &p, &[true, false, false])
            .is_err());
        for dt in [-1., f64::NAN, 2.] {
            let before = (s.clone(), p.clone());
            assert!(step(&mut s, &j, &mut p, &f, [true; 2], Some(0), dt).is_err());
            assert_eq!((s.clone(), p.clone()), before);
        }
    }
}
