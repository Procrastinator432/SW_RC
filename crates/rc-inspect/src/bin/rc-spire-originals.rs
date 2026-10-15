//! Original final geo_01a objective, conversation, bombardment gate and exit door bindings.
use rc_package::{properties, read_package};
use std::{env, fs, path::Path};
fn main() -> Result<(), Box<dyn std::error::Error>> {
    let a: Vec<_> = env::args_os().collect();
    if a.len() != 3 {
        return Err("usage: rc-spire-originals GAME_DATA OUTPUT".into());
    }
    let root = Path::new(&a[1]);
    let out = Path::new(&a[2]);
    fs::create_dir_all(out)?;
    let bytes = fs::read(root.join("Maps/geo_01a.ctm"))?;
    let p = read_package(&bytes)?;
    let catalog = rc_inspect::defaults::load(root)?;
    let ids = [
        9, 16, 23, 24, 32, 34, 35, 39, 50, 751, 780, 848, 860, 861, 862, 1065,
    ];
    let mut selected = ids.to_vec();
    for id in ids {
        let e = &p.exports[id - 1];
        let props = properties::read(&p, &bytes, e)?;
        if let Some(a) = props.values.iter().find(|a| a.name == "Actions") {
            let payload = p.payload(&bytes, e)?;
            selected.extend(
                properties::object_array(
                    &p,
                    &payload[a.payload_offset..a.payload_offset + a.bytes],
                )?
                .iter()
                .map(|(id, _)| *id as usize),
            );
        }
    }
    selected.sort();
    selected.dedup();
    let mut rows = vec![];
    for id in selected {
        let e = &p.exports[id - 1];
        let class = p.object_path(e.class)?;
        let props = properties::read(&p, &bytes, e)?;
        let mut resolved = serde_json::Map::new();
        for name in [
            "Tag",
            "Event",
            "Location",
            "Rotation",
            "CollisionRadius",
            "CollisionHeight",
            "URL",
            "StaticMesh",
            "Mesh",
            "DrawScale",
            "DrawScale3D",
            "PrePivot",
            "KeyPos",
            "MoveTimes",
            "DelayTime",
            "OpeningSound",
            "OpenedSound",
            "ClosingSound",
            "Health",
            "Sound",
            "bWaitForFinish",
            "PauseTime",
            "UpdateAction",
            "Objective",
            "ObjectiveIndex",
        ] {
            if let Ok(v) = catalog.instance(&class, &props, name, 0, "geo_01a") {
                resolved.insert(name.into(), serde_json::to_value(v)?);
            }
        }
        if id == 34 || id == 35 {
            resolved.insert(
                "KeyPos[1]".into(),
                serde_json::to_value(catalog.instance(&class, &props, "KeyPos", 1, "geo_01a")?)?,
            );
        }
        rows.push(serde_json::json!({"original":id,"name":p.object_path(id as i32)?,"class":class,"properties":props,"resolved":resolved}));
    }
    fs::write(
        out.join("original-bindings.json"),
        serde_json::to_vec_pretty(&rows)?,
    )?;
    let mut defaults = vec![];
    for file in [
        "Properties/markerproperties.u",
        "System/ctmarkers.u",
        "System/engine.u",
        "System/ctgame.u",
    ] {
        let data = fs::read(root.join(file))?;
        let pkg = read_package(&data)?;
        for (i, e) in pkg.exports.iter().enumerate() {
            let name = pkg.object_path(i as i32 + 1)?;
            if e.class == 0
                && [
                    "MoverDB_GEO",
                    "MoverDBSlave_GEO",
                    "MoverDB",
                    "MoverDBSlave",
                    "Mover",
                    "MoverDBNative",
                ]
                .contains(&name.as_str())
            {
                defaults.push(serde_json::json!({"class":name,"parent":pkg.object_path(e.super_class)?,"file":file,"properties":rc_package::classes::read(&pkg,&data,e)?.properties,"fields":rc_package::classes::fields(&pkg,&data,e)?}));
            }
        }
    }
    fs::write(
        out.join("door-defaults.json"),
        serde_json::to_vec_pretty(&defaults)?,
    )?;
    println!(
        "Captured {} final objective actor/action bindings",
        rows.len()
    );
    Ok(())
}
