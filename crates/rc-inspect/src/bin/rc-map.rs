use rc_package::{geometry, read_package};
use std::{env, fs, path::Path};
fn main() -> Result<(), Box<dyn std::error::Error>> {
    let args: Vec<_> = env::args_os().collect();
    if args.len() != 3 {
        return Err("Usage: rc-map <map.ctm> <output-directory>".into());
    }
    let data = fs::read(&args[1])?;
    let pkg = read_package(&data)?;
    let out = Path::new(&args[2]);
    fs::create_dir_all(out)?;
    let mut count = 0;
    let mut objects = Vec::new();
    let mut property_errors = 0;
    for (i, e) in pkg.exports.iter().enumerate() {
        let class = pkg.object_path(e.class)?;
        if e.class != 0 {
            match rc_package::properties::read(&pkg,&data,e) {
                Ok(props)=>objects.push(serde_json::json!({"index":i+1,"object_path":pkg.object_path(i as i32+1)?,"class":class,"properties":props})),
                Err(error)=>{property_errors+=1;objects.push(serde_json::json!({"index":i+1,"object_path":pkg.object_path(i as i32+1)?,"class":class,"property_error":error}));},
            }
        }
        if class == "Engine.Model" {
            let bsp = geometry::read_bsp(&pkg, &data, e)?;
            let triangles = bsp.triangles()?;
            fs::write(
                out.join(format!("model-{}.json", i + 1)),
                serde_json::to_vec_pretty(&bsp)?,
            )?;
            let mut obj = String::from("# Original SWRC BSP geometry; original Unreal XYZ units\n");
            for t in &triangles {
                for p in t.points {
                    obj.push_str(&format!("v {} {} {}\n", p[0], p[1], p[2]));
                }
            }
            for j in 0..triangles.len() {
                obj.push_str(&format!("f {} {} {}\n", j * 3 + 1, j * 3 + 2, j * 3 + 3));
            }
            fs::write(out.join(format!("model-{}.obj", i + 1)), obj)?;
            println!(
                "{}: {} points, {} nodes, {} surfaces, {} triangles; {} tail bytes not decoded",
                pkg.object_path(i as i32 + 1)?,
                bsp.points.len(),
                bsp.nodes.len(),
                bsp.surfaces.len(),
                triangles.len(),
                bsp.unparsed_tail_bytes
            );
            count += 1;
        }
    }
    fs::write(
        out.join("objects.json"),
        serde_json::to_vec_pretty(
            &serde_json::json!({"objects":objects,"property_errors":property_errors}),
        )?,
    )?;
    println!("Object properties: {} failures", property_errors);
    if count == 0 {
        return Err("No BSP models found".into());
    }
    let scene = rc_render::Scene::from_map(&data)?;
    let pixels = scene.render(960, 640, -0.7, 0.65, 1.0)?;
    let mut ppm = b"P6\n960 640\n255\n".to_vec();
    for p in pixels {
        ppm.extend_from_slice(&[(p >> 16) as u8, (p >> 8) as u8, p as u8]);
    }
    fs::write(out.join("world-bsp.ppm"), ppm)?;
    Ok(())
}
