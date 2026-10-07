//! Channel continuation on earlier explicitly supplied skeletal diagnostic snapshots.
use rc_package::{
    animation_call::PlayAnimParameters,
    channel_playback::{
        play_channel, ChannelPlaybackHost, ChannelPlaybackSnapshot, PlaybackSequence,
    },
    event_lookup::EventNameSnapshot,
    mesh_animation::AnimationChannel,
};
use std::{env, fs};
struct Host {
    mode: &'static str,
    events: Vec<&'static str>,
}
impl ChannelPlaybackHost for Host {
    fn random_int(&mut self) -> Result<i32, String> {
        self.events.push("SuppliedRand16384");
        Ok(16384)
    }
    fn post_init(&mut self, _: &mut AnimationChannel) -> Result<(), String> {
        self.events.push("PostInitAnim");
        if self.mode == "PostInitBoundary" {
            Err("unresolved PostInitAnim".into())
        } else {
            Ok(())
        }
    }
    fn replicate(&mut self, _: usize, _: &AnimationChannel) -> Result<(), String> {
        self.events.push("ReplicateAnim");
        if self.mode == "ReplicationBoundary" {
            Err("unresolved ReplicateAnim".into())
        } else {
            Ok(())
        }
    }
}
fn main() -> Result<(), Box<dyn std::error::Error>> {
    let args: Vec<_> = env::args().collect();
    if args.len() != 3 {
        return Err(
            "Usage: rc-channel-playback-probe <mesh-animation-channels.json> <report.json>".into(),
        );
    }
    let previous: serde_json::Value = serde_json::from_slice(&fs::read(&args[1])?)?;
    let mut probes = Vec::new();
    for (source_index, source) in previous["probes"]
        .as_array()
        .ok_or("missing mesh scenarios")?
        .iter()
        .enumerate()
    {
        if source["selection"].is_null() {
            continue;
        }
        let value = &source["parameters"];
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
            looping: value["looping"].as_bool().ok_or("missing loop")?,
            channel: value["channel"]
                .as_i64()
                .ok_or("missing channel")?
                .try_into()?,
            float_10: 1.0,
            rate: 1.0,
            start_frame: -1.0,
        };
        let index: usize = source["selection"]["index"]
            .as_u64()
            .ok_or("missing channel index")?
            .try_into()?;
        let channels: Vec<AnimationChannel> =
            serde_json::from_value(source["after_channels"].clone())?;
        for variant in [
            "NewSequence",
            "Reuse",
            "Stopped",
            "ExplicitFrame",
            "LoopContinuity",
            "RandomStart",
            "None",
            "PostInitBoundary",
            "ReplicationBoundary",
        ] {
            let mut state = ChannelPlaybackSnapshot {
                channels: channels.clone(),
                byte_60: 1,
                byte_61: 7,
            };
            let c = &mut state.channels[index];
            c.words[0] = if matches!(variant, "Reuse" | "Stopped") {
                base.sequence.handle
            } else {
                338
            };
            c.words[1] = 0xaabbcc01;
            c.words[5] = 0;
            c.words[6] = (if variant == "Stopped" { 0.0f32 } else { 1.0f32 }).to_bits();
            c.words[7] = 0.3f32.to_bits();
            c.words[12] = 0.4f32.to_bits();
            c.words[10] = 2.0f32.to_bits();
            let mut parameters = base;
            if variant == "ExplicitFrame" {
                parameters.start_frame = 0.25;
            }
            if variant == "LoopContinuity" {
                parameters.looping = true;
            }
            if variant == "None" {
                parameters.sequence = EventNameSnapshot {
                    handle: 0,
                    resolved_index: 0,
                };
            }
            let sequence = if variant == "None" {
                None
            } else {
                Some(PlaybackSequence {
                    token: 123,
                    frames: 10,
                    rate: 40.0,
                    minimum_blend: 0.1,
                    randomize_start: variant == "RandomStart",
                })
            };
            let before = state.clone();
            let mut host = Host {
                mode: variant,
                events: vec![],
            };
            let result = play_channel(&mut state, index, &parameters, sequence, &mut host);
            probes.push(serde_json::json!({"source_index":source_index,"class":source["class"],"source_case":source["source_case"],"source_scenario":source["scenario"],"variant":variant,"channel_index":index,"parameters":parameters,"sequence":sequence,"before":before,"after":state,"events":host.events,"result":result}));
        }
    }
    let report = serde_json::json!({"scope":"Continuation of 768 previously supplied synthetic mesh/channel scenarios from 256 original wrapper requests. Nine additional supplied channel/sequence variants each. Sequence pointer token123, metadata, initial frames/rates, RNG16384 and completed callback outcomes are diagnostics. Two variants deliberately leave PostInitAnim or ReplicateAnim unresolved. No original clip, concrete game instance, native callbacks, animation tick, pose, skinning or Android playback proven.","probes":probes});
    fs::write(&args[2], serde_json::to_string_pretty(&report)? + "\n")?;
    println!("{} supplied channel continuation scenarios", probes.len());
    Ok(())
}
