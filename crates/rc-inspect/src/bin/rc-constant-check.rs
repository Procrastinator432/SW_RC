use rc_inspect::assets::Assets;
use rc_package::material_constants::{cos_time, shader_time, Flicker};
use std::{env, fs, path::Path};
fn main() -> Result<(), Box<dyn std::error::Error>> {
    let args: Vec<_> = env::args_os().collect();
    if args.len() != 3 {
        return Err("usage: rc-constant-check GAME_DATA OUTPUT".into());
    }
    let game = Path::new(&args[1]);
    let dirs = [game.join("Textures"), game.join("System")];
    let mut assets = Assets::new(&dirs.iter().map(|p| p.as_path()).collect::<Vec<_>>())?;
    let bindings = assets
        .shader_constant_bindings("HardwareShaders.Hologram.DynamicHologram", "VSConstants")?;
    let mut time_probes = vec![];
    let mut x87_input = vec![];
    let special = [
        0.,
        -0.,
        119.999,
        120.,
        -120.,
        240.5,
        -121.25,
        f32::MAX,
        -f32::MAX,
        65536.125,
    ];
    for i in 0..1024 {
        let time = if i < special.len() {
            special[i]
        } else {
            (i as f32 - 512.25) * 0.37
        };
        let rate = [0., 1., -1., 0.1, 2., 32., 64.][i % 7];
        let wrapped = shader_time(time)?;
        let cosine = cos_time(time, rate)?;
        x87_input.extend_from_slice(&time.to_bits().to_le_bytes());
        x87_input.extend_from_slice(&rate.to_bits().to_le_bytes());
        time_probes.push(serde_json::json!({"time":time,"rate":rate,"time_words":wrapped.map(f32::to_bits),"cos_words":cosine.map(f32::to_bits)}));
    }
    let mut state = Flicker::default();
    let mut flicker_probes = vec![];
    let flicker: Vec<_> = bindings.iter().filter(|b| b.kind == 27).collect();
    for i in 0..1024 {
        let time = ((i / 4) % 8) as f32 * 0.125;
        let binding = flicker[i % flicker.len()];
        let before = state.clone();
        let mut draws = vec![];
        let result = state.evaluate(time, binding.value, &mut || {
            let value = ((i * 73 + draws.len() * 127) % 32768) as u16;
            draws.push(value);
            Ok(value)
        })?;
        flicker_probes.push(serde_json::json!({"time":time,"slot":binding.slot,"plane":binding.value,"before":before,"after":state,"draws":draws,"words":result.map(f32::to_bits)}));
    }
    let output = Path::new(&args[2]);
    fs::write(output.with_extension("input.bin"), x87_input)?;
    let report = serde_json::json!({"bindings":bindings,"time_probes":time_probes,"flicker_probes":flicker_probes,"scope":"Native signed 120s time remainder and shared finite-input CRT15 flicker state; portable bounded CosTime compared with x87 probes, not universal legacy transcendental equivalence. Host owns engine time and the global random sequence; matrices, lights, buffer persistence and Android runtime remain external."});
    fs::write(output, serde_json::to_vec(&report)?)?;
    Ok(())
}
