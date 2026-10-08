use rc_package::{
    material_constants::Flicker,
    shader_constants::{
        object_to_camera, object_to_screen, world_to_screen, Bank, Constant, Host, Matrix,
    },
};
use std::{env, fs};
fn fixture(index: usize, matrix: usize) -> Matrix {
    if index == 0 {
        return std::array::from_fn(|r| {
            std::array::from_fn(|c| if r == c { 1f32.to_bits() } else { 0 })
        });
    }
    if index == 1 {
        return match matrix {
            0 => [[1f32.to_bits(); 4]; 4],
            1 => [16777216f32, 1., -16777216., 1.].map(|v| [v.to_bits(); 4]),
            _ => fixture(0, 0),
        };
    }
    std::array::from_fn(|r| {
        std::array::from_fn(|c| {
            let mut word = (index as u32)
                .wrapping_mul(0x9e3779b9)
                .wrapping_add((matrix as u32 + 1) * 0x1020304)
                .wrapping_add((r * 4 + c) as u32 * 0x40507);
            word ^= word >> 16;
            word = word.wrapping_mul(0x85ebca6b);
            word ^= word >> 13;
            match (index + r * 4 + c + matrix) % 41 {
                0 => word & 0x80000000,       // both signs of zero
                1 => (word & 0x807fffff) | 1, // subnormal, gradual-underflow probe
                _ => (word & 0x807fffff) | ((90 + ((word >> 24) % 46)) << 23),
            }
        })
    })
}
fn main() -> Result<(), Box<dyn std::error::Error>> {
    let args: Vec<_> = env::args_os().collect();
    if args.len() != 3 {
        return Err("usage: rc-shader-matrix-check JSON INPUT_BIN".into());
    }
    let mut probes = vec![];
    let mut input = vec![];
    for i in 0..1024usize {
        let [object, view, projection] = std::array::from_fn(|m| fixture(i, m));
        for m in [object, view, projection] {
            for row in m {
                for word in row {
                    input.extend_from_slice(&word.to_le_bytes());
                }
            }
        }
        let outputs = [
            world_to_screen(view, projection)?,
            object_to_screen(object, view, projection)?,
            object_to_camera(object, view)?,
        ];
        let host = Host {
            scene: Default::default(),
            lighting: Default::default(),
            object_to_world: object,
            world_to_camera: view,
            projection,
            camera_to_world: None,
            editor: false,
            engine_time: 0.,
        };
        let mut dispatches = vec![];
        for kind in [2, 3, 32] {
            let size = if i % 2 == 0 { 8 } else { 96 };
            let slot = i % 5;
            let mut bindings = vec![Constant::default(); size];
            bindings[slot].kind = kind;
            bindings[slot + 1].kind = 255; // skipped continuation
            let mut bank = Bank::new(vec![[0x7fc12345, 0x80000000, 0xdeadbeef, i as u32]; size])?;
            if i % 7 == 0 {
                bank.count = (slot + 1) as i32;
            }
            let before = bank.clone();
            let result = bank.update(
                &bindings,
                host,
                &mut Flicker::default(),
                &mut |_| panic!(),
                &mut || panic!(),
            );
            dispatches.push(serde_json::json!({"kind":kind,"slot":slot,"before":before,"after":bank,"error":result.err()}));
        }
        probes.push(serde_json::json!({"index":i,"object":object,"view":view,"projection":projection,"outputs":outputs,"dispatches":dispatches}));
    }
    let mut errors = vec![];
    for kind in [2, 3, 32] {
        for mode in 0..4 {
            let mut host = Host {
                scene: Default::default(),
                lighting: Default::default(),
                object_to_world: fixture(0, 0),
                world_to_camera: fixture(0, 0),
                projection: fixture(0, 0),
                camera_to_world: None,
                editor: false,
                engine_time: 0.,
            };
            match mode {
                0 => host.world_to_camera[0][0] = 0x7fc12345,
                1 => {
                    host.object_to_world = [[f32::MAX.to_bits(); 4]; 4];
                    host.world_to_camera = [[f32::MAX.to_bits(); 4]; 4];
                    host.projection = host.world_to_camera;
                }
                2 => {
                    host.object_to_world = [[1f32.to_bits(); 4]; 4];
                    host.world_to_camera = [[f32::MAX.to_bits(); 4]; 4];
                    host.projection = host.object_to_world;
                }
                _ => {}
            }
            let slot = if mode == 3 { 6 } else { 1 };
            let mut bindings = [Constant::default(); 8];
            bindings[0] = Constant {
                kind: 1,
                words: [0x80000000, 0x7fc11111, 7, 8],
            };
            bindings[slot].kind = kind;
            let mut bank = Bank::new(vec![[9; 4]; 8])?;
            if mode == 3 {
                bank.count = 7;
            }
            let before = bank.clone();
            let result = bank.update(
                &bindings,
                host,
                &mut Flicker::default(),
                &mut |_| panic!(),
                &mut || panic!(),
            );
            assert!(result.is_err());
            errors.push(serde_json::json!({"kind":kind,"mode":mode,"slot":slot,"before":before,"after":bank,"error":result.err()}));
        }
    }
    let report = serde_json::json!({"probes":probes,"errors":errors,"scope":"Native cases 2, 3, 32 with separate scalar-f32 arithmetic orders and transposed bank uploads. Synthetic finite matrices, nearest rounding and gradual underflow; offline relocated SSE is not original DLL execution. Nonfinite inputs/products/sums are safe host errors, unlike native propagation. No live scene, light setup, complete material or Android runtime claim."});
    fs::write(&args[1], serde_json::to_vec(&report)?)?;
    fs::write(&args[2], input)?;
    Ok(())
}
