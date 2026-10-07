//! Actor replication gates over supplied channel continuation diagnostics.
use rc_package::{
    animation_replication::{
        diagnostic_frame_binary64, replicate_animation, skeletal_post_init,
        AnimationReplicationHost, AnimationReplicationSnapshot, ReplicatedAnimation,
        SkeletalPostInitHost,
    },
    mesh_animation::AnimationChannel,
};
use std::{env, fs};
struct Host {
    level: bool,
    sequence: Option<u32>,
    frame_error: bool,
    events: Vec<String>,
}
impl AnimationReplicationHost for Host {
    fn level_animation_enabled(&mut self) -> Result<bool, String> {
        self.events.push("LevelAnimationEnabled".into());
        Ok(self.level)
    }
    fn lod_sequence_name(&mut self, i: i32) -> Result<Option<u32>, String> {
        self.events.push(format!("LodGetSequence({i})"));
        Ok(self.sequence)
    }
    fn quantize_frame(&mut self, f: f32) -> Result<u8, String> {
        self.events.push("FrameQuantizer".into());
        if self.frame_error {
            Err("unresolved x87 frame quantization".into())
        } else {
            Ok(diagnostic_frame_binary64(f))
        }
    }
}
struct Post {
    root: bool,
    compose: bool,
    events: Vec<&'static str>,
}
impl SkeletalPostInitHost for Post {
    fn root_location(&mut self, m: &mut [u32; 16]) -> Result<bool, String> {
        self.events.push("GetRootLocation");
        m[0] = 7;
        Ok(self.root)
    }
    fn inverse_actor_compose(&mut self, _: &[u32; 16]) -> Result<[u32; 16], String> {
        self.events.push("InverseActorCompose");
        if self.compose {
            Ok([9; 16])
        } else {
            Err("unresolved inverse/actor matrix composition".into())
        }
    }
}
fn main() -> Result<(), Box<dyn std::error::Error>> {
    let args: Vec<_> = env::args().collect();
    if args.len() != 3 {
        return Err(
            "Usage: rc-animation-replication-probe <channel-playback.json> <report.json>".into(),
        );
    }
    let previous: serde_json::Value = serde_json::from_slice(&fs::read(&args[1])?)?;
    let mut probes = Vec::new();
    for (source_index, source) in previous["probes"]
        .as_array()
        .ok_or("missing playback diagnostics")?
        .iter()
        .enumerate()
    {
        if source["variant"] != "NewSequence" {
            continue;
        }
        let index: usize = source["channel_index"]
            .as_u64()
            .ok_or("missing channel index")?
            .try_into()?;
        let original: AnimationChannel =
            serde_json::from_value(source["after"]["channels"][index].clone())?;
        for scenario in [
            "ActorDisabled",
            "LevelDisabled",
            "ChannelDisabled",
            "SlotHigh",
            "NoNotify",
            "InitialWrite",
            "FrameOnly",
            "NoSequence",
            "FrameBoundary",
        ] {
            let mut state = AnimationReplicationSnapshot {
                flags_68: if matches!(scenario, "ActorDisabled" | "NoNotify") {
                    0
                } else {
                    0x10000
                },
                slots: [ReplicatedAnimation::default(); 6],
            };
            let mut channel = original.clone();
            if matches!(scenario, "ChannelDisabled" | "NoNotify") {
                channel.words[3] = 3;
            }
            let slot = if scenario == "SlotHigh" {
                6
            } else {
                index as i32
            };
            let notify = scenario != "NoNotify";
            let mut host = Host {
                level: scenario != "LevelDisabled",
                sequence: if scenario == "NoSequence" {
                    None
                } else {
                    Some(
                        source["parameters"]["sequence"]["handle"]
                            .as_u64()
                            .ok_or("missing sequence handle")?
                            .try_into()?,
                    )
                },
                frame_error: scenario == "FrameBoundary",
                events: vec![],
            };
            if scenario == "FrameOnly" {
                replicate_animation(&mut state, slot, index as i32, &channel, true, &mut host)?;
                state.flags_68 &= !0x100;
                host.events.clear();
                channel.words[7] = (-0.5f32).to_bits();
            }
            let before = state.clone();
            let result =
                replicate_animation(&mut state, slot, index as i32, &channel, notify, &mut host);
            probes.push(serde_json::json!({"source_index":source_index,"class":source["class"],"source_case":source["source_case"],"scenario":scenario,"slot":slot,"channel_index":index,"notify":notify,"level_enabled":host.level,"supplied_sequence":host.sequence,"channel":channel,"before":before,"after":state,"result":result,"events":host.events}));
        }
    }
    let mut post_init = Vec::new();
    for (root, compose) in [(false, false), (true, false), (true, true)] {
        let mut matrix = [0; 16];
        let mut host = Post {
            root,
            compose,
            events: vec![],
        };
        let result = skeletal_post_init(&mut matrix, &mut host);
        post_init.push(serde_json::json!({"root":root,"supplied_composition":compose,"matrix":matrix,"result":result,"events":host.events}));
    }
    let report = serde_json::json!({"scope":"Replication scalar logic over 768 supplied NewSequence channel scenarios from earlier diagnostics, nine cases each. Actor flags, Level result, sequence name, slot and frame quantizer are supplied. Binary64 frame quantization only an explicit diagnostic assumption; native x87 control word/precision unresolved. Missing sequence is a supplied non-LOD/null/unavailable result, not original mesh discovery. PostInit root and composed matrices also synthetic. No network transport, actual root transform, original clip or Android execution.","probes":probes,"post_init":post_init});
    fs::write(&args[2], serde_json::to_string_pretty(&report)? + "\n")?;
    println!(
        "{} replication scenarios and 3 post-init scenarios",
        probes.len()
    );
    Ok(())
}
