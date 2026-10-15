//! Trace the original objective5 charge, marker, animation notifies and scripts.
use rc_package::{properties, read_package};
use std::{env, fs, path::Path};
fn main() -> Result<(), Box<dyn std::error::Error>> {
    let args: Vec<_> = env::args_os().collect();
    if args.len() != 3 {
        return Err("usage: rc-demolition-originals GAME_DATA OUTPUT".into());
    }
    let root = Path::new(&args[1]);
    let out = Path::new(&args[2]);
    fs::create_dir_all(out)?;
    let data = fs::read(root.join("Maps/geo_01a.ctm"))?;
    let pkg = read_package(&data)?;
    let catalog = rc_inspect::defaults::load(root)?;
    let mut rows = vec![];
    let ids = [195, 465, 466, 533, 536, 851, 1128];
    let mut selected = ids.to_vec();
    for id in ids {
        let e = &pkg.exports[id - 1];
        let props = properties::read(&pkg, &data, e)?;
        if let Some(p) = props.values.iter().find(|p| p.name == "Actions") {
            let payload = pkg.payload(&data, e)?;
            selected.extend(
                properties::object_array(
                    &pkg,
                    &payload[p.payload_offset..p.payload_offset + p.bytes],
                )?
                .into_iter()
                .map(|(id, _)| id as usize),
            );
        }
    }
    for id in selected {
        let e = &pkg.exports[id - 1];
        let class = pkg.object_path(e.class)?;
        let props = properties::read(&pkg, &data, e)?;
        let payload = pkg.payload(&data, e)?;
        let mut resolved = serde_json::Map::new();
        for name in [
            "Tag",
            "Location",
            "Rotation",
            "Anchor",
            "Actors",
            "EventUnderway",
            "PlaceEvent",
            "PropToDestroy",
            "NavPts",
            "PlayerRange",
            "PlayerOffset",
            "ActivateDotProdMax",
            "ActivateItemType",
            "DurationRemaining",
            "Difficulty",
            "bNonPlayerUsesTimer",
            "bIsUsableByPlayer",
            "bTriggeredByDamage",
            "bExplodeOnTimer",
            "bSavesProgress",
            "ChargeUnplaced",
            "ChargePlaced",
            "ChargeArmed",
            "ChargeExplode",
            "DestroyedState",
            "Prototype",
            "Capacity",
            "SpawnInterval",
            "SpawnIntervalMax",
            "SpawnMode",
            "PawnEventFromFactory",
            "Sound",
            "bWaitForFinish",
            "SoundSourceTag",
            "bEnable",
            "PauseTime",
            "ExternalEvent",
            "Event",
            "FactoryTag",
            "PermanentlyDisableFactory",
            "DestroyFactoryOffspring",
            "ObjectiveNum",
            "UpdateAction",
            "ActorTag",
        ] {
            if let Ok(v) = catalog.instance(&class, &props, name, 0, "geo_01a.ctm") {
                resolved.insert(name.into(), serde_json::to_value(v)?);
            }
        }
        let nested = props.values.iter().filter(|p|matches!(p.value,properties::Value::Raw{..})).map(|p|serde_json::json!({"name":p.name,"struct":properties::tagged_struct(&pkg,&payload[p.payload_offset..p.payload_offset+p.bytes]),"objects":properties::object_array(&pkg,&payload[p.payload_offset..p.payload_offset+p.bytes])})).collect::<Vec<_>>();
        let actions = props
            .values
            .iter()
            .find(|p| p.name == "Actions")
            .map(|p| {
                properties::object_array(
                    &pkg,
                    &payload[p.payload_offset..p.payload_offset + p.bytes],
                )
            })
            .transpose()?;
        rows.push(serde_json::json!({"original":id,"class":class,"properties":props,"resolved":resolved,"nested":nested,"actions":actions}));
    }
    let mut defaults = vec![];
    for file in [
        "System/ctmarkers.u",
        "System/engine.u",
        "System/ctinventory.u",
    ] {
        let data = fs::read(root.join(file))?;
        let pkg = read_package(&data)?;
        for (i, e) in pkg.exports.iter().enumerate() {
            let name = pkg.object_path(i as i32 + 1)?;
            if e.class == 0
                && [
                    "ExplosiveCharge",
                    "DemolitionCharge",
                    "MarkerDemolitionNative",
                    "ActivateItem",
                    "HandSignal",
                    "DemolitionPlungerObject",
                    "DemolitionChargeObject",
                ]
                .contains(&name.as_str())
            {
                let p = rc_package::classes::read(&pkg, &data, e)?.properties;
                let payload = pkg.payload(&data, e)?;
                let nested = p.values.iter().filter(|p|matches!(p.value,properties::Value::Raw{..})).map(|p|serde_json::json!({"name":p.name,"struct":properties::tagged_struct(&pkg,&payload[p.payload_offset..p.payload_offset+p.bytes])})).collect::<Vec<_>>();
                defaults.push(
                    serde_json::json!({"file":file,"class":name,"properties":p,"nested":nested}),
                );
            }
        }
    }
    let mut clips = vec![];
    let data = fs::read(root.join("Animations/Clone.ukx"))?;
    let pkg = read_package(&data)?;
    let e = pkg
        .exports
        .iter()
        .enumerate()
        .find_map(|(i, e)| {
            pkg.object_path(i as i32 + 1)
                .ok()
                .filter(|n| n.eq_ignore_ascii_case("CloneCommando"))
                .map(|_| e)
        })
        .ok_or("Missing CloneCommando")?;
    let mesh = rc_package::skeletal_mesh::read_skeletal_mesh_prefix(&pkg, &data, e)?;
    for link in mesh.linkups {
        let path = pkg.object_path(link.animation_index)?;
        let (data, pkg, object) = if link.animation_index > 0 {
            (data.clone(), read_package(&data)?, path)
        } else {
            let (file, object) = path.split_once('.').ok_or("Invalid animation reference")?;
            let data = fs::read(root.join("Animations").join(format!("{file}.ukx")))?;
            let pkg = read_package(&data)?;
            (data, pkg, object.to_owned())
        };
        let e = pkg
            .exports
            .iter()
            .enumerate()
            .find_map(|(i, e)| {
                pkg.object_path(i as i32 + 1)
                    .ok()
                    .filter(|n| n.eq_ignore_ascii_case(&object))
                    .map(|_| e)
            })
            .ok_or("Missing animation")?;
        for seq in rc_package::skeletal_animation::read_mesh_animation(&pkg, &data, e)?.sequences {
            if ["charge", "demo", "plung"]
                .iter()
                .any(|s| seq.name.name.to_lowercase().contains(s))
            {
                let notifies = seq.notifies.iter().map(|n| -> Result<_,String> {
                    let e = pkg.exports.get((n.object_index-1) as usize).ok_or("Missing notify object")?;
                    Ok(serde_json::json!({"time":f32::from_bits(n.time_bits),"original":n.object_index,"class":pkg.object_path(e.class)?,"properties":properties::read(&pkg,&data,e)?}))
                }).collect::<Result<Vec<_>,_>>()?;
                clips.push(serde_json::json!({"animation":object,"name":seq.name.name,"frames":seq.frames,"rate":f32::from_bits(seq.rate_bits),"notifies":notifies}));
            }
        }
    }
    fs::write(
        out.join("original-bindings.json"),
        serde_json::to_vec_pretty(
            &serde_json::json!({"actors_and_actions":rows,"defaults":defaults,"clips":clips}),
        )?,
    )?;
    println!(
        "Traced objective5: {} actors/actions, {} charge clips",
        rows.len(),
        clips.len()
    );
    Ok(())
}
