//! ULodMeshInstance.PlayAnim continuation on explicit channels and sequence metadata.
//! No sequence decoder, animation ticking, pose evaluation or replication transport.
use crate::{animation_call::PlayAnimParameters, mesh_animation::AnimationChannel};
use serde::Serialize;

#[derive(Clone, Copy, Debug, Serialize)]
pub struct PlaybackSequence {
    /// Opaque non-null original sequence pointer identity, not a package index.
    pub token: u32,
    pub frames: i32,
    pub rate: f32,
    pub minimum_blend: f32,
    pub randomize_start: bool,
}
#[derive(Clone, Debug, Serialize)]
pub struct ChannelPlaybackSnapshot {
    pub channels: Vec<AnimationChannel>,
    pub byte_60: u8,
    pub byte_61: u8,
}
pub trait ChannelPlaybackHost {
    fn random_int(&mut self) -> Result<i32, String>;
    /// May alter selected channel words. Must preserve its identity and array length.
    fn post_init(&mut self, channel: &mut AnimationChannel) -> Result<(), String>;
    /// Native ReplicateAnim receives this index twice and a final true argument.
    fn replicate(&mut self, index: usize, channel: &AnimationChannel) -> Result<(), String>;
}
#[derive(Clone, Copy, Debug, Serialize)]
pub struct ChannelPlaybackResult {
    pub reinitialized: bool,
    pub blend_time: f32,
}
fn float(c: &AnimationChannel, offset: usize) -> f32 {
    f32::from_bits(c.words[offset / 4])
}
fn set(c: &mut AnimationChannel, offset: usize, value: f32) {
    c.words[offset / 4] = value.to_bits();
}
// COMISS/JNC skips the assignment only for ordered greater-or-equal.
fn below_or_unordered(a: f32, b: f32) -> bool {
    !matches!(
        a.partial_cmp(&b),
        Some(std::cmp::Ordering::Equal | std::cmp::Ordering::Greater)
    )
}

pub fn play_channel(
    state: &mut ChannelPlaybackSnapshot,
    index: usize,
    parameters: &PlayAnimParameters,
    sequence: Option<PlaybackSequence>,
    host: &mut impl ChannelPlaybackHost,
) -> Result<ChannelPlaybackResult, String> {
    if index >= state.channels.len() {
        return Err("selected animation channel unavailable".into());
    }
    if sequence.is_some_and(|s| s.token == 0) {
        return Err("sequence token must be non-null".into());
    }
    // The preceding LOD entry rejects an absent non-None sequence.
    if sequence.is_none() && parameters.sequence.handle != 0 {
        return Err("missing non-None sequence must stop at LOD entry".into());
    }
    let mut blend = 0.25f32;
    if let Some(s) = sequence {
        if !parameters.looping {
            let duration = (s.frames as f32 / s.rate) * 0.5f32;
            if below_or_unordered(duration, blend) {
                blend = duration;
            }
        }
        if below_or_unordered(blend, s.minimum_blend) {
            blend = s.minimum_blend;
        }
    }
    let c = &mut state.channels[index];
    let reinitialized = c.words[0] != parameters.sequence.handle
        || (float(c, 0x18) == 0.0 && parameters.rate != 0.0);
    if !reinitialized {
        if parameters.start_frame >= 0.0 {
            set(c, 0x30, parameters.start_frame);
            set(c, 0x1c, parameters.start_frame);
        }
    } else {
        c.words[0] = parameters.sequence.handle;
        c.words[17] = sequence.map_or(0, |s| s.token);
        if let Some(s) = sequence {
            set(c, 0x38, 0.0);
            let last = 1.0f32 - 1.0f32 / (s.frames as f32);
            set(c, 0x24, last);
            if parameters.start_frame >= 0.0 {
                set(c, 0x30, parameters.start_frame);
                set(c, 0x1c, parameters.start_frame);
            } else if s.randomize_start {
                let random = host.random_int()? as f32 * f32::from_bits(0x38000100);
                set(c, 0x30, random);
                set(c, 0x1c, random);
            } else if !(float(c, 0x18) > 0.0 && c.words[1] & 0xff != 0 && parameters.looping) {
                set(c, 0x30, 0.0);
                set(c, 0x1c, 0.0);
            }
        } else {
            set(c, 0x24, 0.0);
            let old = float(c, 0x28);
            let target = if old == 0.0 { 1.0 } else { 0.0 };
            for sibling in &mut state.channels[..index] {
                set(sibling, 0x28, old);
                set(sibling, 0x34, target);
                set(sibling, 0x2c, target);
            }
        }
        let c = &mut state.channels[index];
        set(c, 0x20, 0.0);
        // COMISS 0,StartFrame/JBE also disables blending for unordered values.
        let explicit_frame = parameters.sequence.handle != 0
            && parameters.start_frame.partial_cmp(&0.0) != Some(std::cmp::Ordering::Less);
        if blend == 0.0 || state.byte_60 == 0 || explicit_frame {
            set(c, 0x28, 0.0);
            set(c, 0x2c, 1.0);
        } else {
            set(c, 0x28, 1.0f32 / blend);
            set(c, 0x2c, 0.0);
        }
        c.words[13] = c.words[11];
    }
    let c = &mut state.channels[index];
    c.words[1] = (c.words[1] & 0xffffff00) | u32::from(parameters.looping);
    set(c, 0x18, parameters.rate);
    let weight = if sequence.is_some() {
        parameters.float_10
    } else {
        0.0
    };
    set(c, 0x10, weight);
    if float(c, 0x14) == 0.0 {
        set(c, 0x38, weight);
    }
    if blend == 0.0 {
        set(c, 0x2c, 1.0);
    }
    if reinitialized {
        host.post_init(c)?;
        host.replicate(index, c)?;
    }
    state.byte_61 = 0;
    Ok(ChannelPlaybackResult {
        reinitialized,
        blend_time: blend,
    })
}

