//! Prepared blocks of UpdateAnimation, not the complete callback/tick loop.
//! Ordered/unordered comparisons follow the exported SSE branches explicitly.
use crate::{mesh_animation::AnimationChannel, skeletal_director::BoneDirector};
#[cfg(test)]
#[path = "skeletal_animation_tick_transition_tests.rs"]
mod transition_tests;

/// ULodMeshInstance 10450345..104503bd, once per channel before sequence lookup.
/// No lower blend clamp. Unordered blend/weight results select the native target.
pub fn advance_channel_blend(channel: &mut AnimationChannel, delta: f32) {
    let value = |word: usize| f32::from_bits(channel.words[word]);
    let blend = value(10) * delta + value(11);
    // COMISS 1,blend / JNC: unordered takes the clamp branch too.
    channel.words[11] = if blend <= 1.0 { blend } else { 1.0 }.to_bits();
    let target = f32::from_bits(channel.words[4]);
    let current = f32::from_bits(channel.words[14]);
    let rate = f32::from_bits(channel.words[5]);
    let next = if target > current {
        let sum = rate * delta + current;
        if target >= sum {
            sum
        } else {
            target
        }
    } else if current > target {
        let difference = current - rate * delta;
        if difference >= target {
            difference
        } else {
            target
        }
    } else {
        // Equal OR unordered: do not even rewrite the current raw word.
        return;
    };
    channel.words[14] = next.to_bits();
}

/// 104504ad..104504d7 after sequence resolution and optional jitter update.
/// `frames` is the native signed frame count; jitter is channel word 8.
/// No normalization, loop/end/notify handling or zero-frame guard occurs here.
pub fn advance_channel_frame(
    channel: &mut AnimationChannel,
    frames: i32,
    rate: f32,
    remaining: f32,
) {
    let jitter = f32::from_bits(channel.words[8]);
    let channel_rate = f32::from_bits(channel.words[6]);
    let old = f32::from_bits(channel.words[7]);
    let increment = (((jitter + 1.0) / frames as f32) * rate) * remaining * channel_rate;
    channel.words[7] = (increment + old).to_bits();
}

/// Prepared 12-byte native notify record: the middle word is not used here.
#[derive(Clone, Copy, Debug)]
pub struct TickNotify {
    pub time_bits: u32,
    pub object: u32,
}

/// 10450520..10450565: first nearest notify in (old, current], even if unsorted.
/// Equal distances keep the earlier array entry. Unordered comparisons reject.
pub fn select_tick_notify(notifies: &[TickNotify], old: f32, current: f32) -> Option<usize> {
    let mut selected = None;
    let mut distance = 0.0;
    for (index, notify) in notifies.iter().enumerate() {
        let time = f32::from_bits(notify.time_bits);
        if time > old && current >= time {
            let candidate = time - old;
            if selected.is_none() || distance > candidate {
                selected = Some(index);
                distance = candidate;
            }
        }
    }
    selected
}

/// Explicit boundary for native notify virtual +78; may replace the channel list.
pub trait TickNotifyHost {
    fn notify(&mut self, object: u32, channels: &mut Vec<AnimationChannel>) -> Result<(), String>;
}

#[derive(Clone, Copy, Debug, PartialEq)]
pub enum NotifyTransition {
    None,
    Continue {
        remaining: f32,
    },
    /// Original early return skips common-exit cache invalidation and unlock.
    ChannelRemoved {
        remaining: f32,
    },
}

/// Prepared notify branch 104504dc..10450640, called after frame progression.
/// Actor mesh presence is supplied by the caller; object identities are opaque.
pub fn dispatch_tick_notify(
    channels: &mut Vec<AnimationChannel>,
    index: usize,
    old: f32,
    remaining: f32,
    actor_has_mesh: bool,
    notifies: &[TickNotify],
    host: &mut impl TickNotifyHost,
) -> Result<NotifyTransition, String> {
    let channel = channels
        .get_mut(index)
        .ok_or("notify channel index out of range")?;
    if !actor_has_mesh || channel.words[1] & 0xff00 != 0 {
        return Ok(NotifyTransition::None);
    }
    let current = f32::from_bits(channel.words[7]);
    let Some(selected) = select_tick_notify(notifies, old, current) else {
        return Ok(NotifyTransition::None);
    };
    let notify = notifies[selected];
    let time = f32::from_bits(notify.time_bits);
    let rest = ((current - time) * remaining) / (current - old);
    // Native copies the raw timestamp before invoking the callback.
    channel.words[7] = notify.time_bits;
    if notify.object != 0 {
        host.notify(notify.object, channels)?;
        if index >= channels.len() {
            return Ok(NotifyTransition::ChannelRemoved { remaining: rest });
        }
    }
    Ok(NotifyTransition::Continue { remaining: rest })
}

