use rc_inspect::assets::Assets;
use rc_package::{pixel_shader, properties::Value, vertex_shader};
use rc_render::{
    fragment,
    shader_raster::{Blend, State, Target, Vertex},
};
use std::{env, fs, path::Path};
fn main() -> Result<(), Box<dyn std::error::Error>> {
    let args: Vec<_> = env::args_os().collect();
    if args.len() != 4 {
        return Err("usage: rc-hologram-draw-check SKELETAL_DRAW_BIN GAME_DATA OUTPUT".into());
    }
    let bytes = fs::read(&args[1])?;
    if !bytes.starts_with(b"RCSKDRAW\x01\0\0\0") || (bytes.len() - 12) % 132 != 0 {
        return Err("Invalid skeletal draw stream".into());
    }
    let mut source = vec![];
    for record in bytes[12..].chunks_exact(132) {
        let word = |i: usize| u32::from_le_bytes(record[i * 4..i * 4 + 4].try_into().unwrap());
        if word(0) == 25 && word(1) == 0 {
            source.push(std::array::from_fn::<_, 24, _>(|i| word(i + 9)));
        }
    }
    if source.len() != 3500 {
        return Err("Expected CloneCommando LOD0 draw fixture".into());
    }
    let mut min = [f32::INFINITY; 3];
    let mut max = [f32::NEG_INFINITY; 3];
    for triangle in &source {
        for v in triangle.chunks_exact(8) {
            for i in 0..3 {
                let p = f32::from_bits(v[i]);
                if !p.is_finite() {
                    return Err("Nonfinite original mesh position".into());
                }
                min[i] = min[i].min(p);
                max[i] = max[i].max(p);
            }
        }
    }
    let center = std::array::from_fn::<_, 3, _>(|i| (min[i] + max[i]) * 0.5);
    let radius = (0..3).map(|i| (max[i] - min[i]) * 0.5).fold(0f32, f32::max);
    if radius <= 0. {
        return Err("Empty original mesh extent".into());
    }
    let game = Path::new(&args[2]);
    let dirs = [
        game.join("Textures"),
        game.join("StaticMeshes"),
        game.join("Animations"),
        game.join("System"),
    ];
    let mut assets = Assets::new(&dirs.iter().map(|d| d.as_path()).collect::<Vec<_>>())?;
    let name = "HardwareShaders.Hologram.DynamicHologram";
    let (_, properties) = assets.material_properties(name)?;
    let text = |name: &str| {
        properties
            .values
            .iter()
            .find_map(|p| {
                if p.name == name {
                    if let Value::String(s) = &p.value {
                        Some(s.as_str())
                    } else {
                        None
                    }
                } else {
                    None
                }
            })
            .ok_or("Missing shader program")
    };
    let vertex_program = vertex_shader::Program::parse(text("VertexShaderText")?)?;
    let pixel_program = pixel_shader::Program::parse(text("PixelShaderText")?)?;
    let bindings = assets.shader_constant_bindings(name, "VSConstants")?;
    let pc = assets.pixel_constants(name)?;
    let paths = [
        "HardwareShaders.Gradients.GreyHoloFade",
        "CloneTextures.CloneTextures.CloneCommandoSmall",
        "HardwareShaders.Hologram.NoiseDigital",
    ];
    let textures = paths
        .iter()
        .map(|p| assets.diffuse_at_size(p, 256).map(|d| d.texture))
        .collect::<Result<Vec<_>, _>>()?;
    let mut frames = vec![];
    for phase in [0f32, 0.5] {
        let mut constants = vec![[0.; 4]; 96];
        for b in &bindings {
            if b.kind == 1 {
                constants[b.slot] = b.value;
            }
        }
        // Explicit diagnostic camera, matrices, time/distortion and flicker fixtures.
        constants[0] = [0., 0.9, 0., 0.];
        constants[1] = [0., 0., 0.9, 0.];
        constants[2] = [0.4, 0., 0., 0.5];
        constants[3] = [0.3, 0., 0., 1.];
        for start in [5, 11] {
            for i in 0..4 {
                constants[start + i][i] = 1.;
            }
        }
        constants[10] = [4., 0., 0., 1.];
        constants[16] = [2.25, 0.7, 0.2, 0.];
        constants[17] = [phase; 4];
        constants[21] = [1.; 4];
        constants[22] = [0.9 + phase * 0.05, 0.01, 0.02, 0.];
        constants[25] = [0.3 + phase * 0.1, 0.2, 0.1, 0.];
        let mut target = Target::new(128, 128, 0xff101820)?;
        let state = State {
            depth_test: true,
            depth_write: false,
            alpha_reference: Some(0),
            blend: Blend::SourceAlphaAdditive,
        };
        let mut projected = vec![];
        let mut stats = [0usize; 4];
        for triangle in &source {
            let mut vertices = vec![];
            for v in triangle.chunks_exact(8) {
                let mut inputs = [[0.; 4]; 16];
                inputs[0] = [
                    (f32::from_bits(v[0]) - center[0]) / radius,
                    (f32::from_bits(v[1]) - center[1]) / radius,
                    (f32::from_bits(v[2]) - center[2]) / radius,
                    1.,
                ];
                inputs[1] = [
                    f32::from_bits(v[3]),
                    f32::from_bits(v[4]),
                    f32::from_bits(v[5]),
                    0.,
                ];
                inputs[2] = [f32::from_bits(v[6]), f32::from_bits(v[7]), 0., 1.];
                vertices.push(Vertex::from_output(&vertex_program.evaluate(
                    &vertex_shader::Inputs {
                        vertices: inputs,
                        constants: constants.clone(),
                    },
                )?)?);
            }
            let tri: [Vertex; 3] = vertices.try_into().unwrap();
            projected.push(tri.map(|v| serde_json::json!({"clip":v.clip,"varying":v.varying})));
            let s = target.draw(tri, state, &mut |sample| {
                let v = sample.varying;
                let uv = [[v[0], 0.], [v[1], v[2]], [v[3], v[4]]];
                let mut t = [[0.; 4]; 4];
                for i in 0..3 {
                    t[i] = fragment::sample(&textures[i], uv[i])?;
                }
                Ok(pixel_program
                    .evaluate(&pixel_shader::Inputs {
                        textures: t,
                        constants: pc,
                        colors: [[v[5], v[6], v[7], v[8]], [0.; 4]],
                    })?
                    .color)
            })?;
            for (total, n) in
                stats
                    .iter_mut()
                    .zip([s.covered, s.depth_rejected, s.alpha_rejected, s.written])
            {
                *total += n;
            }
        }
        frames.push(serde_json::json!({"phase":phase,"constants":constants,"projected":projected,"width":128,"height":128,"pixels":target.pixels(),"depth_words":target.depth().iter().map(|v|v.to_bits()).collect::<Vec<_>>(),"stats":stats}));
    }
    let report = serde_json::json!({"shader":name,"source_words":source,"center":center,"radius":radius,"frames":frames,"pixel_constants":pc,"texture_paths":paths,"scope":"Original reconstructed CloneCommando LOD0 mesh and shader programs; diagnostic normalized coordinates, host constants, half-pixel raster, float-alpha greater-than-zero test, less-equal depth and source-alpha additive blend. No native render-state/driver equivalence, actor integration or Android verification."});
    fs::write(&args[3], serde_json::to_vec(&report)?)?;
    Ok(())
}
