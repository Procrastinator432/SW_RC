//! Concrete loaded-sequence/channel/root/linkup hosts over resolved identity snapshots.
use crate::{
    mesh_animation::AnimationChannel,
    quaternion_animation::{QuaternionMath, QuaternionTrackHost},
    skeletal_animation::StoredSequence,
    skeletal_channel::{apply_channel, ChannelAngularMath, ChannelPoseInput, TrackChannelHost},
    skeletal_channel_stack::PreparedChannelStackHost,
    skeletal_mesh::{refresh_linkup, LinkupRefresh},
    skeletal_preparation::AnimationPreparationHost,
    skeletal_root_pose::{RootPoseHost, RootSampleRequest, RootSequence, RootTransform},
    skeletal_sequence::{get_sequence, SequenceLookupHost, SnapshotSequenceLookup},
    skeletal_track::{find_linkup, sample_track},
};
use serde::Serialize;
use std::{cell::RefCell, rc::Rc};
pub struct ResolvedSequence<'a> {
    pub identity: u32,
    pub linkup_key: u32,
    pub sequence: &'a StoredSequence,
}
#[derive(Clone, Debug, Serialize)]
pub struct ResolvedLinkup {
    pub key: u32,
    pub animation_names: Option<Vec<u32>>,
    pub mapping: Vec<i32>,
}
#[derive(Debug, Serialize)]
pub enum RuntimeAnimationEvent {
    Transform,
    Refresh { index: usize, result: LinkupRefresh },
    Lookup { name: u32, load: bool, result: u32 },
    Sequence { channel: usize, identity: u32 },
    Linkup { key: u32, index: usize },
    RootSample { track: i32, time_bits: u32 },
}
/// Shared mapping state lets preparation and pose hosts observe the same refreshed arrays.
/// This single-threaded context does not represent native pointer addresses or asset loading.
#[derive(Clone)]
pub struct RuntimeAnimationBindings<'a> {
    pub sequences: &'a [ResolvedSequence<'a>],
    pub names: &'a [(u32, u32)],
    pub linkups: Rc<RefCell<Vec<ResolvedLinkup>>>,
    pub events: Rc<RefCell<Vec<RuntimeAnimationEvent>>>,
}
impl RuntimeAnimationBindings<'_> {
    pub(crate) fn sequence(&self, identity: u32) -> Result<&ResolvedSequence<'_>, String> {
        if identity == 0 {
            return Err("null sequence cannot be dereferenced".into());
        }
        self.sequences
            .iter()
            .find(|s| s.identity == identity)
            .ok_or("non-null sequence identity is not loaded".into())
    }
    fn resolve_channel(
        &self,
        index: usize,
        channels: &mut [AnimationChannel],
        editor: bool,
    ) -> Result<u32, String> {
        let index32 = i32::try_from(index).map_err(|_| "channel index exceeds signed range")?;
        let mut lookup = RuntimeLookup { data: self };
        let identity = get_sequence(channels, index32, editor, &mut lookup)?;
        self.events
            .borrow_mut()
            .push(RuntimeAnimationEvent::Sequence {
                channel: index,
                identity,
            });
        Ok(identity)
    }
    fn mapping(&self, key: u32) -> Result<Vec<i32>, String> {
        let linkups = self.linkups.borrow();
        let entries: Vec<_> = linkups.iter().map(|l| [0, l.key, 0, 0]).collect();
        let index = find_linkup(&entries, Some(key))
            .ok_or("sequence has no matching mesh linkup".to_owned())?;
        self.events
            .borrow_mut()
            .push(RuntimeAnimationEvent::Linkup { key, index });
        Ok(linkups[index].mapping.clone())
    }
}
struct RuntimeLookup<'a, 'b> {
    data: &'a RuntimeAnimationBindings<'b>,
}
impl SequenceLookupHost for RuntimeLookup<'_, '_> {
    fn find_sequence(&mut self, name: u32, load: bool) -> Result<u32, String> {
        let result = SnapshotSequenceLookup {
            bindings: self.data.names,
        }
        .find_sequence(name, load)?;
        self.data
            .events
            .borrow_mut()
            .push(RuntimeAnimationEvent::Lookup { name, load, result });
        Ok(result)
    }
}
pub struct RuntimePreparationHost<'a> {
    pub data: RuntimeAnimationBindings<'a>,
    pub mesh_names: &'a [u32],
    /// Resolved scene transform remains supplied; actor virtual +e8 is not reconstructed here.
    pub mesh_to_world: [u32; 16],
}
impl AnimationPreparationHost for RuntimePreparationHost<'_> {
    fn mesh_to_world(&mut self) -> Result<[u32; 16], String> {
        self.data
            .events
            .borrow_mut()
            .push(RuntimeAnimationEvent::Transform);
        Ok(self.mesh_to_world)
    }
    fn refresh_linkup(&mut self, index: usize) -> Result<(), String> {
        let mut entries = self.data.linkups.borrow_mut();
        let e = entries
            .get_mut(index)
            .ok_or("runtime linkup index out of range")?;
        let result = refresh_linkup(
            &mut e.mapping,
            self.mesh_names,
            e.animation_names.as_deref(),
        );
        self.data
            .events
            .borrow_mut()
            .push(RuntimeAnimationEvent::Refresh { index, result });
        Ok(())
    }
}
pub struct RuntimePoseHost<'a, M, A> {
    pub data: RuntimeAnimationBindings<'a>,
    pub reference: &'a [RootTransform],
    pub move_bone: i32,
    pub editor: bool,
    pub math: M,
    pub angular: A,
}
struct MathRef<'a, M>(&'a mut M);
impl<M: QuaternionMath> QuaternionMath for MathRef<'_, M> {
    fn reciprocal_sqrt_seed(&mut self, n: f32) -> Result<f32, String> {
        self.0.reciprocal_sqrt_seed(n)
    }
    fn spherical_weights(&mut self, d: f32, a: f32) -> Result<[f32; 2], String> {
        self.0.spherical_weights(d, a)
    }
}
struct AngularRef<'a, A>(&'a mut A);
impl<A: ChannelAngularMath> ChannelAngularMath for AngularRef<'_, A> {
    fn limit(
        &mut self,
        o: [u32; 4],
        c: [u32; 4],
        p: [u32; 4],
        a: f32,
    ) -> Result<Option<f32>, String> {
        self.0.limit(o, c, p, a)
    }
}
impl<M: QuaternionMath, A> RootPoseHost for RuntimePoseHost<'_, M, A> {
    fn sequence(
        &mut self,
        index: usize,
        channels: &mut [AnimationChannel],
    ) -> Result<Option<RootSequence>, String> {
        let identity = self.data.resolve_channel(index, channels, self.editor)?;
        if identity == 0 {
            return Ok(None);
        }
        let s = self.data.sequence(identity)?.sequence;
        Ok(Some(RootSequence {
            token: identity,
            frames: s.frames,
            track_count_word: u32::try_from(s.tracks.len())
                .map_err(|_| "too many loaded tracks")?,
        }))
    }
    fn root_track(&mut self, s: RootSequence) -> Result<i32, String> {
        let resolved = self.data.sequence(s.token)?;
        self.data
            .mapping(resolved.linkup_key)?
            .first()
            .copied()
            .ok_or("root linkup mapping is empty".into())
    }
    fn sample(
        &mut self,
        request: RootSampleRequest,
        root: &mut RootTransform,
    ) -> Result<(), String> {
        let sequence = self.data.sequence(request.sequence_token)?.sequence;
        let index = usize::try_from(request.track).map_err(|_| "negative requested root track")?;
        let track = sequence
            .tracks
            .get(index)
            .ok_or("requested root track is not loaded")?;
        // Explicit portable approximation of FILD/FMUL/FSTP; no x87 control-word emulation.
        let time = (request.frames as f64 * request.normalized_frame as f64) as f32;
        self.data
            .events
            .borrow_mut()
            .push(RuntimeAnimationEvent::RootSample {
                track: request.track,
                time_bits: time.to_bits(),
            });
        sample_track(
            track,
            time,
            root,
            &mut QuaternionTrackHost {
                math: MathRef(&mut self.math),
            },
        )?;
        Ok(())
    }
}
impl<M: QuaternionMath, A: ChannelAngularMath> PreparedChannelStackHost
    for RuntimePoseHost<'_, M, A>
{
    fn apply(
        &mut self,
        index: usize,
        channels: &mut [AnimationChannel],
        previous: &[RootTransform],
        scratch: &mut [RootTransform],
    ) -> Result<bool, String> {
        let identity = self.data.resolve_channel(index, channels, self.editor)?;
        let loaded = if identity == 0 {
            None
        } else {
            Some(self.data.sequence(identity)?)
        };
        let valid = loaded.filter(|s| !s.sequence.tracks.is_empty());
        let mapping = if let Some(s) = valid {
            self.data.mapping(s.linkup_key)?
        } else {
            vec![]
        };
        let sequence = valid.map(|s| s.sequence);
        let channel = channels
            .get_mut(index)
            .ok_or("runtime channel index out of range")?;
        apply_channel(
            channel,
            index,
            ChannelPoseInput {
                frames: sequence.map(|s| s.frames),
                mapping: &mapping,
                reference: self.reference,
                previous,
                move_bone: self.move_bone,
            },
            scratch,
            &mut TrackChannelHost {
                tracks: sequence.map_or(&[], |s| s.tracks.as_slice()),
                math: MathRef(&mut self.math),
                angular: AngularRef(&mut self.angular),
            },
        )
    }
}
#[cfg(test)]
#[path = "skeletal_runtime_hosts_tests.rs"]
mod tests;
