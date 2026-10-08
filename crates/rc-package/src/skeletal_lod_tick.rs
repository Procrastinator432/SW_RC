//! Prepared ULodMeshInstance::UpdateAnimation (10450240) with explicit host calls.
//! Native object ownership and end-callback channel relocation remain external.
use crate::{
    mesh_animation::AnimationChannel,
    skeletal_animation_tick::{
        advance_channel_blend, advance_channel_frame, dispatch_tick_end, dispatch_tick_notify,
        NotifyTransition, TickEndHost, TickNotify, TickNotifyHost,
    },
    skeletal_sequence::{get_sequence, SequenceLookupHost},
};

#[derive(Clone, Debug)]
pub struct TickSequence {
    pub frames: i32,
    pub rate: f32,
    pub jitter_amplitude: f32,
    pub notifies: Vec<TickNotify>,
}

/// Call boundaries stand in for original engine objects, not invented gameplay.
pub trait LodTickHost: SequenceLookupHost + TickNotifyHost + TickEndHost {
    fn set_locked(&mut self, locked: bool) -> Result<(), String>;
    fn flags(&mut self) -> Result<u32, String>;
    fn request_destroy(&mut self) -> Result<(), String>;
    /// Identity is non-null. Unknown identities must report an error.
    fn sequence(&mut self, identity: u32) -> Result<TickSequence, String>;
    fn random_int(&mut self) -> Result<i32, String>;
    fn actor_has_mesh(&mut self) -> Result<bool, String>;
    /// Native Actor::ReplicateAnim(index,index,channel,true).
    fn replicate(
        &mut self,
        index: usize,
        channels: &mut Vec<AnimationChannel>,
    ) -> Result<(), String>;
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum TickExit {
    Unlocked,
    DestroyRequested,
    /// Intentionally skips common cache invalidation and final lifecycle calls.
    NotifyChannelRemoved,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct LodTickResult {
    pub exit: TickExit,
    pub attempts: usize,
    pub replicated: usize,
}

/// 10450447..104504a8. Constants verified in original engine.dll.
/// The caller performs the positive-amplitude gate before obtaining random input.
pub fn advance_channel_jitter(
    channel: &mut AnimationChannel,
    amplitude: f32,
    remaining: f32,
    random: i32,
) {
    let prior = f32::from_bits(channel.words[8]);
    let scaled = random as f32 * f32::from_bits(0x38800100);
    let mut jitter = (((scaled - 1.0) * amplitude) * remaining) * 10.0 + prior;
    let lower = 0.0 - amplitude;
    if lower >= jitter {
        jitter = lower;
    }
    if jitter >= amplitude {
        jitter = amplitude;
    }
    channel.words[8] = jitter.to_bits();
}

/// Runs the prepared channel loop; editor selects cached versus name lookup.
/// Errors retain earlier writes and do not synthesize native cleanup/unlock.
/// End callbacks can mutate the selected channel's fields; replacing its storage
/// during clear/AnimEnd is outside this safe fixed-reference host contract.
pub fn update_lod_animation(
    channels: &mut Vec<AnimationChannel>,
    byte_61: &mut u8,
    delta: f32,
    editor: bool,
    host: &mut impl LodTickHost,
) -> Result<LodTickResult, String> {
    host.set_locked(true)?;
    let mut result = LodTickResult {
        exit: TickExit::Unlocked,
        attempts: 0,
        replicated: 0,
    };
    // Zero short-circuits the initial flags query, but not common completion.
    if delta != 0.0 && host.flags()? & 2 == 0 {
        let mut index = 0;
        while index < channels.len() {
            advance_channel_blend(&mut channels[index], delta);
            let mut remaining = delta;
            let mut attempts = 0;
            loop {
                let channel = &channels[index];
                if channel.words[0] == 0
                    || f32::from_bits(channel.words[6]) == 0.0
                    || remaining.partial_cmp(&0.0) != Some(std::cmp::Ordering::Greater)
                {
                    break;
                }
                attempts += 1;
                if attempts > 4 {
                    break;
                }
                result.attempts += 1;
                let old = f32::from_bits(channel.words[7]);
                let signed_index = i32::try_from(index)
                    .map_err(|_| "channel index exceeds native signed range")?;
                let identity = get_sequence(channels, signed_index, editor, host)?;
                if identity == 0 {
                    continue;
                }
                let sequence = host.sequence(identity)?;
                if sequence.jitter_amplitude > 0.0 {
                    let random = host.random_int()?;
                    advance_channel_jitter(
                        &mut channels[index],
                        sequence.jitter_amplitude,
                        remaining,
                        random,
                    );
                }
                advance_channel_frame(
                    &mut channels[index],
                    sequence.frames,
                    sequence.rate,
                    remaining,
                );
                // Native checks actor mesh before channel suppression, only if notifies exist.
                let has_mesh = !sequence.notifies.is_empty() && host.actor_has_mesh()?;
                match dispatch_tick_notify(
                    channels,
                    index,
                    old,
                    remaining,
                    has_mesh,
                    &sequence.notifies,
                    host,
                )? {
                    NotifyTransition::Continue { remaining: rest } => {
                        remaining = rest;
                        continue;
                    }
                    NotifyTransition::ChannelRemoved { .. } => {
                        result.exit = TickExit::NotifyChannelRemoved;
                        return Ok(result);
                    }
                    NotifyTransition::None => {}
                }
                let end = dispatch_tick_end(&mut channels[index], old, remaining, host)?;
                if !end.reached_end {
                    break;
                }
                remaining = end.remaining;
            }
            host.replicate(index, channels)?;
            result.replicated += 1;
            index += 1;
        }
    }
    *byte_61 = 0;
    if host.flags()? & 2 != 0 {
        host.request_destroy()?;
        result.exit = TickExit::DestroyRequested;
    } else {
        host.set_locked(false)?;
    }
    Ok(result)
}

#[cfg(test)]
#[path = "skeletal_lod_tick_tests.rs"]
mod tests;
