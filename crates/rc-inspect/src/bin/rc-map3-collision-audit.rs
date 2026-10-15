use std::{fs, path::Path};
fn main() -> Result<(), Box<dyn std::error::Error>> {
    let root = Path::new("D:/SteamLibrary/steamapps/common/Star Wars Republic Commando/GameData");
    let map = fs::read(root.join("Maps/geo_01c.ctm"))?;
    let pkg = rc_package::read_package(&map)?;
    let catalog = rc_inspect::defaults::load(root)?;
    for (i, e) in pkg.exports.iter().enumerate() {
        let name = pkg.object_path(i as i32 + 1)?;
        if ["Rope5", "Rope6", "StaticMeshActor15"].contains(&name.as_str()) {
            let class = pkg.object_path(e.class)?;
            let props = rc_package::properties::read(&pkg, &map, e)?;
            println!(
                "{} {} {:?}",
                name,
                class,
                catalog.instance(&class, &props, "DrawType", 0, "geo_01c.ctm")?
            );
        }
    }
    let data = fs::read(root.join("StaticMeshes/Arena_HR.usx"))?;
    let p = rc_package::read_package(&data)?;
    let e = p
        .exports
        .iter()
        .enumerate()
        .find(|(i, _)| {
            p.object_path(*i as i32 + 1)
                .is_ok_and(|s| s.ends_with("SunFacAnteRoom"))
        })
        .ok_or("Missing mesh")?
        .1;
    let mesh = rc_package::mesh::read_static_mesh(&p, &data, e)?;
    let collision = rc_package::mesh_collision::read(&p, &data, e, &mesh)?;
    let e = &p.exports[collision.simple_model as usize - 1];
    let bsp = rc_package::geometry::read_bsp(&p, &data, e)?;
    let solid = rc_package::collision::read_model_collision(&p, &data, e, &bsp)?;
    let hull = rc_package::hull::HullSet::read(&bsp, &solid)?;
    fs::write(
        "analysis/mission19-map3/missing-hull.json",
        serde_json::to_vec_pretty(
            &serde_json::json!({"bsp":bsp,"solid":solid,"completed_hulls":rc_package::bsp_cells::complete(&bsp,&solid)?,"hulls":hull}),
        )?,
    )?;
    Ok(())
}
