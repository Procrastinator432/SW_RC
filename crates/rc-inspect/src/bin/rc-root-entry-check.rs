use rc_package::{
    mesh_animation::AnimationChannel,
    quaternion_animation::{PortableQuaternionMath, QuaternionTrackHost},
    read_package,
    skeletal_animation::{read_mesh_animation, MeshAnimation, StoredSequence},
    skeletal_animation_entry::{apply_animation_root, AnimationInstance, AnimationRootInput},
    skeletal_bounds::PoseBounds,
    skeletal_mesh::StoredBone,
    skeletal_preparation::{AnimationPreparationHost, InstanceAnimationBuffers},
    skeletal_root_pose::{RootPoseHost, RootSampleRequest, RootSequence, RootTransform},
    skeletal_track::sample_track,
};
use std::{collections::HashMap, env, fs, path::Path};
struct Preparation {
    events: Vec<String>,
}
impl AnimationPreparationHost for Preparation {
    fn mesh_to_world(&mut self) -> Result<[u32; 16], String> {
        self.events.push("transform".into());
        Ok(std::array::from_fn(|i| {
            if i / 4 == i % 4 {
                1f32.to_bits()
            } else {
                0
            }
        }))
    }
    fn refresh_linkup(&mut self, i: usize) -> Result<(), String> {
        self.events.push(format!("linkup {i}"));
        Ok(())
    }
}
struct RootHost<'a> {
    sequence: &'a StoredSequence,
    track: i32,
    requests: Vec<RootSampleRequest>,
    times: Vec<u32>,
}
impl RootPoseHost for RootHost<'_> {
    fn sequence(
        &mut self,
        _: usize,
        _: &mut [AnimationChannel],
    ) -> Result<Option<RootSequence>, String> {
        Ok(Some(RootSequence {
            token: 0,
            frames: self.sequence.frames,
            track_count_word: self.sequence.tracks.len() as u32,
        }))
    }
    fn root_track(&mut self, _: RootSequence) -> Result<i32, String> {
        Ok(self.track)
    }
    fn sample(&mut self, r: RootSampleRequest, root: &mut RootTransform) -> Result<(), String> {
        let time = (r.frames as f64 * r.normalized_frame as f64) as f32;
        self.requests.push(r);
        self.times.push(time.to_bits());
        let t = self
            .sequence
            .tracks
            .get(r.track as usize)
            .ok_or("root track missing")?;
        sample_track(
            t,
            time,
            root,
            &mut QuaternionTrackHost {
                math: PortableQuaternionMath,
            },
        )?;
        Ok(())
    }
}
fn main() -> Result<(), Box<dyn std::error::Error>> {
    let args: Vec<_> = env::args_os().collect();
    if args.len() != 3 {
        return Err("usage: rc-root-entry-check ORIGINAL_LINKUPS OUTPUT".into());
    }
    let original: serde_json::Value = serde_json::from_slice(&fs::read(&args[1])?)?;
    let mut animations: HashMap<String, MeshAnimation> = HashMap::new();
    let mut objects = vec![];
    let mut cases_count = 0;
    for (source_index, o) in original["objects"]
        .as_array()
        .ok_or("objects")?
        .iter()
        .enumerate()
    {
        if o["status"] == "UnsupportedLegacyPackage" {
            continue;
        }
        let bones: Vec<StoredBone> = serde_json::from_value(o["prefix"]["bones"].clone())?;
        let mappings = o["mappings"].as_array().ok_or("mappings")?;
        let mut inverse = vec![];
        let mut runs = vec![];
        for mapping in mappings {
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
                    .ok_or("animation export")?;
                animations.insert(key.into(), read_mesh_animation(&package, &bytes, export)?);
            }
            let animation = &animations[key];
            let Some(sequence) = animation.sequences.first() else {
                continue;
            };
            let track = mapping["mapping"][0].as_i64().ok_or("root mapping")? as i32;
            let mut host = RootHost {
                sequence,
                track,
                requests: vec![],
                times: vec![],
            };
            let mut prep = Preparation { events: vec![] };
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
            let mut cases = vec![];
            for (i, frame) in [0f32, 0.25, 0.75, 1., -1., f32::NAN, 0.25]
                .into_iter()
                .enumerate()
            {
                state.bounds.byte_61 = 7;
                let mut c = AnimationChannel::default();
                c.words[7] = frame.to_bits();
                c.words[11] = 0.5f32.to_bits();
                c.words[14] = 1f32.to_bits();
                c.words[16] = bones.len() as u32;
                let before = c.clone();
                host.requests.clear();
                host.times.clear();
                prep.events.clear();
                let result = apply_animation_root(
                    &mut state,
                    AnimationRootInput {
                        bones: &bones,
                        linkup_count: mappings.len(),
                        word_11c: u32::from(i == 6),
                    },
                    &mut inverse,
                    std::slice::from_mut(&mut c),
                    &mut prep,
                    &mut host,
                )?;
                let tails_zero = state.buffers.rotations[1..].iter().all(|v| *v == [0; 4])
                    && state.buffers.positions[1..].iter().all(|v| *v == [0; 3])
                    && state.buffers.matrices[1..].iter().all(|v| *v == [0; 16]);
                if !tails_zero {
                    return Err("root-only modified nonroot tail".into());
                }
                cases.push(serde_json::json!({"scenario":i,"before":before,"after":c,"result":result,"root":{"rotation":state.buffers.rotations[0],"position":state.buffers.positions[0]},"matrix":state.buffers.matrices[0],"bounds":state.bounds,"tails_zero":tails_zero,"bone_count":bones.len(),"requests":host.requests,"time_bits":host.times,"preparation_events":prep.events}));
                cases_count += 1;
            }
            runs.push(serde_json::json!({"animation":key,"entry":mapping["entry"],"sequence":0,"track":track,"cases":cases,"inverse":inverse}));
        }
        objects.push(serde_json::json!({"source_index":source_index,"runs":runs}));
    }
    fs::write(
        &args[2],
        serde_json::to_vec_pretty(
            &serde_json::json!({"scope":"Root-only entry with shared instance preparation, original first-sequence root tracks and mesh-owned reference inverse cache. Seven caller scenarios per linkup: 0/.25/.75/1/negative/NaN frame and blocked sampling. Cold instance buffers per run, reference-cache reuse across runs. Portable f64 approximation of native x87 frame multiplication and portable quaternion math. Supplied transform/linkup hosts, no actual actor/runtime binding, scratch, full-pose bounds completion, skinning or Android claim.","cases":cases_count,"objects":objects}),
        )?,
    )?;
    println!("{cases_count} original-track root-entry cases");
    Ok(())
}