#[cfg(test)]
mod tests {
    use super::*;
    #[derive(Default)]
    struct Host {
        events: Vec<&'static str>,
        fail: Option<&'static str>,
    }
    impl Host {
        fn event(&mut self, n: &'static str) -> Result<(), String> {
            self.events.push(n);
            if self.fail == Some(n) {
                Err(n.into())
            } else {
                Ok(())
            }
        }
    }
    impl ChannelPlaybackHost for Host {
        fn random_int(&mut self) -> Result<i32, String> {
            self.event("random")?;
            Ok(16384)
        }
        fn post_init(&mut self, c: &mut AnimationChannel) -> Result<(), String> {
            self.event("post")?;
            c.words[2] = 0xabcdef01;
            Ok(())
        }
        fn replicate(&mut self, _: usize, c: &AnimationChannel) -> Result<(), String> {
            assert_eq!(c.words[2], 0xabcdef01);
            self.event("replicate")
        }
    }
    fn inputs() -> (
        ChannelPlaybackSnapshot,
        PlayAnimParameters,
        PlaybackSequence,
    ) {
        let mut c = AnimationChannel {
            words: [0x12345678; 18],
        };
        c.words[0] = 337;
        c.words[1] = 0xaabbcc01;
        set(&mut c, 0x18, 1.0);
        set(&mut c, 0x14, 0.0);
        set(&mut c, 0x1c, 0.6);
        set(&mut c, 0x30, 0.7);
        (
            ChannelPlaybackSnapshot {
                channels: vec![c],
                byte_60: 1,
                byte_61: 7,
            },
            PlayAnimParameters {
                sequence: crate::event_lookup::EventNameSnapshot {
                    handle: 337,
                    resolved_index: 336,
                },
                bone: crate::event_lookup::EventNameSnapshot {
                    handle: 0,
                    resolved_index: 0,
                },
                channel: 0,
                looping: true,
                float_10: 1.0,
                rate: 2.0,
                start_frame: -1.0,
            },
            PlaybackSequence {
                token: 123,
                frames: 10,
                rate: 20.0,
                minimum_blend: 0.0,
                randomize_start: false,
            },
        )
    }
    #[test]
    fn reuse_preserves_frame_clip_pointer_and_loop_padding() {
        let (mut state, p, s) = inputs();
        let before = state.channels[0].clone();
        let mut host = Host::default();
        let r = play_channel(&mut state, 0, &p, Some(s), &mut host).unwrap();
        assert!(!r.reinitialized);
        assert!(host.events.is_empty());
        assert_eq!(state.byte_61, 0);
        let c = &state.channels[0];
        assert_eq!(c.words[17], before.words[17]);
        assert_eq!(float(c, 0x1c), 0.6);
        assert_eq!(float(c, 0x30), 0.7);
        assert_eq!(c.words[1], 0xaabbcc01);
        assert_eq!(float(c, 0x18), 2.0);
        assert_eq!(float(c, 0x38), 1.0);
    }
    #[test]
    fn reset_rate_zero_transition_and_explicit_frame() {
        let (mut state, mut p, s) = inputs();
        set(&mut state.channels[0], 0x18, -0.0);
        p.start_frame = 0.4;
        p.looping = false;
        let mut host = Host::default();
        let r = play_channel(&mut state, 0, &p, Some(s), &mut host).unwrap();
        assert!(r.reinitialized);
        assert_eq!(host.events, ["post", "replicate"]);
        let c = &state.channels[0];
        assert_eq!(
            (
                float(c, 0x1c),
                float(c, 0x30),
                float(c, 0x24),
                float(c, 0x28),
                float(c, 0x2c)
            ),
            (0.4, 0.4, 0.9, 0.0, 1.0)
        );
        assert_eq!(c.words[17], 123);
        assert_eq!(c.words[1], 0xaabbcc00);
    }
    #[test]
    fn new_loop_keeps_positive_loop_frame_and_computes_blend() {
        let (mut state, mut p, mut s) = inputs();
        p.sequence.handle = 338;
        s.minimum_blend = 0.5;
        let mut host = Host::default();
        let r = play_channel(&mut state, 0, &p, Some(s), &mut host).unwrap();
        assert_eq!(r.blend_time, 0.5);
        assert_eq!(
            (
                float(&state.channels[0], 0x1c),
                float(&state.channels[0], 0x30),
                float(&state.channels[0], 0x28),
                float(&state.channels[0], 0x2c)
            ),
            (0.6, 0.7, 2.0, 0.0)
        );
    }
    #[test]
    fn random_start_and_partial_error_writes() {
        let (mut state, mut p, mut s) = inputs();
        p.sequence.handle = 338;
        s.randomize_start = true;
        let mut host = Host {
            fail: Some("random"),
            ..Host::default()
        };
        assert!(play_channel(&mut state, 0, &p, Some(s), &mut host).is_err());
        assert_eq!(state.channels[0].words[0], 338);
        assert_eq!(state.channels[0].words[17], 123);
        assert_eq!(state.byte_61, 7);
        assert_eq!(float(&state.channels[0], 0x18), 1.0);
        host.fail = None;
        set(&mut state.channels[0], 0x18, 0.0);
        play_channel(&mut state, 0, &p, Some(s), &mut host).unwrap();
        assert_eq!(
            float(&state.channels[0], 0x1c).to_bits(),
            (16384f32 * f32::from_bits(0x38000100)).to_bits()
        );
    }
    #[test]
    fn none_reset_propagates_blend_only_to_preceding_siblings() {
        let (mut state, mut p, _) = inputs();
        state.channels = vec![state.channels[0].clone(); 3];
        let tail = state.channels[2].clone();
        set(&mut state.channels[1], 0x28, 2.0);
        p.sequence.handle = 0;
        p.looping = false;
        play_channel(&mut state, 1, &p, None, &mut Host::default()).unwrap();
        assert_eq!(float(&state.channels[0], 0x28), 2.0);
        assert_eq!(float(&state.channels[0], 0x2c), 0.0);
        assert_eq!(state.channels[2], tail);
        assert_eq!(state.channels[1].words[17], 0);
        assert_eq!(float(&state.channels[1], 0x10), 0.0);
    }
    #[test]
    fn callback_errors_keep_channel_changes_and_uncleared_instance_flag() {
        for fail in ["post", "replicate"] {
            let (mut state, mut p, s) = inputs();
            p.sequence.handle = 338;
            let mut host = Host {
                fail: Some(fail),
                ..Host::default()
            };
            assert_eq!(
                play_channel(&mut state, 0, &p, Some(s), &mut host).unwrap_err(),
                fail
            );
            assert_eq!(state.byte_61, 7);
            assert_eq!(float(&state.channels[0], 0x18), 2.0);
            assert_eq!(host.events.len(), if fail == "post" { 1 } else { 2 });
        }
    }
    #[test]
    fn unordered_float_branches_follow_original_and_invalid_inputs_stop() {
        let (mut state, mut p, mut s) = inputs();
        let before = state.clone();
        assert!(play_channel(&mut state, 3, &p, Some(s), &mut Host::default()).is_err());
        assert_eq!(state.channels, before.channels);
        s.token = 0;
        assert!(play_channel(&mut state, 0, &p, Some(s), &mut Host::default()).is_err());
        s.token = 123;
        set(&mut state.channels[0], 0x18, f32::NAN);
        p.start_frame = f32::NAN;
        assert!(
            !play_channel(&mut state, 0, &p, Some(s), &mut Host::default())
                .unwrap()
                .reinitialized
        );
        assert_eq!(float(&state.channels[0], 0x1c), 0.6);
        p.sequence.handle = 338;
        p.looping = false;
        s.rate = f32::NAN;
        s.minimum_blend = 0.125;
        assert_eq!(
            play_channel(&mut state, 0, &p, Some(s), &mut Host::default())
                .unwrap()
                .blend_time,
            0.125
        );
        assert_eq!(float(&state.channels[0], 0x28), 0.0);
    }
}
