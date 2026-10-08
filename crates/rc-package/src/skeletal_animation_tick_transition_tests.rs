use super::*;

fn channel(old_frame: f32, end: f32, looping: bool) -> AnimationChannel {
    let mut c = AnimationChannel { words: [0; 18] };
    c.words[0] = 12;
    c.words[1] = u32::from(looping);
    c.words[6] = 2f32.to_bits();
    c.words[7] = old_frame.to_bits();
    c.words[9] = end.to_bits();
    c
}
fn notify(time: f32, object: u32) -> TickNotify {
    TickNotify {
        time_bits: time.to_bits(),
        object,
    }
}
#[derive(Default)]
struct Host {
    calls: Vec<String>,
    clear_snapshot: Option<[u32; 8]>,
    remove: bool,
    replace: bool,
    suppress_after_clear: bool,
    fail: bool,
}
impl TickNotifyHost for Host {
    fn notify(&mut self, object: u32, channels: &mut Vec<AnimationChannel>) -> Result<(), String> {
        self.calls.push(format!(
            "notify {object} {}",
            f32::from_bits(channels[0].words[7])
        ));
        if self.remove {
            channels.clear();
        }
        if self.replace {
            *channels = vec![channel(0.875, 1.0, true)];
        }
        if self.fail {
            return Err("notify failed".into());
        }
        Ok(())
    }
}
impl TickEndHost for Host {
    fn clear(&mut self, snapshot: [u32; 8], c: &mut AnimationChannel) -> Result<(), String> {
        self.calls.push(format!(
            "clear {} {}",
            f32::from_bits(c.words[7]),
            f32::from_bits(c.words[6])
        ));
        self.clear_snapshot = Some(snapshot);
        if self.suppress_after_clear {
            c.words[1] |= 0x100;
        }
        if self.fail {
            return Err("clear failed".into());
        }
        Ok(())
    }
    fn anim_end(&mut self, number: i32, c: &mut AnimationChannel) -> Result<(), String> {
        self.calls
            .push(format!("end {number} {}", f32::from_bits(c.words[6])));
        if self.fail {
            return Err("end failed".into());
        }
        Ok(())
    }
}
#[test]
fn notify_selection_unsorted_exclusive_start_inclusive_end_first_tie() {
    let n = [
        notify(0.75, 1),
        notify(0.5, 2),
        notify(0.5, 3),
        notify(0.25, 4),
    ];
    assert_eq!(select_tick_notify(&n, 0.25, 0.75), Some(1));
    assert_eq!(select_tick_notify(&n, 0.5, 0.75), Some(0));
    assert_eq!(select_tick_notify(&n, 0.75, 0.75), None);
    assert_eq!(select_tick_notify(&n, 0.75, 0.25), None);
}
#[test]
fn notify_selection_rejects_nan_and_keeps_infinite_distance_first() {
    let n = [notify(f32::NAN, 1), notify(0.75, 2), notify(0.25, 3)];
    assert_eq!(select_tick_notify(&n, 0.0, f32::NAN), None);
    assert_eq!(select_tick_notify(&n, f32::NAN, 1.0), None);
    assert_eq!(select_tick_notify(&n, f32::NEG_INFINITY, 1.0), Some(1));
}
#[test]
fn notify_commits_time_before_callback_and_preserves_replacement() {
    let mut c = vec![channel(0.75, 1.0, false)];
    let mut h = Host {
        replace: true,
        ..Host::default()
    };
    let result =
        dispatch_tick_notify(&mut c, 0, 0.25, 2.0, true, &[notify(0.5, 7)], &mut h).unwrap();
    assert_eq!(result, NotifyTransition::Continue { remaining: 1.0 });
    assert_eq!(h.calls, ["notify 7 0.5"]);
    assert_eq!(c[0].words[7], 0.875f32.to_bits());
}
#[test]
fn notify_removal_has_explicit_early_return() {
    let mut c = vec![channel(0.75, 1.0, false)];
    let mut h = Host {
        remove: true,
        ..Host::default()
    };
    assert_eq!(
        dispatch_tick_notify(&mut c, 0, 0.25, 2.0, true, &[notify(0.5, 7)], &mut h).unwrap(),
        NotifyTransition::ChannelRemoved { remaining: 1.0 }
    );
    assert!(c.is_empty());
}
#[test]
fn null_notify_still_consumes_time_and_copies_negative_zero() {
    let mut c = vec![channel(1.0, 1.0, false)];
    let mut h = Host::default();
    assert_eq!(
        dispatch_tick_notify(&mut c, 0, -1.0, 2.0, true, &[notify(-0.0, 0)], &mut h).unwrap(),
        NotifyTransition::Continue { remaining: 1.0 }
    );
    assert_eq!(c[0].words[7], (-0.0f32).to_bits());
    assert!(h.calls.is_empty());
}
#[test]
fn notify_gates_and_errors_preserve_expected_state() {
    let mut c = vec![channel(0.75, 1.0, false)];
    let mut h = Host::default();
    assert_eq!(
        dispatch_tick_notify(&mut c, 0, 0.25, 1.0, false, &[notify(0.5, 1)], &mut h).unwrap(),
        NotifyTransition::None
    );
    c[0].words[1] = 0x100;
    assert_eq!(
        dispatch_tick_notify(&mut c, 0, 0.25, 1.0, true, &[notify(0.5, 1)], &mut h).unwrap(),
        NotifyTransition::None
    );
    assert_eq!(c[0].words[7], 0.75f32.to_bits());
    c[0].words[1] = 0;
    h.fail = true;
    assert!(dispatch_tick_notify(&mut c, 0, 0.25, 1.0, true, &[notify(0.5, 1)], &mut h).is_err());
    assert_eq!(c[0].words[7], 0.5f32.to_bits());
    assert!(dispatch_tick_notify(&mut c, 1, 0.25, 1.0, true, &[], &mut h).is_err());
}
#[test]
fn nonloop_clear_stop_end_order_and_snapshot() {
    let mut c = channel(1.25, 0.75, false);
    c.words[3] = 3;
    let mut h = Host::default();
    let r = dispatch_tick_end(&mut c, 0.25, 2.0, &mut h).unwrap();
    assert_eq!(
        r,
        EndTransition {
            reached_end: true,
            remaining: 1.0,
            cleared: true,
            stopped: true,
            notified: true
        }
    );
    assert_eq!(h.calls, ["clear 0.75 2", "end 3 0"]);
    let s = h.clear_snapshot.unwrap();
    assert_eq!(s[0], 0);
    assert_eq!(s[6], 2f32.to_bits());
    assert_eq!(s[7], 0.75f32.to_bits());
    assert_eq!(c.words[0], 12);
    assert_eq!(c.words[6], 0);
}
#[test]
fn clear_mutation_changes_suppression_and_failure_keeps_rate() {
    let mut c = channel(1.25, 0.75, false);
    c.words[15] = 1;
    let mut h = Host {
        suppress_after_clear: true,
        ..Host::default()
    };
    let r = dispatch_tick_end(&mut c, 0.25, 2.0, &mut h).unwrap();
    assert!(r.cleared && r.stopped && !r.notified);
    let mut c = channel(1.25, 0.75, false);
    c.words[3] = 1;
    h.fail = true;
    assert!(dispatch_tick_end(&mut c, 0.25, 2.0, &mut h).is_err());
    assert_eq!(c.words[7], 0.75f32.to_bits());
    assert_eq!(c.words[6], 2f32.to_bits());
}
#[test]
fn before_end_and_nonpositive_rates_do_not_emit_end() {
    let mut h = Host::default();
    let mut c = channel(0.5, 0.75, false);
    assert!(
        !dispatch_tick_end(&mut c, 0.25, 1.0, &mut h)
            .unwrap()
            .reached_end
    );
    for rate in [0.0, -1.0, f32::NAN] {
        let mut c = channel(1.0, 0.75, false);
        c.words[6] = rate.to_bits();
        let r = dispatch_tick_end(&mut c, 0.25, 1.0, &mut h).unwrap();
        assert!(r.reached_end && !r.stopped && !r.notified);
        assert_eq!(c.words[6], rate.to_bits());
    }
    assert!(h.calls.is_empty());
}
#[test]
fn looping_end_before_one_notifies_without_wrapping_then_wraps() {
    let mut h = Host::default();
    let mut c = channel(0.875, 0.75, true);
    let r = dispatch_tick_end(&mut c, 0.5, 2.0, &mut h).unwrap();
    assert_eq!(r.remaining, 0.0);
    assert!(r.notified && !r.stopped);
    assert_eq!(c.words[7], 0.875f32.to_bits());
    c.words[7] = 1.25f32.to_bits();
    let r = dispatch_tick_end(&mut c, 0.875, 3.0, &mut h).unwrap();
    assert_eq!(r.remaining, 2.0);
    assert!(!r.notified);
    assert_eq!(c.words[7], 0);
}
#[test]
fn looping_nan_takes_unordered_wrap_and_end_nan_does_not_notify() {
    let mut h = Host::default();
    let mut c = channel(f32::NAN, 0.75, true);
    let r = dispatch_tick_end(&mut c, 0.5, 1.0, &mut h).unwrap();
    assert!(r.reached_end && r.remaining.is_nan() && r.notified);
    assert_eq!(c.words[7], 0);
    let mut c = channel(1.0, f32::NAN, true);
    assert!(
        !dispatch_tick_end(&mut c, 0.5, 1.0, &mut h)
            .unwrap()
            .notified
    );
}
#[test]
fn nonloop_copies_raw_end_and_end_error_keeps_stopped_state() {
    let mut h = Host {
        fail: true,
        ..Host::default()
    };
    let mut c = channel(0.5, -0.0, false);
    assert!(dispatch_tick_end(&mut c, -0.5, 1.0, &mut h).is_err());
    assert_eq!(c.words[7], (-0.0f32).to_bits());
    assert_eq!(c.words[6], 0);
}

