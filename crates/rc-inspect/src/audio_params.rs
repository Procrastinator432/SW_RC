//! Direct USound::GetParams resolution for the selected PlayUSound leaf path.
use rc_package::{audio_params::Params, defaults::Catalog, properties::Value};
use std::path::Path;
pub fn load(
    root: &Path,
    catalog: &Catalog,
    source: &str,
) -> Result<(Params, serde_json::Value), Box<dyn std::error::Error>> {
    let mut resolver = crate::assets::Assets::new(&[&root.join("Sounds"), &root.join("System")])?;
    let (class, props) = resolver.material_properties(source)?;
    let pointer = catalog.instance(&class, &props, "Params", 0, source)?;
    let selected = match &pointer.value {
        Value::Object { index: 0, .. } => None,
        Value::Object { index, path } => Some(if *index > 0 && pointer.stage == "instance_delta" {
            format!(
                "{}.{}",
                source.split_once('.').ok_or("Invalid sound path")?.0,
                path
            )
        } else {
            path.clone()
        }),
        _ => return Err("Invalid sound Params reference".into()),
    };
    let object = selected
        .as_ref()
        .map(|s| resolver.material_properties(s))
        .transpose()?;
    let mut receipt = serde_json::Map::new();
    let mut get = |name: &str| -> Result<Value, String> {
        let r = if let Some((class, props)) = &object {
            catalog.instance(class, props, name, 0, selected.as_ref().unwrap())?
        } else {
            catalog.resolve("Engine.AudioParams", name, 0)?
        };
        receipt.insert(
            name.into(),
            serde_json::to_value(&r).map_err(|e| e.to_string())?,
        );
        Ok(r.value)
    };
    let mut floats = vec![];
    for name in ["MinVolume", "MaxVolume", "MinPitch", "MaxPitch", "Radius"] {
        let Value::Float(v) = get(name)? else {
            return Err("Invalid audio float".into());
        };
        floats.push(v);
    }
    let Value::Bool(non_positional) = get("NonPositional")? else {
        return Err("Invalid positioning flag".into());
    };
    let Value::Bool(disable_falloff) = get("bDisable3DFalloff")? else {
        return Err("Invalid falloff flag".into());
    };
    let Value::Bool(disable_occlusion) = get("bDisable3DOcclusion")? else {
        return Err("Invalid occlusion flag".into());
    };
    let Value::Byte(category) = get("Category")? else {
        return Err("Invalid audio category".into());
    };
    let p = Params {
        source: selected
            .clone()
            .unwrap_or("Engine.AudioParams default".into()),
        min_volume: floats[0],
        max_volume: floats[1],
        min_pitch: floats[2],
        max_pitch: floats[3],
        radius: floats[4],
        non_positional,
        disable_falloff,
        disable_occlusion,
        category,
    };
    p.validate()?;
    Ok((
        p,
        serde_json::json!({"sound":source,"params_reference":pointer,"selected":selected,"properties":receipt,"policy":"PlayUSound selects leaf USound::GetParams; null leaf Params uses AudioParams default"}),
    ))
}
