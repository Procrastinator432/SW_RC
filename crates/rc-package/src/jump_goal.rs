//! Prepared ordinary (non-Perch) JumpTo landing decisions, ctgame.dll 10032840.
//! Rotation, collision callbacks and queued animation execution belong to the host.
use serde::{Deserialize, Serialize};

#[derive(Clone, Copy, Debug, Default, Deserialize, Serialize)]
pub struct State {
    pub started: bool,
    pub completed: bool,
}
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Decision {
    Active,
    Land,
    Fail,
}
impl State {
    pub fn pre_update(
        &mut self,
        physics: u8,
        position: [f32; 3],
        target: [f32; 3],
        tolerance_squared: f32,
    ) -> Decision {
        if physics == 2 {
            self.started = true;
        }
        let [x, y, z] = std::array::from_fn(|i| target[i] - position[i]);
        let squared = (x * x + y * y) + z * z;
        if self.started && !self.completed && physics == 1 {
            if squared < tolerance_squared {
                self.completed = true;
                return Decision::Land;
            }
            if squared > tolerance_squared {
                return Decision::Fail;
            }
        }
        Decision::Active
    }
    pub fn update_self(&self) -> u8 {
        if self.completed {
            4
        } else {
            3
        }
    }
}

/// UMeshAnimation::PostLoad 10513afe..10513b9c. The caller has established
/// reference bone zero = Move, word_50 = 0, and a nonempty first track.
/// Multiply each endpoint separately before subtracting; do not subtract i16s.
pub fn sequence_translation(first: [i16; 3], last: [i16; 3], scale_bits: u32) -> [f32; 3] {
    let scale = f32::from_bits(scale_bits) * f32::from_bits(0x3800_0100);
    std::array::from_fn(|i| last[i] as f32 * scale - first[i] as f32 * scale)
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn falling_then_strict_ground_landing_and_equal_boundary() {
        let mut state = State::default();
        assert_eq!(state.pre_update(1, [0.; 3], [0.; 3], 4.), Decision::Active);
        state.pre_update(2, [0.; 3], [0.; 3], 4.);
        assert_eq!(
            state.pre_update(1, [2., 0., 0.], [0.; 3], 4.),
            Decision::Active
        );
        assert_eq!(
            state.pre_update(1, [3., 0., 0.], [0.; 3], 4.),
            Decision::Fail
        );
        assert_eq!(
            state.pre_update(1, [1., 0., 0.], [0.; 3], 4.),
            Decision::Land
        );
        assert_eq!(state.update_self(), 4);
        assert_eq!(
            state.pre_update(1, [1., 0., 0.], [0.; 3], 4.),
            Decision::Active
        );
    }
    #[test]
    fn translation_uses_signed_endpoints_and_native_float_order() {
        assert_eq!(
            sequence_translation([-32767, 0, 32767], [32767, 0, -32767], 1f32.to_bits()),
            [2., 0., -2.]
        );
        assert_eq!(
            sequence_translation([123; 3], [123; 3], 123.4f32.to_bits()),
            [0.; 3]
        );
    }
}
