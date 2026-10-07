use rc_package::{
    quaternion_animation::{PortableQuaternionMath, QuaternionTrackHost},
    read_package,
    skeletal_animation::{read_mesh_animation, MeshAnimation},
    skeletal_hierarchy::*,
    skeletal_mesh::StoredBone,
    skeletal_root_pose::RootTransform,
    skeletal_track::sample_track,
};
use std::{collections::HashMap, env, fs};
fn main() -> Result<(), Box<dyn std::error::Error>> {
    let args: Vec<_> = env::args_os().collect();
    if args.len() != 3 {
        return Err("usage: rc-skeletal-pose-check LINKUP_REPORT OUTPUT.json".into());
    }
    let previous: serde_json::Value = serde_json::from_slice(&fs::read(&args[1])?)?;
    let mut animations: HashMap<String, MeshAnimation> = HashMap::new();
    let mut objects = vec![];
    let (mut poses, mut matrices) = (0, 0);
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
        let hierarchy = prepare_hierarchy(&bones)?;
        let reference: Vec<_> = bones
            .iter()
            .map(|b| RootTransform {
                rotation: b.rotation,
                position: b.position,
            })
            .collect();
        let mut cases = vec![];
        for editor in [false, true] {
            let mut output = vec![];
            let result = build_pose_matrices(&reference, &hierarchy, editor, &mut output);
            poses += 1;
            matrices += output.len();
            cases.push(serde_json::json!({"kind":"Reference","editor":editor,"local":reference,"matrices":output,"result":result}));
        }
        for m in o["mappings"].as_array().ok_or("missing mappings")? {
            let Some(key) = m["animation"].as_str() else {
                continue;
            };
            if !animations.contains_key(key) {
                let (file, object) = key.split_once('.').ok_or("invalid animation path")?;
                let source = std::path::Path::new(o["file"].as_str().ok_or("missing file")?)
                    .parent()
                    .unwrap()
                    .join(format!("{file}.ukx"));
                let data = fs::read(source)?;
                let pkg = read_package(&data)?;
                let export = pkg
                    .exports
                    .iter()
                    .enumerate()
                    .find_map(|(i, e)| {
                        pkg.object_path(i as i32 + 1)
                            .ok()
                            .filter(|p| p.eq_ignore_ascii_case(object))
                            .map(|_| e)
                    })
                    .ok_or("animation export not found")?;
                animations.insert(key.into(), read_mesh_animation(&pkg, &data, export)?);
            }
            let animation = &animations[key];
            let Some(sequence) = animation.sequences.first() else {
                continue;
            };
            let mapping: Vec<i32> = serde_json::from_value(m["mapping"].clone())?;
            for time in [0., sequence.frames as f32 * 0.5, sequence.frames as f32] {
                let mut local = reference.clone();
                let mut sampled = 0;
                let mut fallbacks = 0;
                for (i, &track) in mapping.iter().enumerate() {
                    if track < 0 {
                        fallbacks += 1;
                        continue;
                    }
                    let track = sequence
                        .tracks
                        .get(track as usize)
                        .ok_or("missing mapped track")?;
                    sample_track(
                        track,
                        time,
                        &mut local[i],
                        &mut QuaternionTrackHost {
                            math: PortableQuaternionMath,
                        },
                    )?;
                    sampled += 1;
                }
                let mut output = vec![];
                let result = build_pose_matrices(&local, &hierarchy, false, &mut output);
                poses += 1;
                matrices += output.len();
                cases.push(serde_json::json!({"kind":"MappedTrackPose","entry":m["entry"],"animation":key,"sequence":0,"sequence_name":sequence.name.name,"time_bits":time.to_bits(),"editor":false,"local":local,"matrices":output,"sampled_bones":sampled,"reference_fallbacks":fallbacks,"result":result}));
            }
        }
        objects.push(serde_json::json!({"source_index":index,"file":o["file"],"object":o["object"],"hierarchy":hierarchy,"cases":cases}));
    }
    // General finite matrices expose all native summation orders, beyond affine poses.
    let mut compositions = vec![];
    for seed in 1..=32 {
        let a = std::array::from_fn(|i| (((i * 17 + seed * 11) % 43) as f32 - 21.) * 0.125);
        let b = std::array::from_fn(|i| (((i * 13 + seed * 7) % 37) as f32 - 18.) * 0.2);
        let a = a.map(f32::to_bits);
        let b = b.map(f32::to_bits);
        compositions
            .push(serde_json::json!({"local":a,"parent":b,"result":compose_bone_matrix(a,b)}));
    }
    fs::write(
        &args[2],
        serde_json::to_vec_pretty(
            &serde_json::json!({"scope":"Original skeleton PostLoad hierarchy plus local-to-parent matrix stage, without directors/bounds/cache flags. Reference poses in editor/game and diagnostic full-range mapped local poses from first sequence per linkup; portable quaternion math, no channel blend or full ApplyAnimation claim.","poses":poses,"bone_matrices":matrices,"objects":objects,"compositions":compositions}),
        )?,
    )?;
    println!("{poses} diagnostic poses, {matrices} bone matrices");
    Ok(())
}
