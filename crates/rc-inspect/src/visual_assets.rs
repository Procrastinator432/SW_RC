//! Build a local RCPLAY02 bundle from original assets and the existing playable baseline.
use crate::assets;
use rc_game::visual::{Clip, Material, Mesh};
use rc_package::{
    read_package, skeletal_animation::read_mesh_animation, skeletal_lod::read_skeletal_lods,
    skeletal_mesh::read_skeletal_mesh_prefix,
};
use std::{fs, path::Path};
fn opacity_inputs(
    game: &Path,
    path: &str,
) -> Result<(bool, serde_json::Value), Box<dyn std::error::Error>> {
    let (package, object) = path.split_once('.').ok_or("Invalid material path")?;
    let data = fs::read(game.join("Textures").join(format!("{package}.utx")))?;
    let pkg = read_package(&data)?;
    let e = pkg
        .exports
        .iter()
        .enumerate()
        .find_map(|(i, e)| {
            (pkg.object_path(i as i32 + 1)
                .ok()?
                .eq_ignore_ascii_case(object))
            .then_some(e)
        })
        .ok_or("Missing material")?;
    let props = rc_package::properties::read(&pkg, &data, e)?;
    let class = pkg.object_path(e.class)?;
    let reference = |name: &str| -> Option<String> {
        props.values.iter().find_map(|p| {
            if p.name == name {
                if let rc_package::properties::Value::Object { index, .. } = &p.value {
                    if *index == 0 {
                        return None;
                    }
                    let path = pkg.object_path(*index).ok()?;
                    Some(if *index > 0 {
                        format!("{package}.{path}")
                    } else {
                        path
                    })
                } else {
                    None
                }
            } else {
                None
            }
        })
    };
    if class == "Engine.TexPanner2D" {
        return opacity_inputs(game, &reference("Material").ok_or("Panner lacks material")?);
    }
    let opacity = reference("Opacity");
    let relevant: Vec<_> = props
        .values
        .iter()
        .filter(|p| {
            [
                "Diffuse",
                "Opacity",
                "OutputBlending",
                "bTwoSided",
                "bAlphaTest",
                "bMasked",
                "bUseAlpha",
            ]
            .contains(&p.name.as_str())
        })
        .collect();
    if opacity.is_some() && opacity != reference("Diffuse") {
        return Err("Separate opacity texture requires an explicit material adapter".into());
    }
    Ok((
        class == "Engine.Shader" && opacity.is_none(),
        serde_json::json!({"material":path,"class":class,"properties":relevant,"opacity":opacity}),
    ))
}
pub fn load(
    game: &Path,
    file: &str,
    object: &str,
    clips: &[&str],
    assets: &mut assets::Assets,
) -> Result<Mesh, Box<dyn std::error::Error>> {
    load_from_animation(game, file, object, clips, assets, None)
}
/// Select one original animation linkup when multiple sets contain the same clip name.
pub fn load_from_animation(
    game: &Path,
    file: &str,
    object: &str,
    clips: &[&str],
    assets: &mut assets::Assets,
    animation_filter: Option<&str>,
) -> Result<Mesh, Box<dyn std::error::Error>> {
    let data = fs::read(game.join("Animations").join(file))?;
    let pkg = read_package(&data)?;
    let e = pkg
        .exports
        .iter()
        .enumerate()
        .find_map(|(i, e)| {
            (pkg.object_path(i as i32 + 1)
                .ok()?
                .eq_ignore_ascii_case(object))
            .then_some(e)
        })
        .ok_or("Missing mesh")?;
    let prefix = read_skeletal_mesh_prefix(&pkg, &data, e)?;
    let lod = read_skeletal_lods(&pkg, &data, e)?
        .lods
        .into_iter()
        .next()
        .ok_or("Missing mesh LOD")?;
    let stem = Path::new(file).file_stem().unwrap().to_string_lossy();
    let qualify = |index: i32| -> Result<String, String> {
        let path = pkg.object_path(index)?;
        Ok(if index > 0 {
            format!("{stem}.{path}")
        } else {
            path
        })
    };
    let mut materials = vec![];
    for &index in &prefix.materials {
        let path = qualify(index)?;
        // Preserve the existing rejection of separate opacity samplers;
        // only the shared resolver decides whether pixels are opaque.
        opacity_inputs(game, &path)?;
        let diffuse = assets.diffuse_at_time(&path, 256, 0.)?;
        let opacity = serde_json::json!({"material": path, "original_opaque": diffuse.opacity});
        materials.push(Material {
            source: path,
            chain: diffuse.chain,
            opacity,
            width: diffuse.texture.width,
            height: diffuse.texture.height,
            pixels: diffuse.texture.pixels,
            uv_scale: diffuse.scale,
            uv_transform: diffuse.uv_transform,
        });
    }
    let mut selected = vec![];
    let mut available = vec![];
    for link in &prefix.linkups {
        let path = qualify(link.animation_index)?;
        if animation_filter.is_some_and(|set| !path.eq_ignore_ascii_case(set)) {
            continue;
        }
        let (file, object) = path.split_once('.').ok_or("Invalid animation path")?;
        let data = fs::read(game.join("Animations").join(format!("{file}.ukx")))?;
        let p = read_package(&data)?;
        let e = p
            .exports
            .iter()
            .enumerate()
            .find_map(|(i, e)| {
                (p.object_path(i as i32 + 1)
                    .ok()?
                    .eq_ignore_ascii_case(object))
                .then_some(e)
            })
            .ok_or("Missing animation")?;
        let animation = read_mesh_animation(&p, &data, e)?;
        let mapping: Vec<_> = prefix
            .bones
            .iter()
            .map(|b| {
                animation
                    .reference_bones
                    .iter()
                    .position(|r| r.name.name.eq_ignore_ascii_case(&b.name.name))
                    .map_or(-1, |i| i as i32)
            })
            .collect();
        if mapping.iter().any(|&i| i < 0) {
            return Err(format!("Unmapped bone in {path}").into());
        }
        for sequence in animation.sequences {
            available.push(sequence.name.name.clone());
            if clips.contains(&sequence.name.name.as_str())
                || (clips.is_empty() && selected.is_empty())
            {
                selected.push(Clip {
                    source: format!("{path}:{}", sequence.name.name),
                    mapping: mapping.clone(),
                    sequence,
                });
            }
        }
    }
    if selected.len() != clips.len().max(1) {
        return Err(format!("Missing selected clips in {object}; available: {available:?}").into());
    }
    Ok(Mesh {
        source: format!("{stem}.{object}"),
        scale: prefix.mesh_scale,
        origin: prefix.mesh_origin,
        rotation: prefix.mesh_rotation,
        bones: prefix.bones,
        lod,
        materials,
        clips: selected,
    })
}
