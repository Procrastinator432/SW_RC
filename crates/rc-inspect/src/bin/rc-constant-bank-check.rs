use rc_package::{
    material_constants::Flicker,
    shader_constants::{Bank, Constant, Host, Matrix},
};
use std::{env, fs};
fn matrix(seed: u32) -> Matrix {
    std::array::from_fn(|r| {
        std::array::from_fn(|c| {
            seed.wrapping_add((r * 4 + c) as u32 * 0x10203)
                .rotate_left((r + c) as u32)
        })
    })
}
fn main() -> Result<(), Box<dyn std::error::Error>> {
    let args: Vec<_> = env::args_os().collect();
    if args.len() != 2 {
        return Err("usage: rc-constant-bank-check OUTPUT".into());
    }
    let mut banks = [
        Bank::new(vec![[0x7fc12345, 0x80000000, 0xdeadbeef, 7]; 8])?,
        Bank::new(vec![[0x7fc12345, 0x80000000, 0xdeadbeef, 7]; 96])?,
    ];
    let mut flicker = Flicker::default();
    let mut probes = vec![];
    for i in 0..1024usize {
        let which = i % 2;
        let bank = &mut banks[which];
        let size = bank.words.len();
        let reset = i % 17 == 0;
        if reset {
            bank.count = if size == 8 { -3 } else { -2 };
        }
        let mut bindings = vec![Constant::default(); size];
        let mode = i / 2 % 7;
        match mode {
            0 => {
                bindings[0].kind = 4;
                bindings[4].kind = 12;
                bindings[5].kind = 8;
                bindings[6] = Constant {
                    kind: 9,
                    words: [0.1f32.to_bits(), 0, 0, 0],
                };
                bindings[7] = Constant {
                    kind: 27,
                    words: [0.1f32.to_bits(), 0.4f32.to_bits(), 0, 0],
                };
            }
            1 => {
                bindings[0].kind = 12;
                bindings[1].kind = 5;
                bindings[5] = Constant {
                    kind: 27,
                    words: [0.99f32.to_bits(), 0.3f32.to_bits(), 0, 0],
                };
                bindings[6].kind = 9;
                bindings[7] = Constant {
                    kind: 1,
                    words: [0x7fc01000 + i as u32, 0x80000000, i as u32, 0],
                };
            }
            2 => {
                bindings[0].kind = 6;
                bindings[4].kind = 5;
            }
            3 => {
                bindings[0].kind = 4;
                bindings[1].kind = 3;
                bindings[4].kind = 12;
                bindings[7] = Constant {
                    kind: 1,
                    words: [i as u32; 4],
                };
            }
            4 => {
                bindings[0] = Constant {
                    kind: 1,
                    words: [0xdead0000 + i as u32; 4],
                };
                bindings[2].kind = 35;
            }
            5 => {}
            _ => {
                bindings[0].kind = 12;
                bindings[1].kind = 5;
                bindings[5].kind = 12;
            }
        }
        let host = Host {
            scene: Default::default(),
            lighting: Default::default(),
            object_to_world: matrix(0x3f800000 + i as u32),
            world_to_camera: matrix(0x40000000 + i as u32),
            projection: [[0; 4]; 4],
            camera_to_world: (i % 4 != 0).then(|| matrix(0x7fc00000 + i as u32)),
            editor: i % 3 == 0,
            engine_time: (i / 4) as f32 * 0.125,
        };
        let inverse_result = matrix(0x12340000 + i as u32);
        let inverse_error = i % 29 == 0;
        let before = bank.clone();
        let flicker_before = flicker.clone();
        let mut inverse_calls = vec![];
        let mut draws = vec![];
        let result = bank.update(
            &bindings,
            host,
            &mut flicker,
            &mut |input| {
                inverse_calls.push(input);
                if inverse_error {
                    Err("fixture inverse unavailable".into())
                } else {
                    Ok(inverse_result)
                }
            },
            &mut || {
                let value = ((i * 97 + draws.len() * 37) % 32768) as u16;
                draws.push(value);
                Ok(value)
            },
        );
        probes.push(serde_json::json!({"index":i,"bank":which,"reset":reset,"bindings":bindings,"host":{"object_to_world":host.object_to_world,"world_to_camera":host.world_to_camera,"camera_to_world":host.camera_to_world,"editor":host.editor,"engine_time":host.engine_time},"inverse_result":inverse_result,"inverse_error":inverse_error,"inverse_calls":inverse_calls,"draws":draws,"before":before,"after":bank,"flicker_before":flicker_before,"flicker_after":flicker,"error":result.err()}));
    }
    let report = serde_json::json!({"probes":probes,"scope":"Raw native ObjectToWorld/WorldToCamera transpose, CameraToWorld/EyePosition selection and per-update cache, persistent constant words/count/continuation dispatch. Matrix inversion and live matrices are host callbacks/fixtures. Composed matrices have a separate probe; light branches remain unsupported. No complete original-material or Android runtime claim."});
    fs::write(&args[1], serde_json::to_vec(&report)?)?;
    Ok(())
}
