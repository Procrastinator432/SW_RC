//! Actor.ReplicateAnim scalar gates and packet writes; transport and x87 remain host boundaries.
use crate::mesh_animation::AnimationChannel;
use serde::{Deserialize, Serialize};

#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Serialize, Deserialize)]
pub struct ReplicatedAnimation {
    pub sequence_handle: u32,
    pub bone: u8,
    pub channel_loop: u8,
    pub rate: u8,
    pub frame: u8,
}
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct AnimationReplicationSnapshot {
    pub flags_68: u32,
    pub slots: [ReplicatedAnimation; 6],
}
pub trait AnimationReplicationHost {
    /// Reads Actor.Level (+94), then the byte at Level +440; only called after flags permit it.
    fn level_animation_enabled(&mut self) -> Result<bool, String>;
    /// Must check MeshInstance's ancestry for ULodMeshInstance. Non-LOD/null -> None.
    /// GetSequence may refresh channel +44 in editor mode before returning its name.
    fn lod_sequence_name(&mut self, channel_index: i32) -> Result<Option<u32>, String>;
    /// Native x87: FLD(clamped frame), FADD(1), FMUL(127), truncating conversion.
    /// Host supplies precision/control-word semantics rather than assuming binary64.
    fn quantize_frame(&mut self, clamped_frame: f32) -> Result<u8, String>;
}
#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize)]
pub enum ReplicateResult {
    SlotOutOfRange,
    ActorDisabled,
    LevelDisabled,
    ChannelDisabled,
    Unchanged,
    Written,
}

/// Helper for an explicitly supplied binary64 diagnostic quantizer, not a claim
/// about the original process's x87 precision/control word.
pub fn diagnostic_frame_binary64(frame: f32) -> u8 {
    ((f64::from(frame) + 1.0) * 127.0).trunc() as u8
}
fn native_rate(channel: &AnimationChannel) -> u8 {
    if f32::from_bits(channel.words[9]) == 0.0 {
        return 124;
    }
    let mut rate = f32::from_bits(channel.words[6]);
    // COMISS -4,Rate/JBE skips minimum assignment for unordered values.
    if -4.0 > rate {
        rate = -4.0;
    }
    // COMISS 4,Rate/JA retains Rate only for an ordered value below 4.
    if !matches!(4.0f32.partial_cmp(&rate), Some(std::cmp::Ordering::Greater)) {
        rate = 4.0;
    }
    let shifted = rate + 4.0f32;
    (shifted * 31.0f32).trunc() as u8
}
fn native_frame(channel: &AnimationChannel) -> f32 {
    let mut frame = f32::from_bits(channel.words[7]);
    if -1.0 > frame {
        return -1.0;
    }
    if !matches!(
        1.0f32.partial_cmp(&frame),
        Some(std::cmp::Ordering::Greater)
    ) {
        frame = 1.0;
    }
    frame
}
pub fn replicate_animation(
    state: &mut AnimationReplicationSnapshot,
    slot: i32,
    channel_index: i32,
    channel: &AnimationChannel,
    notify: bool,
    host: &mut impl AnimationReplicationHost,
) -> Result<ReplicateResult, String> {
    if slot >= 6 {
        return Ok(ReplicateResult::SlotOutOfRange);
    }
    // Original compares only the upper bound; negative writes would address earlier Actor memory.
    if slot < 0 {
        return Err(
            "negative replication slot would address outside the reviewed six slots".into(),
        );
    }
    if notify {
        if state.flags_68 & 0x10000 == 0 {
            return Ok(ReplicateResult::ActorDisabled);
        }
        if !host.level_animation_enabled()? {
            return Ok(ReplicateResult::LevelDisabled);
        }
        if channel.channel() >= 3 {
            return Ok(ReplicateResult::ChannelDisabled);
        }
    }
    let sequence_handle = host.lod_sequence_name(channel_index)?.unwrap_or(0);
    let packet = ReplicatedAnimation {
        sequence_handle,
        bone: channel.words[15] as u8,
        channel_loop: (channel.words[3] as u8)
            | if channel.words[1] & 0xff != 0 {
                0x80
            } else {
                0
            },
        rate: native_rate(channel),
        frame: host.quantize_frame(native_frame(channel))?,
    };
    let previous = state.slots[slot as usize];
    if notify {
        if (
            previous.sequence_handle,
            previous.bone,
            previous.channel_loop,
            previous.rate,
        ) == (
            packet.sequence_handle,
            packet.bone,
            packet.channel_loop,
            packet.rate,
        ) {
            return Ok(ReplicateResult::Unchanged);
        }
        state.flags_68 |= 0x100;
    }
    state.slots[slot as usize] = packet;
    Ok(ReplicateResult::Written)
}

