use std::{fs, path::Path};
fn main() -> Result<(), Box<dyn std::error::Error>> {
    let root = Path::new("D:/SteamLibrary/steamapps/common/Star Wars Republic Commando/GameData");
    let out = Path::new("analysis/mission25-map3-balcony");
    fs::create_dir_all(out)?;
    let data = fs::read(root.join("Animations/Geonosian.ukx"))?;
    let p = rc_package::read_package(&data)?;
    let mut clips = vec![];
    for (i, e) in p.exports.iter().enumerate() {
        if p.object_path(e.class)? != "Engine.MeshAnimation" {
            continue;
        }
        let a = rc_package::skeletal_animation::read_mesh_animation(&p, &data, e)?;
        for s in a.sequences {
            if ["EmoteSeeEnemy", "DeathTaunt"].contains(&s.name.name.as_str()) {
                println!(
                    "{} {} {} frames",
                    p.object_path(i as i32 + 1)?,
                    s.name.name,
                    s.frames
                );
                clips.push(serde_json::json!({"set":p.object_path(i as i32+1)?,"sequence":s}));
            }
        }
    }
    let mut resolver = rc_inspect::assets::Assets::new(&[
        &root.join("Textures"),
        &root.join("Animations"),
        &root.join("System"),
    ])?;
    let m = rc_inspect::visual_assets::load(
        root,
        "Geonosian.ukx",
        "GeonosianWarrior",
        &["EmoteSeeEnemy", "DeathTaunt"],
        &mut resolver,
    )?;
    fs::write(out.join("sunfac-clips.json"), serde_json::to_vec(&m.clips)?)?;
    let bytes = fs::read(root.join("StaticMeshes/Arena_GM.usx"))?;
    let pkg = rc_package::read_package(&bytes)?;
    let e = pkg
        .exports
        .iter()
        .enumerate()
        .find(|(i, _)| {
            pkg.object_path(*i as i32 + 1)
                .is_ok_and(|p| p == "Arena.GateShield")
        })
        .unwrap()
        .1;
    let mesh = rc_package::mesh::read_static_mesh(&pkg, &bytes, e)?;
    let mut mats = rc_inspect::assets::Assets::new(&[
        &root.join("Textures"),
        &root.join("StaticMeshes"),
        &root.join("System"),
    ])?;
    let mut todo: Vec<String> = mesh
        .materials
        .iter()
        .flatten()
        .map(|m| mats.qualify("Arena_GM", m))
        .collect();
    let mut seen = std::collections::BTreeSet::new();
    let mut materials = vec![];
    while let Some(path) = todo.pop() {
        if !seen.insert(path.clone()) || seen.len() > 32 {
            continue;
        }
        if let Ok((class, props)) = mats.material_properties(&path) {
            for v in &props.values {
                if let rc_package::properties::Value::Object { index, path: p } = &v.value {
                    if *index != 0 {
                        todo.push(mats.qualify(path.split('.').next().unwrap(), p));
                    }
                }
            }
            materials.push(serde_json::json!({"path":path,"class":class,"properties":props}));
        }
    }
    fs::write(
        out.join("shield-materials.json"),
        serde_json::to_vec_pretty(&materials)?,
    )?;
    let catalog = rc_inspect::defaults::load(root)?;
    let mut defaults = vec![];
    for (class, keys) in [
        (
            "CTMarkers.MarkerSnipe",
            vec!["PlayerRange", "EventUnderway", "bCanCancelAfterUnderway"],
        ),
        (
            "Properties.GeonosianSunFac",
            vec!["CollisionRadius", "CollisionHeight", "GroundSpeed"],
        ),
        (
            "CTCharacters.BattleDroid",
            vec!["Health", "CollisionRadius", "CollisionHeight"],
        ),
    ] {
        for key in keys {
            defaults.push(serde_json::json!({"class":class,"property":key,"resolved":catalog.resolve(class,key,0)}));
        }
    }
    fs::write(
        out.join("original-assets.json"),
        serde_json::to_vec_pretty(&serde_json::json!({"clips":clips,"defaults":defaults}))?,
    )?;
    Ok(())
}
