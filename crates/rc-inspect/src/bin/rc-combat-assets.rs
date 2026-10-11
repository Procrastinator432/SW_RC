//! Inspect the selected combat defaults and notify objects directly from local packages.
use rc_inspect::defaults;
use rc_package::{properties, read_package};
use std::{collections::BTreeMap, env, fs, path::Path};
fn main() -> Result<(), Box<dyn std::error::Error>> {
    let args: Vec<_> = env::args_os().collect();
    if args.len() != 3 {
        return Err("usage: rc-combat-assets GAME_DATA OUTPUT".into());
    }
    let game = Path::new(&args[1]);
    let catalog = defaults::load(game)?;
    let mut classes = BTreeMap::new();
    for class in [
        "CTInventory.DC17mBlaster",
        "CTInventory.DC17mBlasterAmmo",
        "CTCharacters.GeonosianWarrior",
        "CTInventory.GeonosianBlaster",
        "CTInventory.GeonosianBlasterAmmo",
        "CTInventory.DC17BlasterProj",
    ] {
        let mut values = BTreeMap::new();
        for property in [
            "AmmoName",
            "ReloadCount",
            "InitAmmoAmount",
            "ShotDelay",
            "FireMode",
            "FireSound",
            "FireAnim",
            "bCanReload",
            "MaxAmmo",
            "bInstantHit",
            "ProjectileClass",
            "Damage",
            "Range",
            "DamageRadius",
            "DamageDampening",
            "ProjPerFire",
            "AmmunitionType",
            "Spread",
            "MyDamageType",
            "PannerDigit1",
            "PannerDigit10",
            "PannerDigit100",
            "Health",
            "GroundSpeed",
            "SightRadius",
            "PeripheralVision",
            "CollisionRadius",
            "CollisionHeight",
            "WeaponType",
            "DefaultWeapon",
            "DamageScaling",
            "DeathSound",
            "HitSound",
            "PainSound",
            "Speed",
            "MaxSpeed",
            "AccelFactor",
            "bLaserBolt",
            "LifeSpan",
            "SpawnSound",
            "ImpactSound",
            "Physics",
            "bCollideWorld",
            "bProjTarget",
        ] {
            values.insert(property, catalog.resolve(class, property, 0));
        }
        classes.insert(class, values);
    }
    let mut objects = vec![];
    for file in ["hudarms.ukx", "geonosian.ukx"] {
        let bytes = fs::read(game.join("Animations").join(file))?;
        let pkg = read_package(&bytes)?;
        for (i, e) in pkg.exports.iter().enumerate() {
            let class = pkg.object_path(e.class)?;
            if class.contains("AnimNotify") {
                objects.push(serde_json::json!({"package":file,"index":i+1,"path":pkg.object_path(i as i32+1)?,"class":class,"properties":properties::read(&pkg,&bytes,e)?}));
            }
        }
    }
    let bytes = fs::read(game.join("Sounds/gen_sound.uax"))?;
    let pkg = read_package(&bytes)?;
    let mut sounds = vec![];
    for (i, e) in pkg.exports.iter().enumerate() {
        let path = pkg.object_path(i as i32 + 1)?;
        if [
            "wep_GEN_dc17Fire_02",
            "fol_GEN_dc17reload_01",
            "geo_BFprim",
            "geo_BFsec",
        ]
        .iter()
        .any(|s| path.ends_with(s))
        {
            let payload = pkg.payload(&bytes, e)?;
            fs::write(
                Path::new(&args[2])
                    .parent()
                    .unwrap()
                    .join(format!("{}.sound", e.name)),
                payload,
            )?;
            sounds.push(serde_json::json!({"path":path,"class":pkg.object_path(e.class)?,"size":payload.len(),"prefix":&payload[..payload.len().min(160)],"properties":properties::read(&pkg,&bytes,e)?}));
        }
    }
    fs::write(
        &args[2],
        serde_json::to_vec_pretty(
            &serde_json::json!({"classes":classes,"notifies":objects,"sounds":sounds}),
        )?,
    )?;
    Ok(())
}
