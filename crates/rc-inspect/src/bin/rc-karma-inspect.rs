//! Export the original selected KarmaProp and projectile defaults without a scene build.
use rc_package::{properties, properties::Value, read_package};
use std::{env, fs, path::Path};
fn main() -> Result<(), Box<dyn std::error::Error>> {
    let args: Vec<_> = env::args().collect();
    if !(3..=4).contains(&args.len()) {
        return Err("usage: rc-karma-inspect GAME_DATA OUT.json [SCENE]".into());
    }
    let root = Path::new(&args[1]);
    let catalog = rc_inspect::defaults::load(root)?;
    let data = fs::read(root.join("Maps/geo_01a.ctm"))?;
    let map = read_package(&data)?;
    let game = if args.len() == 4 {
        Some(rc_game::bundle::decode(&fs::read(&args[3])?)?)
    } else {
        None
    };
    let mut actors = vec![];
    for id in [909, 910, 912, 913] {
        let e = &map.exports[id - 1];
        let class = map.object_path(e.class)?;
        let props = properties::read(&map, &data, e)?;
        let mut row = serde_json::json!({"original":id,"class":class});
        if let Some(g) = &game {
            let obj = g
                .dynamic
                .as_ref()
                .unwrap()
                .objects
                .iter()
                .find(|o| o.original == id as u32)
                .unwrap();
            let bounds: [[f32; 3]; 2] = std::array::from_fn(|side| {
                std::array::from_fn(|axis| {
                    obj.triangles
                        .iter()
                        .flat_map(|t| t.points)
                        .map(|p| p[axis])
                        .fold(
                            if side == 0 {
                                f32::INFINITY
                            } else {
                                f32::NEG_INFINITY
                            },
                            if side == 0 { f32::min } else { f32::max },
                        )
                })
            });
            row["visual_bounds"] = serde_json::to_value(bounds)?;
            row["physical_binding"] = serde_json::to_value(
                g.opening
                    .as_ref()
                    .unwrap()
                    .assets
                    .rigid
                    .iter()
                    .find(|b| b.original == id as u32),
            )?;
        }
        for field in [
            "bKTakeShot",
            "DamageThreshold",
            "LifeTime",
            "Event",
            "Physics",
            "CollisionRadius",
            "Mass",
        ] {
            row[field] =
                serde_json::to_value(catalog.instance(&class, &props, field, 0, "geo_01a"))?;
        }
        actors.push(row);
    }
    let mut projectiles = vec![];
    for class in ["CTInventory.DC15Proj", "CTInventory.ThermalDetonatorProj"] {
        let damage_type = catalog.resolve(class, "MyDamageType", 0)?;
        let Value::Object { ref path, .. } = damage_type.value else {
            return Err("Missing damage type".into());
        };
        projectiles.push(serde_json::json!({"class":class,"damage_type":damage_type,"impulse":catalog.resolve(path,"KDamageImpulse",0)?,"up_kick":catalog.resolve(path,"KDeathUpKick",0)?}));
    }
    fs::write(
        &args[2],
        serde_json::to_vec_pretty(
            &serde_json::json!({"actors":actors,"projectiles":projectiles,"pawn_mass":catalog.resolve("CTGame.CTPawn","Mass",0)?}),
        )?,
    )?;
    Ok(())
}
