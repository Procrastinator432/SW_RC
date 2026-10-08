use super::*;
use crate::{skeletal_animation_tick::update_animation_with_lod, skeletal_director::BoneDirector};
use std::collections::VecDeque;

struct Host {
    events: Vec<String>,
    flags: VecDeque<u32>,
    sequence: TickSequence,
    lookup: u32,
    random: i32,
    remove: bool,
    replace: bool,
    append: bool,
    fail: Option<&'static str>,
}
impl Default for Host {
    fn default() -> Self {
        Self {
            events: vec![],
            flags: VecDeque::new(),
            sequence: TickSequence {
                frames: 30,
                rate: 30.0,
                jitter_amplitude: 0.0,
                notifies: vec![],
            },
            lookup: 1,
            random: 32767,
            remove: false,
            replace: false,
            append: false,
            fail: None,
        }
    }
}
impl Host {
    fn event(&mut self, name: &str) -> Result<(), String> {
        self.events.push(name.into());
        if self.fail == Some(name) {
            Err(format!("failed {name}"))
        } else {
            Ok(())
        }
    }
}
impl SequenceLookupHost for Host {
    fn find_sequence(&mut self, name: u32, load: bool) -> Result<u32, String> {
        assert_eq!(name, 7);
        assert!(!load);
        self.event("lookup")?;
        Ok(self.lookup)
    }
}
impl TickNotifyHost for Host {
    fn notify(&mut self, object: u32, channels: &mut Vec<AnimationChannel>) -> Result<(), String> {
        self.event(&format!("notify {object}"))?;
        if self.remove {
            channels.clear();
        }
        if self.replace {
            self.replace = false;
            *channels = vec![channel()];
            channels[0].words[7] = 0.5f32.to_bits();
            channels[0].words[17] = 2;
        }
        Ok(())
    }
}
impl TickEndHost for Host {
    fn clear(&mut self, snapshot: [u32; 8], _: &mut AnimationChannel) -> Result<(), String> {
        assert_eq!(snapshot[0], 0);
        self.event("clear")
    }
    fn anim_end(&mut self, number: i32, _: &mut AnimationChannel) -> Result<(), String> {
        self.event(&format!("end {number}"))
    }
}
impl LodTickHost for Host {
    fn set_locked(&mut self, locked: bool) -> Result<(), String> {
        self.event(if locked { "lock" } else { "unlock" })
    }
    fn flags(&mut self) -> Result<u32, String> {
        self.event("flags")?;
        Ok(self.flags.pop_front().unwrap_or(0))
    }
    fn request_destroy(&mut self) -> Result<(), String> {
        self.event("destroy")
    }
    fn sequence(&mut self, identity: u32) -> Result<TickSequence, String> {
        self.event(&format!("sequence {identity}"))?;
        Ok(self.sequence.clone())
    }
    fn random_int(&mut self) -> Result<i32, String> {
        self.event("random")?;
        Ok(self.random)
    }
    fn actor_has_mesh(&mut self) -> Result<bool, String> {
        self.event("actor mesh")?;
        Ok(true)
    }
    fn replicate(
        &mut self,
        index: usize,
        channels: &mut Vec<AnimationChannel>,
    ) -> Result<(), String> {
        self.event(&format!("replicate {index}"))?;
        if self.append {
            self.append = false;
            channels.push(channel());
        }
        Ok(())
    }
}
fn channel() -> AnimationChannel {
    let mut c = AnimationChannel::default();
    c.words[0] = 7;
    c.words[6] = 1f32.to_bits();
    c.words[9] = 0.9f32.to_bits();
    c.words[10] = 1f32.to_bits();
    c.words[17] = 1;
    c
}
fn notify(time: f32, object: u32) -> TickNotify {
    TickNotify {
        time_bits: time.to_bits(),
        object,
    }
}

