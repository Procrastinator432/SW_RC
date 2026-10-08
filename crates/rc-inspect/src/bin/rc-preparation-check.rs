use rc_package::{
    skeletal_mesh::StoredBone,
    skeletal_preparation::{
        prepare_animation_instance, prepare_channel_scratch, prepare_instance_buffers,
        AnimationPreparationHost, ChannelScratchBuffers, InstanceAnimationBuffers,
    },
};
use std::{env, fs};
fn data<const N: usize>(count: usize, base: u32) -> Vec<[u32; N]> {
    (0..count)
        .map(|i| std::array::from_fn(|j| base + (i * N + j) as u32))
        .collect()
}
struct Host {
    matrix: [u32; 16],
    events: Vec<String>,
}
impl AnimationPreparationHost for Host {
    fn mesh_to_world(&mut self) -> Result<[u32; 16], String> {
        self.events.push("transform".into());
        Ok(self.matrix)
    }
    fn refresh_linkup(&mut self, i: usize) -> Result<(), String> {
        self.events.push(format!("linkup {i}"));
        Ok(())
    }
}
fn main() -> Result<(), Box<dyn std::error::Error>> {
    let args: Vec<_> = env::args_os().collect();
    if args.len() != 3 {
        return Err("usage: rc-preparation-check ORIGINAL_LINKUPS OUTPUT".into());
    }
    let mut cases = vec![];
    for count in 0..6 {
        for q in 0..6 {
            for p in 0..6 {
                for m in 0..6 {
                    for byte in [0, 7] {
                        let mut state = InstanceAnimationBuffers {
                            rotations: data(q, 0x7fc10000),
                            positions: data(p, 0x80000000),
                            matrices: data(m, 0x3f000000),
                            mesh_to_world: [66; 16],
                        };
                        let mut cache = byte;
                        let result = prepare_instance_buffers(&mut state, count, &mut cache);
                        cases.push(serde_json::json!({"count":count,"q":q,"p":p,"m":m,"byte_before":byte,"byte_after":cache,"result":result,"after":state}));
                    }
                }
            }
        }
    }
    let mut scratch = vec![];
    for count in 0..6 {
        for q in 0..6 {
            for p in 0..6 {
                let mut s = ChannelScratchBuffers {
                    rotations: data(q, 0x7fc10000),
                    positions: data(p, 0x80000000),
                };
                let resized = prepare_channel_scratch(&mut s, count);
                scratch.push(
                    serde_json::json!({"count":count,"q":q,"p":p,"resized":resized,"after":s}),
                );
            }
        }
    }
    let original: serde_json::Value = serde_json::from_slice(&fs::read(&args[1])?)?;
    let mut objects = vec![];
    for (index, o) in original["objects"]
        .as_array()
        .ok_or("objects")?
        .iter()
        .enumerate()
    {
        if o["status"] == "UnsupportedLegacyPackage" {
            continue;
        }
        let bones: Vec<StoredBone> = serde_json::from_value(o["prefix"]["bones"].clone())?;
        let linkups = o["mappings"].as_array().ok_or("mappings")?.len();
        let mut state = InstanceAnimationBuffers {
            rotations: bones.iter().take(2).map(|b| b.rotation).collect(),
            positions: bones.iter().take(1).map(|b| b.position).collect(),
            matrices: vec![],
            mesh_to_world: [66; 16],
        };
        let mut matrix = [0; 16];
        for i in [0, 5, 10, 15] {
            matrix[i] = 1f32.to_bits();
        }
        matrix[12] = (index as f32).to_bits();
        let mut host = Host {
            matrix,
            events: vec![],
        };
        let mut cache = 7;
        let mut inverse = vec![];
        let built = prepare_animation_instance(
            &mut state,
            &bones,
            &mut inverse,
            &mut cache,
            linkups,
            &mut host,
        )?;
        let cache_build = cache;
        let before = serde_json::to_value(&state)?;
        let inverse_before = inverse.clone();
        cache = 7;
        let reused = prepare_animation_instance(
            &mut state,
            &bones,
            &mut inverse,
            &mut cache,
            linkups,
            &mut host,
        )?;
        if serde_json::to_value(&state)? != before || inverse != inverse_before {
            return Err("reuse altered buffers or inverse".into());
        }
        objects.push(serde_json::json!({"source_index":index,"linkups":linkups,"built":built,"reused":reused,"cache_build":cache_build,"cache_reuse":cache,"buffers":state,"inverse":inverse,"events":host.events}));
    }
    fs::write(
        &args[2],
        serde_json::to_vec_pretty(
            &serde_json::json!({"scope":"Original ApplyAnimation instance-buffer content preparation and scratch quaternion-only gate. Connected buffer/transform/inverse/linkup order with original skeletons and supplied transform/linkup hosts. No native heap flags/capacity, allocator failures, actual actor virtual method, complete entry, root/full branch, skinning or Android claim.","instance_cases":cases,"scratch_cases":scratch,"objects":objects}),
        )?,
    )?;
    println!(
        "{} buffer cases, {} scratch cases, {} original preparations and reuses",
        cases.len(),
        scratch.len(),
        objects.len()
    );
    Ok(())
}
