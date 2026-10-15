//! Complete geo_01b actor/action inventory for tracing the next original objective.
use rc_package::{properties, read_package};
use std::{env, fs, path::Path};
fn main() -> Result<(), Box<dyn std::error::Error>> {
    let args: Vec<_> = env::args_os().collect();
    if ![3, 4].contains(&args.len()) {
        return Err("usage: rc-map2-audit GAME_DATA OUTPUT".into());
    }
    let map_name = args
        .get(3)
        .map(|s| s.to_string_lossy().into_owned())
        .unwrap_or_else(|| "geo_01b.ctm".into());
    let root = Path::new(&args[1]);
    let out = Path::new(&args[2]);
    fs::create_dir_all(out)?;
    let data = fs::read(root.join("Maps").join(&map_name))?;
    let pkg = read_package(&data)?;
    let catalog = rc_inspect::defaults::load(root)?;
    let mut rows = vec![];
    for (i, e) in pkg.exports.iter().enumerate() {
        if e.class == 0 {
            continue;
        }
        let class = pkg.object_path(e.class)?;
        if !(catalog
            .derives_from(&class, "Engine.Actor")
            .unwrap_or(false)
            || class.contains("ACTION_"))
        {
            continue;
        }
        let props = properties::read(&pkg, &data, e)?;
        let actions = if let Some(a) = props.values.iter().find(|p| p.name == "Actions") {
            let payload = pkg.payload(&data, e)?;
            properties::object_array(&pkg, &payload[a.payload_offset..a.payload_offset + a.bytes])?
        } else {
            vec![]
        };
        let mut resolved = serde_json::Map::new();
        for name in [
            "Tag",
            "Event",
            "Location",
            "Rotation",
            "CollisionRadius",
            "CollisionHeight",
            "bDisabled",
            "UpdateAction",
            "bWaitForFinish",
            "SoundSourceTag",
            "Prototype",
            "PawnTagFromFactory",
            "PawnEventFromFactory",
            "Capacity",
            "SpawnInterval",
            "SpawnMode",
            "StartStateFromFactory",
            "PreserveDeadBodiesFromFactory",
            "bActive",
            "MaxSpawned",
            "NextURL",
            "StaticMesh",
            "Mesh",
            "DrawScale",
            "DrawScale3D",
            "PrePivot",
        ] {
            if let Ok(v) = catalog.instance(&class, &props, name, 0, &map_name) {
                resolved.insert(name.into(), serde_json::to_value(v)?);
            }
        }
        let prototype = resolved
            .get("Prototype")
            .and_then(|v| v.get("value"))
            .and_then(|v| v.get("value"))
            .and_then(|v| v.get("path"))
            .and_then(|v| v.as_str());
        let mut prototype_defaults = vec![];
        if let Some(prototype) = prototype {
            for name in [
                "Health",
                "Mesh",
                "StaticMesh",
                "DrawScale",
                "DrawScale3D",
                "CollisionRadius",
                "CollisionHeight",
                "bInvulnerable",
                "InitialState",
            ] {
                prototype_defaults.push(serde_json::json!({"property":name,"resolved":catalog.resolve(prototype,name,0)}));
            }
        }
        let payload = pkg.payload(&data, e)?;
        let nested=props.values.iter().filter(|p|p.name=="MissionObj" || p.name=="ChargeArmed" || p.name=="ChargeExplode").map(|p|{
            let raw=&payload[p.payload_offset..p.payload_offset+p.bytes];
            serde_json::json!({"property":p.name,"struct_array":properties::struct_array(&pkg,raw),"tagged_struct":properties::tagged_struct(&pkg,raw)})
        }).collect::<Vec<_>>();
        rows.push(serde_json::json!({"original":i+1,"name":pkg.object_path(i as i32+1)?,"class":class,"properties":props,"resolved":resolved,"actions":actions,"prototype_defaults":prototype_defaults,"nested":nested}));
    }
    fs::write(
        out.join("original-map-inventory.json"),
        serde_json::to_vec_pretty(&rows)?,
    )?;
    println!("Captured {} original actors/actions", rows.len());
    Ok(())
}
