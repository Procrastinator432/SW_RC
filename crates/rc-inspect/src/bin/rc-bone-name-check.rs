use rc_package::{
    mesh_animation::{BoneAlias, ReferenceBone, SkeletonSnapshot},
    name_bindings::diagnostic_bindings,
    skeletal_mesh::StoredBone,
    skeletal_root_pose::{quaternion_translation_matrix, RootTransform},
    skeletal_set_bone_place::{set_bone_place, BonePlaceDefaults, SnapshotBonePlaceNames},
};
use std::{collections::BTreeSet, env, fs};
fn main() -> Result<(), Box<dyn std::error::Error>> {
    let args: Vec<_> = env::args_os().collect();
    if args.len() != 3 {
        return Err("usage: rc-bone-name-check ORIGINAL_LINKUPS OUTPUT".into());
    }
    let source: serde_json::Value = serde_json::from_slice(&fs::read(&args[1])?)?;
    let mut keys = BTreeSet::from([
        "None".to_owned(),
        "__rc_diag_alias__".into(),
        "__rc_diag_chain__".into(),
        "__rc_diag_missing__".into(),
    ]);
    for o in source["objects"].as_array().ok_or("objects")? {
        if o["status"] == "UnsupportedLegacyPackage" {
            continue;
        }
        let bones: Vec<StoredBone> = serde_json::from_value(o["prefix"]["bones"].clone())?;
        for b in bones {
            keys.insert(b.name.name);
        }
    }
    let bindings = diagnostic_bindings(&keys)?;
    let binding = |name: &str| {
        bindings
            .get(&name.to_ascii_lowercase())
            .ok_or("missing diagnostic binding")
    };
    let max_index = bindings
        .values()
        .map(|b| b.name.resolved_index as usize)
        .max()
        .ok_or("names")?;
    let mut global = vec![0; max_index + 1];
    for b in bindings.values() {
        global[b.name.resolved_index as usize] = b.name.handle;
    }
    let alias = binding("__rc_diag_alias__")?.name;
    let chain = binding("__rc_diag_chain__")?.name;
    let missing = binding("__rc_diag_missing__")?.name;
    let defaults = BonePlaceDefaults {
        identity_quaternion: [0, 0, 0, 1f32.to_bits()],
        append_padding: [0xaa, 0xbb, 0xcc],
    };
    let mut objects = vec![];
    let mut count = 0;
    for (source_index, o) in source["objects"]
        .as_array()
        .ok_or("objects")?
        .iter()
        .enumerate()
    {
        if o["status"] == "UnsupportedLegacyPackage" {
            continue;
        }
        let bones: Vec<StoredBone> = serde_json::from_value(o["prefix"]["bones"].clone())?;
        if bones.is_empty() {
            return Err("empty diagnostic skeleton".into());
        }
        let skeleton = SkeletonSnapshot {
            bones: bones
                .iter()
                .map(|b| {
                    Ok(ReferenceBone {
                        name_handle: binding(&b.name.name)?.name.handle,
                        word_38: b.word_38,
                    })
                })
                .collect::<Result<Vec<_>, Box<dyn std::error::Error>>>()?,
            aliases: vec![
                BoneAlias {
                    name_handle: alias.handle,
                    target_handle: binding(&bones[0].name.name)?.name.handle,
                },
                BoneAlias {
                    name_handle: alias.handle,
                    target_handle: binding(&bones[bones.len() - 1].name.name)?.name.handle,
                },
                BoneAlias {
                    name_handle: chain.handle,
                    target_handle: alias.handle,
                },
            ],
        };
        let first = quaternion_translation_matrix(RootTransform {
            rotation: bones[0].rotation,
            position: bones[0].position,
        });
        let last = quaternion_translation_matrix(RootTransform {
            rotation: bones[bones.len() - 1].rotation,
            position: bones[bones.len() - 1].position,
        });
        let alias_matrices = [
            first,
            last,
            quaternion_translation_matrix(RootTransform {
                rotation: [0, 0, 0, 1f32.to_bits()],
                position: [0; 3],
            }),
        ];
        let mut indices = bones
            .iter()
            .map(|b| binding(&b.name.name).map(|n| n.name.resolved_index))
            .collect::<Result<Vec<_>, _>>()?;
        indices.extend([
            alias.resolved_index,
            chain.resolved_index,
            missing.resolved_index,
            0,
        ]);
        let mut queries = vec![];
        for index in indices {
            let mut matrix = [99; 16];
            let matched = skeleton.match_ref_bone_with_matrix(
                global[index as usize],
                &alias_matrices,
                Some(&mut matrix),
            )?;
            let mut names = SnapshotBonePlaceNames {
                global_names: &global,
                skeleton: &skeleton,
            };
            let mut directors = vec![];
            let mut cache = 7;
            let mut input = [0; 22];
            input[0] = 0x80000000 | index as u32;
            input[1] = 0x80000000 | missing.resolved_index as u32;
            input[2..18].copy_from_slice(&first);
            input[18] = 0x101;
            input[19] = 1;
            input[20] = (-1f32).to_bits();
            input[21] = (-1f32).to_bits();
            let created_result = set_bone_place(
                input,
                bones.len(),
                &mut cache,
                &mut directors,
                &defaults,
                &mut names,
            );
            let created = directors.clone();
            let mut updated_result = None;
            if created_result == Ok(true) {
                directors[0].words[22..28].copy_from_slice(&[
                    0.1f32.to_bits(),
                    0.2f32.to_bits(),
                    0.3f32.to_bits(),
                    0.9f32.to_bits(),
                    0.125f32.to_bits(),
                    0x11223302,
                ]);
                input[18] = 0x10101;
                input[20] = 0.25f32.to_bits();
                updated_result = Some(set_bone_place(
                    input,
                    bones.len(),
                    &mut cache,
                    &mut directors,
                    &defaults,
                    &mut names,
                ));
            }
            queries.push(serde_json::json!({"global_index":index,"matched":matched,"matrix":matrix,"created_result":created_result,"created":created,"updated_result":updated_result,"updated":directors,"cache":cache}));
            count += 1;
        }
        objects.push(serde_json::json!({"source_index":source_index,"skeleton":skeleton,"alias_matrices":alias_matrices,"queries":queries}));
    }
    let names:Vec<_>=bindings.iter().map(|(key,b)|serde_json::json!({"key":key,"index":b.name.resolved_index,"handle":b.name.handle,"fixed_native_index":b.fixed_native_index})).collect();
    fs::write(
        &args[2],
        serde_json::to_vec_pretty(
            &serde_json::json!({"scope":"Original reference-bone names with existing diagnostic ASCII FName bindings and supplied global handle table. Three diagnostic alias entries per mesh (duplicate first match and nonrecursive chain), alias matrices from original reference transforms. Connected named SetBonePlace creation/update; no native dynamic registration order or serialized original alias decoding.","names":names,"global_names":global,"objects":objects,"queries":count}),
        )?,
    )?;
    println!("{} skeletons / {count} name queries", objects.len());
    Ok(())
}
