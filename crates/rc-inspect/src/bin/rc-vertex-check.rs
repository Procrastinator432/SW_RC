use rc_inspect::assets::Assets;
use rc_package::{
    pixel_shader,
    properties::Value,
    vertex_shader::{Inputs, Program},
};
use std::{env, fs, path::Path};
fn main() -> Result<(), Box<dyn std::error::Error>> {
    let args: Vec<_> = env::args_os().collect();
    if args.len() != 3 {
        return Err("usage: rc-vertex-check GAME_DATA OUTPUT".into());
    }
    let game = Path::new(&args[1]);
    let dirs = [
        game.join("Textures"),
        game.join("StaticMeshes"),
        game.join("Animations"),
        game.join("System"),
    ];
    let mut assets = Assets::new(&dirs.iter().map(|p| p.as_path()).collect::<Vec<_>>())?;
    let name = "HardwareShaders.Hologram.DynamicHologram";
    let (_, properties) = assets.material_properties(name)?;
    let text = |name: &str| -> Result<String, String> {
        properties
            .values
            .iter()
            .find_map(|p| {
                if p.name == name {
                    if let Value::String(s) = &p.value {
                        Some(s.clone())
                    } else {
                        None
                    }
                } else {
                    None
                }
            })
            .ok_or_else(|| format!("Missing {name}"))
    };
    let source = text("VertexShaderText")?;
    let program = Program::parse(&source)?;
    let pixel = pixel_shader::Program::parse(&text("PixelShaderText")?)?;
    let bindings = assets.shader_constant_bindings(name, "VSConstants")?;
    let pc = assets.pixel_constants(name)?;
    let texture_paths = [
        "HardwareShaders.Gradients.GreyHoloFade",
        "CloneTextures.CloneTextures.CloneCommandoSmall",
        "HardwareShaders.Hologram.NoiseDigital",
    ];
    let textures = texture_paths
        .iter()
        .map(|p| assets.diffuse_at_size(p, 256).map(|d| d.texture))
        .collect::<Result<Vec<_>, _>>()?;
    let mut probes = vec![];
    for index in 0..96 {
        let phase = (index / 32) as f32 * 0.5;
        let mut c = vec![[0.; 4]; 96];
        for b in &bindings {
            if b.kind == 1 {
                c[b.slot] = b.value;
            }
        }
        for (start, count) in [(0, 4), (5, 3), (11, 4)] {
            for i in 0..count {
                c[start + i][i] = 1.;
            }
        }
        // Explicit diagnostic host state, not reconstructed engine constant generation.
        c[10] = [0., 0., 4., 1.];
        c[16] = [2.25, 0.7, 0.2, 0.];
        c[17] = [phase; 4];
        c[18] = [-0.25, -0.5, 0., 0.];
        c[21] = [1.; 4];
        c[22] = [0.9 + phase * 0.05, 0.01, 0.02, 0.];
        c[25] = [0.3 + phase * 0.1, 0.2, 0.1, 0.];
        let mut vertices = [[0.; 4]; 16];
        vertices[0] = [
            ((index % 8) as f32 - 3.5) * 0.2,
            ((index / 8 % 4) as f32 - 1.5) * 0.2,
            [-0.75, -0.4, 0., 0.75][index % 4],
            1.,
        ];
        vertices[1] = [0., 0., 1., 0.];
        vertices[2] = [(index % 8) as f32 / 8., (index / 8 % 4) as f32 / 4., 0., 1.];
        let input = Inputs {
            vertices,
            constants: c,
        };
        let evaluation = program.evaluate(&input)?;
        let pixel = rc_render::fragment::shade_vertex_output(
            &evaluation,
            &pixel,
            [&textures[0], &textures[1], &textures[2]],
            pc,
            0xff101820,
        )?;
        probes.push(serde_json::json!({"index":index,"phase":phase,"input":input,"evaluation":evaluation,"pixel":pixel}));
    }
    let textures:Vec<_>=textures.into_iter().zip(texture_paths).map(|(t,p)|serde_json::json!({"path":p,"width":t.width,"height":t.height,"pixels":t.pixels})).collect();
    let report = serde_json::json!({"shader":name,"source":source,"instructions":program.instruction_count(),"bindings":bindings,"pixel_constants":pc,"textures":textures,"probes":probes,"scope":"CPU f32 vertex+pixel point probes; dynamic host constants and scan thresholds are explicit fixtures; no triangle interpolation, full render-state, original GPU precision or Android runtime verification"});
    fs::write(&args[2], serde_json::to_vec(&report)?)?;
    Ok(())
}
