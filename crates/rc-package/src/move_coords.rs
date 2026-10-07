//! IsChannelActive and GetMoveCoords(..., false) on explicit skeletal channel snapshots.
use crate::mesh_animation::AnimationChannel;

pub fn channel_active(channels: &[AnimationChannel], index: i32, bone: i32) -> bool {
    let Ok(index) = usize::try_from(index) else {
        return false;
    };
    let Some(c) = channels.get(index) else {
        return false;
    };
    let start = c.words[15] as i32;
    let end = c.words[16] as i32;
    if index != 0
        && !matches!(
            f32::from_bits(c.words[14]).partial_cmp(&0.0),
            Some(std::cmp::Ordering::Greater)
        )
    {
        return false;
    }
    if bone >= 0 && (bone < start || bone >= end) {
        return false;
    }
    for later in &channels[index + 1..] {
        let weight = f32::from_bits(later.words[14]) * f32::from_bits(later.words[11]);
        if weight != 1.0 {
            continue;
        }
        let later_start = later.words[15] as i32;
        let later_end = later.words[16] as i32;
        if (bone < 0 && later_start <= start && end <= later_end)
            || (bone >= 0 && later_start <= bone && bone < later_end)
        {
            return false;
        }
    }
    true
}
pub trait MoveCoordsHost {
    /// GetSequence(index), including possible editor refresh of channel +44.
    /// Return the sequence byte at +54, or None for a null sequence.
    fn sequence_flags(
        &mut self,
        index: usize,
        channels: &mut [AnimationChannel],
    ) -> Result<Option<u8>, String>;
    /// Native GetActor then instance virtual +dc with the reviewed zero arguments,
    /// flags=3 and relative=false. Return the matrix subsequently read at instance +a0.
    /// Actual frame/pose evaluation remains a host boundary.
    fn evaluate_move_matrix(
        &mut self,
        index: usize,
        channels: &mut [AnimationChannel],
    ) -> Result<[u32; 16], String>;
}

/// Absolute variant used by PostInitAnim. Relative=true matrix composition is not modeled.
/// Continues through all eligible channels: the last matrix copy wins.
pub fn get_move_coords_absolute(
    channels: &mut [AnimationChannel],
    move_bone: i32,
    mut output: Option<&mut [u32; 16]>,
    host: &mut impl MoveCoordsHost,
) -> Result<bool, String> {
    let mut matched = false;
    for index in 0..channels.len() {
        if channels[index].words[1] & 0xff != 0
            || channels[index].words[15] as i32 != move_bone
            || !channel_active(channels, index as i32, -1)
            || f32::from_bits(channels[index].words[6]) == 0.0
        {
            continue;
        }
        let Some(flags) = host.sequence_flags(index, channels)? else {
            continue;
        };
        if flags & 1 == 0 {
            continue;
        }
        if let Some(matrix) = output.as_deref_mut() {
            let evaluated = host.evaluate_move_matrix(index, channels)?;
            *matrix = evaluated;
        }
        matched = true;
    }
    Ok(matched)
}

