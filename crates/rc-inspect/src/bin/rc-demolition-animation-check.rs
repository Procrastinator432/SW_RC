//! Load/skin the original charge clips with Scorch's verified template materials.
use std::{env, fs, path::Path};
fn main() -> Result<(), Box<dyn std::error::Error>> {
    let args: Vec<_> = env::args_os().collect();
    if args.len() != 4 {
        return Err("usage: rc-demolition-animation-check GAME_DATA SCORCH_MESH OUTPUT".into());
    }
    let root = Path::new(&args[1]);
    let out = Path::new(&args[3]);
    fs::create_dir_all(out)?;
    let mut scorch: rc_game::visual::Mesh = serde_json::from_slice(&fs::read(&args[2])?)?;
    let mut resolver = rc_inspect::assets::Assets::new(&[
        &root.join("Textures"),
        &root.join("Animations"),
        &root.join("System"),
    ])?;
    let names = [
        "DemolitionSetChargeStart",
        "DemolitionSetChargeLoop",
        "DemolitionSetChargeStop",
    ];
    let mut loaded =
        rc_inspect::visual_assets::load(root, "Clone.ukx", "CloneCommando", &names, &mut resolver)?;
    if serde_json::to_vec(&loaded.bones)? != serde_json::to_vec(&scorch.bones)? {
        return Err("Charge clips changed Scorch hierarchy".into());
    }
    assert!(names
        .iter()
        .all(|name| loaded.clips.iter().any(|c| c.sequence.name.name == *name)));
    scorch.clips.append(&mut loaded.clips);
    fs::write(
        out.join("scorch-demolition-mesh.json"),
        serde_json::to_vec(&scorch)?,
    )?;
    let prepared = scorch.prepare()?;
    let mut samples = vec![];
    for name in names {
        for frame in [0., 0.25, 0.5, 0.75, 0.99] {
            let clip = prepared.clip(name)?;
            let geometry = prepared.geometry_with_root(&prepared.channel(clip, frame)?, true)?;
            assert!(!geometry.triangles.is_empty());
            assert!(geometry
                .triangles
                .iter()
                .flat_map(|t| t.points.iter().flatten())
                .all(|v| v.is_finite()));
            let mut scene = rc_render::Scene::new(geometry.triangles)?;
            scene.appearance = geometry.appearance;
            scene.textures = prepared
                .mesh
                .materials
                .iter()
                .map(|m| rc_render::Texture {
                    width: m.width,
                    height: m.height,
                    pixels: m.pixels.clone(),
                })
                .collect();
            let pixels = scene.render_view(
                320,
                400,
                rc_render::View {
                    yaw: 0.,
                    pitch: 0.,
                    zoom: 1.,
                    position: Some([2., 0., 0.]),
                },
            )?;
            let visible = pixels.iter().filter(|&&p| p != pixels[0]).count();
            assert!(
                visible > 300,
                "Charge pose did not appear in the diagnostic camera"
            );
            let mut ppm = b"P6\n320 400\n255\n".to_vec();
            for p in pixels {
                ppm.extend([(p >> 16) as u8, (p >> 8) as u8, p as u8]);
            }
            let image = format!("{name}-{:02}.ppm", (frame * 100.) as u32);
            fs::write(out.join(&image), ppm)?;
            samples.push(serde_json::json!({"clip":name,"frame":frame,"triangles":scene.triangles.len(),"visible_pixels":visible,"image":image}));
        }
    }
    fs::write(
        out.join("animation-validation.json"),
        serde_json::to_vec_pretty(
            &serde_json::json!({"passed":true,"samples":samples,"materials":prepared.mesh.materials.iter().map(|m|&m.source).collect::<Vec<_>>(),"scope":"Prepared original pose/skin/texture samples; no live marker animation controller or item attachment"}),
        )?,
    )?;
    println!("Original Scorch demolition start/loop/stop: fifteen textured pose samples passed");
    Ok(())
}
