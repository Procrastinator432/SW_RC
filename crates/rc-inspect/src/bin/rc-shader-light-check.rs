use rc_package::{
    material_constants::Flicker,
    shader_constants::{Bank, Constant, Host},
    shader_lights::{self, Light, LightCache, Lighting},
};
use std::{env, fs};
fn light(i: usize, slot: usize) -> Light {
    let color = std::array::from_fn(|c| {
        if c == 3 {
            0x7fc12345
        } else if (i + slot + c).is_multiple_of(17) {
            0x80000000
        } else {
            (((i * 7919 + slot * 127 + c * 31) as u32) & 0x807fffff)
                | ((118 + ((i + slot + c) % 13) as u32) << 23)
                | ((((i + slot + c) % 2) as u32) << 31)
        }
    });
    Light {
        radius: [0; 2],
        kind: 0,
        position: [0; 3],
        actor_present: true,
        cone: 1 + ((i + slot * 37) % 255) as u8,
        color,
        brightness: ((i % 23) as f32 * 0.125 - 1.).to_bits(),
        direction: [0x7fc10000 + i as u32, 0x80000000, (slot * 19 + i) as u32],
        flags: match i / 256 % 4 {
            0 => [0, 0],
            1 => [1, 0],
            2 => [0, 0x80000000],
            _ => [7, 9],
        },
    }
}
fn identity() -> [[u32; 4]; 4] {
    std::array::from_fn(|r| std::array::from_fn(|c| if r == c { 1f32.to_bits() } else { 0 }))
}
fn main() -> Result<(), Box<dyn std::error::Error>> {
    let args: Vec<_> = env::args_os().collect();
    if args.len() != 3 {
        return Err("usage: rc-shader-light-check JSON SSE_INPUT".into());
    }
    let mut probes = vec![];
    let mut scalar = vec![];
    let mut bytes = vec![];
    for i in 0..1024usize {
        let slots = std::array::from_fn(|slot| {
            let mode = (i >> (slot * 2)) & 3;
            let mut l = light(i, slot);
            match mode {
                0 => None,
                1 => {
                    l.actor_present = false;
                    Some(l)
                }
                2 => {
                    l.cone = 0;
                    Some(l)
                }
                _ => Some(l),
            }
        });
        let lighting = Lighting {
            slots,
            alpha_gate: i % 2 == 0,
            ambient_bgra: (i as u32).wrapping_mul(0x1020304),
        };
        let cache = LightCache::new(lighting);
        let selection = (0..4)
            .map(|slot| cache.select(lighting, slot))
            .collect::<Vec<_>>();
        let mut host = Host {
            scene: Default::default(),
            lighting,
            object_to_world: identity(),
            world_to_camera: identity(),
            projection: identity(),
            camera_to_world: None,
            editor: false,
            engine_time: 0.,
        };
        let mut dispatches = vec![];
        for kind in [15, 18, 21, 24, 26, 28, 29] {
            let mut b = Bank::new(vec![[0x7fc12345, 0x80000000, 7, i as u32]; 8])?;
            let before = b.clone();
            let mut c = [Constant::default(); 8];
            c[0] = Constant {
                kind: 1,
                words: [i as u32; 4],
            };
            c[1].kind = kind;
            let result = b.update(
                &c,
                host,
                &mut Flicker::default(),
                &mut |_| panic!(),
                &mut || panic!(),
            );
            dispatches.push(
                serde_json::json!({"kind":kind,"before":before,"after":b,"error":result.err()}),
            );
        }
        let mut bank = Bank::new(vec![[99; 4]; 8])?;
        let mut c = [Constant::default(); 8];
        for (slot, kind) in [15, 18, 21, 24, 26, 28, 29].into_iter().enumerate() {
            c[slot].kind = kind;
        }
        let first = bank
            .update(
                &c,
                host,
                &mut Flicker::default(),
                &mut |_| panic!(),
                &mut || panic!(),
            )
            .err();
        let after_first = bank.clone();
        host.lighting.slots = [None; 4];
        let second = bank
            .update(
                &c,
                host,
                &mut Flicker::default(),
                &mut |_| panic!(),
                &mut || panic!(),
            )
            .err();
        probes.push(serde_json::json!({"index":i,"lighting":lighting,"cache":cache,"selection":selection,"dispatches":dispatches,"combined":{"first_error":first,"after_first":after_first,"second_error":second,"after_second":bank}}));
        // Separate finite scalar fixtures exercise every cone byte and RGB/brightness pair.
        let mut l = light(i, 0);
        l.cone = (i % 256) as u8;
        let bgra = lighting.ambient_bgra;
        let color = shader_lights::color(Some(l), lighting.alpha_gate)?;
        let ambient = shader_lights::ambient(bgra);
        let cone = shader_lights::spotlight_cone(Some(l));
        for word in [
            l.color[0],
            l.color[1],
            l.color[2],
            l.brightness,
            bgra,
            l.cone as u32,
        ] {
            bytes.extend_from_slice(&word.to_le_bytes());
        }
        scalar.push(serde_json::json!({"light":l,"alpha_gate":lighting.alpha_gate,"bgra":bgra,"color":color,"ambient":ambient,"cone":cone}));
    }
    let mut errors = vec![];
    for mode in 0..4 {
        let mut l = light(3, 0);
        match mode {
            0 => l.color[0] = 0x7fc12345,
            1 => l.brightness = f32::INFINITY.to_bits(),
            2 => {
                l.color[0] = f32::MAX.to_bits();
                l.brightness = 0f32.to_bits();
            }
            _ => {
                l.color[0] = (f32::MAX * 0.25).to_bits();
                l.brightness = 8f32.to_bits();
            }
        }
        let mut h = Host {
            scene: Default::default(),
            lighting: Lighting {
                slots: [Some(l); 4],
                ..Default::default()
            },
            object_to_world: identity(),
            world_to_camera: identity(),
            projection: identity(),
            camera_to_world: None,
            editor: false,
            engine_time: 0.,
        };
        let mut b = Bank::new(vec![[7; 4]; 8])?;
        let mut c = [Constant::default(); 8];
        c[0].kind = 26;
        c[1].kind = 15;
        h.lighting.ambient_bgra = 0x99ff0080;
        let error = b
            .update(
                &c,
                h,
                &mut Flicker::default(),
                &mut |_| panic!(),
                &mut || panic!(),
            )
            .err();
        assert!(error.is_some());
        errors.push(serde_json::json!({"mode":mode,"light":l,"after":b,"error":error}));
    }
    let report = serde_json::json!({"probes":probes,"scalar":scalar,"errors":errors,"scope":"Native sparse four-source light count/index table, four light colors, ambient RGB and spotlight direction/cone constants. Stable synthetic host fields; no light positions/radius, live scene acquisition or complete DynamicHologram dispatch. Scalar precision checked with separate offline SSE at nearest rounding and gradual underflow. Sparse -1 dereference and nonfinite color arithmetic are explicit safe host errors. Android stays last."});
    fs::write(&args[1], serde_json::to_vec(&report)?)?;
    fs::write(&args[2], bytes)?;
    Ok(())
}
