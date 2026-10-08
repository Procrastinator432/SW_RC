//! Original loaded sequence metadata adapter for the prepared LOD tick.
use crate::{
    mesh_animation::AnimationChannel,
    skeletal_animation_tick::{TickEndHost, TickNotify, TickNotifyHost},
    skeletal_lod_tick::{LodTickHost, TickSequence},
    skeletal_runtime_hosts::RuntimeAnimationBindings,
    skeletal_sequence::{SequenceLookupHost, SnapshotSequenceLookup},
};

/// Archive object indices are package-local, never native object pointers.
/// Sequence identity scopes a supplied resolution to its owning loaded package.
pub struct NotifyObjectBinding {
    pub sequence_identity: u32,
    pub object_index: i32,
    pub identity: u32,
}

pub trait RuntimeTickEnvironment: TickNotifyHost + TickEndHost {
    fn set_locked(&mut self, locked: bool) -> Result<(), String>;
    fn flags(&mut self) -> Result<u32, String>;
    fn request_destroy(&mut self) -> Result<(), String>;
    fn random_int(&mut self) -> Result<i32, String>;
    fn actor_has_mesh(&mut self) -> Result<bool, String>;
    fn replicate(
        &mut self,
        index: usize,
        channels: &mut Vec<AnimationChannel>,
    ) -> Result<(), String>;
}

pub struct RuntimeTickHost<'a, H> {
    pub data: RuntimeAnimationBindings<'a>,
    pub notify_objects: &'a [NotifyObjectBinding],
    pub environment: H,
}

impl<H> RuntimeTickHost<'_, H> {
    /// Resolve all non-null archive notify references before returning a snapshot.
    /// Missing/zero identities fail explicitly; names do not replace notify objects.
    pub fn loaded_sequence(&self, identity: u32) -> Result<TickSequence, String> {
        let sequence = self.data.sequence(identity)?.sequence;
        let mut notifies = Vec::with_capacity(sequence.notifies.len());
        for notify in &sequence.notifies {
            let object = if notify.object_index == 0 {
                0
            } else {
                let binding = self
                    .notify_objects
                    .iter()
                    .find(|binding| {
                        binding.sequence_identity == identity
                            && binding.object_index == notify.object_index
                    })
                    .ok_or("non-null archive notify object is not resolved for this sequence")?;
                if binding.identity == 0 {
                    return Err("non-null notify object resolved to null identity".into());
                }
                binding.identity
            };
            notifies.push(TickNotify {
                time_bits: notify.time_bits,
                object,
            });
        }
        Ok(TickSequence {
            frames: sequence.frames,
            rate: f32::from_bits(sequence.rate_bits),
            jitter_amplitude: f32::from_bits(sequence.word_18),
            notifies,
        })
    }
}
impl<H> SequenceLookupHost for RuntimeTickHost<'_, H> {
    fn find_sequence(&mut self, name: u32, load: bool) -> Result<u32, String> {
        SnapshotSequenceLookup {
            bindings: self.data.names,
        }
        .find_sequence(name, load)
    }
}
impl<H: RuntimeTickEnvironment> TickNotifyHost for RuntimeTickHost<'_, H> {
    fn notify(&mut self, object: u32, channels: &mut Vec<AnimationChannel>) -> Result<(), String> {
        self.environment.notify(object, channels)
    }
}
impl<H: RuntimeTickEnvironment> TickEndHost for RuntimeTickHost<'_, H> {
    fn clear(&mut self, snapshot: [u32; 8], channel: &mut AnimationChannel) -> Result<(), String> {
        self.environment.clear(snapshot, channel)
    }
    fn anim_end(&mut self, number: i32, channel: &mut AnimationChannel) -> Result<(), String> {
        self.environment.anim_end(number, channel)
    }
}
impl<H: RuntimeTickEnvironment> LodTickHost for RuntimeTickHost<'_, H> {
    fn set_locked(&mut self, value: bool) -> Result<(), String> {
        self.environment.set_locked(value)
    }
    fn flags(&mut self) -> Result<u32, String> {
        self.environment.flags()
    }
    fn request_destroy(&mut self) -> Result<(), String> {
        self.environment.request_destroy()
    }
    fn sequence(&mut self, identity: u32) -> Result<TickSequence, String> {
        self.loaded_sequence(identity)
    }
    fn random_int(&mut self) -> Result<i32, String> {
        self.environment.random_int()
    }
    fn actor_has_mesh(&mut self) -> Result<bool, String> {
        self.environment.actor_has_mesh()
    }
    fn replicate(
        &mut self,
        index: usize,
        channels: &mut Vec<AnimationChannel>,
    ) -> Result<(), String> {
        self.environment.replicate(index, channels)
    }
}

#[cfg(test)]
#[path = "skeletal_runtime_tick_tests.rs"]
mod tests;
