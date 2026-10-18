//! Read-only evidence for the selected injury/barrier presentation correction.
use rc_package::{properties, read_package};
use std::{env, fs, path::Path};
fn main() -> Result<(), Box<dyn std::error::Error>> {
    let args: Vec<_> = env::args().collect();
    if args.len() != 3 {
        return Err("usage: rc-presentation-assets GAME_DATA OUTPUT".into());
    }
    let root = Path::new(&args[1]);
    let out = Path::new(&args[2]);
    fs::create_dir_all(out)?;
    let data = fs::read(root.join("Maps/geo_01a.ctm"))?;
    let pkg = read_package(&data)?;
    let cat = rc_inspect::defaults::load(root)?;
    let mut rows = vec![];
    for (index, e) in pkg.exports.iter().enumerate() {
        let class = pkg.object_path(e.class)?;
        if ![617, 964, 497, 2397, 909, 910, 912, 913].contains(&(index + 1))
            && !class.ends_with("ProjectileFactory")
            && !class.ends_with("FXFactory")
        {
            continue;
        }
        let props = properties::read(&pkg, &data, e)?;
        let payload = pkg.payload(&data, e)?;
        let mut resolved = serde_json::Map::new();
        for field in [
            "Tag",
            "Event",
            "Location",
            "Rotation",
            "Prototype",
            "Health",
            "DamagedRatio",
            "InitialState",
            "ExplosionEffect",
            "ExplosionSounds",
            "ExplosionRadius",
            "ExplosionDamage",
            "DeathSpawnActorClass",
            "Speed",
            "LifeSpan",
            "SpawnMode",
            "Capacity",
            "MaxAlive",
            "InitialSpeed",
            "SpawnInterval",
            "Damage",
            "DamageRadius",
        ] {
            resolved.insert(
                field.into(),
                serde_json::to_value(cat.instance(&class, &props, field, 0, "geo_01a"))?,
            );
        }
        let nested:Vec<_>=props.values.iter().filter(|p|p.struct_name.is_some() || p.kind==9).map(|p|{
            let bytes=&payload[p.payload_offset..p.payload_offset+p.bytes];
            serde_json::json!({"name":p.name,"slot":p.array_index,"struct":properties::tagged_struct(&pkg,bytes),"array":properties::struct_array(&pkg,bytes),"raw":bytes})
        }).collect();
        rows.push(serde_json::json!({"original":index+1,"class":class,"properties":props,"resolved":resolved,"nested":nested}));
    }
    fs::write(
        out.join("injury-barriers.json"),
        serde_json::to_vec_pretty(&rows)?,
    )?;
    let mut resolver =
        rc_inspect::assets::Assets::new(&[&root.join("Textures"), &root.join("System")])?;
    for class in [
        "CTEffects.Explosion_Dirt_lrg",
        "CTEffects.Dirt_Explo_Lrg",
        "CTEffects.Dispenser_Landing_Dirt",
        "CTEffects.Fire_Medium_NoFire",
    ] {
        let result = rc_inspect::particle_assets::effect(root, &mut resolver, class);
        fs::write(
            out.join(format!("{class}.json")),
            serde_json::to_vec_pretty(&result)?,
        )?;
        let defaults: [_; 4] = ["SpawnSound", "AmbientSound", "LifeSpan", "AutoDestroy"]
            .map(|f| serde_json::json!({"property":f,"resolved":cat.resolve(class,f,0)}));
        fs::write(
            out.join(format!("{class}.defaults.json")),
            serde_json::to_vec_pretty(&defaults)?,
        )?;
    }
    Ok(())
}
