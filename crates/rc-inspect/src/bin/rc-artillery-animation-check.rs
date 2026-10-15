use std::{env, fs, path::Path};
fn main() -> Result<(), Box<dyn std::error::Error>> {
    let a: Vec<_> = env::args_os().collect();
    let out = Path::new(&a[2]);
    let g = rc_game::bundle::decode(&fs::read(&a[1])?)?;
    let p = g.artillery.as_ref().unwrap();
    let mut rows = vec![];
    for (i, m) in p.actors.iter().enumerate() {
        for frame in [0., 0.25, 0.5, 0.75, 0.99] {
            let actor = &p.assets.actors[i];
            let c = m.clip("GunshipAmbushSpiderdroid")?;
            let mut scene = m.geometry_with_root(&m.channel(c, frame)?, false)?;
            let delta = m.cinematic_root_delta(c, frame)?;
            for t in &mut scene.triangles {
                t.points = t.points.map(|v| {
                    rc_package::mesh::transform_point(
                        std::array::from_fn(|j| v[j] + delta[j]),
                        actor.location,
                        actor.rotation,
                        [1.; 3],
                        [0.; 3],
                    )
                });
            }
            let mut min = [f32::INFINITY; 3];
            let mut max = [f32::NEG_INFINITY; 3];
            for p in scene.triangles.iter().flat_map(|t| t.points) {
                for j in 0..3 {
                    min[j] = min[j].min(p[j]);
                    max[j] = max[j].max(p[j]);
                }
            }
            rows.push(
                serde_json::json!({"original":actor.original,"frame":frame,"bounds":[min,max]}),
            );
        }
    }
    fs::write(
        out.join("animation-bounds.json"),
        serde_json::to_vec_pretty(&rows)?,
    )?;
    println!("{} original animation bounds sampled", rows.len());
    Ok(())
}
