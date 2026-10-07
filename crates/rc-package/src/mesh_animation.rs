//! Reviewed mesh animation entry and skeletal channel selection on supplied snapshots.
//! No package skeleton decoder, channel playback/reset, pose evaluation or allocator ABI.
use crate::animation_call::PlayAnimParameters;
use serde::{Deserialize, Serialize};

#[derive(Clone, Copy, Debug, Serialize, Deserialize)]
pub struct ReferenceBone {
    pub name_handle: u32,
    /// Raw signed value at FMeshBone +0x38; meaning not inferred.
    pub word_38: i32,
}
#[derive(Clone, Copy, Debug, Serialize, Deserialize)]
pub struct BoneAlias {
    pub name_handle: u32,
    pub target_handle: u32,
}
#[derive(Debug, Serialize, Deserialize)]
pub struct SkeletonSnapshot {
    pub bones: Vec<ReferenceBone>,
    pub aliases: Vec<BoneAlias>,
}
impl SkeletonSnapshot {
    /// MatchRefBone with a null output matrix. Alias lookup is one pass, not recursive.
    pub fn match_ref_bone(&self, name: u32) -> Option<usize> {
        if name == 0 {
            return None;
        }
        let target = self
            .aliases
            .iter()
            .find(|a| a.name_handle == name)
            .map_or(name, |a| a.target_handle);
        self.bones.iter().position(|b| b.name_handle == target)
    }
}

/// Exact 0x48-byte channel words. Existing values survive channel insertion/reuse.
#[derive(Clone, Debug, Default, PartialEq, Eq, Serialize, Deserialize)]
pub struct AnimationChannel {
    pub words: [u32; 18],
}
impl AnimationChannel {
    pub fn bone_handle(&self) -> u32 {
        self.words[2]
    }
    pub fn channel(&self) -> i32 {
        self.words[3] as i32
    }
    pub fn bone_index(&self) -> i32 {
        self.words[15] as i32
    }
}
#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize)]
pub struct ChannelSelection {
    pub index: usize,
    pub inserted: bool,
}

/// USkeletalMeshInstance.GetChannel. A missing skeleton at the unconditional
/// root-bone fallback is unresolved, rather than guessed into a native null result.
pub fn skeletal_channel(
    channels: &mut Vec<AnimationChannel>,
    skeleton: Option<&SkeletonSnapshot>,
    parameters: &PlayAnimParameters,
) -> Result<Option<ChannelSelection>, String> {
    let bone = if parameters.bone.handle == 0 || channels.is_empty() {
        skeleton
            .and_then(|s| s.bones.first())
            .ok_or("root bone unavailable at native fallback")?
            .name_handle
    } else {
        parameters.bone.handle
    };
    if let Some(index) = channels
        .iter()
        .position(|c| c.bone_handle() == bone && c.channel() == parameters.channel)
    {
        return Ok(Some(ChannelSelection {
            index,
            inserted: false,
        }));
    }
    let Some(skeleton) = skeleton else {
        return Ok(None);
    };
    let Some(bone_index) = skeleton.match_ref_bone(bone) else {
        return Ok(None);
    };
    let signed_index = i32::try_from(bone_index)
        .map_err(|_| "reference bone index exceeds native signed range")?;
    let index = channels
        .iter()
        .position(|c| {
            parameters.channel < c.channel()
                || (parameters.channel == c.channel() && signed_index < c.bone_index())
        })
        .unwrap_or(channels.len());
    let mut channel = AnimationChannel::default();
    channel.words[2] = bone;
    channel.words[3] = parameters.channel as u32;
    channel.words[15] = signed_index as u32;
    channel.words[16] = skeleton.bones[bone_index]
        .word_38
        .wrapping_add(1)
        .wrapping_add(signed_index) as u32;
    channels.insert(index, channel);
    Ok(Some(ChannelSelection {
        index,
        inserted: true,
    }))
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize)]
pub enum LodPlayResult {
    MissingSequence,
    NoActor,
    NoChannel,
    Playback(bool),
}
pub trait LodAnimationHost {
    /// The original passes !GIsEditor to mesh virtual +0xac.
    fn find_sequence(&mut self, name_handle: u32, load: bool) -> Result<bool, String>;
    /// Missing-sequence warning cache and logging remain a host boundary.
    fn missing_sequence(&mut self, name_handle: u32) -> Result<(), String>;
    fn actor_present(&mut self) -> Result<bool, String>;
    fn get_channel(&mut self, parameters: &PlayAnimParameters) -> Result<Option<usize>, String>;
    /// Native continuation includes blending, channel updates, PostInitAnim and replication.
    fn continue_playback(
        &mut self,
        channel: usize,
        sequence_found: bool,
        parameters: &PlayAnimParameters,
    ) -> Result<bool, String>;
}
pub fn lod_play_anim(
    parameters: &PlayAnimParameters,
    is_editor: bool,
    host: &mut impl LodAnimationHost,
) -> Result<LodPlayResult, String> {
    let found = host.find_sequence(parameters.sequence.handle, !is_editor)?;
    if !found && parameters.sequence.handle != 0 {
        host.missing_sequence(parameters.sequence.handle)?;
        return Ok(LodPlayResult::MissingSequence);
    }
    if !host.actor_present()? {
        return Ok(LodPlayResult::NoActor);
    }
    let Some(channel) = host.get_channel(parameters)? else {
        return Ok(LodPlayResult::NoChannel);
    };
    Ok(LodPlayResult::Playback(
        host.continue_playback(channel, found, parameters)?,
    ))
}

