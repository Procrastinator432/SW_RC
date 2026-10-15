use std::{env, fs, path::Path};
fn main() -> Result<(), Box<dyn std::error::Error>> {
    let args: Vec<_> = env::args_os().collect();
    let out = Path::new(&args[2]);
    fs::create_dir_all(out)?;
    let mut g = rc_game::bundle::decode(&fs::read(&args[1])?)?;
    for id in [1122, 915, 617, 909, 910, 912, 913, 964, 850, 1093] {
        g.disable_dynamic(id)?;
    }
    let mut rows = vec![];
    for (i, p) in [
        [4328., -7239., -2239.],
        [4672., -7253., -2251.],
        [5000., -7200., -2251.],
        [4500., -8000., -2000.],
    ]
    .into_iter()
    .enumerate()
    {
        let eye = [p[0], p[1], p[2] + 60.];
        let pixels = g.scene.render_view(
            960,
            600,
            rc_render::View {
                position: Some(std::array::from_fn(|i| {
                    (eye[i] as f32 - g.scene.center[i]) / g.scene.radius
                })),
                zoom: 1.,
                yaw: std::f32::consts::PI,
                pitch: 0.1,
            },
        )?;
        let mut ppm = b"P6\n960 600\n255\n".to_vec();
        for p in pixels {
            ppm.extend([(p >> 16) as u8, (p >> 8) as u8, p as u8]);
        }
        fs::write(out.join(format!("probe{i}.ppm")), ppm)?;
        rows.push(serde_json::json!({"position":p,"placement":g.world.body_placement(p,[40.,40.,84.]),"crouch":g.world.body_placement([p[0],p[1],p[2]-28.],[40.,40.,56.]),"floor":g.world.floor_contact(p,[40.,40.,84.],400.,0.7)}));
    }
    fs::write(
        out.join("collision-probes.json"),
        serde_json::to_vec_pretty(&rows)?,
    )?;
    Ok(())
}
