//! Captured-layout adapter -> original DynamicHologram bindings, against direct snapshots.
use rc_inspect::assets::Assets;
use rc_package::{
    material_constants::Flicker,
    properties::Value,
    shader_constants::{
        scene::{portable_seed, Scene},
        Bank, Constant, Host, Matrix,
    },
    shader_lights::Lighting,
    shader_snapshot::{self, Globals, Memory, Region},
    skeletal_matrix_inverse::inverse_matrix,
    vertex_shader,
};
use serde::Deserialize;
use std::{fs, path::Path};
#[derive(Deserialize)]
struct Expected {
    object: Matrix,
    view: Matrix,
    projection: Matrix,
    camera: Option<Matrix>,
    editor: bool,
    time: u32,
    actor_scale: Option<[u32; 3]>,
    editor_eye: [u32; 3],
    runtime_eye: Option<[u32; 3]>,
    fog: [u32; 2],
    lighting: Lighting,
}
impl Expected {
    fn host(&self) -> Host {
        Host {
            object_to_world: self.object,
            world_to_camera: self.view,
            projection: self.projection,
            camera_to_world: self.camera,
            editor: self.editor,
            engine_time: f32::from_bits(self.time),
            lighting: self.lighting,
            scene: Scene {
                actor_draw_scale: self.actor_scale,
                editor_eye: self.editor_eye,
                runtime_eye: self.runtime_eye,
                fog: self.fog,
                reciprocal_sqrt_seed: Some(portable_seed),
            },
        }
    }
}
fn describe(h: Host) -> serde_json::Value {
    serde_json::json!({"object":h.object_to_world,"view":h.world_to_camera,"projection":h.projection,"camera":h.camera_to_world,"editor":h.editor,"time":h.engine_time.to_bits(),"actor_scale":h.scene.actor_draw_scale,"editor_eye":h.scene.editor_eye,"runtime_eye":h.scene.runtime_eye,"fog":h.scene.fog,"lighting":h.lighting})
}
#[derive(Deserialize)]
struct Case {
    id: usize,
    renderer: u32,
    globals: Globals,
    regions: Vec<Region>,
    expected: Option<Expected>,
}
#[derive(Deserialize)]
struct Input {
    cases: Vec<Case>,
}
struct Pipeline {
    vertex: Bank,
    pixel: Bank,
    flicker: Flicker,
}
impl Pipeline {
    fn new() -> Result<Self, String> {
        let mut vertex = Bank::new(vec![[0; 4]; 96])?;
        vertex.words[17] = [0.25f32.to_bits(); 4];
        Ok(Self {
            vertex,
            pixel: Bank::new(vec![[0; 4]; 8])?,
            flicker: Flicker::default(),
        })
    }
    fn run(
        &mut self,
        host: Host,
        id: usize,
        vc: &[Constant],
        pc: &[Constant],
        program: &vertex_shader::Program,
    ) -> serde_json::Value {
        let before =
            serde_json::json!({"vertex":self.vertex,"pixel":self.pixel,"flicker":self.flicker});
        let mut calls = Vec::new();
        let mut draws = Vec::new();
        let mut inverse = |m: Matrix| {
            calls.push(m);
            let raw = inverse_matrix(std::array::from_fn(|k| m[k / 4][k % 4]));
            Ok(std::array::from_fn(|r| {
                std::array::from_fn(|c| raw[r * 4 + c])
            }))
        };
        let mut random = || {
            let n = ((id * 73 + draws.len() * 127) % 32768) as u16;
            draws.push(n);
            Ok(n)
        };
        let status = self
            .vertex
            .update(vc, host, &mut self.flicker, &mut inverse, &mut random)
            .and_then(|()| {
                self.pixel
                    .update(pc, host, &mut self.flicker, &mut inverse, &mut random)
            });
        let output = if status.is_ok() {
            let mut vertices = [[0.; 4]; 16];
            vertices[0] = [0., 0., 0.125, 1.];
            vertices[1] = [1., 0., 0., 0.];
            vertices[2] = [0.25, 0.75, 0., 1.];
            Some(program.evaluate(&vertex_shader::Inputs {vertices,constants:self.vertex.words.iter().map(|r|r.map(f32::from_bits)).collect()}).map(|e|serde_json::json!({"words":e.output.map(|r|r.map(f32::to_bits)),"defined":e.defined})))
        } else {
            None
        };
        serde_json::json!({"before":before,"status":status,"vertex":self.vertex,"pixel":self.pixel,"flicker":self.flicker,"inverse_calls":calls,"rng":draws,"vertex_output":output})
    }
}
fn main() -> Result<(), Box<dyn std::error::Error>> {
    let args: Vec<_> = std::env::args().collect();
    if args.len() != 4 {
        return Err("Expected GAME_DATA INPUT_JSON OUTPUT_JSON".into());
    }
    let game = Path::new(&args[1]);
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
                    Some(s.as_str())
                } else {
                    None
                }
            } else {
                None
            }
        })
        .ok_or("Missing program")?;
    let program = vertex_shader::Program::parse(source)?;
    let mut vc = vec![Constant::default(); 96];
    let mut pc = vec![Constant::default(); 8];
    for (bindings, bank) in [(&vs, &mut vc), (&ps, &mut pc)] {
        for b in bindings {
            bank[b.slot] = Constant {
                kind: b.kind,
                words: b.value.map(f32::to_bits),
            };
        }
    }
    let input: Input = serde_json::from_slice(&fs::read(&args[2])?)?;
    let mut native = Pipeline::new()?;
    let mut direct = Pipeline::new()?;
    let mut probes = Vec::new();
    for case in input.cases {
        let memory = Memory::new(case.regions)?;
        match shader_snapshot::capture(&memory, case.renderer, case.globals, Some(portable_seed)) {
            Err(error) => {
                if case.expected.is_some() {
                    return Err(format!("Unexpected capture failure {}: {error}", case.id).into());
                }
                probes.push(serde_json::json!({"id":case.id,"capture_error":error}));
            }
            Ok((host, addresses)) => {
                let expected = case
                    .expected
                    .ok_or("Malformed fixture unexpectedly captured")?
                    .host();
                if describe(host) != describe(expected) {
                    return Err(format!("Captured host differs at {}", case.id).into());
                }
                let output = native.run(host, case.id, &vc, &pc, &program);
                let reference = direct.run(expected, case.id, &vc, &pc, &program);
                if output != reference {
                    return Err(format!("Snapshot/direct pipeline mismatch {}", case.id).into());
                }
                probes.push(serde_json::json!({"id":case.id,"host":describe(host),"addresses":addresses,"pipeline":output,"direct_snapshot_equal":true}));
            }
        }
    }
    fs::write(
        &args[3],
        serde_json::to_vec(
            &serde_json::json!({"scope":"Synthetic relocated x86 memory images -> original material bindings -> vertex program. Persistent banks/shared flicker; portable seed explicitly supplied, c17 explicitly seeded .25. No live capture, renderer or Android execution.","vs":vs,"ps":ps,"probes":probes}),
        )?,
    )?;
    Ok(())
}