#[test]
fn cached_tick_advances_active_channel_and_replicates_inactive_channel() {
    let mut c = vec![channel(), AnimationChannel::default()];
    let mut cache = 9;
    let mut host = Host::default();
    let r = update_lod_animation(&mut c, &mut cache, 0.25, false, &mut host).unwrap();
    assert_eq!(
        r,
        LodTickResult {
            exit: TickExit::Unlocked,
            attempts: 1,
            replicated: 2
        }
    );
    assert_eq!(
        host.events,
        [
            "lock",
            "flags",
            "sequence 1",
            "replicate 0",
            "replicate 1",
            "flags",
            "unlock"
        ]
    );
    assert_eq!(c[0].words[7], 0.25f32.to_bits());
    assert_eq!(c[0].words[11], 0.25f32.to_bits());
    assert_eq!(cache, 0);
}
#[test]
fn editor_null_lookup_retries_four_times_then_replicates() {
    let mut c = vec![channel()];
    let mut cache = 1;
    let mut host = Host {
        lookup: 0,
        ..Host::default()
    };
    let r = update_lod_animation(&mut c, &mut cache, 0.25, true, &mut host).unwrap();
    assert_eq!(r.attempts, 4);
    assert_eq!(r.replicated, 1);
    assert_eq!(c[0].words[17], 0);
    assert_eq!(
        host.events
            .iter()
            .filter(|e| e.as_str() == "lookup")
            .count(),
        4
    );
    assert!(!host.events.iter().any(|e| e.starts_with("sequence")));
}
#[test]
fn production_loop_consumes_two_notifies_wrap_and_remaining_frame() {
    let mut c = vec![channel()];
    c[0].words[1] = 1;
    let mut cache = 7;
    let mut host = Host::default();
    host.sequence.notifies = vec![notify(0.75, 2), notify(0.25, 1)];
    let r = update_lod_animation(&mut c, &mut cache, 1.2, false, &mut host).unwrap();
    assert_eq!(r.attempts, 4);
    assert_eq!(r.replicated, 1);
    assert_eq!(cache, 0);
    let callbacks: Vec<_> = host
        .events
        .iter()
        .filter(|e| e.starts_with("notify") || e.starts_with("end"))
        .map(String::as_str)
        .collect();
    assert_eq!(callbacks, ["notify 1", "notify 2", "end 0"]);
    assert!((f32::from_bits(c[0].words[7]) - 0.2).abs() < 0.000001);
}
#[test]
fn iteration_cap_limits_notify_storm_and_still_replicates() {
    let mut c = vec![channel()];
    let mut cache = 1;
    let mut host = Host::default();
    host.sequence.notifies = (1..=6).map(|i| notify(i as f32 * 0.125, i)).collect();
    let r = update_lod_animation(&mut c, &mut cache, 1.0, false, &mut host).unwrap();
    assert_eq!(r.attempts, 4);
    assert_eq!(c[0].words[7], 0.5f32.to_bits());
    assert_eq!(r.replicated, 1);
    assert_eq!(cache, 0);
}
#[test]
fn notify_removal_bypasses_replication_cache_clear_and_unlock() {
    let mut c = vec![channel()];
    let mut cache = 7;
    let mut host = Host {
        remove: true,
        ..Host::default()
    };
    host.sequence.notifies = vec![notify(0.125, 4)];
    let r = update_lod_animation(&mut c, &mut cache, 0.25, false, &mut host).unwrap();
    assert_eq!(r.exit, TickExit::NotifyChannelRemoved);
    assert_eq!(r.replicated, 0);
    assert_eq!(cache, 7);
    assert_eq!(
        host.events,
        ["lock", "flags", "sequence 1", "actor mesh", "notify 4"]
    );
}
#[test]
fn notify_replacement_is_reacquired_for_sequence_and_pose() {
    let mut c = vec![channel()];
    let mut cache = 1;
    let mut host = Host {
        replace: true,
        ..Host::default()
    };
    host.sequence.notifies = vec![notify(0.125, 4)];
    let r = update_lod_animation(&mut c, &mut cache, 0.25, false, &mut host).unwrap();
    assert_eq!(r.attempts, 2);
    assert!(host.events.contains(&"sequence 2".into()));
    assert_eq!(c[0].words[7], 0.625f32.to_bits());
}
#[test]
fn zero_time_skips_initial_flags_and_channels_but_invalidates_cache() {
    for delta in [0.0, -0.0] {
        let mut c = vec![channel()];
        let before = c[0].words;
        let mut cache = 1;
        let mut host = Host::default();
        let r = update_lod_animation(&mut c, &mut cache, delta, false, &mut host).unwrap();
        assert_eq!(r.attempts, 0);
        assert_eq!(c[0].words, before);
        assert_eq!(cache, 0);
        assert_eq!(host.events, ["lock", "flags", "unlock"]);
    }
}
#[test]
fn initial_destroy_flag_skips_channels_but_final_flags_are_requeried() {
    let mut c = vec![channel()];
    let mut cache = 1;
    let mut host = Host {
        flags: VecDeque::from([2, 0]),
        ..Host::default()
    };
    let r = update_lod_animation(&mut c, &mut cache, 0.25, false, &mut host).unwrap();
    assert_eq!(r.exit, TickExit::Unlocked);
    assert_eq!(r.attempts, 0);
    assert_eq!(host.events, ["lock", "flags", "flags", "unlock"]);
    assert_eq!(cache, 0);
}
#[test]
fn final_destroy_flag_replaces_unlock_after_cache_clear() {
    let mut c = vec![channel()];
    let mut cache = 1;
    let mut host = Host {
        flags: VecDeque::from([0, 2]),
        ..Host::default()
    };
    let r = update_lod_animation(&mut c, &mut cache, 0.25, false, &mut host).unwrap();
    assert_eq!(r.exit, TickExit::DestroyRequested);
    assert_eq!(cache, 0);
    assert_eq!(host.events.last().unwrap(), "destroy");
    assert!(!host.events.contains(&"unlock".into()));
}
#[test]
fn sequence_error_retains_blend_and_skips_completion() {
    let mut c = vec![channel()];
    let mut cache = 7;
    let mut host = Host {
        fail: Some("sequence 1"),
        ..Host::default()
    };
    assert!(update_lod_animation(&mut c, &mut cache, 0.25, false, &mut host).is_err());
    assert_eq!(c[0].words[11], 0.25f32.to_bits());
    assert_eq!(c[0].words[7], 0);
    assert_eq!(cache, 7);
    assert_eq!(host.events, ["lock", "flags", "sequence 1"]);
}
#[test]
fn jitter_clamps_and_preserves_unordered_prior() {
    let mut c = channel();
    advance_channel_jitter(&mut c, 0.5, 1.0, 0);
    assert_eq!(c.words[8], (-0.5f32).to_bits());
    advance_channel_jitter(&mut c, 0.5, 1.0, 32767);
    assert_eq!(c.words[8], 0.5f32.to_bits());
    c.words[8] = f32::NAN.to_bits();
    advance_channel_jitter(&mut c, 0.5, 1.0, 0);
    assert!(f32::from_bits(c.words[8]).is_nan());
}
#[test]
fn positive_amplitude_calls_random_once_before_frame_progression() {
    let mut c = vec![channel()];
    let mut cache = 1;
    let mut host = Host::default();
    host.sequence.jitter_amplitude = 0.5;
    update_lod_animation(&mut c, &mut cache, 0.25, false, &mut host).unwrap();
    assert_eq!(c[0].words[8], 0.5f32.to_bits());
    assert_eq!(c[0].words[7], 0.375f32.to_bits());
    assert_eq!(
        host.events,
        [
            "lock",
            "flags",
            "sequence 1",
            "random",
            "replicate 0",
            "flags",
            "unlock"
        ]
    );
    for amplitude in [0.0, -1.0, f32::NAN] {
        let mut c = vec![channel()];
        let mut host = Host::default();
        host.sequence.jitter_amplitude = amplitude;
        update_lod_animation(&mut c, &mut cache, 0.25, false, &mut host).unwrap();
        assert!(!host.events.contains(&"random".into()));
    }
}
#[test]
fn random_error_keeps_frame_and_cache() {
    let mut c = vec![channel()];
    let mut cache = 7;
    let mut host = Host {
        fail: Some("random"),
        ..Host::default()
    };
    host.sequence.jitter_amplitude = 1.0;
    assert!(update_lod_animation(&mut c, &mut cache, 0.25, false, &mut host).is_err());
    assert_eq!(c[0].words[7], 0);
    assert_eq!(c[0].words[8], 0);
    assert_eq!(cache, 7);
}
#[test]
fn final_flags_error_happens_after_cache_invalidation() {
    let mut c = vec![channel()];
    let mut cache = 7;
    let mut host = Host {
        fail: Some("flags"),
        ..Host::default()
    };
    assert!(update_lod_animation(&mut c, &mut cache, 0.0, false, &mut host).is_err());
    assert_eq!(cache, 0);
}
#[test]
fn skeletal_wrapper_accrues_after_notify_early_exit() {
    let mut c = vec![channel()];
    let mut cache = 7;
    let mut host = Host {
        remove: true,
        ..Host::default()
    };
    host.sequence.notifies = vec![notify(0.125, 4)];
    let mut d = vec![BoneDirector { words: [0; 28] }];
    d[0].words[21] = 2f32.to_bits();
    let r = update_animation_with_lod(&mut d, 0.25, |_, delta| {
        update_lod_animation(&mut c, &mut cache, delta, false, &mut host)
    })
    .unwrap();
    assert_eq!(r.exit, TickExit::NotifyChannelRemoved);
    assert_eq!(d[0].words[26], 0.5f32.to_bits());
    assert_eq!(cache, 7);
}
#[test]
fn replication_can_append_a_channel_processed_in_the_same_tick() {
    let mut c = vec![channel()];
    let mut cache = 1;
    let mut host = Host {
        append: true,
        ..Host::default()
    };
    let r = update_lod_animation(&mut c, &mut cache, 0.25, false, &mut host).unwrap();
    assert_eq!(r.replicated, 2);
    assert_eq!(r.attempts, 2);
    assert_eq!(c[1].words[7], 0.25f32.to_bits());
}
#[test]
fn negative_or_nan_delta_skips_frames_after_channel_prefix() {
    for delta in [-0.25, f32::NAN] {
        let mut c = vec![channel()];
        let mut cache = 7;
        let mut host = Host::default();
        let r = update_lod_animation(&mut c, &mut cache, delta, false, &mut host).unwrap();
        assert_eq!(r.attempts, 0);
        assert_eq!(r.replicated, 1);
        assert_eq!(c[0].words[7], 0);
        assert_eq!(cache, 0);
    }
}

#[test]
fn nan_channel_rate_is_not_treated_as_zero() {
    let mut c = vec![channel()];
    c[0].words[6] = f32::NAN.to_bits();
    let mut cache = 1;
    let mut host = Host::default();
    let r = update_lod_animation(&mut c, &mut cache, 0.25, false, &mut host).unwrap();
    assert_eq!(r.attempts, 1);
    assert_eq!(r.replicated, 1);
    assert_eq!(c[0].words[7], c[0].words[9]);
    assert!(f32::from_bits(c[0].words[6]).is_nan());
}

#[test]
fn actor_mesh_is_queried_before_notify_suppression_byte() {
    let mut c = vec![channel()];
    c[0].words[1] = 0x100;
    let mut cache = 1;
    let mut host = Host::default();
    host.sequence.notifies = vec![notify(0.125, 4)];
    update_lod_animation(&mut c, &mut cache, 0.25, false, &mut host).unwrap();
    assert_eq!(
        host.events,
        [
            "lock",
            "flags",
            "sequence 1",
            "actor mesh",
            "replicate 0",
            "flags",
            "unlock"
        ]
    );
    assert_eq!(c[0].words[7], 0.25f32.to_bits());
}