/// Boundaries for virtual instance +b0 (clear/play) and Actor +100 (AnimEnd).
/// The fixed channel reference permits field mutation, not array replacement.
/// Native callback-driven channel storage/lifetime changes remain a caller concern.
pub trait TickEndHost {
    fn clear(
        &mut self,
        first_eight_words: [u32; 8],
        channel: &mut AnimationChannel,
    ) -> Result<(), String>;
    fn anim_end(&mut self, number: i32, channel: &mut AnimationChannel) -> Result<(), String>;
}

#[derive(Clone, Copy, Debug, PartialEq)]
pub struct EndTransition {
    pub reached_end: bool,
    pub remaining: f32,
    pub cleared: bool,
    pub stopped: bool,
    pub notified: bool,
}

/// 10450645..10450769 after the notify branch found no eligible notify.
/// Calls clear before stopping the rate, then reads the current suppress byte.
pub fn dispatch_tick_end(
    channel: &mut AnimationChannel,
    old: f32,
    remaining: f32,
    host: &mut impl TickEndHost,
) -> Result<EndTransition, String> {
    let current = f32::from_bits(channel.words[7]);
    let end = f32::from_bits(channel.words[9]);
    let mut result = EndTransition {
        reached_end: false,
        remaining,
        cleared: false,
        stopped: false,
        notified: false,
    };
    if end > current {
        return Ok(result);
    }
    result.reached_end = true;
    if channel.words[1] & 0xff != 0 {
        // COMISS 1,current / JBE also enters the wrap branch for NaN.
        if current < 1.0 {
            result.remaining = 0.0;
        } else {
            result.remaining = ((current - 1.0) * remaining) / (current - old);
            channel.words[7] = 0;
        }
        if end > old && channel.words[1] & 0xff00 == 0 {
            host.anim_end(channel.words[3] as i32, channel)?;
            result.notified = true;
        }
    } else {
        result.remaining = ((current - end) * remaining) / (current - old);
        channel.words[7] = channel.words[9];
        if f32::from_bits(channel.words[6]) > 0.0 {
            if channel.words[15] as i32 > 0 || channel.words[3] as i32 > 0 {
                let mut snapshot: [u32; 8] = channel.words[..8].try_into().unwrap();
                snapshot[0] = 0;
                host.clear(snapshot, channel)?;
                result.cleared = true;
            }
            channel.words[6] = 0;
            result.stopped = true;
            if channel.words[1] & 0xff00 == 0 {
                host.anim_end(channel.words[3] as i32, channel)?;
                result.notified = true;
            }
        }
    }
    Ok(result)
}

/// USkeletalMeshInstance 10500226..10500242, including disabled directors.
pub fn accrue_director_budgets(directors: &mut [BoneDirector], delta: f32) {
    for director in directors {
        let rate = f32::from_bits(director.words[21]);
        if rate >= 0.0 {
            let prior = f32::from_bits(director.words[26]);
            director.words[26] = (rate * delta + prior).to_bits();
        }
    }
}

/// Skeletal wrapper 105001f0: LOD work first, then the current director array.
/// The caller supplies the still-external LOD tick, including cache invalidation.
/// Rust host failures stop the wrapper; normal early LOD returns still accrue.
pub fn update_animation_with_lod<T>(
    directors: &mut Vec<BoneDirector>,
    delta: f32,
    lod: impl FnOnce(&mut Vec<BoneDirector>, f32) -> Result<T, String>,
) -> Result<T, String> {
    let result = lod(directors, delta)?;
    accrue_director_budgets(directors, delta);
    Ok(result)
}

