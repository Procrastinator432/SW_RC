//! Resolve actual geo_01a actors and original skeletal/weapon asset dependencies.
use rc_inspect::defaults;
use rc_package::{
    properties, read_package, skeletal_animation::read_mesh_animation,
    skeletal_lod::read_skeletal_lods, skeletal_mesh::read_skeletal_mesh_prefix,
};
use std::{collections::BTreeMap, env, fs, path::Path};
fn main() -> Result<(), Box<dyn std::error::Error>> {
    let args: Vec<_> = env::args_os().collect();
    if args.len() != 3 {
        return Err("usage: rc-play-assets GAME_DATA OUTPUT".into());
    }
    let game = Path::new(&args[1]);
    let catalog = defaults::load(game)?;
    let bytes = fs::read(game.join("Maps/geo_01a.ctm"))?;
    let pkg = read_package(&bytes)?;
    let mut classes = BTreeMap::new();
    let mut actors = vec![];
    for (i, e) in pkg.exports.iter().enumerate() {
        let class = pkg.object_path(e.class)?;
        if !catalog.derives_from(&class, "Engine.Pawn").unwrap_or(false) {
            continue;
        }
        *classes.entry(class.clone()).or_insert(0usize) += 1;
        let props = properties::read(&pkg, &bytes, e)?;
        let mut values = BTreeMap::new();
        for name in [
            "Mesh",
            "DrawScale",
            "DrawScale3D",
            "Location",
            "Rotation",
            "CollisionRadius",
            "CollisionHeight",
        ] {
            values.insert(
                name,
                catalog.instance(&class, &props, name, 0, "geo_01a.ctm"),
            );
        }
        actors.push(serde_json::json!({"actor":pkg.object_path(i as i32+1)?,"class":class,"properties":values}));
    }
    let mut defaults = BTreeMap::new();
    for class in catalog.names().filter(|n| {
        n.to_ascii_lowercase().contains("dc17")
            || n.ends_with("GeonosianDrone")
            || n.ends_with("BattleDroid")
            || n.ends_with("CloneCommando")
    }) {
        let mut values = BTreeMap::new();
        for name in [
            "Mesh",
            "DrawScale",
            "DrawScale3D",
            "HUDArmsMesh",
            "HudArms",
            "PlayerViewOffset",
            "PlayerViewPivot",
            "PlayerViewMesh",
            "PlayerViewScale",
            "IdleAnim",
            "FireAnim",
            "ReloadAnim",
            "SelectAnim",
            "CollisionRadius",
            "CollisionHeight",
        ] {
            values.insert(name, catalog.resolve(class, name, 0));
        }
        defaults.insert(class, values);
    }
    let mut meshes = vec![];
    for file in ["geonosian.ukx", "droid.ukx", "hudarms.ukx"] {
        let path = game.join("Animations").join(file);
        let data = fs::read(&path)?;
        let p = read_package(&data)?;
        for (i, e) in p.exports.iter().enumerate() {
            if p.object_path(e.class)? != "Engine.SkeletalMesh" {
                continue;
            }
            let object = p.object_path(i as i32 + 1)?;
            if ![
                "GeonosianDrone",
                "GeonosianWarrior",
                "BattleDroid",
                "HudArmsDC17m",
            ]
            .contains(&object.as_str())
            {
                continue;
            }
            let prefix = read_skeletal_mesh_prefix(&p, &data, e)?;
            let lods = read_skeletal_lods(&p, &data, e)?;
            let qualify = |index: i32| -> Result<String, String> {
                let name = p.object_path(index)?;
                Ok(if index > 0 {
                    format!("{}.{}", path.file_stem().unwrap().to_string_lossy(), name)
                } else {
                    name
                })
            };
            let mut animations = vec![];
            for link in &prefix.linkups {
                if link.animation_index == 0 {
                    continue;
                }
                let name = qualify(link.animation_index)?;
                let (file, object) = name.split_once('.').ok_or("animation path")?;
                let data = fs::read(game.join("Animations").join(format!("{file}.ukx")))?;
                let a = read_package(&data)?;
                let e = a
                    .exports
                    .iter()
                    .enumerate()
                    .find_map(|(i, e)| {
                        (a.object_path(i as i32 + 1)
                            .ok()?
                            .eq_ignore_ascii_case(object))
                        .then_some(e)
                    })
                    .ok_or("animation export")?;
                let animation = read_mesh_animation(&a, &data, e)?;
                let sequences:Vec<_>=animation.sequences.iter().map(|s|serde_json::json!({"name":s.name.name,"frames":s.frames,"rate":f32::from_bits(s.rate_bits),"tracks":s.tracks.len(),"notifies":s.notifies})).collect();
                animations.push(serde_json::json!({"asset":name,"bones":animation.reference_bones,"sequences":sequences}));
            }
            let materials: Vec<_> = prefix
                .materials
                .iter()
                .map(|&i| qualify(i))
                .collect::<Result<_, _>>()?;
            let lods:Vec<_>=lods.lods.iter().map(|l|serde_json::json!({"commands":l.commands.len(),"vertices":l.bind_vertices.len(),"rigid_vertices":l.stream_vertices.len(),"sections":l.sections})).collect();
            meshes.push(serde_json::json!({"file":path,"export":i+1,"object":object,"materials":materials,"bones":prefix.bones,"animations":animations,"lods":lods}));
        }
    }
    let output = Path::new(&args[2]);
    if let Some(parent) = output.parent() {
        fs::create_dir_all(parent)?;
    }
    fs::write(
        output,
        serde_json::to_vec_pretty(
            &serde_json::json!({"map":"geo_01a","classes":classes,"actors":actors,"defaults":defaults,"meshes":meshes,"scope":"Original package/default/instance references and skeletal dependencies; no inferred gameplay parity."}),
        )?,
    )?;
    println!(
        "Resolved {} pawn actors and {} selected original meshes",
        actors.len(),
        meshes.len()
    );
    Ok(())
}
