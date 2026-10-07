use rc_package::{
    quaternion_animation::{PortableQuaternionMath, QuaternionTrackHost},
    read_package,
    skeletal_animation::read_mesh_animation,
    skeletal_mesh::*,
    skeletal_root_pose::{quaternion_translation_matrix, RootTransform},
    skeletal_track::sample_track,
};
use std::{collections::HashMap, env, fs};
fn main() -> Result<(), Box<dyn std::error::Error>> {
    let args: Vec<_> = env::args_os().collect();
    if args.len() != 3 {
        return Err("usage: rc-skeletal-linkup-check ANIMATIONS_DIRECTORY REPORT.json".into());
    }
    let mut files = fs::read_dir(&args[1])?
        .map(|e| e.map(|e| e.path()))
        .collect::<Result<Vec<_>, _>>()?;
    files.sort();
    let mut packages = vec![];
    let mut animations = HashMap::new();
    for file in files
        .into_iter()
        .filter(|p| p.extension().is_some_and(|e| e.eq_ignore_ascii_case("ukx")))
    {
        let data = fs::read(&file)?;
        let pkg = read_package(&data)?;
        if pkg.summary.version >= 151 {
            for (i, e) in pkg.exports.iter().enumerate() {
                if pkg.object_path(e.class)? == "Engine.MeshAnimation" {
                    let key = format!(
                        "{}.{}",
                        file.file_stem().unwrap().to_string_lossy(),
                        pkg.object_path(i as i32 + 1)?
                    )
                    .to_ascii_lowercase();
                    if animations
                        .insert(key, read_mesh_animation(&pkg, &data, e)?)
                        .is_some()
                    {
                        return Err("duplicate animation path".into());
                    }
                }
            }
        }
        packages.push((file, data, pkg));
    }
    let mut objects = vec![];
    let (mut meshes, mut bones, mut links, mut samples, mut errors) = (0, 0, 0, 0, 0);
    for (file, data, pkg) in &packages {
        for (i, e) in pkg.exports.iter().enumerate() {
            if pkg.object_path(e.class)? != "Engine.SkeletalMesh" {
                continue;
            }
            let object = pkg.object_path(i as i32 + 1)?;
            if pkg.summary.version < 151 {
                objects.push(serde_json::json!({"file":file,"object":object,"export_index":i+1,"status":"UnsupportedLegacyPackage"}));
                continue;
            }
            meshes += 1;
            match read_skeletal_mesh_prefix(pkg, data, e) {
                Err(error) => {
                    errors += 1;
                    objects.push(serde_json::json!({"file":file,"object":object,"export_index":i+1,"error":error}));
                }
                Ok(prefix) => {
                    bones += prefix.bones.len();
                    let mut mappings = vec![];
                    for (entry, link) in prefix.linkups.iter().enumerate() {
                        links += 1;
                        let path = pkg.object_path(link.animation_index)?;
                        let key = if link.animation_index > 0 {
                            format!("{}.{}", file.file_stem().unwrap().to_string_lossy(), path)
                        } else {
                            path.clone()
                        }
                        .to_ascii_lowercase();
                        let Some(anim) = animations.get(&key) else {
                            mappings.push(serde_json::json!({"entry":entry,"animation":key,"status":"UnresolvedAnimationReference"}));
                            continue;
                        };
                        if prefix.bones.iter().any(|b| !b.name.name.is_ascii())
                            || anim.reference_bones.iter().any(|b| !b.name.name.is_ascii())
                        {
                            return Err("non-ASCII bone name needs native name folding".into());
                        }
                        let mesh_names: Vec<_> = prefix
                            .bones
                            .iter()
                            .map(|b| b.name.name.to_ascii_lowercase())
                            .collect();
                        let anim_names: Vec<_> = anim
                            .reference_bones
                            .iter()
                            .map(|b| b.name.name.to_ascii_lowercase())
                            .collect();
                        let mut mapping = link.mapping.clone();
                        let result = refresh_linkup(&mut mapping, &mesh_names, Some(&anim_names));
                        let mut root_samples = vec![];
                        if let Some(&track_index) = mapping.first().filter(|&&t| t >= 0) {
                            for (sindex, seq) in anim.sequences.iter().enumerate() {
                                let Some(track) = seq.tracks.get(track_index as usize) else {
                                    root_samples.push(serde_json::json!({"sequence":sindex,"status":"MissingMappedTrack"}));
                                    continue;
                                };
                                for time in [0., seq.frames as f32 * 0.5, seq.frames as f32] {
                                    let reference = &prefix.bones[0];
                                    let mut root = RootTransform {
                                        rotation: reference.rotation,
                                        position: reference.position,
                                    };
                                    let result = sample_track(
                                        track,
                                        time,
                                        &mut root,
                                        &mut QuaternionTrackHost {
                                            math: PortableQuaternionMath,
                                        },
                                    );
                                    samples += 1;
                                    root_samples.push(serde_json::json!({"sequence":sindex,"sequence_name":seq.name.name,"time_bits":time.to_bits(),"track":track_index,"result":result,"root":root,"matrix":quaternion_translation_matrix(root)}));
                                }
                            }
                        }
                        mappings.push(serde_json::json!({"entry":entry,"animation":key,"animation_names":anim_names,"mesh_names":mesh_names,"refresh":result,"mapping":mapping,"root_samples":root_samples}));
                    }
                    objects.push(serde_json::json!({"file":file,"object":object,"export_index":i+1,"prefix":prefix,"mappings":mappings}));
                }
            }
        }
    }
    fs::write(
        &args[2],
        serde_json::to_vec_pretty(
            &serde_json::json!({"scope":"Original SkeletalMesh prefixes through bones/linkups, native name-match/cache semantics, mapped root track samples with portable quaternion math. Remaining mesh/skin data, full ApplyAnimation, rendering, x87 parity and Android remain open.","meshes":meshes,"bones":bones,"linkups":links,"root_samples":samples,"errors":errors,"objects":objects}),
        )?,
    )?;
    println!("{meshes} meshes, {bones} bones, {links} linkups, {samples} mapped root samples, {errors} errors");
    if errors != 0 {
        return Err("skeletal prefix errors; see report".into());
    }
    Ok(())
}