#[cfg(test)]
mod tests {
    use super::*;
    fn channel(blend: f32, target: f32, current: f32, speed: f32) -> AnimationChannel {
        let mut c = AnimationChannel { words: [0; 18] };
        for (word, value) in [
            (10, speed),
            (11, blend),
            (4, target),
            (5, speed),
            (14, current),
        ] {
            c.words[word] = value.to_bits();
        }
        c
    }
    fn director(rate: f32, budget: f32) -> BoneDirector {
        let mut d = BoneDirector { words: [0; 28] };
        d.words[21] = rate.to_bits();
        d.words[26] = budget.to_bits();
        d
    }
    #[test]
    fn blend_and_weight_crossings_preserve_other_words() {
        for (target, current, expected) in [(0.75, 0.0, 0.75), (0.25, 1.0, 0.25)] {
            let mut c = channel(0.75, target, current, 2.0);
            let old = c.words;
            advance_channel_blend(&mut c, 0.5);
            assert_eq!(c.words[11], 1f32.to_bits());
            assert_eq!(c.words[14], f32::to_bits(expected));
            for (i, word) in old.into_iter().enumerate() {
                if i != 11 && i != 14 {
                    assert_eq!(c.words[i], word);
                }
            }
        }
    }
    #[test]
    fn negative_time_has_no_lower_clamp_or_direction_correction() {
        let mut c = channel(0.25, 1.0, 0.25, 2.0);
        advance_channel_blend(&mut c, -0.5);
        assert_eq!(c.words[11], (-0.75f32).to_bits());
        assert_eq!(c.words[14], (-0.75f32).to_bits());
    }
    #[test]
    fn unordered_blend_and_computed_weights_clamp_like_jnc() {
        for (target, current) in [(1.0, 0.0), (0.0, 1.0)] {
            let mut c = channel(0.0, target, current, f32::NAN);
            advance_channel_blend(&mut c, 1.0);
            assert_eq!(c.words[11], 1f32.to_bits());
            assert_eq!(c.words[14], target.to_bits());
        }
    }
    #[test]
    fn unordered_target_current_and_equal_zero_keep_raw_weight() {
        for (target, current) in [(f32::NAN, 0x80000000), (0.0, 0x7fc12345), (0.0, 0x80000000)] {
            let mut c = channel(0.0, target, f32::from_bits(current), 1.0);
            advance_channel_blend(&mut c, 1.0);
            assert_eq!(c.words[14], current);
        }
    }
    #[test]
    fn frame_uses_sequence_rate_jitter_and_remaining_without_wrapping() {
        let mut c = channel(0.0, 0.0, 0.0, 0.0);
        c.words[6] = 2f32.to_bits();
        c.words[7] = 0.75f32.to_bits();
        c.words[8] = 0.5f32.to_bits();
        advance_channel_frame(&mut c, 30, 20.0, 0.5);
        assert_eq!(c.words[7], 1.75f32.to_bits());
        advance_channel_frame(&mut c, 0, 20.0, 0.5);
        assert_eq!(c.words[7], f32::INFINITY.to_bits());
    }
    #[test]
    fn director_budget_gate_ignores_enabled_but_skips_negative_and_nan() {
        let mut d = vec![
            director(2.0, 1.0),
            director(-1.0, 1.0),
            director(f32::NAN, 1.0),
            director(-0.0, -0.0),
        ];
        accrue_director_budgets(&mut d, 0.25);
        assert_eq!(d[0].words[26], 1.5f32.to_bits());
        assert_eq!(d[1].words[26], 1f32.to_bits());
        assert_eq!(d[2].words[26], 1f32.to_bits());
        assert_eq!(d[3].words[26], (-0.0f32).to_bits());
        accrue_director_budgets(&mut d[..1], -1.0);
        assert_eq!(d[0].words[26], (-0.5f32).to_bits());
    }
    #[test]
    fn wrapper_accrues_after_lod_mutations_and_normal_early_return() {
        let mut d = vec![director(99.0, 0.0)];
        let result = update_animation_with_lod(&mut d, 0.5, |d, delta| {
            assert_eq!(delta, 0.5);
            d.clear();
            d.push(director(2.0, 3.0));
            Ok("early")
        })
        .unwrap();
        assert_eq!(result, "early");
        assert_eq!(d[0].words[26], 4f32.to_bits());
    }
    #[test]
    fn lod_error_keeps_mutations_and_does_not_accrue() {
        let mut d = vec![director(2.0, 3.0)];
        let result: Result<(), _> = update_animation_with_lod(&mut d, 1.0, |d, _| {
            d[0].words[26] = 7f32.to_bits();
            Err("callback failed".into())
        });
        assert!(result.is_err());
        assert_eq!(d[0].words[26], 7f32.to_bits());
    }
}
