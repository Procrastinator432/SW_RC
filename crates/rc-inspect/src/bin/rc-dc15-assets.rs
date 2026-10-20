//! Export original DC-15s energy and fire defaults with provenance.
use std::{env, fs, path::Path};
fn main() -> Result<(), Box<dyn std::error::Error>> {
    let args: Vec<_> = env::args().collect();
    if args.len() != 3 {
        return Err("usage: rc-dc15-assets GAME_DATA OUTPUT.json".into());
    }
    let catalog = rc_inspect::defaults::load(Path::new(&args[1]))?;
    let mut values = serde_json::Map::new();
    for (class, fields) in [
        (
            "CTInventory.DC15Ammo",
            vec![
                "MaxAmmo",
                "Charge",
                "RechargeRate",
                "CoolDownTime",
                "bWaitForRecharge",
                "bCoolAndRecharge",
                "bNoRecharge",
                "Damage",
            ],
        ),
        (
            "CTInventory.DC15s",
            vec!["ShotDelay", "bCanReload", "bRapidFire", "AmmoName"],
        ),
        ("CTInventory.DC15Proj", vec!["Damage", "Speed", "LifeSpan"]),
    ] {
        for field in fields {
            values.insert(
                format!("{class}.{field}"),
                serde_json::to_value(catalog.resolve(class, field, 0))?,
            );
        }
    }
    fs::write(&args[2], serde_json::to_vec_pretty(&values)?)?;
    Ok(())
}
