//! Supplied time/frequency fixtures through persistent shader constant banks.
use rc_package::{
    material_constants::Flicker,
    shader_constants::{Bank, Constant, Host},
};
use std::fs;
fn main() -> Result<(), String> {
    let args: Vec<_> = std::env::args().collect();
    if args.len() != 3 {
        return Err("Expected JSON and binary input paths".into());
    }
    let mut probes = Vec::new();
    let mut input = Vec::new();
    let specials = [
        0.,
        -0.,
        119.999f32,
        120.,
        -120.,
        240.5,
        -121.25,
        f32::MAX,
        -f32::MAX,
        65536.125,
        std::f32::consts::FRAC_PI_2,
        -std::f32::consts::FRAC_PI_2,
    ];
    for i in 0..2048usize {
        let time = if i < specials.len() {
            specials[i]
        } else if i < 1024 {
            (i as f32 - 512.25) * 0.37
        } else {
            std::f32::consts::FRAC_PI_2 * ((i % 39) as f32 - 19.)
                + ((i % 17) as f32 - 8.) * 0.000001
        };
        let rate: f32 = [0., -0., 1., -1., 0.1, 2., 32., 64.][i % 8];
        input.extend_from_slice(&time.to_le_bytes());
        input.extend_from_slice(&rate.to_le_bytes());
        let len = if i.is_multiple_of(2) { 8 } else { 96 };
        let mut bank = Bank::new(vec![[0x7fc12345, 0x80000000, 7, i as u32]; len])?;
        let before = bank.clone();
        let mut bindings = vec![Constant::default(); len];
        for (slot, kind) in [(1, 10), (2, 11), (3, 13)] {
            bindings[slot] = Constant {
                kind,
                words: if kind == 13 {
                    [0x7fc12345; 4]
                } else {
                    [rate.to_bits(), 0x7fc12345, 0xff800000, 7]
                },
            };
        }
        bindings[4] = Constant {
            kind: 1,
            words: [0x80000000, 0x7fc12345, i as u32, 9],
        };
        let host = Host {
            scene: Default::default(),
            lighting: Default::default(),
            object_to_world: [[0; 4]; 4],
            world_to_camera: [[0; 4]; 4],
            projection: [[0; 4]; 4],
            camera_to_world: None,
            editor: false,
            engine_time: time,
        };
        bank.update(
            &bindings,
            host,
            &mut Flicker::default(),
            &mut |_| Err("Unexpected inverse".into()),
            &mut || Err("Unexpected RNG".into()),
        )?;
        probes.push(serde_json::json!({"index":i,"time":time.to_bits(),"frequency":rate.to_bits(),"before":before,"after":bank}));
    }
    fs::write(&args[1],serde_json::to_vec(&serde_json::json!({"scope":"Bounded portable SinTime/TanTime and fixed-radius XYCircle on supplied time snapshots. No live engine, universal x87 precision or Android parity claim.","probes":probes})).map_err(|e| e.to_string())?).map_err(|e| e.to_string())?;
    fs::write(&args[2], input).map_err(|e| e.to_string())
}
