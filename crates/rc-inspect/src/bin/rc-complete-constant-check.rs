//! All native constant kinds in mixed persistent-bank updates on supplied snapshots.
use rc_package::{
    material_constants::Flicker,
    shader_constants::{
        scene::{draw_scale, Scene},
        Bank, Constant, Host, Matrix,
    },
    shader_lights::{Light, Lighting},
    skeletal_matrix_inverse::inverse_matrix,
};
fn seed(q: f32) -> Result<f32, String> {
    #[cfg(target_arch = "x86_64")]
    {
        use std::arch::x86_64::{_mm_cvtss_f32, _mm_rsqrt_ss, _mm_set_ss};
        Ok(unsafe { _mm_cvtss_f32(_mm_rsqrt_ss(_mm_set_ss(q))) })
    }
    #[cfg(not(target_arch = "x86_64"))]
    {
        let _ = q;
        Err("Diagnostic requires x86_64 RSQRTSS".into())
    }
}
fn inverse(m: Matrix) -> Matrix {
    let flat = std::array::from_fn(|k| m[k / 4][k % 4]);
    let output = inverse_matrix(flat);
    std::array::from_fn(|r| std::array::from_fn(|c| output[r * 4 + c]))
}
fn main() -> Result<(), String> {
    let args: Vec<_> = std::env::args().collect();
    if args.len() != 3 {
        return Err("Expected JSON and rotator binary input paths".into());
    }
    let identity: Matrix =
        std::array::from_fn(|r| std::array::from_fn(|c| if r == c { 1f32.to_bits() } else { 0 }));
    let mut probes = Vec::new();
    let mut input = Vec::new();
    for i in 0..128usize {
        let object: Matrix = std::array::from_fn(|r| {
            std::array::from_fn(|c| {
                ((if r == c { 2. } else { 0. }) + (i + r * 7 + c * 3) as f32 * 0.001953125)
                    .to_bits()
            })
        });
        let mut camera = identity;
        camera[3] = [4., 5., 6., 1.].map(f32::to_bits);
        let actor = if i % 3 == 0 {
            Some([0x80000000, 0x7fc12345, i as u32])
        } else {
            None
        };
        let lighting = Lighting {
            slots: std::array::from_fn(|s| {
                Some(Light {
                    radius: [4f32.to_bits(), 2f32.to_bits()],
                    kind: if (i + s).is_multiple_of(2) { 19 } else { 7 },
                    actor_present: true,
                    position: [2., 3., 4.].map(f32::to_bits),
                    direction: [0.125, 0.25, -0.5].map(f32::to_bits),
                    cone: [37, 0, 255, 3][s],
                    color: [0.25, 0.5, 0.75, 1.].map(f32::to_bits),
                    brightness: 0.75f32.to_bits(),
                    flags: if i % 3 == 0 { [1, 0] } else { [0, 0] },
                })
            }),
            alpha_gate: actor.is_some() && i.is_multiple_of(2),
            ambient_bgra: 0x99ff8040,
        };
        let mut bank = Bank::new(vec![[0x7fc12345, 0x80000000, 7, i as u32]; 96])?;
        let mut flicker = Flicker::default();
        for pass in 0..2usize {
            let time = i as f32 * 0.125 + pass as f32 * 0.03125;
            let rate: f32 = [0., -0., 0.1, 2., -1., 33.3, 0.37, 64.][i % 8];
            let scene = Scene {
                actor_draw_scale: actor,
                fog: [2f32.to_bits(), 8f32.to_bits()],
                editor_eye: [1., 2., 3.].map(f32::to_bits),
                runtime_eye: Some([7., 8., 9.].map(f32::to_bits)),
                reciprocal_sqrt_seed: Some(seed),
            };
            let host = Host {
                scene,
                lighting,
                object_to_world: object,
                world_to_camera: identity,
                projection: identity,
                camera_to_world: Some(camera),
                editor: i.is_multiple_of(2),
                engine_time: time,
            };
            let mut bindings = vec![Constant::default(); 96];
            let mut slot = 0;
            let kinds: Vec<u8> = if pass == 0 {
                (0..35).collect()
            } else {
                (0..35).rev().collect()
            };
            for kind in kinds {
                let span = match kind {
                    2..=7 | 32 => 4,
                    34 => 2,
                    _ => 1,
                };
                bindings[slot] = Constant {
                    kind,
                    words: if kind == 27 {
                        [0.1, 0.4, f32::NAN, f32::INFINITY].map(f32::to_bits)
                    } else if kind == 1 {
                        [0x80000000, 0x7fc12345, i as u32, pass as u32]
                    } else {
                        [rate.to_bits(), 0x7fc12345, 7, 8]
                    },
                };
                for continuation in &mut bindings[slot + 1..slot + span] {
                    continuation.kind = 255;
                }
                slot += span;
            }
            bank.count = -2;
            let before = bank.clone();
            let flicker_before = flicker.clone();
            let mut calls = Vec::new();
            let mut draws = Vec::new();
            bank.update(
                &bindings,
                host,
                &mut flicker,
                &mut |m| {
                    calls.push(m);
                    Ok(inverse(m))
                },
                &mut || {
                    let n = ((i * 73 + draws.len() * 127) % 32768) as u16;
                    draws.push(n);
                    Ok(n)
                },
            )?;
            let q: Vec<u32> = (0..3)
                .map(|r| {
                    let v = [object[r][0], object[r][1], object[r][2]].map(f32::from_bits);
                    let v = v.map(|x| x * x);
                    (if r == 0 {
                        (v[1] + v[2]) + v[0]
                    } else {
                        (v[0] + v[1]) + v[2]
                    })
                    .to_bits()
                })
                .collect();
            let row = object[0].map(f32::from_bits);
            let light_q = (row[0] * row[0] + row[1] * row[1]) + row[2] * row[2];
            let seeds: Vec<u32> = q
                .iter()
                .map(|q| seed(f32::from_bits(*q)).unwrap().to_bits())
                .collect();
            let mut portable = scene;
            portable.reciprocal_sqrt_seed =
                Some(rc_package::shader_constants::scene::portable_seed);
            input.extend_from_slice(&time.to_le_bytes());
            input.extend_from_slice(&rate.to_le_bytes());
            probes.push(serde_json::json!({"index":i,"pass":pass,"time":time.to_bits(),"rate":rate.to_bits(),"object":object,"actor_scale":actor,"editor":host.editor,"lighting":lighting,"bindings":bindings,"before":before,"after":bank,"flicker_before":flicker_before,"flicker_after":flicker,"inverse_calls":calls,"rng":draws,"draw_squared":q,"draw_seeds":seeds,"light_squared":light_q.to_bits(),"light_seed":seed(light_q)?.to_bits(),"portable_draw":draw_scale(portable,object)?}));
        }
    }
    std::fs::write(&args[1],serde_json::to_vec(&serde_json::json!({"scope":"All kinds 0..34 on supplied snapshots, actual Core inverse reconstruction and host RSQRTSS seeds. Mixed forward/reverse layouts, persistent bank words and shared flicker. No original live scene or universal x87/Android parity claim.","probes":probes})).map_err(|e|e.to_string())?).map_err(|e|e.to_string())?;
    std::fs::write(&args[2], input).map_err(|e| e.to_string())
}
