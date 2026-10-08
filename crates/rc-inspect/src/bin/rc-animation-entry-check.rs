use rc_package::{
    mesh_animation::AnimationChannel,
    quaternion_animation::PortableQuaternionMath,
    read_package,
    skeletal_animation::{read_mesh_animation, MeshAnimation, StoredSequence},
    skeletal_animation_entry::{
        apply_animation_full, AnimationFullInput, AnimationHosts, AnimationInstance,
    },
    skeletal_bounds::{BoundsPadding, PoseBounds, PoseBoundsHost},
    skeletal_channel::{
        apply_channel, ChannelPoseInput, PortableChannelAngularMath, TrackChannelHost,
    },
    skeletal_channel_stack::PreparedChannelStackHost,
    skeletal_director_rotation::PortableDirectorRotationHost,
    skeletal_full_pose::FullPoseHosts,
    skeletal_hierarchy::prepare_hierarchy,
    skeletal_mesh::StoredBone,
    skeletal_preparation::{
        AnimationPreparationHost, ChannelScratchBuffers, InstanceAnimationBuffers,
    },
    skeletal_root_pose::quaternion_translation_matrix,
    skeletal_root_pose::RootTransform,
};
use std::{collections::HashMap, env, fs, path::Path};
fn joined_pose(rotations: &[[u32; 4]], positions: &[[u32; 3]]) -> Vec<RootTransform> {
    rotations
        .iter()
        .zip(positions)
        .map(|(&rotation, &position)| RootTransform { rotation, position })
        .collect()
}
struct Preparation {
    transform: [u32; 16],
    events: Vec<String>,
}
impl AnimationPreparationHost for Preparation {
    fn mesh_to_world(&mut self) -> Result<[u32; 16], String> {
        self.events.push("transform".into());
        Ok(self.transform)
    }
    fn refresh_linkup(&mut self, i: usize) -> Result<(), String> {
        self.events.push(format!("linkup {i}"));
        Ok(())
    }
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
        return Err("usage: rc-animation-entry-check LINKUPS OUTPUT".into());
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
            let mapping: Vec<i32> = serde_json::from_value(m["mapping"].clone())?;
            let mut host = Host {
                sequences: [s0, s1, s0],
                mapping: &mapping,
                reference: &reference,
                move_bone: h.move_bone,
                calls: vec![],
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
            let mut preparation = Preparation {
                transform,
                events: vec![],
            };
            let linkup_count = o["mappings"].as_array().ok_or("mappings")?.len();
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
                preparation.events.clear();
                host.calls.clear();
                let result = apply_animation_full(
                    &mut state,
                    AnimationFullInput {
                        bones: &bones,
                        linkup_count,
                        hierarchy: &h,
                        editor: false,
                        word_11c: 0,
                        actor_scale: [1f32.to_bits(); 3],
                        padding: &padding,
                    },
                    &mut inverse,
                    &mut scratch,
                    &mut channels,
                    &mut [],
                    AnimationHosts {
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
                let calls = host.calls.clone();
                let cached = apply_animation_full(
                    &mut state,
                    AnimationFullInput {
                        bones: &bones,
                        linkup_count,
                        hierarchy: &h,
                        editor: false,
                        word_11c: 0,
                        actor_scale: [1f32.to_bits(); 3],
                        padding: &padding,
                    },
                    &mut inverse,
                    &mut scratch,
                    &mut channels,
                    &mut [],
                    AnimationHosts {
                        preparation: &mut preparation,
                        pose: FullPoseHosts {
                            channels: &mut host,
                            rotation: &mut rotation,
                            publication: &mut publication,
                        },
                    },
                );
                if host.calls != calls || publication.calls != step + 1 {
                    return Err("cache invoked a host".into());
                }
                let output = &state.buffers.matrices;
                let local = joined_pose(&state.buffers.rotations, &state.buffers.positions);
                let scratch_after = joined_pose(&scratch.rotations, &scratch.positions);
                poses += 1;
                matrices += output.len();
                ticks.push(serde_json::json!({"step":step,"before":before,"after":channels,"previous":previous,"scratch_before":scratch_before,"scratch_after":scratch_after,"local":local,"calls":host.calls,"result":result,"cached":cached,"bounds_before":bounds_before,"bounds":state.bounds,"published":publication.snapshot,"matrices":output,"preparation_events":preparation.events,"mesh_to_world":state.buffers.mesh_to_world,"byte_61":state.bounds.byte_61}));
            }
            runs.push(serde_json::json!({"entry":m["entry"],"animation":key,"sequences":[0,if a.sequences.len()>1 {1} else {0},0],"ticks":ticks,"inverse":inverse}));
        }
        objects.push(serde_json::json!({"source_index":index,"object":o["object"],"file":o["file"],"runs":runs}));
    }
    fs::write(
        &args[2],
        serde_json::to_vec_pretty(
            &serde_json::json!({"scope":"Full-branch animation entry with native-shaped arrays, instance preparation, supplied transform/linkup hosts, mesh-owned inverse cache, original-track channels, directed hierarchy and local bounds completion. Four caller-invalidated ticks per diagnostic instance and cached repeat after each. Reference-initialized caller scratch; portable math; no actor/runtime binding, actual linkup refresh in preparation host, directors in this probe, root-only branch, skinning or Android claim.","poses":poses,"bone_matrices":matrices,"objects":objects}),
        )?,
    )?;
    println!("{poses} integrated full poses with cached repeats, {matrices} matrices");
    Ok(())
}
