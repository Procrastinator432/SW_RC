use rc_inspect::assets::Assets;
use rc_package::{
    pixel_shader::{Inputs, Program},
    properties::Value,
};
use rc_render::fragment;
use std::{env, fs, path::Path};
fn main() -> Result<(), Box<dyn std::error::Error>> {
    let args: Vec<_> = env::args_os().collect();
    if args.len() != 4 {
        return Err("usage: rc-pixel-check HOLOGRAM_REPORT GAME_DATA OUTPUT".into());
    }
    let old: serde_json::Value = serde_json::from_slice(&fs::read(&args[1])?)?;
    let game = Path::new(&args[2]);
    let dirs = [
        game.join("Textures"),
        game.join("StaticMeshes"),
        game.join("Animations"),
        game.join("System"),
    ];
    let mut assets = Assets::new(&dirs.iter().map(|d| d.as_path()).collect::<Vec<_>>())?;
    let name = "HardwareShaders.Hologram.DynamicHologram";
    let (_, p) = assets.material_properties(name)?;
    let source = p
        .values
        .iter()
        .find_map(|p| {
            if p.name == "PixelShaderText" {
                if let Value::String(s) = &p.value {
                    Some(s.clone())
                } else {
                    None
                }
            } else {
                None
            }
        })
        .ok_or("Missing pixel program")?;
    let program = Program::parse(&source)?;
    let constants = assets.pixel_constants(name)?;
    let mut probes = vec![];
    for index in 0..1024u32 {
        let mut input = Inputs {
            textures: [[0.; 4]; 4],
            constants,
            colors: [[0.; 4]; 2],
        };
        for stage in 0..3 {
            for axis in 0..4 {
                input.textures[stage][axis] =
                    ((index * 37 + stage as u32 * 53 + axis as u32 * 19) % 256) as f32 / 255.;
            }
        }
        input.colors[0] = [-0.25, ((index * 11) % 256) as f32 / 255., 1.25, 1.];
        input.constants[1] = [0.2, 0.4, 0.8, ((index * 7) % 256) as f32 / 255.];
        let evaluation = program.evaluate(&input)?;
        probes.push(serde_json::json!({"input":input,"evaluation":evaluation}));
    }
    let mut previews = vec![];
    let mut texture_cache = std::collections::BTreeMap::new();
    for h in old["holograms"].as_array().ok_or("holograms")? {
        let Some(base) = h["diffuse"].as_str() else {
            continue;
        };
        let paths = [
            "HardwareShaders.Gradients.GreyHoloFade",
            base,
            "HardwareShaders.Hologram.NoiseDigital",
        ];
        let mut textures = vec![];
        for path in paths {
            let d = assets.diffuse_at_size(path, 256)?;
            texture_cache.entry(path.to_owned()).or_insert_with(||serde_json::json!({"width":d.texture.width,"height":d.texture.height,"pixels":d.texture.pixels}));
            textures.push(d.texture);
        }
        let color = h["color"].as_u64().ok_or("color")? as u32;
        let mut c = constants;
        c[1] = [16, 8, 0]
            .map(|s| ((color >> s) & 255) as f32 * f32::from_bits(0x3b808081))
            .into_iter()
            .chain([1.])
            .collect::<Vec<_>>()
            .try_into()
            .unwrap();
        for phase in [0f32, 0.5] {
            let mut pixels = vec![];
            for y in 0..128 {
                for x in 0..128 {
                    let u = (x as f32 + 0.5) / 128.;
                    let v = (y as f32 + 0.5) / 128.;
                    let uv = [
                        [u, 0.5],
                        [u, v],
                        [u * 2. + phase * 0.17, v * 2. + phase * 0.07],
                    ];
                    pixels.push(fragment::shade(
                        &program,
                        [&textures[0], &textures[1], &textures[2]],
                        uv,
                        c,
                        [0.8; 4],
                        0xff101820,
                    )?);
                }
            }
            previews.push(serde_json::json!({"material":h["material"],"textures":paths,"phase":phase,"constants":c,"width":128,"height":128,"pixels":pixels}));
        }
    }
    let report = serde_json::json!({"shader":name,"source":source,"constants":constants,"instructions":program.instructions,"blocks":program.block_count(),"probes":probes,"textures":texture_cache,"previews":previews,"scope":"CPU f32 pixel shader subset; explicit fixture UV/color and additive composite; vertex shader, alpha test, fog, depth, legacy GPU precision and Android runtime unverified"});
    fs::write(&args[3], serde_json::to_vec(&report)?)?;
    Ok(())
}
