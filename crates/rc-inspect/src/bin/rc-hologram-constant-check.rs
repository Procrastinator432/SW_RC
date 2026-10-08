use rc_inspect::assets::Assets;
use rc_package::{
    material_constants::Flicker,
    properties::Value,
    shader_constants::{Bank, Constant, Host, Matrix},
    shader_lights::{Light, Lighting},
    skeletal_matrix_inverse::inverse_matrix,
    vertex_shader,
};
use std::{env, fs, path::Path};
fn words(m: [[f32; 4]; 4]) -> Matrix {
    m.map(|r| r.map(f32::to_bits))
}
fn main() -> Result<(), Box<dyn std::error::Error>> {
    let a: Vec<_> = env::args_os().collect();
    if a.len() != 3 {
        return Err("usage: rc-hologram-constant-check GAME_DATA OUTPUT".into());
    }
    let game = Path::new(&a[1]);
    let dirs = [game.join("Textures"), game.join("System")];
    let mut assets = Assets::new(&dirs.iter().map(|p| p.as_path()).collect::<Vec<_>>())?;
    let name = "HardwareShaders.Hologram.DynamicHologram";
    let vs = assets.shader_constant_bindings(name, "VSConstants")?;
    let ps = assets.shader_constant_bindings(name, "PSConstants")?;
    let (_, props) = assets.material_properties(name)?;
    let source = props
        .values
        .iter()
        .find_map(|p| {
            if p.name == "VertexShaderText" {
                if let Value::String(s) = &p.value {
                    Some(s.clone())
                } else {
                    None
                }
            } else {
                None
            }
        })
        .ok_or("Missing vertex program")?;
    let program = vertex_shader::Program::parse(&source)?;
    let mut vc = vec![Constant::default(); 96];
    let mut pc = vec![Constant::default(); 8];
    for (input, output) in [(&vs, &mut vc), (&ps, &mut pc)] {
        for b in input {
            output[b.slot] = Constant {
                kind: b.kind,
                words: b.value.map(f32::to_bits),
            };
        }
    }
    let mut vb = Bank::new(vec![[0; 4]; 96])?;
    vb.words[17] = [0.25f32.to_bits(); 4];
    let mut pb = Bank::new(vec![[0; 4]; 8])?;
    let mut flicker = Flicker::default();
    let mut probes = vec![];
    let object = words([
        [2., 0., 0., 0.],
        [0., 1., 0., 0.],
        [0., 0., 0.5, 0.],
        [0.125, -0.25, 0.5, 1.],
    ]);
    let view = words([
        [1., 0., 0., 0.],
        [0., 1., 0., 0.],
        [0., 0., 1., 0.],
        [0., 0., 0., 1.],
    ]);
    let projection = words([
        [0., 0., 0.4, 0.3],
        [0.9, 0., 0., 0.],
        [0., 0.9, 0., 0.],
        [0., 0., 0.5, 1.],
    ]);
    let camera = words([
        [1., 0., 0., 0.],
        [0., 1., 0., 0.],
        [0., 0., 1., 0.],
        [4., 0., 0., 1.],
    ]);
    for i in 0..256usize {
        let slots = std::array::from_fn(|slot| {
            if slot == 3 {
                None
            } else {
                Some(Light {
                    radius: [0; 2],
                    actor_present: true,
                    kind: if i % 2 == 0 { 0x13 } else { 7 },
                    position: [2., 3., 4.].map(f32::to_bits),
                    direction: [0.125, 0.25, -0.5].map(f32::to_bits),
                    color: [0.25, 0.5, 0.75, 1.].map(f32::to_bits),
                    brightness: 0.75f32.to_bits(),
                    ..Default::default()
                })
            }
        });
        let host = Host {
            scene: Default::default(),
            lighting: Lighting {
                slots,
                ..Default::default()
            },
            object_to_world: object,
            world_to_camera: view,
            projection,
            camera_to_world: Some(camera),
            editor: false,
            engine_time: (i / 4) as f32 * 0.125,
        };
        let before = vb.clone();
        let pixel_before = pb.clone();
        let flicker_before = flicker.clone();
        let mut calls = vec![];
        let mut draws = vec![];
        let mut inverse = |m: Matrix| {
            calls.push(m);
            let raw = inverse_matrix(std::array::from_fn(|n| m[n / 4][n % 4]));
            Ok(std::array::from_fn(|r| {
                std::array::from_fn(|c| raw[r * 4 + c])
            }))
        };
        let mut random = || {
            let value = ((i * 73 + draws.len() * 127) % 32768) as u16;
            draws.push(value);
            Ok(value)
        };
        vb.update(&vc, host, &mut flicker, &mut inverse, &mut random)?;
        pb.update(&pc, host, &mut flicker, &mut inverse, &mut random)?;
        let mut vertices = [[0.; 4]; 16];
        vertices[0] = [
            (i % 8) as f32 * 0.0625 - 0.25,
            ((i / 8) % 8) as f32 * 0.0625 - 0.25,
            0.125,
            1.,
        ];
        vertices[1] = [1., 0., 0., 0.];
        vertices[2] = [0.25, 0.75, 0., 1.];
        let evaluation = program.evaluate(&vertex_shader::Inputs {
            vertices,
            constants: vb.words.iter().map(|r| r.map(f32::from_bits)).collect(),
        })?;
        probes.push(serde_json::json!({"index":i,"time":host.engine_time,"light_kind":if i%2==0{0x13}else{7},"before":before,"after":vb,"pixel_before":pixel_before,"pixel_after":pb,"inverse_calls":calls,"draws":draws,"flicker_before":flicker_before,"flicker_after":flicker,"vertices":vertices,"output_words":evaluation.output.map(|r|r.map(f32::to_bits)),"defined":evaluation.defined}));
    }
    fs::write(
        &a[2],
        serde_json::to_vec(
            &serde_json::json!({"shader":name,"bindings":vs,"pixel_bindings":ps,"source":source,"object":object,"view":view,"projection":projection,"camera":camera,"probes":probes,"scope":"All serialized DynamicHologram VS/PS bindings dispatched without register overrides, linked to original vertex program. Synthetic stable transforms/lights/time and host CRT15 sequence, c17 explicitly seeded to .25 and retained. Core inverse reuses existing reconstruction. No original runtime register-history, wrapper state, live actor, native GPU/render-state or Android equivalence claim."}),
        )?,
    )?;
    Ok(())
}
