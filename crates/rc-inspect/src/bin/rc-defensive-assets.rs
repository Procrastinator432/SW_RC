//! Inspect original defensive-jump defaults and map node eligibility.
use rc_inspect::defaults;
use rc_package::{properties, read_package};
use std::{env, fs, path::Path};
fn main() -> Result<(), Box<dyn std::error::Error>> {
    let args: Vec<_> = env::args_os().collect();
    if args.len() != 3 {
        return Err("usage: rc-defensive-assets GAME_DATA OUTPUT_DIRECTORY".into());
    }
    let data = Path::new(&args[1]);
    let out = Path::new(&args[2]);
    fs::create_dir_all(out)?;
    let catalog = defaults::load(data)?;
    let class = "CTGame.StateGeonosianWarriorAttack";
    let probability = catalog.resolve(class, "DefensiveJumpProbability", 0)?;
    let player_probability = catalog.resolve(class, "DefensiveJumpProbabilityForPlayer", 0)?;
    let bytes = fs::read(data.join("Maps/geo_01a.ctm"))?;
    let pkg = read_package(&bytes)?;
    let level = pkg
        .exports
        .iter()
        .find(|e| {
            pkg.object_path(e.class)
                .is_ok_and(|c| c == "Engine.LevelInfo")
        })
        .ok_or("Missing LevelInfo")?;
    let level_props = properties::read(&pkg, &bytes, level)?;
    let head = catalog.instance(
        "Engine.LevelInfo",
        &level_props,
        "NavigationPointList",
        0,
        "geo_01a",
    )?;
    let properties::Value::Object {
        index: mut next, ..
    } = head.value
    else {
        return Err("Invalid navigation head".into());
    };
    let mut linked = vec![];
    let mut seen = std::collections::BTreeSet::new();
    while next != 0 {
        if next < 0 || linked.len() >= 4096 || !seen.insert(next) {
            return Err("Invalid original navigation chain".into());
        }
        let e = pkg
            .exports
            .get(next as usize - 1)
            .ok_or("Invalid navigation reference")?;
        let class = pkg.object_path(e.class)?;
        if !catalog.derives_from(&class, "Engine.NavigationPoint")? {
            return Err("Non-navigation actor in linked list".into());
        }
        linked
            .push(serde_json::json!({"original":next,"name":pkg.object_path(next)?,"class":class}));
        let props = properties::read(&pkg, &bytes, e)?;
        let properties::Value::Object { index, .. } = catalog
            .instance(&class, &props, "nextNavigationPoint", 0, "geo_01a")?
            .value
        else {
            return Err("Invalid navigation next reference".into());
        };
        next = index;
    }
    let mut nodes = vec![];
    for (i, e) in pkg.exports.iter().enumerate() {
        let class = pkg.object_path(e.class)?;
        if !catalog
            .derives_from(&class, "Engine.Perch")
            .unwrap_or(false)
            && !catalog
                .derives_from(&class, "Engine.JumpDest")
                .unwrap_or(false)
        {
            continue;
        }
        let props = properties::read(&pkg, &bytes, e)?;
        let mut values = serde_json::Map::new();
        for field in [
            "Location",
            "bEnabled",
            "bBlocked",
            "referenceCount",
            "MostRecentOccupant",
            "nextNavigationPoint",
        ] {
            values.insert(
                field.into(),
                serde_json::to_value(catalog.instance(&class, &props, field, 0, "geo_01a")?)?,
            );
        }
        nodes.push(serde_json::json!({"original":i+1,"name":pkg.object_path(i as i32+1)?,"class":class,"values":values}));
    }
    let unlinked: Vec<_> = nodes
        .iter()
        .filter(|n| !seen.contains(&(n["original"].as_i64().unwrap() as i32)))
        .map(|n| n["name"].clone())
        .collect();
    fs::write(
        out.join("defensive-receipt.json"),
        serde_json::to_vec_pretty(
            &serde_json::json!({"probability":probability,"player_probability":player_probability,"nodes":nodes,"navigation_head":head,"original_linked_order":linked,"unlinked_special_nodes":unlinked,"scope":"Original serialized defaults and navigation chain; live availability, zone/anchor queries, reachability and goal execution remain prepared boundaries; no scene modification."}),
        )?,
    )?;
    Ok(())
}
