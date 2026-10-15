//! Prepared UGOAL_MeleeAttack constructor/OnActivate/PreUpdate decisions.
//! ctgame.dll 10007d90, 100339a0, 10033a70. Caller supplies valid target,
//! squared distance, facing and navigation/animation queries; no full GoalQueue.
use serde::Serialize;

#[derive(Debug, Clone, Serialize, serde::Deserialize)]
pub struct State {
    pub remaining: i32,
    pub max_attack_time: f32,
    pub completion_time: f32,
    pub next_reachability: f32,
}
impl State {
    pub fn activate(time: f32, count: i32, max_attack_time: f32) -> Self {
        Self {
            remaining: count,
            max_attack_time,
            completion_time: time + max_attack_time,
            next_reachability: time + 1.,
        }
    }
    /// Reachability precedes attack; expiry follows attack. A newly started
    /// animation keeps the goal alive even if the last count was consumed.
    pub fn update(&mut self, time: f32, query: Query) -> (bool, bool) {
        if !query.target_valid {
            return (false, true);
        }
        if time > self.next_reachability {
            self.next_reachability = time + 1.;
            if query.distance_squared > query.range_squared && !query.reachable {
                return (false, true);
            }
        }
        let start = self.remaining != 0
            && query.distance_squared <= query.range_squared
            && query.facing > 0.5
            && !query.playing;
        if start {
            self.remaining = self.remaining.wrapping_sub(1);
        }
        (
            start,
            (time >= self.completion_time || self.remaining == 0) && !query.playing && !start,
        )
    }
}
pub struct Query {
    pub target_valid: bool,
    pub distance_squared: f32,
    pub range_squared: f32,
    pub facing: f32,
    pub playing: bool,
    pub reachable: bool,
}
/// Native UpdateSelf CVTSI2SS/MULSS uses constant 1004d91c = 2.0.
pub fn attack_time(count: i32) -> f32 {
    count as f32 * 2.
}

#[cfg(test)]
mod tests {
    use super::*;
    fn query() -> Query {
        Query {
            target_valid: true,
            distance_squared: 122500.,
            range_squared: 122500.,
            facing: 1.,
            playing: false,
            reachable: true,
        }
    }
    #[test]
    fn exact_count_and_final_animation_lifetime() {
        let mut s = State::activate(10., 3, attack_time(3));
        assert_eq!(s.completion_time, 16.);
        for remaining in [2, 1, 0] {
            assert_eq!(s.update(10., query()), (true, false));
            assert_eq!(s.remaining, remaining);
            assert_eq!(
                s.update(
                    10.,
                    Query {
                        playing: true,
                        ..query()
                    }
                ),
                (false, false)
            );
        }
        assert_eq!(s.update(10., query()), (false, true));
    }
    #[test]
    fn reachability_priority_range_facing_and_expiry_boundaries() {
        let mut s = State::activate(0., 3, 6.);
        assert_eq!(
            s.update(
                1.,
                Query {
                    distance_squared: 122501.,
                    reachable: false,
                    ..query()
                }
            ),
            (false, false)
        );
        assert_eq!(
            s.update(
                f32::from_bits(1f32.to_bits() + 1),
                Query {
                    distance_squared: 122501.,
                    reachable: false,
                    ..query()
                }
            ),
            (false, true)
        );
        let mut s = State::activate(0., 3, 6.);
        assert_eq!(
            s.update(
                0.,
                Query {
                    facing: 0.5,
                    ..query()
                }
            ),
            (false, false)
        );
        assert_eq!(s.update(6., query()), (true, false));
        assert_eq!(
            s.update(
                6.,
                Query {
                    playing: true,
                    ..query()
                }
            ),
            (false, false)
        );
        assert_eq!(
            s.update(
                6.,
                Query {
                    distance_squared: 122501.,
                    ..query()
                }
            ),
            (false, true)
        );
        assert_eq!(
            s.update(
                0.,
                Query {
                    target_valid: false,
                    playing: true,
                    ..query()
                }
            ),
            (false, true)
        );
    }
    #[test]
    fn native_float_clock_and_signed_count_are_preserved() {
        let mut s = State::activate(16777216., 1, 2.);
        assert_eq!(s.next_reachability, 16777216.);
        assert_eq!(s.update(16777218., query()), (true, false));
        assert_eq!(s.next_reachability, 16777220.);
        let mut s = State::activate(0., -1, -2.);
        assert_eq!(s.update(0., query()), (true, false));
        assert_eq!(s.remaining, -2);
        assert_eq!(attack_time(16777217), 33554432.);
    }
}
