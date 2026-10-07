use rc_package::{
    quaternion_animation::PortableQuaternionMath,
    skeletal_director::BoneDirector,
    skeletal_director_apply::build_mutable_directed_pose,
    skeletal_director_products::{compose_relative_director, correct_ancestor_rows},
    skeletal_director_rotation::PortableDirectorRotationHost,
    skeletal_hierarchy::prepare_hierarchy,
    skeletal_mesh::StoredBone,
    skeletal_root_pose::RootTransform,
};
use std::{env, fs};
fn main() -> Result<(), Box<dyn std::error::Error>> {
    let args: Vec<_> = env::args_os().collect();
    if args.len() != 3 {
        return Err("usage: rc-directed-rotation-pose-check LOCAL_DIRECTOR_REPORT OUTPUT".into());
    }
    let prior: serde_json::Value = serde_json::from_slice(&fs::read(&args[1])?)?;
    let stacks: serde_json::Value =
        serde_json::from_slice(&fs::read("analysis/reports/original-channel-stacks.json")?)?;
    let links: serde_json::Value = serde_json::from_slice(&fs::read(
        "analysis/reports/original-skeletal-linkups.json",
    )?)?;
    let mesh = [
        2f32, 0., 0., 0., 0., 4., 0., 0., 0., 0., 0.5, 0., 10., 20., 30., 1.,
    ]
    .map(f32::to_bits);
    let scale = [2f32, 0.5, 3.].map(f32::to_bits);
    let mut cases = vec![];
    let mut count = 0;
    for (si, c) in prior["cases"].as_array().ok_or("cases")?.iter().enumerate() {
        if c["editor"].as_bool().ok_or("editor")? {
            continue;
        }
        let oi = c["source_object"].as_u64().ok_or("object")? as usize;
        let ri = c["run"].as_u64().ok_or("run")? as usize;
        let ti = c["tick"].as_u64().ok_or("tick")? as usize;
        let o = &stacks["objects"][oi];
        let index = o["source_index"].as_u64().ok_or("index")? as usize;
        let bones: Vec<StoredBone> =
            serde_json::from_value(links["objects"][index]["prefix"]["bones"].clone())?;
        let h = prepare_hierarchy(&bones)?;
        let local: Vec<RootTransform> =
            serde_json::from_value(o["runs"][ri]["ticks"][ti]["local"].clone())?;
        let target = ((h.move_bone + 3) as usize).min(local.len() - 1);
        let supplied: [u32; 16] = serde_json::from_value(c["matrices"][target].clone())?;
        for scenario in 0..4 {
            let mut words = [0; 28];
            words[0] = target as u32;
            words[19] = 1;
            let disabled = BoneDirector { words };
            words[0] |= 0x80000000;
            words[2..18].copy_from_slice(&supplied);
            words[18] = if scenario == 2 {
                0x100
            } else if scenario == 3 {
                0x101
            } else {
                0x10101
            };
            words[19] = [1, 3, 2, 0][scenario];
            words[20] = (if scenario % 2 == 1 { 0.3f32 } else { -1. }).to_bits();
            words[21] = (if scenario == 1 || scenario == 2 {
                0f32
            } else {
                -1.
            })
            .to_bits();
            words[26] = 0.25f32.to_bits();
            words[27] = 0xaabbcc00;
            let selected = BoneDirector { words };
            let mut skipped = selected.clone();
            skipped.words[19] = 0xffffffff;
            let mut directors = [disabled, selected, skipped];
            let mut host = PortableDirectorRotationHost {
                math: PortableQuaternionMath,
            };
            for iteration in 0..2 {
                if iteration == 1 {
                    directors[1].words[26] = 0.125f32.to_bits();
                }
                let before = directors.clone();
                let mut output = vec![];
                let result = build_mutable_directed_pose(
                    &local,
                    &h,
                    false,
                    &mut directors,
                    mesh,
                    scale,
                    &mut host,
                    &mut output,
                );
                count += output.len();
                cases.push(serde_json::json!({"source_case":si,"scenario":scenario,"iteration":iteration,"target":target,"before":before,"after":directors,"result":result,"matrices":output}));
            }
        }
    }
    let mut products = vec![];
    for seed in 0..64 {
        let m =
            std::array::from_fn::<_, 16, _>(|i| (((i * 13 + seed * 17) % 47) as f32 - 23.) * 0.125)
                .map(f32::to_bits);
        let d =
            std::array::from_fn::<_, 16, _>(|i| (((i * 7 + seed * 11) % 41) as f32 - 20.) * 0.2)
                .map(f32::to_bits);
        products.push(serde_json::json!({"current":m,"director":d,"relative":compose_relative_director(m,d),"ancestor":correct_ancestor_rows(m,d)}));
    }
    fs::write(
        &args[2],
        serde_json::to_vec_pretty(
            &serde_json::json!({"scope":"Full directed bone-matrix evaluation on original-track-derived local poses with supplied mutable director/actor/MeshToWorld snapshots and portable math. Four rotation configurations and two successive history evaluations per source pose, including relative composition and previous-index correction. No original director/runtime binding, full animation preparation, bounds publication, mesh skinning or Android integration.","mesh_to_world":mesh,"actor_scale":scale,"poses":cases.len(),"bone_matrices":count,"cases":cases,"products":products}),
        )?,
    )?;
    println!("{} poses / {count} matrices", cases.len());
    Ok(())
}
