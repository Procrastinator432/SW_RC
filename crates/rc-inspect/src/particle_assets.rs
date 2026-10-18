//! Selected SpriteEmitter defaults, with explicit field-wise tagged struct overlays.
//! This does not change the generic catalog's compound-default rejection.
use rc_package::{
    properties::{self, Properties, Value},
    Package,
};
use serde_json::{Map, Value as Json};
use std::{fs, path::Path};
fn decode(pkg: &Package, data: &[u8], props: &Properties) -> Result<Json, String> {
    let mut result = Map::new();
    for p in &props.values {
        let bytes = data
            .get(p.payload_offset..p.payload_offset + p.bytes)
            .ok_or("Particle property bounds")?;
        let value = match &p.value {
            Value::Byte(n) => Json::from(*n),
            Value::Int(n) => Json::from(*n),
            Value::Float(n) => Json::from(*n),
            Value::Bool(b) => Json::from(*b),
            Value::Object { path, .. } => Json::from(path.clone()),
            Value::Name(s) | Value::String(s) => Json::from(s.clone()),
            Value::Vector(v) => serde_json::to_value(v).map_err(|e| e.to_string())?,
            Value::Rotator(v) => serde_json::to_value(v).map_err(|e| e.to_string())?,
            Value::Raw { .. } if p.struct_name.as_deref() == Some("Color") && bytes.len() == 4 => {
                serde_json::json!([
                    f32::from(bytes[2]) / 255.,
                    f32::from(bytes[1]) / 255.,
                    f32::from(bytes[0]) / 255.,
                    f32::from(bytes[3]) / 255.
                ])
            }
            Value::Raw { .. } if p.kind == 10 => {
                decode(pkg, bytes, &properties::tagged_struct(pkg, bytes)?)?
            }
            Value::Raw { .. } if p.kind == 9 => Json::Array(
                properties::struct_array(pkg, bytes)?
                    .iter()
                    .map(|props| decode(pkg, bytes, props))
                    .collect::<Result<_, _>>()?,
            ),
            _ => return Err(format!("Unsupported particle property {}", p.name)),
        };
        if p.array_index != 0 {
            return Err("Unexpected static particle property slot".into());
        }
        result.insert(p.name.clone(), value);
    }
    Ok(Json::Object(result))
}
fn overlay(base: &mut Json, delta: Json) {
    if let (Some(base), Some(delta)) = (base.as_object_mut(), delta.as_object()) {
        for (key, value) in delta {
            if value.is_object() && base.get(key).is_some_and(Json::is_object) {
                overlay(base.get_mut(key).unwrap(), value.clone());
            } else {
                base.insert(key.clone(), value.clone());
            }
        }
    }
}
pub fn bacta(
    root: &Path,
    resolver: &mut crate::assets::Assets,
) -> Result<Vec<rc_game::particles::Template>, String> {
    let result = load(
        root,
        resolver,
        &[
            ("BactaZap_Dispenser", false),
            ("BactaZap_Dispenser_Body", true),
        ],
    )?;
    if result.len() != 14 {
        return Err("Original Bacta component count changed".into());
    }
    Ok(result)
}
pub fn effect(
    root: &Path,
    resolver: &mut crate::assets::Assets,
    class: &str,
) -> Result<Vec<rc_game::particles::Template>, String> {
    let name = class
        .strip_prefix("CTEffects.")
        .ok_or("Expected original CTEffects class")?;
    load(root, resolver, &[(name, false)])
}
fn load(
    root: &Path,
    resolver: &mut crate::assets::Assets,
    classes: &[(&str, bool)],
) -> Result<Vec<rc_game::particles::Template>, String> {
    let engine_bytes = fs::read(root.join("System/engine.u")).map_err(|e| e.to_string())?;
    let engine = rc_package::read_package(&engine_bytes)?;
    let mut base = serde_json::json!({});
    for name in ["ParticleEmitter", "SpriteEmitter"] {
        let e = engine
            .exports
            .iter()
            .find(|e| e.class == 0 && e.name.eq_ignore_ascii_case(name))
            .ok_or("Particle base class")?;
        let props = rc_package::classes::read(&engine, &engine_bytes, e)?.properties;
        overlay(
            &mut base,
            decode(&engine, engine.payload(&engine_bytes, e)?, &props)?,
        );
    }
    let data = fs::read(root.join("Properties/cteffects.u")).map_err(|e| e.to_string())?;
    let pkg = rc_package::read_package(&data)?;
    let mut result = vec![];
    for &(name, body) in classes {
        let e = pkg
            .exports
            .iter()
            .find(|e| e.class == 0 && e.name == name)
            .ok_or("Bacta effect class")?;
        let props = rc_package::classes::read(&pkg, &data, e)?.properties;
        let field = props
            .values
            .iter()
            .find(|p| p.name == "Emitters")
            .ok_or("Bacta emitter array")?;
        let payload = pkg.payload(&data, e)?;
        for (index, _) in properties::object_array(
            &pkg,
            &payload[field.payload_offset..field.payload_offset + field.bytes],
        )? {
            let e = pkg
                .exports
                .get(index as usize - 1)
                .ok_or("Emitter template index")?;
            if pkg.object_path(e.class)? != "Engine.SpriteEmitter" {
                return Err(format!(
                    "Unsupported original emitter {} in {name}",
                    pkg.object_path(e.class)?
                ));
            }
            let props = properties::read(&pkg, &data, e)?;
            let mut parameters = base.clone();
            overlay(
                &mut parameters,
                decode(&pkg, pkg.payload(&data, e)?, &props)?,
            );
            let source = parameters["Texture"]
                .as_str()
                .ok_or("Particle texture")?
                .to_string();
            let diffuse = resolver.diffuse_at_size(&source, 256)?;
            let template = rc_game::particles::Template {
                original: index as u32,
                body,
                parameters,
                material: rc_game::visual::Material {
                    source,
                    chain: diffuse.chain,
                    opacity: serde_json::json!({"particle":"Original particle texture alpha preserved; PTDS3/6 explicit draw policy"}),
                    width: diffuse.texture.width,
                    height: diffuse.texture.height,
                    pixels: diffuse.texture.pixels,
                    uv_scale: diffuse.scale,
                    uv_transform: diffuse.uv_transform,
                },
            };
            template.validate().map_err(|e|format!("Emitter {index}: {e}; max={}, style={}, direction={}, coordinates={}, lifetime={}",template.parameters["MaxParticles"],template.parameters["DrawStyle"],template.parameters["UseDirectionAs"],template.parameters["CoordinateSystem"],template.parameters["LifetimeRange"]))?;
            result.push(template);
        }
    }
    Ok(result)
}

#[cfg(test)]
mod tests {
    #[test]
    fn tagged_overlays_keep_parent_axes_and_replace_arrays() {
        let mut base = serde_json::json!({"range":{"Min":1,"Max":4},"vector":{"X":{"Min":1},"Y":{"Max":5}},"curve":[1,2]});
        super::overlay(
            &mut base,
            serde_json::json!({"range":{"Min":2},"vector":{"X":{"Max":3}},"curve":[9]}),
        );
        assert_eq!(
            base,
            serde_json::json!({"range":{"Min":2,"Max":4},"vector":{"X":{"Min":1,"Max":3},"Y":{"Max":5}},"curve":[9]})
        );
    }
}
