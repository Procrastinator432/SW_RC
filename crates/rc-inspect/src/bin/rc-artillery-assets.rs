use rc_package::{
    properties::{self, Value},
    read_package,
};
use std::{env, fs, path::Path};
fn main() -> Result<(), Box<dyn std::error::Error>> {
    let a: Vec<_> = env::args_os().collect();
    let root = Path::new(&a[1]);
    let out = Path::new(&a[2]);
    fs::create_dir_all(out)?;
    let cat = rc_inspect::defaults::load(root)?;
    let bytes = fs::read(root.join("Maps/geo_01a.ctm"))?;
    let p = read_package(&bytes)?;
    let mut rows = vec![];
    for id in [
        31, 580, 533, 1013, 1014, 1015, 1016, 1017, 1018, 1028, 1029, 3552, 3553, 3554, 3555,
    ] {
        let e = &p.exports[id - 1];
        let class = p.object_path(e.class)?;
        let props = properties::read(&p, &bytes, e)?;
        let mut resolved = serde_json::Map::new();
        for name in [
            "Mesh",
            "MeshSets",
            "Skins",
            "CollisionHeight",
            "CollisionRadius",
            "Health",
            "Damage",
            "DamageRadius",
            "Speed",
            "LifeSpan",
            "ExplosionSounds",
            "ExplosionEffect",
            "TrailClass",
            "DrawScale",
            "SpawnInterval",
            "SpawnIntervalMax",
            "InitialSpeed",
            "Prototype",
            "SpawnMode",
            "Capacity",
            "MaxAlive",
            "InitialState",
            "bHidden",
            "bWaitUntilAnimEnd",
            "RandomSeconds",
            "PauseTime",
        ] {
            if let Ok(v) = cat.instance(&class, &props, name, 0, "geo_01a") {
                resolved.insert(name.into(), serde_json::to_value(v)?);
            }
        }
        rows.push(
            serde_json::json!({"original":id,"class":class,"properties":props,"resolved":resolved}),
        );
    }
    let empty = properties::Properties {
        values: vec![],
        native_offset: 0,
    };
    for class in [
        "Properties.GEO_01AProj",
        "Properties.GunshipHigh",
        "CTCharacters.SpiderDroid",
        "CTCharacters.GeonosianWarrior",
    ] {
        let mut resolved = serde_json::Map::new();
        for name in [
            "Mesh",
            "MeshSets",
            "Skins",
            "CollisionHeight",
            "CollisionRadius",
            "Health",
            "Damage",
            "DamageRadius",
            "Speed",
            "LifeSpan",
            "ExplosionSounds",
            "ExplosionEffect",
            "TrailClass",
            "DrawScale",
            "GroundSpeed",
            "SpawnInterval",
            "Physics",
            "bCollideWorld",
        ] {
            if let Ok(v) = cat.instance(class, &empty, name, 0, class) {
                resolved.insert(name.into(), serde_json::to_value(v)?);
            }
        }
        rows.push(serde_json::json!({"class":class,"resolved":resolved}));
    }
    for file in ["Properties/properties.u", "System/ctcharacters.u"] {
        let d = fs::read(root.join(file))?;
        let q = read_package(&d)?;
        for (i, e) in q.exports.iter().enumerate() {
            let name = q.object_path(i as i32 + 1)?;
            if e.class == 0
                && [
                    "GEO_01AProj",
                    "GunshipHigh",
                    "SpiderDroid",
                    "SpiderDroidDefaults",
                    "GeonosianWarriorDefaults",
                    "Gunship",
                ]
                .contains(&name.as_str())
            {
                rows.push(serde_json::json!({"file":file,"class":name,"properties":rc_package::classes::read(&q,&d,e)?.properties}));
            }
        }
    }
    fs::write(
        out.join("originals.json"),
        serde_json::to_vec_pretty(&rows)?,
    )?;
    for file in ["vehicles.ukx"] {
        let d = fs::read(root.join("Animations").join(file))?;
        let q = read_package(&d)?;
        let mut entries = vec![];
        for (i, e) in q.exports.iter().enumerate() {
            let class = q.object_path(e.class)?;
            if class == "Engine.SkeletalMesh" || class == "Engine.MeshAnimation" {
                let name = q.object_path(i as i32 + 1)?;
                let clips = if class == "Engine.MeshAnimation" {
                    Some(rc_package::skeletal_animation::read_mesh_animation(
                        &q, &d, e,
                    )?)
                } else {
                    None
                };
                entries.push(serde_json::json!({"name":name,"class":class,"animation":clips}));
            }
        }
        fs::write(
            out.join(format!("{file}.json")),
            serde_json::to_vec_pretty(&entries)?,
        )?;
    }
    let mut resolver = rc_inspect::assets::Assets::new(&[
        &root.join("Textures"),
        &root.join("Animations"),
        &root.join("System"),
    ])?;
    for (name, clips) in [
        (
            "Gunship",
            vec![
                "WarriorIntroGunshipLoop",
                "GunshipAmbushSpiderdroid",
                "GunshipAmbushSpiderdroidLoop",
            ],
        ),
        (
            "SpiderDroid",
            vec!["GunshipAmbushSpiderdroid", "DeathBreathe"],
        ),
    ] {
        let m = rc_inspect::visual_assets::load_from_animation(
            root,
            "vehicles.ukx",
            name,
            &clips,
            &mut resolver,
            Some(&format!("Vehicles.cGEO01{name}Set")),
        )?;
        let d = fs::read(root.join("Animations/vehicles.ukx"))?;
        let q = read_package(&d)?;
        let mut notifies = vec![];
        for clip in &m.clips {
            for n in &clip.sequence.notifies {
                if n.object_index > 0 {
                    let e = &q.exports[n.object_index as usize - 1];
                    notifies.push(serde_json::json!({"clip":clip.sequence.name.name,"time":f32::from_bits(n.time_bits),"class":q.object_path(e.class)?,"original":n.object_index,"properties":properties::read(&q,&d,e)?}));
                }
            }
        }
        fs::write(
            out.join(format!("{name}-mesh.json")),
            serde_json::to_vec(&m)?,
        )?;
        fs::write(
            out.join(format!("{name}-notifies.json")),
            serde_json::to_vec_pretty(&notifies)?,
        )?;
    }
    let warrior = rc_inspect::visual_assets::load(
        root,
        "Geonosian.ukx",
        "GeonosianWarrior",
        &["ActionBreathe", "RunForward", "Death", "hit", "Fire"],
        &mut resolver,
    )?;
    fs::write(out.join("Warrior-mesh.json"), serde_json::to_vec(&warrior)?)?;
    let ed = fs::read(root.join("Properties/cteffects.u"))?;
    let ep = read_package(&ed)?;
    let mut effect_rows = vec![];
    for (i, e) in ep.exports.iter().enumerate() {
        let name = ep.object_path(i as i32 + 1)?;
        if e.class == 0
            && [
                "SpiderDroid_BeamAutoDestroy",
                "GEO_01AProjectile",
                "REP_Gunship_GroundDust",
            ]
            .contains(&name.as_str())
        {
            let props = rc_package::classes::read(&ep, &ed, e)?.properties;
            let payload = ep.payload(&ed, e)?;
            let mut emitters = vec![];
            for prop in &props.values {
                if prop.name == "Emitters" {
                    for (id, path) in properties::object_array(
                        &ep,
                        &payload[prop.payload_offset..prop.payload_offset + prop.bytes],
                    )? {
                        emitters.push(serde_json::json!({"original":id,"name":path,"properties":properties::read(&ep,&ed,&ep.exports[id as usize-1])?}));
                    }
                }
            }
            effect_rows.push(serde_json::json!({"name":name,"parent":ep.object_path(e.super_class)?,"properties":props,"emitters":emitters}));
        }
    }
    fs::write(
        out.join("effect-originals.json"),
        serde_json::to_vec_pretty(&effect_rows)?,
    )?;
    let _ = Value::Int(0);
    Ok(())
}
