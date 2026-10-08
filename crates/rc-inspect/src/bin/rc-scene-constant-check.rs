//! Diagnostic supplied snapshots; no original engine execution.
use rc_package::{
    material_constants::Flicker,
    shader_constants::{
        scene::{self, Scene},
        Bank, Constant, Host, Matrix,
    },
    shader_lights::{Light, Lighting},
};
fn native_seed(q: f32) -> Result<f32, String> {
    #[cfg(target_arch = "x86_64")]
    {
        use std::arch::x86_64::{_mm_cvtss_f32, _mm_rsqrt_ss, _mm_set_ss};
        // Diagnostic host CPU seed only; no game DLL is loaded or executed.
        Ok(unsafe { _mm_cvtss_f32(_mm_rsqrt_ss(_mm_set_ss(q))) })
    }
    #[cfg(not(target_arch = "x86_64"))]
    {
        let _ = q;
        Err("This diagnostic requires an x86_64 RSQRTSS host".into())
    }
}
fn main() -> Result<(), String> {
    let output = std::env::args().nth(1).ok_or("Expected output JSON path")?;
    let mut probes = Vec::new();
    for i in 0..1024usize {
        let object: Matrix = std::array::from_fn(|r| {
            std::array::from_fn(|c| {
                ((if r == c { 2.0 } else { 0.0 }) + (i + r * 7 + c * 3) as f32 * 0.001953125)
                    .to_bits()
            })
        });
        let inverse: Matrix = std::array::from_fn(|r| {
            std::array::from_fn(|c| {
                if c == 3 {
                    0x7fc12345
                } else {
                    (((i + r * 11 + c * 5) % 31) as f32 * 0.125 - 1.).to_bits()
                }
            })
        });
        let eye = [
            (i as f32 * 0.125).to_bits(),
            (-2.5f32).to_bits(),
            3.25f32.to_bits(),
        ];
        let editor_eye = [eye[2], eye[0], eye[1]];
        let fog = [
            (i as f32 * 0.25).to_bits(),
            ((i as f32 * 0.25) + if i.is_multiple_of(2) { 4.0 } else { -2.0 }).to_bits(),
        ];
        let slots = std::array::from_fn(|s| {
            Some(Light {
                actor_present: true,
                kind: if (i + s).is_multiple_of(2) { 19 } else { 7 },
                radius: [
                    ((i + s) as f32 * 0.125 + 4.).to_bits(),
                    (s as f32 * 0.25).to_bits(),
                ],
                ..Default::default()
            })
        });
        let host = Host {
            scene: Scene {
                actor_draw_scale: None,
                fog,
                editor_eye,
                runtime_eye: Some(eye),
                reciprocal_sqrt_seed: Some(native_seed),
            },
            lighting: Lighting {
                slots,
                ..Default::default()
            },
            object_to_world: object,
            world_to_camera: object,
            projection: object,
            camera_to_world: None,
            editor: i.is_multiple_of(2),
            engine_time: 0.0,
        };
        let mut bindings = [Constant::default(); 8];
        for (slot, kind) in [16, 19, 22, 25, 33, 31, 33].into_iter().enumerate() {
            bindings[slot].kind = kind;
        }
        let mut bank = Bank::new(vec![[0x7fc12345, 0x80000000, 7, i as u32]; 8])?;
        let mut calls = Vec::new();
        bank.update(
            &bindings,
            host,
            &mut Flicker::default(),
            &mut |m| {
                calls.push(m);
                Ok(inverse)
            },
            &mut || Err("Unexpected RNG".into()),
        )?;
        let row = object[0].map(f32::from_bits);
        let squared = (row[0] * row[0] + row[1] * row[1]) + row[2] * row[2];
        let portable = rc_package::shader_lights::inverse_radius(
            slots[if host.editor { 1 } else { 0 }],
            object,
            Some(scene::portable_seed),
        )?;
        probes.push(serde_json::json!({"index":i,"object":object,"inverse":inverse,"editor":host.editor,"editor_eye":editor_eye,"runtime_eye":eye,"fog":fog,"lights":slots,"squared":squared.to_bits(),"seed":native_seed(squared)?.to_bits(),"bank":bank,"inverse_calls":calls,"portable_radius":portable}));
    }
    std::fs::write(output, serde_json::to_vec(&serde_json::json!({"scope":"Supplied scene snapshots; native host RSQRTSS seeds, no live engine or Android execution. Portable radius result reported separately without native parity claim.","probes":probes})).map_err(|e| e.to_string())?).map_err(|e| e.to_string())
}
