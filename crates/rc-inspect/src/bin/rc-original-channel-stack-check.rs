use rc_package::{
    mesh_animation::AnimationChannel,
    quaternion_animation::PortableQuaternionMath,
    read_package,
    skeletal_animation::{read_mesh_animation, MeshAnimation, StoredSequence},
    skeletal_channel::{
        apply_channel, ChannelPoseInput, PortableChannelAngularMath, TrackChannelHost,
    },
    skeletal_channel_stack::{
        apply_channels_prepared, PreparedChannelPose, PreparedChannelStackHost,
    },
    skeletal_hierarchy::{build_pose_matrices, prepare_hierarchy},
    skeletal_mesh::StoredBone,
    skeletal_root_pose::RootTransform,
};
use std::{collections::HashMap, env, fs, path::Path};
struct Host<'a> {
    sequences: [&'a StoredSequence; 3],
    mapping: &'a [i32],
    reference: &'a [RootTransform],
    move_bone: i32,
    calls: Vec<usize>,
}
impl PreparedChannelStackHost for Host<'_> {
    fn apply(
        &mut self,
        i: usize,
        c: &mut [AnimationChannel],
        previous: &[RootTransform],
        scratch: &mut [RootTransform],
    ) -> Result<bool, String> {
        self.calls.push(i);
        let s = self.sequences[i];
        apply_channel(
            &mut c[i],
            i,
            ChannelPoseInput {
                frames: (!s.tracks.is_empty()).then_some(s.frames),
                mapping: self.mapping,
                reference: self.reference,
                previous,
                move_bone: self.move_bone,
            },
            scratch,
            &mut TrackChannelHost {
                tracks: &s.tracks,
                math: PortableQuaternionMath,
                angular: PortableChannelAngularMath,
            },
        )
    }
}
fn main() -> Result<(), Box<dyn std::error::Error>> {
    let args: Vec<_> = env::args_os().collect();
    if args.len() != 3 {
        return Err("usage: rc-original-channel-stack-check LINKUPS OUTPUT".into());
    }
    let links: serde_json::Value = serde_json::from_slice(&fs::read(&args[1])?)?;
    let mut animations: HashMap<String, MeshAnimation> = HashMap::new();
    let mut objects = vec![];
    let mut poses = 0;
    let mut matrices = 0;
    for (index, o) in links["objects"]
        .as_array()
        .ok_or("objects")?
        .iter()
        .enumerate()
    {
        let Some(prefix) = o.get("prefix") else {
            continue;
        };
        let bones: Vec<StoredBone> = serde_json::from_value(prefix["bones"].clone())?;
        let h = prepare_hierarchy(&bones)?;
        let reference: Vec<_> = bones
            .iter()
            .map(|b| RootTransform {
                rotation: b.rotation,
                position: b.position,
            })
            .collect();
        let mut runs = vec![];
        for m in o["mappings"].as_array().ok_or("mappings")? {
            let key = m["animation"].as_str().ok_or("animation")?;
            if !animations.contains_key(key) {
                let (file, object) = key.split_once('.').ok_or("path")?;
                let source = Path::new(o["file"].as_str().ok_or("file")?)
                    .parent()
                    .unwrap()
                    .join(format!("{file}.ukx"));
                let data = fs::read(source)?;
                let pkg = read_package(&data)?;
                let e = pkg
                    .exports
                    .iter()
                    .enumerate()
                    .find_map(|(i, e)| {
                        pkg.object_path(i as i32 + 1)
                            .ok()
                            .filter(|p| p.eq_ignore_ascii_case(object))
                            .map(|_| e)
                    })
                    .ok_or("export")?;
                animations.insert(key.into(), read_mesh_animation(&pkg, &data, e)?);
            }
            let a = &animations[key];
            let Some(s0) = a.sequences.first() else {
                continue;
            };
            let s1 = a.sequences.get(1).unwrap_or(s0);
            let mapping: Vec<i32> = serde_json::from_value(m["mapping"].clone())?;
            let mut host = Host {
                sequences: [s0, s1, s0],
                mapping: &mapping,
                reference: &reference,
                move_bone: h.move_bone,
                calls: vec![],
            };
            let mut state = PreparedChannelPose {
                local: reference.clone(),
                scratch: reference.clone(),
                byte_61: 0,
            };
            let mut channels = vec![AnimationChannel::default(); 3];
            for c in &mut channels {
                c.words[16] = bones.len() as u32;
                c.words[11] = 1f32.to_bits();
            }
            channels[0].words[14] = 1f32.to_bits();
            channels[1].words[15] = 1;
            channels[1].words[14] = 0.5f32.to_bits();
            channels[1].words[11] = 0;
            let mut ticks = vec![];
            for step in 0..4 {
                let frame = step as f32 * 0.25;
                for c in &mut channels {
                    c.words[7] = frame.to_bits();
                }
                channels[1].words[11] = ((step + 1) as f32 * 0.25).to_bits();
                channels[2].words[14] = if step == 3 { 1f32.to_bits() } else { 0 };
                let before = channels.clone();
                let previous = state.local.clone();
                let scratch_before = state.scratch.clone();
                host.calls.clear();
                let result = apply_channels_prepared(
                    &mut state,
                    &reference,
                    &mut channels,
                    false,
                    0,
                    &mut host,
                );
                let mut output = vec![];
                let matrix_result = build_pose_matrices(&state.local, &h, false, &mut output);
                poses += 1;
                matrices += output.len();
                ticks.push(serde_json::json!({"step":step,"before":before,"after":channels,"previous":previous,"scratch_before":scratch_before,"scratch_after":state.scratch,"local":state.local,"calls":host.calls,"result":result,"matrix_result":matrix_result,"matrices":output,"byte_61":state.byte_61}));
            }
            runs.push(serde_json::json!({"entry":m["entry"],"animation":key,"sequences":[0,if a.sequences.len()>1 {1} else {0},0],"ticks":ticks}));
        }
        objects.push(serde_json::json!({"source_index":index,"object":o["object"],"file":o["file"],"runs":runs}));
    }
    fs::write(
        &args[2],
        serde_json::to_vec_pretty(
            &serde_json::json!({"scope":"Prepared native channel selection/commit stage over four caller-driven ticks per original linkup. Three channels, second sequence on partial layer when present, fourth tick full occlusion. Persistent scratch initially reference. Portable track/quaternion/angle math; no native ticking, automatic cache completion, directors, bounds, skinning or Android claim.","poses":poses,"bone_matrices":matrices,"objects":objects}),
        )?,
    )?;
    println!("{poses} sequential stack poses, {matrices} matrices");
    Ok(())
}
