use rc_package::read_package;
use std::{env, fs, path::Path};
fn main() -> Result<(), Box<dyn std::error::Error>> {
    let a: Vec<_> = env::args_os().collect();
    let root = Path::new(&a[1]);
    let out = Path::new(&a[2]);
    fs::create_dir_all(out)?;
    let cat = rc_inspect::defaults::load(root)?;
    let mut rows = vec![];
    for class in [
        "CTCharacters.Commando62Delta",
        "CTCharacters.PlayerCommando",
        "CTGame.MarkerReviveOther",
        "CTInventory.DC17mBlaster",
        "CTInventory.DC17mBlasterAttachment",
        "CTInventory.DC17BlasterProj",
        "CTGame.StateGeonosianWarriorAttack",
    ] {
        for name in [
            "MyDamageVariance",
            "ShieldDamageVariance",
            "ArmorFactor",
            "ShieldRechargeRate",
            "ShieldRechargeDelay",
            "Health",
            "MaxHealth",
            "Shields",
            "MaxShields",
            "bCannotDie",
            "BleedOutTime",
            "Duration",
            "GroundSpeed",
            "CollisionRadius",
            "CollisionHeight",
            "Mesh",
            "StaticMesh",
            "Skins",
            "FireSound",
            "FireInterval",
            "FireRate",
            "ShotDelay",
            "Damage",
            "Speed",
            "MaxSpeed",
            "LifeSpan",
            "DrawScale",
            "Rotation",
            "CrouchHeight",
            "CrouchRadius",
            "CrouchSpeedRatio",
            "AirSpeed",
            "DefensiveJumpProbability",
            "DefensiveJumpProbabilityForPlayer",
        ] {
            if let Ok(v) = cat.resolve(class, name, 0) {
                rows.push(serde_json::json!({"class":class,"property":name,"resolved":v}));
            }
        }
    }
    fs::write(out.join("defaults.json"), serde_json::to_vec_pretty(&rows)?)?;
    let b = fs::read(root.join("Animations/Clone.ukx"))?;
    let p = read_package(&b)?;
    let mut anims = vec![];
    for (i, e) in p.exports.iter().enumerate() {
        if p.object_path(e.class)? == "Engine.MeshAnimation" {
            anims.push(serde_json::json!({"source":p.object_path(i as i32+1)?,"animation":rc_package::skeletal_animation::read_mesh_animation(&p,&b,e)?}));
        }
    }
    fs::write(
        out.join("clone-animations.json"),
        serde_json::to_vec(&anims)?,
    )?;
    Ok(())
}
