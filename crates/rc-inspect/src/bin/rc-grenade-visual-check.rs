//! Prepare the original pickup mesh with base textures before scene integration.
use rc_package::{
    properties::{self, Value},
    read_package,
};
use std::{env, fs, path::Path};
fn main() -> Result<(), Box<dyn std::error::Error>> {
    let args: Vec<_> = env::args_os().collect();
    if args.len() != 3 {
        return Err("usage: rc-grenade-visual-check GAME_DATA OUTPUT_DIR".into());
    }
    let root = Path::new(&args[1]);
    let out = Path::new(&args[2]);
    fs::create_dir_all(out)?;
    let map = fs::read(root.join("Maps/geo_01a.ctm"))?;
    let pkg = read_package(&map)?;
    let e = &pkg.exports[609];
    let class = pkg.object_path(e.class)?;
    if class != "CTInventory.ThermalDetonatorBox" {
        return Err("Wrong original pickup class".into());
    }
    let p = properties::read(&pkg, &map, e)?;
    let catalog = rc_inspect::defaults::load(root)?;
    let value = |name| -> Result<Value, String> {
        Ok(catalog.instance(&class, &p, name, 0, "geo_01a.ctm")?.value)
    };
    let path = match value("StaticMesh")? {
        Value::Object { path, .. } => path,
        _ => return Err("Missing pickup mesh".into()),
    };
    let (package, object) = path.split_once('.').ok_or("Bad pickup mesh path")?;
    let data = fs::read(root.join("StaticMeshes").join(format!("{package}.usx")))?;
    let mesh_pkg = read_package(&data)?;
    let e = mesh_pkg
        .exports
        .iter()
        .enumerate()
        .find_map(|(i, e)| {
            mesh_pkg
                .object_path(i as i32 + 1)
                .is_ok_and(|s| s.eq_ignore_ascii_case(object))
                .then_some(e)
        })
        .ok_or("Missing pickup mesh export")?;
    let mesh = rc_package::mesh::read_static_mesh(&mesh_pkg, &data, e)?;
    let vector = |name| -> Result<[f32; 3], String> {
        match value(name)? {
            Value::Vector(v) => Ok(v),
            _ => Err("Invalid actor vector".into()),
        }
    };
    let location = vector("Location")?;
    let pivot = vector("PrePivot")?;
    let scale = match value("DrawScale")? {
        Value::Float(v) => v,
        _ => return Err("Bad pickup scale".into()),
    };
    let scale = vector("DrawScale3D")?.map(|v| v * scale);
    let rotation = match value("Rotation")? {
        Value::Rotator(v) => v,
        _ => return Err("Bad pickup rotation".into()),
    };
    let mut triangles = mesh.triangles()?;
    for t in &mut triangles {
        t.points = t
            .points
            .map(|p| rc_package::mesh::transform_point(p, location, rotation, scale, pivot));
    }
    let triangles = triangles
        .into_iter()
        .map(|t| rc_package::geometry::Triangle {
            points: t.points,
            surface: t.surface,
        })
        .collect();
    let mut scene = rc_render::Scene::new(triangles)?;
    let mut resolver = rc_inspect::assets::Assets::new(&[
        &root.join("Textures"),
        &root.join("StaticMeshes"),
        &root.join("System"),
    ])?;
    let mut materials = vec![];
    let mut scales = vec![];
    for (i, m) in mesh.materials.iter().enumerate() {
        let source = resolver.qualify(package, m.as_ref().ok_or("Missing pickup material")?);
        let mut d = resolver.diffuse_at_time(&source, 256, 0.)?;
        // This shader is opaque. Its diffuse texture's alpha carries material data,
        // not cutout coverage; passing that alpha through would erase most of the box.
        let (material_class, props) = resolver.material_properties(&source)?;
        let opacity = catalog.instance(&material_class, &props, "Opacity", 0, &source)?;
        let blend = catalog.instance(&material_class, &props, "OutputBlending", 0, &source)?;
        if material_class != "Engine.Shader"
            || !matches!(opacity.value, Value::Object { index: 0, .. })
            || !matches!(blend.value, Value::Byte(0))
        {
            return Err("Pickup shader requires another coverage adapter".into());
        }
        let original_nonopaque_texels =
            d.texture.pixels.iter().filter(|p| **p >> 24 != 255).count();
        for pixel in &mut d.texture.pixels {
            *pixel |= 0xff000000;
        }
        materials.push(serde_json::json!({"slot":i,"source":source,"diffuse":d.source,"chain":d.chain,"uv_channel":0,
            "opacity":opacity,"output_blending":blend,"opaque_coverage":true,
            "original_nonopaque_texels":original_nonopaque_texels}));
        scales.push(d.scale);
        scene.textures.push(d.texture);
    }
    let stream = mesh.uv_streams.first().ok_or("No pickup UV stream")?;
    let mut face = 0;
    for section in &mesh.sections {
        let start = section.first_index as usize;
        for indices in mesh.indices[start..start + section.face_count as usize * 3].chunks_exact(3)
        {
            let slot = scene.triangles[face].surface;
            let uv = std::array::from_fn(|i| stream[indices[i] as usize].map(|v| v * scales[slot]));
            scene
                .appearance
                .push(Some(rc_render::FaceTexture { texture: slot, uv }));
            face += 1;
        }
    }
    if scene.triangles.is_empty()
        || face != scene.triangles.len()
        || scene.appearance.iter().any(Option::is_none)
    {
        return Err("Untextured pickup faces".into());
    }
    let snapshot = scene.snapshot()?;
    let checked = rc_render::Scene::from_snapshot(&snapshot)?;
    if checked.triangles.len() != face {
        return Err("Pickup snapshot roundtrip failed".into());
    }
    fs::write(out.join("pickup.rcscene"), snapshot)?;
    for (name, yaw) in [("pickup-front", 0.6), ("pickup-back", 3.7)] {
        let pixels = scene.render(640, 400, yaw, 0.3, 1.)?;
        let bytes = pixels
            .iter()
            .flat_map(|p| p.to_le_bytes())
            .collect::<Vec<_>>();
        fs::write(out.join(format!("{name}.argb")), bytes)?;
    }
    fs::write(
        out.join("visual-validation.json"),
        serde_json::to_vec_pretty(&serde_json::json!({
        "passed":true,"original":610,"mesh":path,"location":location,"rotation":rotation,"scale":scale,
        "triangles":face,"textured_faces":scene.appearance.len(),"untextured_faces":0,"textures":scene.textures.len(),
        "snapshot_roundtrip":true,"materials":materials,"scope":"Original pickup mesh with resolved base diffuse/UV0 and actor transform only. Not attached to playable scene; Karma, native full shaders, pickup lifecycle and Android appearance remain open."}))?,
    )?;
    println!(
        "Original thermal pickup: {face} textured triangles, {} resolved base textures",
        scene.textures.len()
    );
    Ok(())
}
