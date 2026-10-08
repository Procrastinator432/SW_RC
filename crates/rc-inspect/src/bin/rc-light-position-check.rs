use rc_package::{
    material_constants::Flicker,
    shader_constants::{Bank, Constant, Host, Matrix},
    shader_lights::{self, Light, Lighting},
};
use std::{env, fs};
fn matrix(i: usize) -> Matrix {
    std::array::from_fn(|r| {
        std::array::from_fn(|c| {
            ((if r == c { 2. } else { 0. }) + (i + r * 7 + c * 3) as f32 * 0.001953125).to_bits()
        })
    })
}
fn inverse_fixture(i: usize) -> Matrix {
    std::array::from_fn(|r| {
        std::array::from_fn(|c| {
            if c == 3 {
                0x7fc10000 + i as u32
            } else {
                (((i + r * 11 + c * 5) % 31) as f32 * 0.125 - 1.).to_bits()
            }
        })
    })
}
fn source(i: usize, slot: usize) -> Light {
    Light {
        radius: [0; 2],
        actor_present: true,
        kind: if (i + slot).is_multiple_of(2) {
            0x13
        } else {
            7
        },
        position: [i as f32 * 0.125, slot as f32 - 2., 3.5].map(f32::to_bits),
        direction: [0.125, (slot + 1) as f32 * 0.25, -0.5].map(f32::to_bits),
        ..Default::default()
    }
}
fn main() -> Result<(), Box<dyn std::error::Error>> {
    let args: Vec<_> = env::args_os().collect();
    if args.len() != 3 {
        return Err("usage: rc-light-position-check JSON SSE_INPUT".into());
    }
    let mut probes = vec![];
    let mut scalar = vec![];
    let mut data = vec![];
    for i in 0..1024usize {
        let object = matrix(i);
        let inv = inverse_fixture(i);
        let slots = std::array::from_fn(|slot| {
            let mut light = source(i, slot);
            match (i >> (slot * 2)) & 3 {
                0 => None,
                1 => {
                    light.actor_present = false;
                    Some(light)
                }
                _ => Some(light),
            }
        });
        let host = Host {
            scene: Default::default(),
            lighting: Lighting {
                slots,
                ..Default::default()
            },
            object_to_world: object,
            world_to_camera: matrix(2000),
            projection: matrix(2001),
            camera_to_world: None,
            editor: true,
            engine_time: 0.,
        };
        let mut c = vec![Constant::default(); 96];
        let positions = if i % 2 == 0 { 4 } else { 0 };
        let inverse_slot = if i % 2 == 0 { 0 } else { 4 };
        c[inverse_slot].kind = 7;
        for (slot, kind) in [14, 17, 20, 23].into_iter().enumerate() {
            c[positions + slot].kind = kind;
        }
        c[8].kind = 12;
        c[9].kind = 12;
        let mut b = Bank::new(vec![[0x7fc12345, 0x80000000, 7, i as u32]; 96])?;
        let before = b.clone();
        let mut calls = vec![];
        let fail = i % 31 == 0;
        let error = b
            .update(
                &c,
                host,
                &mut Flicker::default(),
                &mut |m| {
                    calls.push(m);
                    if fail {
                        Err("fixture inverse failure".into())
                    } else {
                        Ok(inv)
                    }
                },
                &mut || panic!(),
            )
            .err();
        probes.push(serde_json::json!({"index":i,"object":object,"inverse":inv,"slots":slots,"inverse_slot":inverse_slot,"positions":positions,"fail":fail,"before":before,"after":b,"calls":calls,"error":error}));
        let light = source(i, 0);
        let point = shader_lights::position_point(light, object)?;
        let result = shader_lights::transform_position(point, inv, light.kind == 0x13)?;
        for word in std::iter::once(light.kind as u32)
            .chain(light.position)
            .chain(light.direction)
            .chain(object[3][..3].iter().copied())
            .chain(inv.into_iter().flatten())
        {
            data.extend_from_slice(&word.to_le_bytes());
        }
        scalar.push(
            serde_json::json!({"light":light,"point":point.map(f32::to_bits),"words":result}),
        );
    }
    fs::write(
        &args[1],
        serde_json::to_vec(
            &serde_json::json!({"probes":probes,"scalar":scalar,"scope":"Native light position cases 14/17/20/23 and shared ObjectToWorld inverse cache/case7. Synthetic matrices and raw inverse callback fixtures, not mathematical inverse claims; separate camera inverse cache. Sparse and callback failures preserve partial state. No live scene or Android claim."}),
        )?,
    )?;
    fs::write(&args[2], data)?;
    Ok(())
}