#[cfg(test)]
mod tests {
    use super::*;
    fn parameters(bone: u32, channel: i32) -> PlayAnimParameters {
        PlayAnimParameters {
            sequence: crate::event_lookup::EventNameSnapshot {
                handle: 337,
                resolved_index: 336,
            },
            looping: false,
            bone: crate::event_lookup::EventNameSnapshot {
                handle: bone,
                resolved_index: 0,
            },
            channel,
            float_10: 1.0,
            rate: 1.0,
            start_frame: -1.0,
        }
    }
    fn skeleton() -> SkeletonSnapshot {
        SkeletonSnapshot {
            bones: vec![
                ReferenceBone {
                    name_handle: 10,
                    word_38: 2,
                },
                ReferenceBone {
                    name_handle: 20,
                    word_38: 1,
                },
                ReferenceBone {
                    name_handle: 30,
                    word_38: 0,
                },
            ],
            aliases: vec![
                BoneAlias {
                    name_handle: 40,
                    target_handle: 20,
                },
                BoneAlias {
                    name_handle: 50,
                    target_handle: 40,
                },
            ],
        }
    }
    #[test]
    fn first_channel_always_uses_root_and_zeroes_remaining_words() {
        let mut channels = Vec::new();
        let s = skeleton();
        let chosen = skeletal_channel(&mut channels, Some(&s), &parameters(999, -3))
            .unwrap()
            .unwrap();
        assert_eq!(
            chosen,
            ChannelSelection {
                index: 0,
                inserted: true
            }
        );
        let mut expected = [0; 18];
        expected[2] = 10;
        expected[3] = (-3i32) as u32;
        expected[16] = 3;
        assert_eq!(channels[0].words, expected);
    }
    #[test]
    fn none_root_reuse_keeps_all_existing_words_without_skeleton_lookup() {
        let s = skeleton();
        let mut c = AnimationChannel {
            words: [0x12345678; 18],
        };
        c.words[2] = 10;
        c.words[3] = 7;
        let mut channels = vec![c.clone()];
        assert_eq!(
            skeletal_channel(&mut channels, Some(&s), &parameters(0, 7)).unwrap(),
            Some(ChannelSelection {
                index: 0,
                inserted: false
            })
        );
        assert_eq!(channels, [c.clone()]);
        assert_eq!(
            skeletal_channel(&mut channels, None, &parameters(10, 7)).unwrap(),
            Some(ChannelSelection {
                index: 0,
                inserted: false
            })
        );
        assert_eq!(channels, [c]);
    }
    #[test]
    fn aliases_first_match_no_recursion_and_missing_bones() {
        let mut s = skeleton();
        s.aliases.push(BoneAlias {
            name_handle: 40,
            target_handle: 30,
        });
        assert_eq!(s.match_ref_bone(40), Some(1));
        assert_eq!(s.match_ref_bone(50), None);
        assert_eq!(s.match_ref_bone(0), None);
        let mut channels = vec![AnimationChannel::default()];
        let chosen = skeletal_channel(&mut channels, Some(&s), &parameters(40, 0))
            .unwrap()
            .unwrap();
        assert_eq!(
            (
                channels[chosen.index].bone_handle(),
                channels[chosen.index].bone_index(),
                channels[chosen.index].words[16]
            ),
            (40, 1, 3)
        );
        let before = channels.clone();
        assert!(
            skeletal_channel(&mut channels, Some(&s), &parameters(999, 0))
                .unwrap()
                .is_none()
        );
        assert_eq!(channels, before);
    }
    #[test]
    fn insert_order_is_signed_channel_then_bone_index_and_preserves_payload() {
        let s = skeleton();
        let mut channels = vec![];
        skeletal_channel(&mut channels, Some(&s), &parameters(30, 5)).unwrap(); // empty forces root
        skeletal_channel(&mut channels, Some(&s), &parameters(30, 5)).unwrap();
        channels[1].words[7] = 0xdeadbeef;
        skeletal_channel(&mut channels, Some(&s), &parameters(20, 5)).unwrap();
        skeletal_channel(&mut channels, Some(&s), &parameters(20, -1)).unwrap();
        assert_eq!(
            channels
                .iter()
                .map(|c| (c.channel(), c.bone_index()))
                .collect::<Vec<_>>(),
            [(-1, 1), (5, 0), (5, 1), (5, 2)]
        );
        assert_eq!(channels[3].words[7], 0xdeadbeef);
    }
    #[test]
    fn invalid_root_stops_before_mutation_and_native_word_add_wraps() {
        let mut channels = vec![];
        assert!(skeletal_channel(&mut channels, None, &parameters(20, 0)).is_err());
        assert!(channels.is_empty());
        let mut s = skeleton();
        s.bones[0].word_38 = i32::MAX;
        skeletal_channel(&mut channels, Some(&s), &parameters(0, 0)).unwrap();
        assert_eq!(channels[0].words[16], 0x80000000);
    }
    struct Host {
        events: Vec<String>,
        found: bool,
        actor: bool,
        channel: Option<usize>,
        fail: Option<&'static str>,
    }
    impl Host {
        fn event(&mut self, name: &'static str) -> Result<(), String> {
            self.events.push(name.into());
            if self.fail == Some(name) {
                Err(name.into())
            } else {
                Ok(())
            }
        }
    }
    impl LodAnimationHost for Host {
        fn find_sequence(&mut self, _: u32, load: bool) -> Result<bool, String> {
            self.event(if load { "find-load" } else { "find-editor" })?;
            Ok(self.found)
        }
        fn missing_sequence(&mut self, _: u32) -> Result<(), String> {
            self.event("missing")
        }
        fn actor_present(&mut self) -> Result<bool, String> {
            self.event("actor")?;
            Ok(self.actor)
        }
        fn get_channel(&mut self, _: &PlayAnimParameters) -> Result<Option<usize>, String> {
            self.event("channel")?;
            Ok(self.channel)
        }
        fn continue_playback(
            &mut self,
            c: usize,
            found: bool,
            _: &PlayAnimParameters,
        ) -> Result<bool, String> {
            assert_eq!(c, 4);
            assert_eq!(found, self.found);
            self.event("continue")?;
            Ok(true)
        }
    }
    #[test]
    fn native_entry_gates_none_and_editor_flag() {
        for editor in [false, true] {
            for found in [false, true] {
                for actor in [false, true] {
                    for channel in [None, Some(4)] {
                        for none in [false, true] {
                            let mut h = Host {
                                events: vec![],
                                found,
                                actor,
                                channel,
                                fail: None,
                            };
                            let mut p = parameters(0, 0);
                            if none {
                                p.sequence.handle = 0;
                            }
                            let outcome = lod_play_anim(&p, editor, &mut h).unwrap();
                            let mut expected =
                                vec![if editor { "find-editor" } else { "find-load" }.to_string()];
                            let result = if !found && !none {
                                expected.push("missing".into());
                                LodPlayResult::MissingSequence
                            } else {
                                expected.push("actor".into());
                                if !actor {
                                    LodPlayResult::NoActor
                                } else {
                                    expected.push("channel".into());
                                    if channel.is_none() {
                                        LodPlayResult::NoChannel
                                    } else {
                                        expected.push("continue".into());
                                        LodPlayResult::Playback(true)
                                    }
                                }
                            };
                            assert_eq!(outcome, result);
                            assert_eq!(h.events, expected);
                        }
                    }
                }
            }
        }
    }
    #[test]
    fn entry_host_failures_preserve_call_prefix() {
        for (found, fail, expected) in [
            (false, "missing", vec!["find-load", "missing"]),
            (true, "actor", vec!["find-load", "actor"]),
            (true, "channel", vec!["find-load", "actor", "channel"]),
            (
                true,
                "continue",
                vec!["find-load", "actor", "channel", "continue"],
            ),
            (true, "find-load", vec!["find-load"]),
        ] {
            let mut h = Host {
                events: vec![],
                found,
                actor: true,
                channel: Some(4),
                fail: Some(fail),
            };
            assert_eq!(
                lod_play_anim(&parameters(0, 0), false, &mut h),
                Err(fail.into())
            );
            assert_eq!(h.events, expected);
        }
    }
}
