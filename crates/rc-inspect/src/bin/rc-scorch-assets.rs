use rc_package::{properties::Value, read_package};
use std::{env, fs, path::Path};
fn main() -> Result<(), Box<dyn std::error::Error>> {
    let args: Vec<_> = env::args_os().collect();
    let root = Path::new(&args[1]);
    let out = Path::new(&args[2]);
    fs::create_dir_all(out)?;
    let catalog = rc_inspect::defaults::load(root)?;
    let mut receipts = vec![];
    for name in [
        "Mesh",
        "GroundSpeed",
        "CollisionHeight",
        "CollisionRadius",
        "Skins",
        "DrawScale",
        "CrouchHeight",
        "CrouchRadius",
        "CrouchSpeedRatio",
    ] {
        for index in 0..if name == "Skins" { 4 } else { 1 } {
            let v = catalog.instance(
                "CTCharacters.Commando62Delta",
                &rc_package::properties::Properties {
                    values: vec![],
                    native_offset: 0,
                },
                name,
                index,
                "factory457",
            );
            receipts.push(serde_json::json!({"property":name,"index":index,"resolved":v}));
        }
    }
    let data = fs::read(root.join("System/ctcharacters.u"))?;
    let pkg = read_package(&data)?;
    for (i, e) in pkg.exports.iter().enumerate() {
        if e.class == 0
            && ["Commando62Delta", "CloneCommando"].iter().any(|n| {
                pkg.object_path(i as i32 + 1)
                    .is_ok_and(|p| p.eq_ignore_ascii_case(n))
            })
        {
            let props = rc_package::classes::read(&pkg, &data, e)?.properties;
            let payload = pkg.payload(&data, e)?;
            fs::write(
                out.join(format!(
                    "{}-properties.json",
                    pkg.object_path(i as i32 + 1)?
                )),
                serde_json::to_vec_pretty(&props)?,
            )?;
            let skin=props.values.iter().filter(|p|p.name=="Skins" || p.name=="MeshSets").map(|p|serde_json::json!({"property":p,"objects":rc_package::properties::object_array(&pkg,&payload[p.payload_offset..p.payload_offset+p.bytes]),"structs":rc_package::properties::struct_array(&pkg,&payload[p.payload_offset..p.payload_offset+p.bytes])})).collect::<Vec<_>>();
            fs::write(
                out.join(format!("{}-skins.json", pkg.object_path(i as i32 + 1)?)),
                serde_json::to_vec_pretty(&skin)?,
            )?;
        }
    }
    let mut scorch_skins = vec![];
    let data = fs::read(root.join("Properties/properties.u"))?;
    let pkg = read_package(&data)?;
    for (i, e) in pkg.exports.iter().enumerate() {
        if e.class == 0
            && pkg
                .object_path(i as i32 + 1)?
                .eq_ignore_ascii_case("Commando62DeltaDefaults")
        {
            let props = rc_package::classes::read(&pkg, &data, e)?.properties;
            let payload = pkg.payload(&data, e)?;
            for p in &props.values {
                if p.name == "Skins" {
                    scorch_skins = rc_package::properties::object_array(
                        &pkg,
                        &payload[p.payload_offset..p.payload_offset + p.bytes],
                    )?
                    .into_iter()
                    .map(|(_, path)| path)
                    .collect();
                }
            }
            let nested=props.values.iter().filter(|p|matches!(p.value,Value::Raw{..})).map(|p|serde_json::json!({"name":p.name,"objects":rc_package::properties::object_array(&pkg,&payload[p.payload_offset..p.payload_offset+p.bytes])})).collect::<Vec<_>>();
            fs::write(
                out.join("scorch-template.json"),
                serde_json::to_vec_pretty(
                    &serde_json::json!({"properties":props,"nested":nested}),
                )?,
            )?;
        }
    }
    let mut assets = rc_inspect::assets::Assets::new(&[
        &root.join("Textures"),
        &root.join("Animations"),
        &root.join("System"),
    ])?;
    let data = fs::read(root.join("Animations/Clone.ukx"))?;
    let pkg = read_package(&data)?;
    for (i, e) in pkg.exports.iter().enumerate() {
        if pkg
            .object_path(i as i32 + 1)?
            .eq_ignore_ascii_case("CloneCommandoSPSet")
        {
            let props = rc_package::properties::read(&pkg, &data, e)?;
            let payload = pkg.payload(&data, e)?;
            let nested=props.values.iter().filter(|p|matches!(p.value,Value::Raw{..})).map(|p|serde_json::json!({"name":p.name,"structs":rc_package::properties::struct_array(&pkg,&payload[p.payload_offset..p.payload_offset+p.bytes]),"objects":rc_package::properties::object_array(&pkg,&payload[p.payload_offset..p.payload_offset+p.bytes])})).collect::<Vec<_>>();
            fs::write(
                out.join("scorch-mesh-set.json"),
                serde_json::to_vec_pretty(
                    &serde_json::json!({"class":pkg.object_path(e.class)?,"properties":props,"nested":nested}),
                )?,
            )?;
        }
    }
    let (class, props) =
        assets.material_properties("CloneTextures.CloneTextures.CloneCommandoProxy")?;
    fs::write(
        out.join("scorch-proxy.json"),
        serde_json::to_vec_pretty(&serde_json::json!({"class":class,"properties":props}))?,
    )?;
    let mut mesh = rc_inspect::visual_assets::load(
        root,
        "Clone.ukx",
        "CloneCommando",
        &["ActionBreathe", "RunForward", "GestureSignalOK"],
        &mut assets,
    )?;
    if scorch_skins.len() != mesh.materials.len() {
        return Err("Incomplete original Scorch template skin".into());
    }
    for (mat, path) in mesh.materials.iter_mut().zip(scorch_skins) {
        let mut d = assets.diffuse_at_time(&path, 256, 0.)?;
        let (class, props) = assets.material_properties(&path)?;
        if class == "Engine.Shader"
            && matches!(
                catalog.instance(&class, &props, "Opacity", 0, &path)?.value,
                Value::Object { index: 0, .. }
            )
        {
            for p in &mut d.texture.pixels {
                *p |= 0xff000000;
            }
        }
        mat.source = path;
        mat.chain = d.chain;
        mat.width = d.texture.width;
        mat.height = d.texture.height;
        mat.pixels = d.texture.pixels;
        mat.uv_scale = d.scale;
        mat.uv_transform = d.uv_transform;
    }
    fs::write(out.join("scorch-mesh.json"), serde_json::to_vec(&mesh)?)?;
    fs::write(
        out.join("scorch-defaults.json"),
        serde_json::to_vec_pretty(&receipts)?,
    )?;
    let data = fs::read(root.join("Sounds/geo_sound.uax"))?;
    let pkg = read_package(&data)?;
    for (i, e) in pkg.exports.iter().enumerate() {
        if pkg
            .object_path(i as i32 + 1)?
            .eq_ignore_ascii_case("GEO_01.ambPS_GEO_rockImpact")
        {
            let props = rc_package::properties::read(&pkg, &data, e)?;
            fs::write(
                out.join("rock-impact-source.json"),
                serde_json::to_vec_pretty(
                    &serde_json::json!({"class":pkg.object_path(e.class)?,"properties":props}),
                )?,
            )?;
        }
    }
    Ok(())
}
