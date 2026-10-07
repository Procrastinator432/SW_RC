use rc_package::{
    mesh_animation::AnimationChannel,
    quaternion_animation::PortableQuaternionMath,
    read_package,
    skeletal_animation::{read_mesh_animation, MeshAnimation},
    skeletal_channel::{
        apply_channel, ChannelPoseInput, PortableChannelAngularMath, TrackChannelHost,
    },
    skeletal_hierarchy::{build_pose_matrices, prepare_hierarchy},
    skeletal_mesh::StoredBone,
    skeletal_root_pose::RootTransform,
};
use std::{collections::HashMap, env, fs, path::Path};
fn main() -> Result<(), Box<dyn std::error::Error>> {
    let args: Vec<_> = env::args_os().collect();
    if args.len() != 3 {
        return Err("usage: rc-original-channel-check LINKUPS.json OUTPUT.json".into());
    }
    let previous: serde_json::Value = serde_json::from_slice(&fs::read(&args[1])?)?;
    let mut animations: HashMap<String, MeshAnimation> = HashMap::new();
    let mut objects = vec![];
    let mut poses = 0;
    let mut matrices = 0;
    for (index, o) in previous["objects"]
        .as_array()
        .ok_or("missing objects")?
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
        let mut cases = vec![];
        for m in o["mappings"].as_array().ok_or("missing mappings")? {
            let key = m["animation"].as_str().ok_or("missing animation")?;
            if !animations.contains_key(key) {
                let (file, object) = key.split_once('.').ok_or("invalid path")?;
                let source = Path::new(o["file"].as_str().ok_or("missing file")?)
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
                    .ok_or("animation export absent")?;
                animations.insert(key.into(), read_mesh_animation(&pkg, &data, e)?);
            }
            let animation = &animations[key];
            let Some(s) = animation.sequences.first() else {
                continue;
            };
            let mapping: Vec<i32> = serde_json::from_value(m["mapping"].clone())?;
            for (kind, channel_index, blend, weight) in [
                ("Replace", 0, 1f32, 0.5f32),
                ("Layer", 1, 1., 0.5),
                ("Transition", 0, 0.25, 0.5),
                ("LayerTransition", 1, 0.25, 0.5),
            ] {
                let mut channel = AnimationChannel::default();
                channel.words[7] = 0.75f32.to_bits();
                channel.words[12] = 0.25f32.to_bits();
                channel.words[11] = blend.to_bits();
                channel.words[14] = weight.to_bits();
                channel.words[16] = bones.len() as u32;
                let before = channel.clone();
                let mut local = reference.clone();
                let mut host = TrackChannelHost {
                    tracks: &s.tracks,
                    math: PortableQuaternionMath,
                    angular: PortableChannelAngularMath,
                };
                let result = apply_channel(
                    &mut channel,
                    channel_index,
                    ChannelPoseInput {
                        frames: (!s.tracks.is_empty()).then_some(s.frames),
                        mapping: &mapping,
                        reference: &reference,
                        previous: &reference,
                        move_bone: h.move_bone,
                    },
                    &mut local,
                    &mut host,
                );
                let mut output = vec![];
                let matrix_result = build_pose_matrices(&local, &h, false, &mut output);
                poses += 1;
                matrices += output.len();
                cases.push(serde_json::json!({"kind":kind,"entry":m["entry"],"animation":key,"sequence":0,"sequence_name":s.name.name,"channel_index":channel_index,"before":before,"after":channel,"result":result,"matrix_result":matrix_result,"local":local,"matrices":output}));
            }
        }
        objects.push(serde_json::json!({"source_index":index,"object":o["object"],"file":o["file"],"cases":cases}));
    }
    fs::write(
        &args[2],
        serde_json::to_vec_pretty(
            &serde_json::json!({"scope":"Original first sequence per resolved mesh linkup, four full-range prepared-channel scenarios using original tracks and portable quaternion/angle math; previous/scratch/reference pose supplied identically. No native x87 equivalence, automatic channel scheduling, directors/bounds/skinning or Android claim.","poses":poses,"bone_matrices":matrices,"objects":objects}),
        )?,
    )?;
    println!("{poses} original-track channel poses, {matrices} matrices");
    Ok(())
}
