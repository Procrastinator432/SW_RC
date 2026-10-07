use rc_package::{
    quaternion_animation::QuaternionMath, skeletal_bounds::PoseBounds, skeletal_world_bounds::*,
};
use std::{env, fs};
struct Math {
    supplied: bool,
    fail: bool,
    seen: Vec<u32>,
}
impl QuaternionMath for Math {
    fn reciprocal_sqrt_seed(&mut self, n: f32) -> Result<f32, String> {
        self.seen.push(n.to_bits());
        if self.fail {
            Err("seed boundary".into())
        } else {
            Ok(if self.supplied { 0.125 } else { 1. / n.sqrt() })
        }
    }
    fn spherical_weights(&mut self, _: f32, _: f32) -> Result<[f32; 2], String> {
        unreachable!()
    }
}
fn state() -> WorldPoseBounds {
    WorldPoseBounds {
        minimum: [99; 3],
        maximum: [99; 3],
        valid: 0,
        sphere: [88; 4],
    }
}
fn main() -> Result<(), Box<dyn std::error::Error>> {
    let args: Vec<_> = env::args_os().collect();
    if args.len() != 3 {
        return Err("usage: rc-world-bounds-check LOCAL_BOUNDS OUTPUT".into());
    }
    let source: serde_json::Value = serde_json::from_slice(&fs::read(&args[1])?)?;
    let transforms = [
        [
            1f32, 0., 0., 0., 0., 1., 0., 0., 0., 0., 1., 0., 0., 0., 0., 1.,
        ],
        [
            0., -2., 0., 0., 3., 0., 0., 0., 0., 0., 0.5, 0., 10., -7., 3., 1.,
        ],
        [
            1., 0.25, 0., 0., 0.5, 1., 0.75, 0., 0., -0.25, 1.5, 0., -5., 2., 11., 1.,
        ],
        [
            0., 0., 0., 0., 0., 0., 0., 0., 0., 0., 0., 0., 4., 5., 6., 1.,
        ],
    ]
    .map(|m| m.map(f32::to_bits));
    let mut cases = vec![];
    for (index, c) in source["cases"]
        .as_array()
        .ok_or("cases")?
        .iter()
        .enumerate()
    {
        let local: PoseBounds = serde_json::from_value(c["after"].clone())?;
        for (ti, m) in transforms.iter().enumerate() {
            let mut after = state();
            let before = after.clone();
            let mut math = Math {
                supplied: false,
                fail: false,
                seen: vec![],
            };
            let result = publish_world_bounds(&local, *m, &mut after, &mut math);
            cases.push(serde_json::json!({"source_case":index,"transform":ti,"before":before,"after":after,"result":result,"squared":math.seen}));
        }
    }
    let local: PoseBounds = serde_json::from_value(source["cases"][0]["after"].clone())?;
    let mut synthetic = vec![];
    for seed in 1..=64 {
        let matrix = std::array::from_fn::<_, 16, _>(|i| {
            ((((i * 17 + seed * 11) % 43) as f32 - 21.) * 0.125).to_bits()
        });
        let mut after = state();
        let before = after.clone();
        let mut math = Math {
            supplied: true,
            fail: seed % 4 == 0,
            seen: vec![],
        };
        let result = publish_world_bounds(&local, matrix, &mut after, &mut math);
        synthetic.push(serde_json::json!({"matrix":matrix,"fail":math.fail,"before":before,"after":after,"result":result,"squared":math.seen}));
    }
    fs::write(
        &args[2],
        serde_json::to_vec_pretty(
            &serde_json::json!({"scope":"World box/sphere publication from 900 original-track-derived local bounds with supplied diagnostic padding, under four supplied transforms. No original runtime actor matrix claim. Portable sqrt seeds; 64 general finite matrix cases with supplied .125 seed and 16 failures. Exact original SSE expression order, no RSQRTSS CPU emulation or skinning.","transforms":transforms,"cases":cases,"synthetic":synthetic}),
        )?,
    )?;
    println!(
        "{} world bounds and {} general matrix cases",
        cases.len(),
        synthetic.len()
    );
    Ok(())
}
