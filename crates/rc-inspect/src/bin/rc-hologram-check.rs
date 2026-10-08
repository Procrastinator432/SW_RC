use rc_inspect::assets::Assets;
use rc_package::{
    material_hologram::{
        constant_color, setup_hologram, stored_color, HologramShader, HologramWrapper,
    },
    properties::{Properties, Value},
};
use std::{env, fs, path::Path};
fn reference(
    assets: &Assets,
    p: &Properties,
    package: &str,
    name: &str,
    slot: u32,
) -> Option<String> {
    p.values.iter().find_map(|p| {
        if p.name == name && p.array_index == slot {
            if let Value::Object { index, path } = &p.value {
                (*index != 0).then(|| assets.qualify(package, path))
            } else {
                None
            }
        } else {
            None
        }
    })
}
fn main() -> Result<(), Box<dyn std::error::Error>> {
    let args: Vec<_> = env::args_os().collect();
    if args.len() != 4 {
        return Err("usage: rc-hologram-check MATERIAL_REPORT GAME_DATA OUTPUT".into());
    }
    let old: serde_json::Value = serde_json::from_slice(&fs::read(&args[1])?)?;
    let game = Path::new(&args[2]);
    let dirs = [
        game.join("Textures"),
        game.join("StaticMeshes"),
        game.join("Animations"),
        game.join("System"),
    ];
    let mut assets = Assets::new(&dirs.iter().map(|p| p.as_path()).collect::<Vec<_>>())?;
    let defaults = assets.class_properties("Engine.HsHologram")?;
    let color_defaults = assets.class_properties("Engine.ConstantColor")?;
    let mut paths = std::collections::BTreeSet::new();
    let (mut holo_slots, mut color_slots) = (0, 0);
    for o in old["objects"].as_array().ok_or("objects")? {
        for b in o["bindings"].as_array().ok_or("bindings")? {
            if b["error"] == "Unsupported material class Engine.HsHologram" {
                holo_slots += 1;
                paths.insert(b["material"].as_str().ok_or("material")?.to_owned());
            }
            if b["error"] == "Unsupported material class Engine.ConstantColor" {
                color_slots += 1;
                paths.insert(b["material"].as_str().ok_or("material")?.to_owned());
            }
        }
    }
    let mut holograms = vec![];
    let mut colors = vec![];
    let mut shaders = std::collections::BTreeMap::new();
    for path in paths {
        let (class, p) = assets.material_properties(&path)?;
        let package = path.split_once('.').ok_or("package")?.0;
        if class == "Engine.ConstantColor" {
            let color = stored_color(&color_defaults, &p, "Color")?;
            colors.push(serde_json::json!({"material":path,"stored_color":color,"samples":[constant_color(color,-100.),constant_color(color,0.),constant_color(color,100.)]}));
            continue;
        }
        if class != "Engine.HsHologram" {
            return Err("unexpected material class".into());
        }
        let shader = reference(&assets, &p, package, "ShaderImplementation", 0)
            .or_else(|| reference(&assets, &defaults, "Engine", "ShaderImplementation", 0))
            .ok_or("missing hologram shader")?;
        if !shaders.contains_key(&shader) {
            let (class, sp) = assets.material_properties(&shader)?;
            if class != "Engine.HardwareShader" {
                return Err("unexpected shader class".into());
            }
            let mut programs = std::collections::BTreeMap::new();
            let mut textures = vec![];
            for property in &sp.values {
                if let Value::String(text) = &property.value {
                    if property.name.ends_with("ShaderText") {
                        programs.insert(property.name.clone(), text.clone());
                    }
                }
                if property.name == "Textures" {
                    textures.push(serde_json::json!({"slot":property.array_index,"material":reference(&assets,&sp,shader.split_once('.').unwrap().0,"Textures",property.array_index)}));
                }
            }
            shaders.insert(
                shader.clone(),
                serde_json::json!({"programs":programs,"textures":textures,"properties":sp}),
            );
        }
        let diffuse = reference(&assets, &p, package, "DiffuseTexture", 0);
        let fallback = reference(&assets, &p, package, "FallbackMaterial", 0);
        let color = stored_color(&defaults, &p, "HologramColor")?;
        let marker = p
            .values
            .iter()
            .find_map(|p| {
                if p.name == "UseMarkerColorInstead" {
                    if let Value::Bool(b) = p.value {
                        Some(b)
                    } else {
                        None
                    }
                } else {
                    None
                }
            })
            .unwrap_or(false);
        let mut w = HologramWrapper {
            diffuse: if diffuse.is_some() { 1 } else { 0 },
            color,
            use_marker_color: marker,
            fallback: 99,
        };
        let mut s = HologramShader {
            color_words: [0x7fc01234, 0x80000000, 0x3e800000, 0],
            texture0: 2,
            texture1: 3,
        };
        let before = s;
        let mut observed = None;
        let result = setup_hologram(&mut w, Some(&mut s), &mut |s, a, b| {
            observed = Some(serde_json::json!({"shader":s,"arguments":[a,b]}));
            1
        });
        holograms.push(serde_json::json!({"material":path,"shader_implementation":shader,"diffuse":diffuse,"serialized_fallback":fallback,"color":color,"use_marker_color":marker,"host_fixture_before":before,"callback":observed,"host_fixture_after":s,"wrapper_after":w,"result":result}));
    }
    let mut probes = vec![];
    for i in 0u32..1024 {
        let mut w = HologramWrapper {
            diffuse: if i % 2 == 0 { 0 } else { 0x100 + i },
            color: i.wrapping_mul(0x315719),
            use_marker_color: i % 4 >= 2,
            fallback: 0x12345678,
        };
        let before = w;
        let mut s = HologramShader {
            color_words: [
                0x7fc00000 | i,
                0x80000000,
                i.wrapping_mul(0x10203),
                0x3f800000,
            ],
            texture0: 0x2000 + i,
            texture1: 0x3000 + i,
        };
        let original = s;
        let mut observed = None;
        let result = setup_hologram(&mut w, Some(&mut s), &mut |s, a, b| {
            observed = Some(serde_json::json!({"shader":s,"arguments":[a,b]}));
            s.color_words = [i; 4];
            s.texture0 = 0x4000 + i;
            s.texture1 = 0x5000 + i;
            (i % 3) as i32 - 1
        });
        probes.push(serde_json::json!({"before":before,"shader_before":original,"callback":observed,"after":w,"shader_after":s,"result":result}));
    }
    let mut texture_paths = std::collections::BTreeSet::new();
    for h in &holograms {
        if let Some(path) = h["diffuse"].as_str() {
            texture_paths.insert(path.to_owned());
        }
    }
    for s in shaders.values() {
        for t in s["textures"].as_array().ok_or("shader textures")? {
            if let Some(path) = t["material"].as_str() {
                texture_paths.insert(path.to_owned());
            }
        }
    }
    let mut textures = std::collections::BTreeMap::new();
    for path in texture_paths {
        let d = assets.diffuse_at_size(&path, 256)?;
        textures.insert(path,serde_json::json!({"source":d.source,"chain":d.chain,"width":d.texture.width,"height":d.texture.height,"pixels":d.texture.pixels,"format":d.format,"source_mip":d.source_mip}));
    }
    fs::write(
        &args[3],
        serde_json::to_vec(
            &serde_json::json!({"hologram_slots":holo_slots,"constant_color_slots":color_slots,"holograms":holograms,"colors":colors,"shaders":shaders,"textures":textures,"probes":probes,"scope":"Native HsHologram wrapper host contract and ConstantColor values; original hardware shader programs/texture bindings and decoded input textures inventoried. Wrapper resource handles/color state in tests are explicit host fixtures. Hardware shaders are not executed; existing diffuse resolver continues to omit holograms and constant colors."}),
        )?,
    )?;
    println!(
        "{holo_slots} hologram slots, {color_slots} constant-color slots, {} wrapper probes",
        probes.len()
    );
    Ok(())
}
