use rc_package::{
    quaternion_animation::PortableQuaternionMath,
    skeletal_director::BoneDirector,
    skeletal_director_rotation::{prepare_director_rotation, PortableDirectorRotationHost},
};
use std::{env, fs};
fn main() -> Result<(), Box<dyn std::error::Error>> {
    let args: Vec<_> = env::args_os().collect();
    if args.len() != 3 {
        return Err("usage: rc-director-rotation-check WORLD_DIRECTOR_REPORT OUTPUT".into());
    }
    let source: serde_json::Value = serde_json::from_slice(&fs::read(&args[1])?)?;
    let mut cases = vec![];
    for (source_index, c) in source["cases"]
        .as_array()
        .ok_or("cases")?
        .iter()
        .enumerate()
    {
        if c["transform"] != 0 {
            continue;
        }
        let matrices: Vec<[u32; 16]> = serde_json::from_value(c["matrices"].clone())?;
        let current = matrices[0];
        let target = matrices[1 % matrices.len()];
        for scenario in 0..4 {
            let mut words = [0; 28];
            words[18] = 0x100;
            words[19] = 1;
            words[2..18].copy_from_slice(&target);
            words[20] = (if scenario >= 2 { 0.3f32 } else { -1. }).to_bits();
            words[21] = (if scenario == 0 { -1f32 } else { 0. }).to_bits();
            words[26] = 0.25f32.to_bits();
            words[22..26].copy_from_slice(&[0, 0, 0, 1f32.to_bits()]);
            words[27] = 0xaabbcc00 | u32::from(scenario % 2 == 1);
            let mut director = BoneDirector { words };
            let before = director.clone();
            let mut host = PortableDirectorRotationHost {
                math: PortableQuaternionMath,
            };
            let scale = [2f32, 0.5, 3.].map(f32::to_bits);
            let result =
                prepare_director_rotation(&mut director, current, target, scale, &mut host);
            cases.push(serde_json::json!({"source_case":source_index,"scenario":scenario,"before":before,"after":director,"result":result}));
        }
    }
    fs::write(
        &args[2],
        serde_json::to_vec_pretty(
            &serde_json::json!({"scope":"Rotation preparation only: history and temporal/absolute limits on supplied director snapshots and original-track-derived matrices. Four configurations per source pose, portable f64 x87/transcendental approximation and portable RSQRT seed. No row application, relative composition, actor/runtime binding or ancestor correction.","cases":cases}),
        )?,
    )?;
    println!("{} prepared director rotations", cases.len());
    Ok(())
}
