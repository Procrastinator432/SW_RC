//! Root-only branch after explicitly supplied native preparation; track samples are diagnostic.
use rc_package::{
    mesh_animation::AnimationChannel,
    skeletal_root_pose::{
        apply_root_prepared, quaternion_translation_matrix, PreparedRootPose, RootPoseHost,
        RootSampleRequest, RootSequence, RootTransform,
    },
};
use std::{env, fs};
struct Host {
    mode: &'static str,
    events: Vec<String>,
    requests: Vec<RootSampleRequest>,
}
impl RootPoseHost for Host {
    fn sequence(
        &mut self,
        i: usize,
        c: &mut [AnimationChannel],
    ) -> Result<Option<RootSequence>, String> {
        self.events.push(format!("GetSequence({i})"));
        c[i].words[17] = 123;
        Ok(if self.mode == "NoSequence" {
            None
        } else {
            Some(RootSequence {
                token: 123,
                frames: 10,
                track_count_word: if self.mode == "NoTracks" {
                    0xe0000000
                } else {
                    0xa0000001
                },
            })
        })
    }
    fn root_track(&mut self, _: RootSequence) -> Result<i32, String> {
        self.events.push("GetLinkupRootTrack".into());
        Ok(if self.mode == "NoLinkup" { -1 } else { 4 })
    }
    fn sample(&mut self, r: RootSampleRequest, root: &mut RootTransform) -> Result<(), String> {
        self.events.push("GetRotPosBoundary".into());
        self.requests.push(r);
        root.position = [
            r.normalized_frame.to_bits(),
            4.0f32.to_bits(),
            5.0f32.to_bits(),
        ];
        if self.mode == "SampleBoundary" {
            Err("unresolved track decoding/interpolation".into())
        } else {
            Ok(())
        }
    }
}
fn main() -> Result<(), Box<dyn std::error::Error>> {
    let args: Vec<_> = env::args().collect();
    if args.len() != 3 {
        return Err("Usage: rc-skeletal-root-probe <channel-playback.json> <report.json>".into());
    }
    let previous: serde_json::Value = serde_json::from_slice(&fs::read(&args[1])?)?;
    let mut probes = Vec::new();
    let reference = RootTransform {
        rotation: [0, 0, 0, 1.0f32.to_bits()],
        position: [1.0f32.to_bits(), 2.0f32.to_bits(), 3.0f32.to_bits()],
    };
    for (source_index, source) in previous["probes"]
        .as_array()
        .ok_or("missing playback probes")?
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
        let original: Vec<AnimationChannel> =
            serde_json::from_value(source["after"]["channels"].clone())?;
        for scenario in [
            "Blocked",
            "BoneMismatch",
            "EmptyRange",
            "NoSequence",
            "NoTracks",
            "NoLinkup",
            "Sampled",
            "SampleBoundary",
            "LoopZeroRate",
        ] {
            let mut channels = original.clone();
            for c in &mut channels {
                c.words[15] = 1;
                c.words[14] = 0;
            }
            let c = &mut channels[index];
            c.words[15] = if scenario == "BoneMismatch" { 1 } else { 0 };
            c.words[16] = if scenario == "EmptyRange" { 0 } else { 3 };
            c.words[14] = 1.0f32.to_bits();
            c.words[11] = 0.5f32.to_bits();
            c.words[7] = 0.25f32.to_bits();
            if scenario == "LoopZeroRate" {
                c.words[1] |= 1;
                c.words[6] = 0;
            }
            let before_channels = channels.clone();
            let before = PreparedRootPose {
                root: RootTransform {
                    rotation: [9; 4],
                    position: [9; 3],
                },
                matrix: [7; 16],
                byte_60: 1,
                byte_61: 9,
            };
            let mut state = before.clone();
            let mut host = Host {
                mode: scenario,
                events: vec![],
                requests: vec![],
            };
            let word_11c = u32::from(scenario == "Blocked");
            let result =
                apply_root_prepared(&mut state, reference, &mut channels, word_11c, &mut host);
            probes.push(serde_json::json!({"source_index":source_index,"class":source["class"],"source_case":source["source_case"],"scenario":scenario,"channel_index":index,"word_11c":word_11c,"reference":reference,"before":before,"after":state,"before_channels":before_channels,"after_channels":channels,"requests":host.requests,"events":host.events,"result":result}));
        }
    }
    let mut matrices = Vec::new();
    for q in [
        [0.0, 0.0, 0.0, 1.0],
        [0.0, 0.0, 2.0, 0.0],
        [0.1, -0.2, 0.3, 0.9],
        [-0.7, 0.4, 0.2, -0.1],
    ] {
        let root = RootTransform {
            rotation: q.map(f32::to_bits),
            position: [(-0.0f32).to_bits(), 0x7fc01234, 0x7f800000],
        };
        matrices
            .push(serde_json::json!({"root":root,"matrix":quaternion_translation_matrix(root)}));
    }
    let report = serde_json::json!({"scope":"Root-only ApplyAnimation branch after explicitly supplied buffer, inverse-reference cache, MeshToWorld and director preparation. 768 earlier diagnostic channel snapshots with nine supplied variants. Root references, sequence pointers/counts/linkups and GetRotPos outputs are synthetic. Matrix constructor runs reviewed SSE-order arithmetic in Rust; no original track decoding, interpolation, full pose, preparation implementation or visible Android animation proven.","probes":probes,"matrices":matrices});
    fs::write(&args[2], serde_json::to_string_pretty(&report)? + "\n")?;
    println!(
        "{} prepared root-pose scenarios and 4 matrix cases",
        probes.len()
    );
    Ok(())
}
