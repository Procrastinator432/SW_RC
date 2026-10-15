//! Prepare original first-person hands, detonator and charge appearance states.
use rc_package::{mesh_query::ActorTransform, properties, read_package};
use std::{env, fs, path::Path};
fn main() -> Result<(), Box<dyn std::error::Error>> {
    let args: Vec<_> = env::args_os().collect();
    if args.len() != 3 {
        return Err("usage: rc-demolition-equipment GAME_DATA OUTPUT".into());
    }
    let root = Path::new(&args[1]);
    let out = Path::new(&args[2]);
    fs::create_dir_all(out)?;
    let catalog = rc_inspect::defaults::load(root)?;
    let mut resolver = rc_inspect::assets::Assets::new(&[
        &root.join("Textures"),
        &root.join("Animations"),
        &root.join("System"),
    ])?;
    let hands = rc_inspect::visual_assets::load(
        root,
        "HudArms.ukx",
        "HudArmsHands",
        &["Detonate"],
        &mut resolver,
    )?;
    let source = hands.clips[0].source.clone();
    let (path, _) = source.split_once(':').ok_or("Missing hands animation")?;
    let (file, object) = path
        .split_once('.')
        .ok_or("Missing hands animation package")?;
    let data = fs::read(root.join("Animations").join(format!("{file}.ukx")))?;
    let pkg = read_package(&data)?;
    let mut notifies = vec![];
    for n in &hands.clips[0].sequence.notifies {
        notifies.push(serde_json::json!({"time":f32::from_bits(n.time_bits),"name":n.name.name,"object":n.object_index,"properties":if n.object_index>0{Some(properties::read(&pkg,&data,&pkg.exports[n.object_index as usize-1])?)}else{None},"class":if n.object_index>0{pkg.object_path(pkg.exports[n.object_index as usize-1].class)?}else{String::new()}}));
    }
    fs::write(
        out.join("hands-notifies.json"),
        serde_json::to_vec_pretty(
            &serde_json::json!({"source":source,"animation":object,"notifies":notifies,"bones":hands.bones.iter().map(|b|&b.name.name).collect::<Vec<_>>()}),
        )?,
    )?;
    fs::write(out.join("hands-mesh.json"), serde_json::to_vec(&hands)?)?;
    let t = ActorTransform {
        location: [0.; 3],
        rotation: [0; 3],
        scale: [1.; 3],
        pivot: [0.; 3],
    };
    let mut receipts = vec![];
    let plunger = rc_inspect::mission_assets::mesh_scene(
        root,
        "MarkerIcons.Demolition.Detonator",
        &t,
        &catalog,
        &mut receipts,
    )?;
    fs::write(out.join("plunger.rcscene"), plunger.snapshot()?)?;
    for (index, path) in [
        "MarkerIconTextures.Demolition.DemolitionCharge_Inactive",
        "MarkerIconTextures.Demolition.DemolitionCharge_Active",
    ]
    .iter()
    .enumerate()
    {
        let charge = rc_inspect::mission_assets::mesh_scene_skinned(
            root,
            "MarkerIcons.Demolition.DemolitionCharge",
            &t,
            &catalog,
            &mut receipts,
            Some(path),
        )?;
        fs::write(
            out.join(format!("charge-{index}.rcscene")),
            charge.snapshot()?,
        )?;
    }
    fs::write(
        out.join("equipment-bindings.json"),
        serde_json::to_vec_pretty(
            &serde_json::json!({"hands":"CTInventory.HandSignal:HudArms.HudArmsHands","plunger":"CTInventory.DemolitionPlungerObject:MarkerIcons.Demolition.Detonator","attach_bone":"weaponAttach_L","materials":receipts}),
        )?,
    )?;
    let hands = hands.prepare_reference()?;
    for f in [0., 0.5, 0.8] {
        let mesh = hands.geometry(&hands.channel(0, f)?)?;
        let bounds = std::array::from_fn::<_, 3, _>(|i| {
            [
                mesh.triangles
                    .iter()
                    .flat_map(|t| t.points)
                    .map(|p| p[i])
                    .fold(f32::INFINITY, f32::min),
                mesh.triangles
                    .iter()
                    .flat_map(|t| t.points)
                    .map(|p| p[i])
                    .fold(f32::NEG_INFINITY, f32::max),
            ]
        });
        println!("hands frame {f}: {bounds:?}");
    }
    println!("Prepared original HandSignal hands/Detonate, detonator and two charge appearances");
    Ok(())
}
