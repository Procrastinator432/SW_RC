use rc_package::{
    mesh_animation::AnimationChannel,
    name_bindings::diagnostic_bindings,
    quaternion_animation::PortableQuaternionMath,
    read_package,
    skeletal_animation::{read_mesh_animation, MeshAnimation},
    skeletal_animation_entry::{
        evaluate_animation_frame, AnimationFrameHosts, AnimationFrameInput, AnimationFullInput,
        AnimationInstance,
    },
    skeletal_bounds::{BoundsPadding, PoseBounds, PoseBoundsHost},
    skeletal_channel::PortableChannelAngularMath,
    skeletal_director_rotation::PortableDirectorRotationHost,
    skeletal_full_pose::FullPoseHosts,
    skeletal_hierarchy::prepare_hierarchy,
    skeletal_mesh::StoredBone,
    skeletal_preparation::{ChannelScratchBuffers, InstanceAnimationBuffers},
    skeletal_root_pose::{quaternion_translation_matrix, RootTransform},
    skeletal_runtime_hosts::{
        ResolvedLinkup, ResolvedSequence, RuntimeAnimationBindings, RuntimeAnimationEvent,
        RuntimePoseHost, RuntimePreparationHost,
    },
};
use std::{
    cell::RefCell,
    collections::{BTreeSet, HashMap},
    env, fs,
    path::Path,
    rc::Rc,
};
fn sequence_calls(data: &RuntimeAnimationBindings<'_>) -> Vec<usize> {
    data.events
        .borrow()
        .iter()
        .filter_map(|e| {
            if let RuntimeAnimationEvent::Sequence { channel, .. } = e {
                Some(*channel)
            } else {
                None
            }
        })
        .collect()
}
fn joined_pose(rotations: &[[u32; 4]], positions: &[[u32; 3]]) -> Vec<RootTransform> {
    rotations
        .iter()
        .zip(positions)
        .map(|(&rotation, &position)| RootTransform { rotation, position })
        .collect()
}
struct Publication {
    snapshot: Option<PoseBounds>,
    calls: usize,
}
impl PoseBoundsHost for Publication {
    fn reciprocal_sqrt_seed(&mut self, n: f32) -> Result<f32, String> {
        Ok(1. / n.sqrt())
    }
    fn publish(&mut self, b: &PoseBounds) -> Result<(), String> {
        self.snapshot = Some(b.clone());
        self.calls += 1;
        Ok(())
    }
}
fn main() -> Result<(), Box<dyn std::error::Error>> {
    let args: Vec<_> = env::args_os().collect();
    if args.len() != 3 {
        return Err("usage: rc-runtime-hosts-check LINKUPS OUTPUT".into());
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
        let mut inverse = vec![]; // Mesh-owned cache survives across diagnostic instance runs.
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
            let names: BTreeSet<_> = bones
                .iter()
                .map(|b| b.name.name.clone())
                .chain(a.reference_bones.iter().map(|b| b.name.name.clone()))
                .collect();
            let names = diagnostic_bindings(&names)?;
            let mesh_names: Vec<_> = bones
                .iter()
                .map(|b| names[&b.name.name.to_ascii_lowercase()].name.handle)
                .collect();
            let animation_names: Vec<_> = a
                .reference_bones
                .iter()
                .map(|b| names[&b.name.name.to_ascii_lowercase()].name.handle)
                .collect();
            let catalog = [
                ResolvedSequence {
                    identity: 1,
                    linkup_key: 7,
                    sequence: s0,
                },
                ResolvedSequence {
                    identity: 2,
                    linkup_key: 7,
                    sequence: s1,
                },
            ];
            let sequence_names = [(101, 1), (102, 2)];
            let data = RuntimeAnimationBindings {
                sequences: &catalog,
                names: &sequence_names,
                linkups: Rc::new(RefCell::new(vec![ResolvedLinkup {
                    key: 7,
                    animation_names: Some(animation_names),
                    mapping: vec![],
                }])),
                events: Rc::new(RefCell::new(vec![])),
            };
            let mut host = RuntimePoseHost {
                data: data.clone(),
                reference: &reference,
                move_bone: h.move_bone,
                editor: false,
                math: PortableQuaternionMath,
                angular: PortableChannelAngularMath,
            };
            let mut root_host = RuntimePoseHost {
                data: data.clone(),
                reference: &reference,
                move_bone: h.move_bone,
                editor: true,
                math: PortableQuaternionMath,
                angular: PortableChannelAngularMath,
            };
            let mut scratch = ChannelScratchBuffers {
                rotations: reference.iter().map(|p| p.rotation).collect(),
                positions: reference.iter().map(|p| p.position).collect(),
            };
            let mut state = AnimationInstance {
                buffers: InstanceAnimationBuffers {
                    rotations: reference.iter().map(|p| p.rotation).collect(),
                    positions: reference.iter().map(|p| p.position).collect(),
                    matrices: vec![],
                    mesh_to_world: [66; 16],
                },
                bounds: PoseBounds {
                    minimum: [99; 3],
                    maximum: [100; 3],
                    sphere: [123; 4],
                    byte_60: 7,
                    byte_61: 0,
                    byte_179: 9,
                },
            };
            let padding = BoundsPadding {
                minimum: [0.25f32, 1., 2.].map(f32::to_bits),
                maximum: [2f32, 0.5, 1.].map(f32::to_bits),
                k_one: [1f32.to_bits(); 3],
            };
            let transform = quaternion_translation_matrix(RootTransform {
                rotation: [0, 0, 0, 1f32.to_bits()],
                position: [0; 3],
            });
            let mut preparation = RuntimePreparationHost {
                data: data.clone(),
                mesh_names: &mesh_names,
                mesh_to_world: transform,
            };
            let linkup_count = 1;
            let mut rotation = PortableDirectorRotationHost {
                math: PortableQuaternionMath,
            };
            let mut publication = Publication {
                snapshot: None,
                calls: 0,
            };
            let mut channels = vec![AnimationChannel::default(); 3];
            for c in &mut channels {
                c.words[16] = bones.len() as u32;
                c.words[11] = 1f32.to_bits();
            }
            for (i, c) in channels.iter_mut().enumerate() {
                c.words[0] = if i == 1 { 102 } else { 101 };
                c.words[17] = if i == 1 { 2 } else { 1 };
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
                state.bounds.byte_61 = 0; // Caller invalidates for a new diagnostic tick.
                let bounds_before = state.bounds.clone();
                let before = channels.clone();
                let previous = joined_pose(&state.buffers.rotations, &state.buffers.positions);
                let scratch_before = joined_pose(&scratch.rotations, &scratch.positions);
                data.events.borrow_mut().clear();
                let result = evaluate_animation_frame(
                    &mut state,
                    AnimationFrameInput {
                        frame_type: 0,
                        animation: AnimationFullInput {
                            bones: &bones,
                            linkup_count,
                            hierarchy: &h,
                            editor: false,
                            word_11c: 0,
                            actor_scale: [1f32.to_bits(); 3],
                            padding: &padding,
                        },
                    },
                    &mut inverse,
                    &mut scratch,
                    &mut channels,
                    &mut [],
                    AnimationFrameHosts {
                        root: &mut root_host,
                        preparation: &mut preparation,
                        pose: FullPoseHosts {
                            channels: &mut host,
                            rotation: &mut rotation,
                            publication: &mut publication,
                        },
                    },
                );
                if result.is_err() {
                    return Err(format!("full pose failed: {result:?}").into());
                }
                let calls = sequence_calls(&data);
                let cached = evaluate_animation_frame(
                    &mut state,
                    AnimationFrameInput {
                        frame_type: 0,
                        animation: AnimationFullInput {
                            bones: &bones,
                            linkup_count,
                            hierarchy: &h,
                            editor: false,
                            word_11c: 0,
                            actor_scale: [1f32.to_bits(); 3],
                            padding: &padding,
                        },
                    },
                    &mut inverse,
                    &mut scratch,
                    &mut channels,
                    &mut [],
                    AnimationFrameHosts {
                        root: &mut root_host,
                        preparation: &mut preparation,
                        pose: FullPoseHosts {
                            channels: &mut host,
                            rotation: &mut rotation,
                            publication: &mut publication,
                        },
                    },
                );
                if sequence_calls(&data) != calls || publication.calls != step + 1 {
                    return Err("cache invoked a host".into());
                }
                let output = &state.buffers.matrices;
                let local = joined_pose(&state.buffers.rotations, &state.buffers.positions);
                let scratch_after = joined_pose(&scratch.rotations, &scratch.positions);
                poses += 1;
                matrices += output.len();
                ticks.push(serde_json::json!({"step":step,"before":before,"after":channels,"previous":previous,"scratch_before":scratch_before,"scratch_after":scratch_after,"local":local,"calls":sequence_calls(&data),"result":result,"cached":cached,"bounds_before":bounds_before,"bounds":state.bounds,"published":publication.snapshot,"matrices":output,"events":*data.events.borrow(),"mesh_to_world":state.buffers.mesh_to_world,"byte_61":state.bounds.byte_61}));
            }
            data.events.borrow_mut().clear();
            let root_bounds_before = state.bounds.clone();
            let tail_before = state.buffers.matrices[1..].to_vec();
            let scratch_snapshot = serde_json::to_value(&scratch)?;
            let root_result = evaluate_animation_frame(
                &mut state,
                AnimationFrameInput {
                    frame_type: 3,
                    animation: AnimationFullInput {
                        bones: &bones,
                        hierarchy: &h,
                        linkup_count,
                        editor: true,
                        word_11c: 0,
                        actor_scale: [1f32.to_bits(); 3],
                        padding: &padding,
                    },
                },
                &mut inverse,
                &mut scratch,
                &mut channels,
                &mut [],
                AnimationFrameHosts {
                    preparation: &mut preparation,
                    root: &mut root_host,
                    pose: FullPoseHosts {
                        channels: &mut host,
                        rotation: &mut rotation,
                        publication: &mut publication,
                    },
                },
            )?;
            let root_tails_unchanged = state.buffers.matrices[1..] == tail_before;
            if !root_tails_unchanged
                || serde_json::to_value(&scratch)? != scratch_snapshot
                || publication.calls != 4
            {
                return Err("root branch touched full-pose state".into());
            }
            let root = serde_json::json!({"result":root_result,"root":{"rotation":state.buffers.rotations[0],"position":state.buffers.positions[0]},"matrix":state.buffers.matrices[0],"bounds_before":root_bounds_before,"bounds":state.bounds,"tails_unchanged":root_tails_unchanged,"events":*data.events.borrow()});
            runs.push(serde_json::json!({"entry":m["entry"],"animation":key,"sequences":[0,if a.sequences.len()>1 {1} else {0},0],"ticks":ticks,"inverse":inverse,"mapping":data.linkups.borrow()[0].mapping,"root":root}));
        }
        objects.push(serde_json::json!({"source_index":index,"object":o["object"],"file":o["file"],"runs":runs}));
    }
    fs::write(
        &args[2],
        serde_json::to_vec_pretty(
            &serde_json::json!({"scope":"Concrete loaded-sequence/root/channel hosts and actual shared linkup refresh through common GetFrame dispatch. Four original-track full ticks with noneditor cached repeats, then editor root evaluation. Diagnostic sequence/name/key identities, one resolved animation linkup per run, reference-initialized scratch, portable math, supplied scene transform, no actor/script tick, skinning or Android claim.","poses":poses,"bone_matrices":matrices,"objects":objects}),
        )?,
    )?;
    println!("{poses} integrated full poses with cached repeats, {matrices} matrices");
    Ok(())
}