#[cfg(test)]
mod tests {
    use super::*;
    fn channel(start: i32, end: i32) -> AnimationChannel {
        let mut c = AnimationChannel::default();
        c.words[15] = start as u32;
        c.words[16] = end as u32;
        c.words[14] = 1.0f32.to_bits();
        c.words[11] = 0.5f32.to_bits();
        c.words[6] = 1.0f32.to_bits();
        c
    }
    #[test]
    fn index_zero_weight_exception_and_signed_bone_ranges() {
        let mut channels = vec![channel(2, 5), channel(8, 10)];
        channels[0].words[14] = 0;
        channels[1].words[14] = 0;
        assert!(channel_active(&channels, 0, -1));
        assert!(!channel_active(&channels, 1, -1));
        assert!(!channel_active(&channels, -1, -1));
        assert!(!channel_active(&channels, 2, -1));
        for (bone, expected) in [(1, false), (2, true), (4, true), (5, false)] {
            assert_eq!(channel_active(&channels, 0, bone), expected);
        }
    }
    #[test]
    fn suffix_occlusion_requires_exact_product_and_full_or_point_coverage() {
        let mut channels = vec![channel(1, 9), channel(3, 7)];
        channels[1].words[11] = 1.0f32.to_bits();
        assert!(channel_active(&channels, 0, -1));
        assert!(!channel_active(&channels, 0, 4));
        assert!(channel_active(&channels, 0, 8));
        channels[1].words[15] = 0;
        channels[1].words[16] = 10;
        assert!(!channel_active(&channels, 0, -1));
        channels[1].words[11] = f32::from_bits(0x3f7fffff).to_bits();
        assert!(channel_active(&channels, 0, -1));
        channels[1].words[11] = f32::NAN.to_bits();
        assert!(channel_active(&channels, 0, -1));
        channels[1].words[14] = f32::NAN.to_bits();
        assert!(!channel_active(&channels, 1, -1));
    }
    struct Host {
        flags: Vec<Option<u8>>,
        events: Vec<String>,
        fail: Option<usize>,
    }
    impl MoveCoordsHost for Host {
        fn sequence_flags(
            &mut self,
            i: usize,
            c: &mut [AnimationChannel],
        ) -> Result<Option<u8>, String> {
            self.events.push(format!("seq{i}"));
            c[i].words[17] = 900 + i as u32;
            Ok(self.flags[i])
        }
        fn evaluate_move_matrix(
            &mut self,
            i: usize,
            _: &mut [AnimationChannel],
        ) -> Result<[u32; 16], String> {
            self.events.push(format!("eval{i}"));
            if self.fail == Some(i) {
                Err("frame evaluation".into())
            } else {
                Ok([100 + i as u32; 16])
            }
        }
    }
    #[test]
    fn loop_bone_zero_rate_sequence_and_flag_gates_preserve_output() {
        let mut c = vec![channel(0, 3); 6];
        c[0].words[1] = 1;
        c[1].words[15] = 1;
        c[2].words[6] = (-0.0f32).to_bits();
        let mut h = Host {
            flags: vec![Some(1), Some(1), Some(1), None, Some(2), Some(1)],
            events: vec![],
            fail: None,
        };
        let mut m = [7; 16];
        assert!(get_move_coords_absolute(&mut c, 0, Some(&mut m), &mut h).unwrap());
        assert_eq!(h.events, ["seq3", "seq4", "seq5", "eval5"]);
        assert_eq!(m, [105; 16]);
    }
    #[test]
    fn no_output_skips_evaluation_but_keeps_sequence_refresh_and_nan_rate_is_nonzero() {
        let mut c = vec![channel(0, 3)];
        c[0].words[6] = f32::NAN.to_bits();
        let mut h = Host {
            flags: vec![Some(1)],
            events: vec![],
            fail: None,
        };
        assert!(get_move_coords_absolute(&mut c, 0, None, &mut h).unwrap());
        assert_eq!(h.events, ["seq0"]);
        assert_eq!(c[0].words[17], 900);
    }
    #[test]
    fn last_matrix_wins_and_later_error_keeps_prior_copy() {
        for fail in [None, Some(1)] {
            let mut c = vec![channel(0, 3); 2];
            let mut m = [7; 16];
            let mut h = Host {
                flags: vec![Some(1); 2],
                events: vec![],
                fail,
            };
            let r = get_move_coords_absolute(&mut c, 0, Some(&mut m), &mut h);
            if fail.is_some() {
                assert!(r.is_err());
                assert_eq!(m, [100; 16]);
            } else {
                assert!(r.unwrap());
                assert_eq!(m, [101; 16]);
            }
            assert_eq!(h.events, ["seq0", "eval0", "seq1", "eval1"]);
        }
    }
    #[test]
    fn no_eligible_channel_leaves_matrix_unchanged() {
        let mut c = vec![channel(0, 3)];
        c[0].words[1] = 1;
        let mut m = [7; 16];
        let mut h = Host {
            flags: vec![Some(1)],
            events: vec![],
            fail: None,
        };
        assert!(!get_move_coords_absolute(&mut c, 0, Some(&mut m), &mut h).unwrap());
        assert_eq!(m, [7; 16]);
        assert!(h.events.is_empty());
    }
}
