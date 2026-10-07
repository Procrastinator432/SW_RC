use rc_package::{
    quaternion_animation::PortableQuaternionMath,
    skeletal_bounds::{finish_prepared_bounds, BoundsPadding, PoseBounds},
    skeletal_director::*,
    skeletal_hierarchy::prepare_hierarchy,
    skeletal_mesh::StoredBone,
    skeletal_root_pose::RootTransform,
    skeletal_world_bounds::WorldPoseBoundsHost,
};
use std::{env, fs};
fn main() -> Result<(), Box<dyn std::error::Error>> {
    let args: Vec<_> = env::args_os().collect();
    if args.len() != 3 {
        return Err("usage: rc-director-pose-check STACKS OUTPUT".into());
    }
    let stacks: serde_json::Value = serde_json::from_slice(&fs::read(&args[1])?)?;
    let links: serde_json::Value = serde_json::from_slice(&fs::read(
        "analysis/reports/original-skeletal-linkups.json",
    )?)?;
    let padding = BoundsPadding {
        minimum: [0; 3],
        maximum: [0; 3],
        k_one: [1f32.to_bits(); 3],
    };
    let mut cases = vec![];
    let mut matrices = 0;
    for (oi, o) in stacks["objects"]
        .as_array()
        .ok_or("objects")?
        .iter()
        .enumerate()
    {
        let index = o["source_index"].as_u64().ok_or("index")? as usize;
        let bones: Vec<StoredBone> =
            serde_json::from_value(links["objects"][index]["prefix"]["bones"].clone())?;
        let hierarchy = prepare_hierarchy(&bones)?;
        let target = (hierarchy.move_bone + 1) as usize;
        if target >= bones.len() {
            return Err("diagnostic target bone absent".into());
        }
        let mut disabled = BoneDirector { words: [0; 28] };
        disabled.words[0] = target as u32;
        disabled.words[19] = 1;
        let mut selected = disabled.clone();
        selected.words[0] |= 0x80000000;
        selected.words[18] = 0x10001;
        for i in 0..12 {
            selected.words[2 + i] = [0.75f32, 1.25, 1.5, 1.][i % 4].to_bits();
        }
        selected.words[14..18].copy_from_slice(&[3f32, -2., 5., 1.].map(f32::to_bits));
        let mut skipped = selected.clone();
        skipped.words[18] = 0x100;
        let directors = [disabled, selected, skipped];
        for (ri, r) in o["runs"].as_array().ok_or("runs")?.iter().enumerate() {
            for (ti, t) in r["ticks"].as_array().ok_or("ticks")?.iter().enumerate() {
                let local: Vec<RootTransform> = serde_json::from_value(t["local"].clone())?;
                for editor in [false, true] {
                    let mut output = vec![];
                    let result = build_local_directed_pose(
                        &local,
                        &hierarchy,
                        editor,
                        &directors,
                        &mut output,
                    );
                    matrices += output.len();
                    let mut bounds = PoseBounds {
                        minimum: [0; 3],
                        maximum: [0; 3],
                        sphere: [0; 4],
                        byte_60: 0,
                        byte_61: 0,
                        byte_179: 1,
                    };
                    let bounds_result = finish_prepared_bounds(
                        &mut bounds,
                        &output,
                        hierarchy.move_bone,
                        &padding,
                        &mut WorldPoseBoundsHost {
                            math: PortableQuaternionMath,
                            actor: None,
                        },
                    );
                    cases.push(serde_json::json!({"source_object":oi,"run":ri,"tick":ti,"editor":editor,"target":target,"directors":directors,"result":result,"matrices":output,"bounds":bounds,"bounds_result":bounds_result}));
                }
            }
        }
    }
    fs::write(
        &args[2],
        serde_json::to_vec_pretty(
            &serde_json::json!({"scope":"Original track-stack local poses plus supplied local translation/plane-scale director snapshots. First inactive ignored, first active sign-bit-tagged bone selected, later rotation director skipped. Editor/game hierarchy rules and downstream local bounds use diagnostic zero mesh padding/kOne1, no actor. No original director track loader, rotation/ancestor correction, inverse MeshToWorld or skinning claim.","poses":cases.len(),"bone_matrices":matrices,"cases":cases}),
        )?,
    )?;
    println!("{} directed poses, {matrices} matrices", cases.len());
    Ok(())
}
