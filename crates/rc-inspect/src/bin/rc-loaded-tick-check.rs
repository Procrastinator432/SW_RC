use rc_package::{
    mesh_animation::AnimationChannel,
    name_bindings::diagnostic_bindings,
    quaternion_animation::PortableQuaternionMath,
    read_package,
    skeletal_animation::{read_mesh_animation, MeshAnimation},
    skeletal_animation_entry::{
        apply_animation_full, apply_animation_root, AnimationFullInput, AnimationHosts,
        AnimationInstance, AnimationRootInput,
    },
    skeletal_animation_tick::{TickEndHost, TickNotifyHost},
    skeletal_bounds::{BoundsPadding, PoseBounds, PoseBoundsHost},
    skeletal_channel::PortableChannelAngularMath,
    skeletal_director_rotation::PortableDirectorRotationHost,
    skeletal_full_pose::FullPoseHosts,
    skeletal_hierarchy::prepare_hierarchy,
    skeletal_lod_tick::update_lod_animation,
    skeletal_mesh::StoredBone,
    skeletal_preparation::{ChannelScratchBuffers, InstanceAnimationBuffers},
    skeletal_root_pose::RootTransform,
    skeletal_runtime_hosts::{
        ResolvedLinkup, ResolvedSequence, RuntimeAnimationBindings, RuntimePoseHost,
        RuntimePreparationHost,
    },
    skeletal_runtime_tick::{NotifyObjectBinding, RuntimeTickEnvironment, RuntimeTickHost},
};
use std::{
    cell::RefCell,
    collections::{BTreeSet, HashMap},
    env, fs,
    path::Path,
    rc::Rc,
};
#[derive(Default)]
struct Environment {
    events: Vec<String>,
}
#[derive(Default)]
struct Publication {
    snapshot: Option<PoseBounds>,
    calls: usize,
}
impl PoseBoundsHost for Publication {
    fn reciprocal_sqrt_seed(&mut self, n: f32) -> Result<f32, String> {
        Ok(1.0 / n.sqrt())
    }
    fn publish(&mut self, b: &PoseBounds) -> Result<(), String> {
        self.snapshot = Some(b.clone());
        self.calls += 1;
        Ok(())
    }
}
impl TickNotifyHost for Environment {
    fn notify(&mut self, object: u32, _: &mut Vec<AnimationChannel>) -> Result<(), String> {
        self.events.push(format!("notify {object}"));
        Ok(())
    }
}
impl TickEndHost for Environment {
    fn clear(&mut self, _: [u32; 8], _: &mut AnimationChannel) -> Result<(), String> {
        self.events.push("clear".into());
        Ok(())
    }
    fn anim_end(&mut self, number: i32, _: &mut AnimationChannel) -> Result<(), String> {
        self.events.push(format!("end {number}"));
        Ok(())
    }
}
impl RuntimeTickEnvironment for Environment {
    fn set_locked(&mut self, locked: bool) -> Result<(), String> {
        self.events.push(format!("lock {locked}"));
        Ok(())
    }
    fn flags(&mut self) -> Result<u32, String> {
        self.events.push("flags".into());
        Ok(0)
    }
    fn request_destroy(&mut self) -> Result<(), String> {
        Err("unexpected destruction".into())
    }
    fn random_int(&mut self) -> Result<i32, String> {
        self.events.push("random 16384".into());
        Ok(16384)
    }
    fn actor_has_mesh(&mut self) -> Result<bool, String> {
        self.events.push("actor mesh".into());
        Ok(true)
    }
    fn replicate(&mut self, index: usize, _: &mut Vec<AnimationChannel>) -> Result<(), String> {
        self.events.push(format!("replicate {index}"));
        Ok(())
    }
}
fn main() -> Result<(), Box<dyn std::error::Error>> {
    let args: Vec<_> = env::args_os().collect();
    let full = args.get(3).and_then(|a| a.to_str()) == Some("--full");
    if args.len() != 3 && !(args.len() == 4 && full) {
        return Err("usage: rc-loaded-tick-check ORIGINAL_LINKUPS OUTPUT [--full]".into());
    }
    let links: serde_json::Value = serde_json::from_slice(&fs::read(&args[1])?)?;
    let mut animations: HashMap<String, MeshAnimation> = HashMap::new();
    let mut objects = vec![];
    let mut count = 0;
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
        let hierarchy = prepare_hierarchy(&bones)?;
        let reference: Vec<_> = bones
            .iter()
            .map(|b| RootTransform {
                rotation: b.rotation,
                position: b.position,
            })
            .collect();
        let mut inverse = vec![];
        let mut runs = vec![];
        for mapping in o["mappings"].as_array().ok_or("mappings")? {
            let key = mapping["animation"].as_str().ok_or("animation")?;
            if !animations.contains_key(key) {
                let (file, object) = key.split_once('.').ok_or("animation path")?;
                let path = Path::new(o["file"].as_str().ok_or("file")?)
                    .parent()
                    .ok_or("parent")?
                    .join(format!("{file}.ukx"));
                let bytes = fs::read(path)?;
                let package = read_package(&bytes)?;
                let export = package
                    .exports
                    .iter()
                    .enumerate()
                    .find_map(|(i, e)| {
                        package
                            .object_path(i as i32 + 1)
                            .ok()
                            .filter(|p| p.eq_ignore_ascii_case(object))
                            .map(|_| e)
                    })
                    .ok_or("export")?;
                animations.insert(key.into(), read_mesh_animation(&package, &bytes, export)?);
            }
            let animation = &animations[key];
            let Some(sequence) = animation.sequences.first() else {
                continue;
            };
            let names: BTreeSet<_> = bones
                .iter()
                .map(|b| b.name.name.clone())
                .chain(
                    animation
                        .reference_bones
                        .iter()
                        .map(|b| b.name.name.clone()),
                )
                .collect();
            let names = diagnostic_bindings(&names)?;
            let mesh_names: Vec<_> = bones
                .iter()
                .map(|b| names[&b.name.name.to_ascii_lowercase()].name.handle)
                .collect();
            let animation_names: Vec<_> = animation
                .reference_bones
                .iter()
                .map(|b| names[&b.name.name.to_ascii_lowercase()].name.handle)
                .collect();
            let catalog = [ResolvedSequence {
                identity: 1,
                linkup_key: 7,
                sequence,
            }];
            let data = RuntimeAnimationBindings {
                sequences: &catalog,
                names: &[(101, 1)],
                linkups: Rc::new(RefCell::new(vec![ResolvedLinkup {
                    key: 7,
                    animation_names: Some(animation_names),
                    mapping: vec![],
                }])),
                events: Rc::new(RefCell::new(vec![])),
            };
            let object_indices: BTreeSet<_> = sequence
                .notifies
                .iter()
                .filter_map(|n| (n.object_index != 0).then_some(n.object_index))
                .collect();
            let notify_objects: Vec<_> = object_indices
                .iter()
                .enumerate()
                .map(|(i, &object_index)| NotifyObjectBinding {
                    sequence_identity: 1,
                    object_index,
                    identity: i as u32 + 1,
                })
                .collect();
            let bindings: Vec<_> = notify_objects
                .iter()
                .map(|b| serde_json::json!({"index":b.object_index,"identity":b.identity}))
                .collect();
            let mut tick = RuntimeTickHost {
                data: data.clone(),
                notify_objects: &notify_objects,
                environment: Environment::default(),
            };
            let mut pose = RuntimePoseHost {
                data: data.clone(),
                reference: &reference,
                move_bone: hierarchy.move_bone,
                editor: false,
                math: PortableQuaternionMath,
                angular: PortableChannelAngularMath,
            };
            let identity = std::array::from_fn(|i| if i / 4 == i % 4 { 1f32.to_bits() } else { 0 });
            let mut prep = RuntimePreparationHost {
                data: data.clone(),
                mesh_names: &mesh_names,
                mesh_to_world: identity,
            };
            let mut state = AnimationInstance {
                buffers: InstanceAnimationBuffers {
                    rotations: vec![],
                    positions: vec![],
                    matrices: vec![],
                    mesh_to_world: [66; 16],
                },
                bounds: PoseBounds {
                    minimum: [99; 3],
                    maximum: [100; 3],
                    sphere: [123; 4],
                    byte_60: 7,
                    byte_61: 7,
                    byte_179: 9,
                },
            };
            let mut channels = vec![AnimationChannel::default()];
            let mut scratch = ChannelScratchBuffers {
                rotations: vec![],
                positions: vec![],
            };
            let padding = BoundsPadding {
                minimum: [0.25f32, 1.0, 2.0].map(f32::to_bits),
                maximum: [2f32, 0.5, 1.0].map(f32::to_bits),
                k_one: [1f32.to_bits(); 3],
            };
            let mut rotation = PortableDirectorRotationHost {
                math: PortableQuaternionMath,
            };
            let mut publication = Publication::default();
            channels[0].words[0] = 101;
            channels[0].words[1] = 1;
            channels[0].words[6] = 1f32.to_bits();
            channels[0].words[9] = (1.0 - 1.0 / sequence.frames as f32).to_bits();
            channels[0].words[11] = 1f32.to_bits();
            channels[0].words[14] = 1f32.to_bits();
            channels[0].words[16] = bones.len() as u32;
            channels[0].words[17] = 1;
            let mut cases = vec![];
            for (step, delta) in [0.125, 0.25, 0.5, 1.25].into_iter().enumerate() {
                let before = channels.clone();
                let cache_before = state.bounds.byte_61;
                tick.environment.events.clear();
                data.events.borrow_mut().clear();
                let tick_result = update_lod_animation(
                    &mut channels,
                    &mut state.bounds.byte_61,
                    delta,
                    step % 2 == 1,
                    &mut tick,
                )?;
                let after_tick = channels.clone();
                let bounds_before = state.bounds.clone();
                let mut full_details = serde_json::Value::Null;
                let result = if full {
                    let input = || AnimationFullInput {
                        bones: &bones,
                        hierarchy: &hierarchy,
                        linkup_count: 1,
                        editor: false,
                        word_11c: 0,
                        actor_scale: [1f32.to_bits(); 3],
                        padding: &padding,
                    };
                    let result = apply_animation_full(
                        &mut state,
                        input(),
                        &mut inverse,
                        &mut scratch,
                        &mut channels,
                        &mut [],
                        AnimationHosts {
                            preparation: &mut prep,
                            pose: FullPoseHosts {
                                channels: &mut pose,
                                rotation: &mut rotation,
                                publication: &mut publication,
                            },
                        },
                    )?;
                    let cached_before = serde_json::json!({"buffers":state.buffers,"bounds":state.bounds,"scratch":scratch,"channels":channels});
                    let events_before = data.events.borrow().len();
                    let calls_before = publication.calls;
                    let cached = apply_animation_full(
                        &mut state,
                        input(),
                        &mut inverse,
                        &mut scratch,
                        &mut channels,
                        &mut [],
                        AnimationHosts {
                            preparation: &mut prep,
                            pose: FullPoseHosts {
                                channels: &mut pose,
                                rotation: &mut rotation,
                                publication: &mut publication,
                            },
                        },
                    )?;
                    let cached_after = serde_json::json!({"buffers":state.buffers,"bounds":state.bounds,"scratch":scratch,"channels":channels});
                    if cached_before != cached_after
                        || publication.calls != calls_before
                        || data.events.borrow().len() != events_before + 2
                    {
                        return Err(
                            "cached full pose changed state or invoked unexpected hosts".into()
                        );
                    }
                    full_details = serde_json::json!({"cached":cached,"cached_unchanged":true,"local":
                        state.buffers.rotations.iter().zip(&state.buffers.positions).map(|(&rotation,&position)| RootTransform {rotation,position}).collect::<Vec<_>>(),
                        "scratch":scratch,"matrices":state.buffers.matrices,"published":publication.snapshot,
                        "publication_calls":publication.calls,"bounds_before":bounds_before});
                    serde_json::to_value(result)?
                } else {
                    serde_json::to_value(apply_animation_root(
                        &mut state,
                        AnimationRootInput {
                            bones: &bones,
                            linkup_count: 1,
                            word_11c: 0,
                        },
                        &mut inverse,
                        &mut channels,
                        &mut prep,
                        &mut pose,
                    )?)?
                };
                let tails_zero = state.buffers.rotations[1..].iter().all(|v| *v == [0; 4])
                    && state.buffers.positions[1..].iter().all(|v| *v == [0; 3])
                    && state.buffers.matrices[1..].iter().all(|v| *v == [0; 16]);
                if !full && !tails_zero {
                    return Err("nonroot tail modified".into());
                }
                cases.push(serde_json::json!({"step":step,"delta_bits":delta.to_bits(),"editor":step%2==1,
                    "cache_before":cache_before,"before":before,"after_tick":after_tick,"after_pose":channels,
                    "tick":{"exit":format!("{:?}",tick_result.exit),"attempts":tick_result.attempts,"replicated":tick_result.replicated},
                    "tick_events":tick.environment.events,"pose_result":result,"pose_events":*data.events.borrow(),
                    "root":{"rotation":state.buffers.rotations[0],"position":state.buffers.positions[0]},"matrix":state.buffers.matrices[0],
                    "bounds":state.bounds,"tails_zero":tails_zero,"full":full_details}));
                count += 1;
            }
            runs.push(serde_json::json!({"animation":key,"entry":mapping["entry"],"sequence":0,"bindings":bindings,"mapping":data.linkups.borrow()[0].mapping,"cases":cases}));
        }
        objects.push(serde_json::json!({"source_index":index,"runs":runs}));
    }
    fs::write(
        &args[2],
        serde_json::to_vec_pretty(&serde_json::json!({"cases":count,"objects":objects,
        "mode":if full {"Full"} else {"Root"},
        "scope":"Original first-sequence metadata and tracks, real mesh skeletons/linkup refresh: four continuous prepared LOD ticks then selected pose entry per linkup. Full mode includes hierarchy, bounds and cached repeats. Diagnostic identities, looping channel, supplied random=16384, callback logging and scene transform; no actual Actor/Script callbacks, rendering or Android claim."}))?,
    )?;
    println!("{count} loaded-sequence tick/pose cases, full={full}");
    Ok(())
}
