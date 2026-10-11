//! Add the selected original fire origin and prepared native projectile arithmetic.
use rc_game::{bundle, combat};
use rc_inspect::defaults;
use rc_package::properties::Value;
use std::{env, fs, path::Path};
fn main() -> Result<(), Box<dyn std::error::Error>> {
    let args: Vec<_> = env::args_os().collect();
    if args.len() != 4 {
        return Err("usage: rc-play-ballistics GAME_DATA MELEE_SCENE OUTPUT_DIRECTORY".into());
    }
    let bytes = fs::read(&args[2])?;
    if !bytes.starts_with(b"RCPLAY03") || bytes.len() < 12 {
        return Err("Need combat scene".into());
    }
    let n = u32::from_le_bytes(bytes[8..12].try_into()?) as usize;
    let mut assets: combat::Assets =
        serde_json::from_slice(bytes.get(12..12 + n).ok_or("Truncated combat layer")?)?;
    if assets.ballistics.is_some() || assets.melee.is_none() {
        return Err("Need unextended melee scene".into());
    }
    let catalog = defaults::load(Path::new(&args[1]))?;
    let mut receipt = serde_json::Map::new();
    for (class, names) in [
        ("CTInventory.DC17mBlaster", vec!["FPFireOffset", "AimError"]),
        (
            "CTInventory.DC17mBlasterAmmo",
            vec![
                "Damage",
                "DamageRadius",
                "bTossed",
                "MyDamageType",
                "bInstantHit",
                "ProjectileClass",
                "ProjPerFire",
            ],
        ),
        (
            "CTInventory.DC17BlasterProj",
            vec![
                "Speed",
                "MaxSpeed",
                "AccelFactor",
                "bLaserBolt",
                "LifeSpan",
                "CollisionRadius",
                "CollisionHeight",
                "MyDamageType",
            ],
        ),
    ] {
        for name in names {
            receipt.insert(
                format!("{class}.{name}"),
                serde_json::to_value(catalog.resolve(class, name, 0)?)?,
            );
        }
    }
    let offset = match catalog
        .resolve("CTInventory.DC17mBlaster", "FPFireOffset", 0)?
        .value
    {
        Value::Vector(v) => v,
        _ => return Err("Expected vector fire offset".into()),
    };
    let ammo = "CTInventory.DC17mBlasterAmmo";
    let damage = match catalog.resolve(ammo, "Damage", 0)?.value {
        Value::Int(v) if v >= 0 => v as u32,
        Value::Float(v) if v >= 0. && v.fract() == 0. => v as u32,
        _ => return Err("Invalid selected proximity damage".into()),
    };
    for (name, expected) in [("bTossed", false), ("bInstantHit", false)] {
        if !matches!(catalog.resolve(ammo,name,0)?.value,Value::Bool(v) if v==expected) {
            return Err("Unsupported ammunition mode".into());
        }
    }
    if !matches!(
        catalog.resolve(ammo, "DamageRadius", 0)?.value,
        Value::Float(0.) | Value::Int(0)
    ) {
        return Err("Proximity area damage requires a radius host".into());
    }
    let type_path = |class| -> Result<String, String> {
        match catalog.resolve(class, "MyDamageType", 0)?.value {
            Value::Object { path, .. } => Ok(path),
            _ => Err("Invalid damage type".into()),
        }
    };
    if !type_path(ammo)?.eq_ignore_ascii_case(&type_path("CTInventory.DC17BlasterProj")?) {
        return Err("Distinct proximity damage type needs another profile".into());
    }
    assets.ballistics = Some(combat::Ballistics {
        fire_offset: offset,
        proximity_damage: damage,
        provenance: serde_json::json!({"defaults":receipt,"motion":"engine.dll physProjectile non-bouncing arithmetic; AProjectile BoundProjectileVelocity; portable 1/sqrt seed before native Newton correction","spawn":"human unzoomed Weapon.GetFireStart; non-tossed zero-radius Ammunition.CreateProximityProjectile","remaining_adapters":"prepared camera/eye; static world and box actor trace; no PickTarget/bone assistance or stochastic aim error, recoil, bone trace, reflection, particles or GameRules"}),
    });
    let result = bundle::encode_combat(
        bytes.get(12 + n..).ok_or("Missing animated layer")?,
        &assets,
    )?;
    bundle::decode(&result)?;
    let out = Path::new(&args[3]);
    fs::create_dir_all(out)?;
    fs::write(
        out.join("ballistics-receipt.json"),
        serde_json::to_vec_pretty(&assets.ballistics)?,
    )?;
    fs::write(out.join("geo_01a-ballistics.rcplay"), result)?;
    println!("Fire offset {offset:?}; proximity damage {damage}");
    Ok(())
}
