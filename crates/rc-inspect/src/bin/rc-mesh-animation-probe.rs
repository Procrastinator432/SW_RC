//! Original wrapper requests through supplied LOD entry and skeletal channel scenarios.
use rc_package::{
    animation_call::PlayAnimParameters,
    event_lookup::EventNameSnapshot,
    mesh_animation::{
        lod_play_anim, skeletal_channel, AnimationChannel, BoneAlias, ChannelSelection,
        LodAnimationHost, ReferenceBone, SkeletonSnapshot,
    },
};
use std::{env, fs};
struct Host {
    found: bool,
    actor: bool,
    skeleton: SkeletonSnapshot,
    channels: Vec<AnimationChannel>,
    events: Vec<String>,
    selection: Option<ChannelSelection>,
}
impl LodAnimationHost for Host {
    fn find_sequence(&mut self, _: u32, load: bool) -> Result<bool, String> {
        self.events.push(format!("FindSequence(load={load})"));
        Ok(self.found)
    }
    fn missing_sequence(&mut self, _: u32) -> Result<(), String> {
        self.events.push("SuppliedMissingSequenceWarning".into());
        Ok(())
    }
    fn actor_present(&mut self) -> Result<bool, String> {
        self.events.push("GetActor".into());
        Ok(self.actor)
    }
    fn get_channel(&mut self, p: &PlayAnimParameters) -> Result<Option<usize>, String> {
        self.events.push("SkeletalGetChannel".into());
        self.selection = skeletal_channel(&mut self.channels, Some(&self.skeleton), p)?;
        Ok(self.selection.map(|c| c.index))
    }
    fn continue_playback(
        &mut self,
        _: usize,
        _: bool,
        _: &PlayAnimParameters,
    ) -> Result<bool, String> {
        self.events.push("ChannelPlaybackBoundary".into());
        Err("unresolved LOD channel playback".into())
    }
}
fn main() -> Result<(), Box<dyn std::error::Error>> {
    let args: Vec<_> = env::args().collect();
    if args.len() != 3 {
        return Err("Usage: rc-mesh-animation-probe <animation-calls.json> <report.json>".into());
    }
    let original: serde_json::Value = serde_json::from_slice(&fs::read(&args[1])?)?;
    let mut probes = Vec::new();
    for (source_index, case) in original["cases"]
        .as_array()
        .ok_or("missing original animation requests")?
        .iter()
        .enumerate()
    {
        let value = &case["parameters"];
        let name = |key: &str| -> Result<EventNameSnapshot, Box<dyn std::error::Error>> {
            Ok(EventNameSnapshot {
                handle: value[key]["handle"]
                    .as_u64()
                    .ok_or("missing name handle")?
                    .try_into()?,
                resolved_index: value[key]["resolved_index"]
                    .as_i64()
                    .ok_or("missing name index")?
                    .try_into()?,
            })
        };
        let base = PlayAnimParameters {
            sequence: name("sequence")?,
            bone: name("bone")?,
            looping: value["looping"].as_bool().ok_or("missing looping")?,
            channel: value["channel"]
                .as_i64()
                .ok_or("missing channel")?
                .try_into()?,
            float_10: value["float_10"].as_f64().ok_or("missing float10")? as f32,
            rate: value["rate"].as_f64().ok_or("missing rate")? as f32,
            start_frame: value["start_frame"].as_f64().ok_or("missing frame")? as f32,
        };
        for scenario in [
            "MissingSequence",
            "NoActor",
            "NoChannel",
            "CreateRoot",
            "ReuseRoot",
            "InsertAlias",
            "UnknownBone",
        ] {
            let mut parameters = base;
            let skeleton = SkeletonSnapshot {
                bones: vec![
                    ReferenceBone {
                        name_handle: if scenario == "NoChannel" { 0 } else { 10 },
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
                aliases: vec![BoneAlias {
                    name_handle: 40,
                    target_handle: 20,
                }],
            };
            let mut channels = Vec::new();
            if matches!(scenario, "ReuseRoot" | "InsertAlias" | "UnknownBone") {
                let mut root = AnimationChannel {
                    words: [0x12345678; 18],
                };
                root.words[2] = 10;
                root.words[3] = base.channel as u32;
                root.words[15] = 0;
                channels.push(root);
            }
            if matches!(scenario, "CreateRoot" | "UnknownBone") {
                parameters.bone.handle = 999;
                parameters.bone.resolved_index = 998;
            }
            if scenario == "InsertAlias" {
                parameters.bone.handle = 40;
                parameters.bone.resolved_index = 39;
                parameters.channel = -1;
            }
            let before = channels.clone();
            let mut host = Host {
                found: scenario != "MissingSequence",
                actor: scenario != "NoActor",
                skeleton,
                channels,
                events: vec![],
                selection: None,
            };
            let editor = source_index % 2 == 1;
            let result = lod_play_anim(&parameters, editor, &mut host);
            probes.push(serde_json::json!({"source_index":source_index,"class":case["class"],"source_case":case["source_case"],"scenario":scenario,"is_editor":editor,"sequence_found":host.found,"actor_present":host.actor,"parameters":parameters,"skeleton":host.skeleton,"before_channels":before,"after_channels":host.channels,"selection":host.selection,"result":result,"events":host.events}));
        }
    }
    let report = serde_json::json!({"scope":"256 original AnimProp wrapper requests; seven explicitly supplied LOD virtual targets, sequence/actor results and synthetic skeleton/channel snapshots per request. Synthetic bone handles are not original package bindings. No real sequence, skeleton, channel playback, pose or Android execution proven. Missing-sequence cache/log operation supplied completed; playback continuation unresolved. Rejection results map to native false; preceding channel insertion remains visible on continuation failure.","probes":probes});
    fs::write(&args[2], serde_json::to_string_pretty(&report)? + "\n")?;
    println!("{} supplied mesh entry/channel scenarios", probes.len());
    Ok(())
}