pub trait SkeletalPostInitHost {
    /// Native virtual +e0 receives the persistent matrix at instance +128 and zero.
    /// Writes performed even when false is returned remain visible.
    fn root_location(&mut self, matrix: &mut [u32; 16]) -> Result<bool, String>;
    /// Native remainder: GetActor, matrix Inverse, Actor virtual +e4, compose, copy.
    fn inverse_actor_compose(&mut self, matrix: &[u32; 16]) -> Result<[u32; 16], String>;
}
#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize)]
pub enum SkeletalPostInitResult {
    RootUnavailable,
    Updated,
}
pub fn skeletal_post_init(
    matrix: &mut [u32; 16],
    host: &mut impl SkeletalPostInitHost,
) -> Result<SkeletalPostInitResult, String> {
    if !host.root_location(matrix)? {
        return Ok(SkeletalPostInitResult::RootUnavailable);
    }
    let result = host.inverse_actor_compose(matrix)?;
    *matrix = result;
    Ok(SkeletalPostInitResult::Updated)
}

#[cfg(test)]
mod tests {
    use super::*;
    #[derive(Default)]
    struct Host {
        events: Vec<&'static str>,
        level: bool,
        sequence: Option<u32>,
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
    impl AnimationReplicationHost for Host {
        fn level_animation_enabled(&mut self) -> Result<bool, String> {
            self.event("level")?;
            Ok(self.level)
        }
        fn lod_sequence_name(&mut self, i: i32) -> Result<Option<u32>, String> {
            assert_eq!(i, 4);
            self.event("sequence")?;
            Ok(self.sequence)
        }
        fn quantize_frame(&mut self, f: f32) -> Result<u8, String> {
            self.event("frame")?;
            Ok(diagnostic_frame_binary64(f))
        }
    }
    fn input() -> (AnimationReplicationSnapshot, AnimationChannel, Host) {
        let mut c = AnimationChannel::default();
        c.words[1] = 1;
        c.words[3] = 2;
        c.words[6] = 1.0f32.to_bits();
        c.words[7] = 0.5f32.to_bits();
        c.words[9] = 0.9f32.to_bits();
        c.words[15] = 258;
        (
            AnimationReplicationSnapshot {
                flags_68: 0x10000,
                slots: [ReplicatedAnimation::default(); 6],
            },
            c,
            Host {
                level: true,
                sequence: Some(337),
                ..Host::default()
            },
        )
    }
    #[test]
    fn packet_writes_and_frame_only_changes_are_suppressed() {
        let (mut s, mut c, mut h) = input();
        assert_eq!(
            replicate_animation(&mut s, 2, 4, &c, true, &mut h).unwrap(),
            ReplicateResult::Written
        );
        assert_eq!(
            s.slots[2],
            ReplicatedAnimation {
                sequence_handle: 337,
                bone: 2,
                channel_loop: 130,
                rate: 155,
                frame: 190
            }
        );
        assert_eq!(s.flags_68, 0x10100);
        s.flags_68 &= !0x100;
        let before = s.clone();
        c.words[7] = (-0.5f32).to_bits();
        assert_eq!(
            replicate_animation(&mut s, 2, 4, &c, true, &mut h).unwrap(),
            ReplicateResult::Unchanged
        );
        assert_eq!(s, before);
        assert_eq!(
            h.events,
            ["level", "sequence", "frame", "level", "sequence", "frame"]
        );
    }
    #[test]
    fn notify_false_bypasses_all_gates_and_always_writes_without_dirty() {
        let (mut s, mut c, mut h) = input();
        s.flags_68 = 0;
        c.words[3] = i32::MAX as u32;
        assert_eq!(
            replicate_animation(&mut s, 0, 4, &c, false, &mut h).unwrap(),
            ReplicateResult::Written
        );
        assert_eq!(h.events, ["sequence", "frame"]);
        assert_eq!(s.flags_68, 0);
        assert_eq!(
            replicate_animation(&mut s, 0, 4, &c, false, &mut h).unwrap(),
            ReplicateResult::Written
        );
    }
    #[test]
    fn gate_order_and_negative_slots_do_not_write() {
        for mode in 0..5 {
            let (mut s, mut c, mut h) = input();
            let slot = match mode {
                0 => 6,
                1 => -1,
                _ => 0,
            };
            if mode == 2 {
                s.flags_68 = 0;
            }
            if mode == 3 {
                h.level = false;
            }
            if mode == 4 {
                c.words[3] = 3;
            }
            let before = s.clone();
            let r = replicate_animation(&mut s, slot, 4, &c, true, &mut h);
            if mode == 1 {
                assert!(r.is_err());
            } else {
                assert_eq!(
                    r.unwrap(),
                    match mode {
                        0 => ReplicateResult::SlotOutOfRange,
                        2 => ReplicateResult::ActorDisabled,
                        3 => ReplicateResult::LevelDisabled,
                        _ => ReplicateResult::ChannelDisabled,
                    }
                );
            }
            assert_eq!(s, before);
            assert_eq!(h.events.len(), usize::from(mode >= 3));
        }
    }
    #[test]
    fn clamp_sentinel_nan_and_byte_truncation() {
        let (_, mut c, _) = input();
        for (rate, expected) in [
            (-9.0, 0),
            (-4.0, 0),
            (-0.5, 108),
            (0.0, 124),
            (4.0, 248),
            (9.0, 248),
            (f32::NAN, 248),
        ] {
            c.words[6] = rate.to_bits();
            assert_eq!(native_rate(&c), expected);
        }
        c.words[9] = (-0.0f32).to_bits();
        assert_eq!(native_rate(&c), 124);
        for (frame, expected) in [(-2.0, -1.0f32), (2.0, 1.0), (f32::NAN, 1.0), (-0.0, -0.0)] {
            c.words[7] = frame.to_bits();
            assert_eq!(native_frame(&c).to_bits(), expected.to_bits());
        }
    }
    #[test]
    fn absent_sequence_becomes_none_and_host_errors_precede_actor_writes() {
        let (mut s, c, mut h) = input();
        h.sequence = None;
        replicate_animation(&mut s, 0, 4, &c, true, &mut h).unwrap();
        assert_eq!(s.slots[0].sequence_handle, 0);
        for fail in ["level", "sequence", "frame"] {
            let (mut s, c, mut h) = input();
            h.fail = Some(fail);
            let before = s.clone();
            assert_eq!(
                replicate_animation(&mut s, 0, 4, &c, true, &mut h).unwrap_err(),
                fail
            );
            assert_eq!(s, before);
        }
    }
    struct Post {
        root: bool,
        fail: bool,
        events: Vec<&'static str>,
    }
    impl SkeletalPostInitHost for Post {
        fn root_location(&mut self, m: &mut [u32; 16]) -> Result<bool, String> {
            self.events.push("root");
            m[0] = 7;
            Ok(self.root)
        }
        fn inverse_actor_compose(&mut self, _: &[u32; 16]) -> Result<[u32; 16], String> {
            self.events.push("compose");
            if self.fail {
                Err("matrix boundary".into())
            } else {
                Ok([9; 16])
            }
        }
    }
    #[test]
    fn post_init_gate_keeps_root_writes_and_defers_matrix_remainder() {
        for (root, fail) in [(false, false), (true, true), (true, false)] {
            let mut matrix = [0; 16];
            let mut h = Post {
                root,
                fail,
                events: vec![],
            };
            let result = skeletal_post_init(&mut matrix, &mut h);
            if !root {
                assert_eq!(result.unwrap(), SkeletalPostInitResult::RootUnavailable);
                assert_eq!(matrix[0], 7);
                assert_eq!(h.events, ["root"]);
            } else if fail {
                assert!(result.is_err());
                assert_eq!(matrix[0], 7);
            } else {
                assert_eq!(result.unwrap(), SkeletalPostInitResult::Updated);
                assert_eq!(matrix, [9; 16]);
            }
        }
    }
}
