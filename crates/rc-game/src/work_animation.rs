//! Original work poses sampled from retained progress. Timing adaptation is port policy.
use crate::visual::PreparedMesh;
#[derive(Clone, Debug)]
pub struct Prepared {
    pub clips: [usize; 3],
    pub seconds: [f64; 3],
    pub work: f64,
    pub cycles: u32,
}
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Sample {
    pub clip: usize,
    pub frame: f32,
    pub phase: usize,
}
impl Prepared {
    pub fn new(mesh: &PreparedMesh, names: &[String; 3], work: f64) -> Result<Self, String> {
        let clips = [
            mesh.clip(&names[0])?,
            mesh.clip(&names[1])?,
            mesh.clip(&names[2])?,
        ];
        let seconds = clips.map(|i| {
            let s = &mesh.mesh.clips[i].sequence;
            s.frames as f64 / f32::from_bits(s.rate_bits) as f64
        });
        Self::timed(clips, seconds, work)
    }
    fn timed(clips: [usize; 3], seconds: [f64; 3], work: f64) -> Result<Self, String> {
        if !work.is_finite()
            || seconds.iter().any(|s| !s.is_finite() || *s <= 0.)
            || work <= seconds[0] + seconds[2] + 1e-6
        {
            return Err("Invalid work animation timing".into());
        }
        let cycles = ((work - seconds[0] - seconds[2]) / seconds[1]).ceil() as u32;
        if cycles == 0 || cycles > 256 {
            return Err("Work animation cycle budget".into());
        }
        Ok(Self {
            clips,
            seconds,
            work,
            cycles,
        })
    }
    pub fn sample(&self, progress: f64) -> Option<Sample> {
        if !progress.is_finite() || progress <= 0. || progress >= self.work {
            return None;
        }
        let (phase, frame) = if progress < self.seconds[0] {
            (0, progress / self.seconds[0])
        } else if progress >= self.work - self.seconds[2] {
            (
                2,
                (progress - (self.work - self.seconds[2])) / self.seconds[2],
            )
        } else {
            (
                1,
                ((progress - self.seconds[0]) / (self.work - self.seconds[0] - self.seconds[2])
                    * self.cycles as f64)
                    .rem_euclid(1.),
            )
        };
        Some(Sample {
            clip: self.clips[phase],
            frame: (frame as f32).clamp(0., 1.),
            phase,
        })
    }
}
impl crate::Game {
    pub fn map_work_pose(&self, member: usize) -> Option<Sample> {
        let p = self.map.as_ref()?.gameplay.as_ref()?;
        let m = self.state.map.as_ref()?;
        let s = m.gameplay.as_ref()?;
        let t = s.execution.as_ref()?.tasks.get(member)?.as_ref()?;
        let c = if member == 0 {
            m.companion.as_ref()?
        } else {
            &s.fixer
        };
        if !c.joined
            || c.down
            || c.health == 0
            || c.moving
            || crate::distance(
                c.position,
                p.work_sites.as_ref()?.get(t.interaction)?.position,
            ) > 16.
        {
            return None;
        }
        p.work_animations
            .as_ref()?
            .get(t.interaction)?
            .sample(s.progress[t.interaction])
    }
}
#[cfg(test)]
mod tests {
    use super::*;
    fn demo() -> Prepared {
        Prepared::timed([10, 11, 12], [29. / 30., 2., 39. / 30.], 3.).unwrap()
    }
    #[test]
    fn original_start_loop_stop_are_selected_in_order() {
        let p = demo();
        assert_eq!(p.sample(0.1).unwrap().clip, 10);
        assert_eq!(p.sample(1.1).unwrap().clip, 11);
        assert_eq!(p.sample(2.).unwrap().clip, 12);
        assert_eq!(p.sample(3.), None);
    }
    #[test]
    fn clock_outside_work_never_produces_a_pose() {
        let p = demo();
        for t in [0., -1., f64::NAN, f64::INFINITY, 3., 4.] {
            assert_eq!(p.sample(t), None);
        }
    }
    #[test]
    fn paused_progress_keeps_the_same_pose() {
        let p = demo();
        let a = p.sample(1.2);
        for _ in 0..1000 {
            assert_eq!(p.sample(1.2), a);
        }
    }
    #[test]
    fn natural_transition_durations_and_integer_loops_fit_retained_work() {
        let p = demo();
        assert_eq!(p.cycles, 1);
        assert_eq!(p.sample(p.seconds[0]).unwrap().frame, 0.);
        assert_eq!(p.sample(p.work - p.seconds[2]).unwrap().frame, 0.);
        assert_eq!(p.sample(p.work - p.seconds[2]).unwrap().phase, 2);
        let console = Prepared::timed([0, 1, 2], [1.4, 19. / 30., 31. / 30.], 6.).unwrap();
        assert_eq!(console.cycles, 6);
    }
    #[test]
    fn invalid_or_excessively_retimed_bindings_are_rejected() {
        for (s, w) in [
            ([1., 0., 1.], 6.),
            ([1., 1., 1.], 2.),
            ([1., 0.000001, 1.], 6.),
            ([f64::NAN, 1., 1.], 6.),
        ] {
            assert!(Prepared::timed([0, 1, 2], s, w).is_err());
        }
    }
}