#[test]
fn composed_frame_notify_and_loop_steps_consume_one_tick_without_duplicate_notifies() {
    let mut channels = vec![channel(0.0, 0.9, true)];
    channels[0].words[6] = 1f32.to_bits();
    let mut host = Host::default();
    let notifies = [notify(0.75, 2), notify(0.25, 1)];
    let mut remaining = 1.2;
    let mut steps = 0;
    while remaining > 0.0 && steps < 4 {
        steps += 1;
        let old = f32::from_bits(channels[0].words[7]);
        advance_channel_frame(&mut channels[0], 30, 30.0, remaining);
        match dispatch_tick_notify(&mut channels, 0, old, remaining, true, &notifies, &mut host)
            .unwrap()
        {
            NotifyTransition::Continue { remaining: rest } => {
                remaining = rest;
                continue;
            }
            NotifyTransition::None => {}
            NotifyTransition::ChannelRemoved { .. } => panic!("unexpected removal"),
        }
        let end = dispatch_tick_end(&mut channels[0], old, remaining, &mut host).unwrap();
        if !end.reached_end {
            break;
        }
        remaining = end.remaining;
    }
    assert_eq!(steps, 4);
    assert_eq!(host.calls, ["notify 1 0.25", "notify 2 0.75", "end 0 1"]);
    assert!((f32::from_bits(channels[0].words[7]) - 0.2).abs() < 0.000001);
}
