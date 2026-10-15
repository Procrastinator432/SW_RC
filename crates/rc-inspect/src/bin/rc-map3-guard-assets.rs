use rc_package::{properties, read_package};
use std::{fs, path::Path};
fn main() -> Result<(), Box<dyn std::error::Error>> {
    let root = Path::new("D:/SteamLibrary/steamapps/common/Star Wars Republic Commando/GameData");
    let out = Path::new("analysis/mission23-map3-guard");
    fs::create_dir_all(out)?;
    let catalog = rc_inspect::defaults::load(root)?;
    let mut defaults = vec![];
    for class in [
        "CTCharacters.SuperBattleDroid",
        "Properties.SuperBattleDroid_NoDetach",
        "CTCharacters.BattleDroid",
    ] {
        for key in [
            "Mesh",
            "Health",
            "CollisionRadius",
            "CollisionHeight",
            "GroundSpeed",
            "bInvulnerable",
        ] {
            defaults.push(serde_json::json!({"class":class,"property":key,"resolved":catalog.resolve(class,key,0)}));
        }
    }
    let data = fs::read(root.join("Animations/droid.ukx"))?;
    let p = read_package(&data)?;
    let mut clips = vec![];
    for (i, e) in p.exports.iter().enumerate() {
        if p.object_path(e.class)? != "Engine.MeshAnimation" {
            continue;
        }
        let a = rc_package::skeletal_animation::read_mesh_animation(&p, &data, e)?;
        for s in a.sequences {
            if !["IntroSBD", "IntroSBDRightPush", "IntroSBDLeftSmash"]
                .contains(&s.name.name.as_str())
            {
                continue;
            }
            let mut notifies = vec![];
            for n in &s.notifies {
                let e = &p.exports[n.object_index as usize - 1];
                notifies.push(serde_json::json!({"original":n.object_index,"time_bits":n.time_bits,"class":p.object_path(e.class)?,"properties":properties::read(&p,&data,e)?}));
            }
            clips.push(serde_json::json!({"set":p.object_path(i as i32+1)?,"sequence":s,"notifies":notifies}));
        }
    }
    fs::write(
        out.join("original-assets.json"),
        serde_json::to_vec_pretty(&serde_json::json!({"defaults":defaults,"clips":clips}))?,
    )?;
    println!("{}", serde_json::to_string_pretty(&defaults)?);
    for c in clips {
        println!(
            "{} {} frames={} flags={}",
            c["set"], c["sequence"]["name"], c["sequence"]["frames"], c["sequence"]["flags"]
        );
    }
    Ok(())
}
