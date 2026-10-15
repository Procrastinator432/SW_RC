use rc_package::{properties, read_package};
use std::{fs, path::Path};
fn main() -> Result<(), Box<dyn std::error::Error>> {
    let root = Path::new("D:/SteamLibrary/steamapps/common/Star Wars Republic Commando/GameData");
    let out = Path::new("analysis/mission26-map3-finale");
    fs::create_dir_all(out)?;
    let catalog = rc_inspect::defaults::load(root)?;
    let mut defaults = vec![];
    for (class, keys) in [
        (
            "CTCharacters.GeoFighter",
            vec!["Mesh", "DrawScale", "DrawScale3D"],
        ),
        ("Properties.CloneCommando07", vec!["Mesh", "WeaponClass"]),
        ("Properties.GeonosianSunFac", vec!["GroundSpeed"]),
    ] {
        for key in keys {
            defaults.push(serde_json::json!({"class":class,"property":key,"resolved":catalog.resolve(class,key,0)}));
        }
    }
    let names: Vec<_> = catalog
        .names()
        .filter(|s| s.to_lowercase().contains("snipe"))
        .collect();
    for class in names {
        for key in [
            "Mesh",
            "StaticMesh",
            "FireSound",
            "RefireRate",
            "FireInterval",
            "TimeBetweenShots",
        ] {
            defaults.push(serde_json::json!({"class":class,"property":key,"resolved":catalog.resolve(class,key,0)}));
        }
    }
    let mut clips = vec![];
    for file in [
        "vehicles.ukx",
        "geoanimprops.ukx",
        "clone.ukx",
        "geonosian.ukx",
    ] {
        let data = fs::read(root.join("Animations").join(file))?;
        let p = read_package(&data)?;
        for (i, e) in p.exports.iter().enumerate() {
            if p.object_path(e.class)? != "Engine.MeshAnimation" {
                continue;
            }
            let a = rc_package::skeletal_animation::read_mesh_animation(&p, &data, e)?;
            for s in a.sequences {
                let name = s.name.name.as_str();
                if !(name.contains("SunFac")
                    || (file == "geonosian.ukx"
                        && ["Stunned", "EmoteHearEnemy", "WalkForward"].contains(&name))
                    || (file == "clone.ukx" && name.to_lowercase().contains("snipe")))
                {
                    continue;
                }
                let mut notifies = vec![];
                for n in &s.notifies {
                    let e = &p.exports[n.object_index as usize - 1];
                    notifies.push(serde_json::json!({"original":n.object_index,"class":p.object_path(e.class)?,"properties":properties::read(&p,&data,e)?}));
                }
                println!(
                    "{file} {} {name} frames={} flags={} notifies={}",
                    p.object_path(i as i32 + 1)?,
                    s.frames,
                    s.flags,
                    s.notifies.len()
                );
                clips.push(serde_json::json!({"file":file,"set":p.object_path(i as i32+1)?,"sequence":s,"notifies":notifies}));
            }
        }
    }
    let mut materials = vec![];
    for (package, object) in [
        ("VehicleMeshes", "Confederacy.GeoFighterBroken"),
        ("Weapons", "CloneWeapons.DC17mSniper"),
    ] {
        let bytes = fs::read(root.join("StaticMeshes").join(format!("{package}.usx")))?;
        let pkg = read_package(&bytes)?;
        let e = pkg
            .exports
            .iter()
            .enumerate()
            .find(|(i, _)| pkg.object_path(*i as i32 + 1).is_ok_and(|p| p == object))
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
            .map(|m| mats.qualify(package, m))
            .collect();
        let mut seen = std::collections::BTreeSet::new();
        while let Some(path) = todo.pop() {
            if !seen.insert(path.clone()) || seen.len() > 40 {
                continue;
            }
            if let Ok((class, props)) = mats.material_properties(&path) {
                for v in &props.values {
                    if let properties::Value::Object { index, path: p } = &v.value {
                        if *index != 0 {
                            todo.push(mats.qualify(path.split('.').next().unwrap(), p));
                        }
                    }
                }
                materials.push(serde_json::json!({"mesh":format!("{package}.{object}"),"path":path,"class":class,"properties":props}));
            }
        }
    }
    fs::write(
        out.join("materials.json"),
        serde_json::to_vec_pretty(&materials)?,
    )?;
    let mut resolver = rc_inspect::assets::Assets::new(&[
        &root.join("Textures"),
        &root.join("Animations"),
        &root.join("System"),
    ])?;
    let ship = rc_inspect::visual_assets::load_from_animation(
        root,
        "vehicles.ukx",
        "GeoFighter",
        &[
            "SevKillSunFacIntro",
            "SevKillSunFacIdle",
            "SevKillSunFacDeath",
            "SevKillSunFacFailMission",
        ],
        &mut resolver,
        Some("Vehicles.cGEO01GeoFighterSet"),
    )?;
    let prepared = ship.prepare_reference()?;
    let mut roots = vec![];
    for (i, c) in prepared.mesh.clips.iter().enumerate() {
        roots.push(serde_json::json!({"clip":c.sequence.name.name,"first":prepared.root_matrix(i,0.)?.map(f32::from_bits),"last":prepared.root_matrix(i,(c.sequence.frames-1) as f32/c.sequence.frames as f32)?.map(f32::from_bits)}));
    }
    fs::write(
        out.join("ship-mesh.json"),
        serde_json::to_vec(&prepared.mesh)?,
    )?;
    fs::write(
        out.join("ship-roots.json"),
        serde_json::to_vec_pretty(&roots)?,
    )?;
    let sun = rc_inspect::visual_assets::load(
        root,
        "geonosian.ukx",
        "GeonosianWarrior",
        &["Stunned", "EmoteHearEnemy", "WalkForward"],
        &mut resolver,
    )?;
    fs::write(
        out.join("sunfac-clips.json"),
        serde_json::to_vec(&sun.clips)?,
    )?;
    let clone = rc_inspect::visual_assets::load_from_animation(
        root,
        "clone.ukx",
        "CloneCommando",
        &["FireSniper"],
        &mut resolver,
        Some("Clone.CloneCommandoSet"),
    )?;
    fs::write(
        out.join("sniper-clips.json"),
        serde_json::to_vec(&clone.clips)?,
    )?;
    fs::write(
        out.join("original-assets.json"),
        serde_json::to_vec_pretty(&serde_json::json!({"defaults":defaults,"clips":clips}))?,
    )?;
    Ok(())
}
