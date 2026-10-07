use rc_package::{
    skeletal_director::BoneDirector,
    skeletal_set_bone_place::{set_bone_place, BonePlaceDefaults, BonePlaceNames},
};
use std::{env, fs};
struct Names {
    target: i32,
    calls: Vec<i32>,
}
impl BonePlaceNames for Names {
    fn match_name(&mut self, index: i32) -> Result<i32, String> {
        self.calls.push(index);
        Ok(if index == 1 { self.target } else { -1 })
    }
}
fn main() -> Result<(), Box<dyn std::error::Error>> {
    let args: Vec<_> = env::args_os().collect();
    if args.len() != 2 {
        return Err("usage: rc-set-bone-place-check OUTPUT".into());
    }
    let defaults = BonePlaceDefaults {
        identity_quaternion: [0, 0, 0, 1f32.to_bits()],
        append_padding: [0xaa, 0xbb, 0xcc],
    };
    let mut cases = vec![];
    for seed in 0..128u32 {
        for scenario in 0..4 {
            let target = seed % 4;
            let mut input = std::array::from_fn::<_, 22, _>(|i| {
                seed.wrapping_mul(1234567) ^ (i as u32).wrapping_mul(7919)
            });
            input[0] = target;
            input[1] = 9;
            if scenario == 1 {
                input[0] = 0x80000001;
                input[1] = 0x80000002;
            }
            if scenario == 2 {
                input[0] = 8;
            }
            let mut words =
                std::array::from_fn::<_, 28, _>(|i| 0xaabb0000 ^ seed ^ (i as u32 * 137));
            words[0] = 0x80000000 | target;
            let mut directors = if scenario >= 2 {
                vec![BoneDirector { words }, BoneDirector { words }]
            } else {
                vec![]
            };
            let before = directors.clone();
            let mut cache = 7;
            let mut names = Names {
                target: target as i32,
                calls: vec![],
            };
            let result =
                set_bone_place(input, 4, &mut cache, &mut directors, &defaults, &mut names);
            cases.push(serde_json::json!({"seed":seed,"scenario":scenario,"input":input,"before":before,"after":directors,"cache":cache,"name_calls":names.calls,"result":result}));
        }
    }
    fs::write(
        &args[1],
        serde_json::to_vec_pretty(
            &serde_json::json!({"scope":"Prepared SetBonePlace state machine on 512 synthetic snapshots; positive global name resolution/aliases supplied by host, identity quaternion and native undefined padding supplied explicitly. No global FName table or live Actor/Script binding.","cases":cases}),
        )?,
    )?;
    println!("512 SetBonePlace cases");
    Ok(())
}
