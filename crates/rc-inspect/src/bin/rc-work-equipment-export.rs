//! Compile original work sounds and charge appearance from explicit assets.
use rc_package::mesh_query::ActorTransform;
use std::{
    collections::{BTreeMap, BTreeSet},
    env, fs,
    path::Path,
};
fn key(t: &rc_package::geometry::Triangle) -> Vec<[i64; 3]> {
    let mut p: Vec<_> = t
        .points
        .iter()
        .map(|p| p.map(|v| (v as f64 * 512.).round() as i64))
        .collect();
    p.sort();
    p
}
fn main() -> Result<(), Box<dyn std::error::Error>> {
    let a: Vec<_> = env::args_os().collect();
    if a.len() != 6 {
        return Err(
            "usage: rc-work-equipment-export GAME_DATA SCENE INVENTORY NOTIFIES OUTPUT".into(),
        );
    }
    let root = Path::new(&a[1]);
    let scene = fs::read(&a[2])?;
    let rows: Vec<serde_json::Value> = serde_json::from_slice(&fs::read(&a[3])?)?;
    let notifies: Vec<serde_json::Value> = serde_json::from_slice(&fs::read(&a[4])?)?;
    let out = Path::new(&a[5]);
    fs::create_dir_all(out)?;
    let game = rc_game::bundle::decode(&scene)?;
    let map = game.map.as_ref().ok_or("Missing map")?;
    let g = map.gameplay.as_ref().ok_or("Missing B host")?;
    let catalog = rc_inspect::defaults::load(root)?;
    let mut receipts = vec![];
    let mut sounds = vec![];
    let mut bindings = BTreeMap::new();
    let mut parameters = vec![];
    for n in &notifies {
        if n["class"] != "Engine.AnimNotify_Sound" {
            continue;
        }
        let path = n["properties"]["values"]
            .as_array()
            .unwrap()
            .iter()
            .find(|p| p["name"] == "Sound")
            .ok_or("Missing notify Sound")?["value"]["value"]["path"]
            .as_str()
            .ok_or("Null sound")?;
        if bindings.contains_key(path) {
            continue;
        }
        let mut selection = vec![];
        let (sound, _) = rc_inspect::mission_assets::sound_variant(root, path, &mut selection)?;
        let leaf = selection
            .last()
            .and_then(|r| r["children"].as_array())
            .and_then(|c| c.first())
            .and_then(|s| s.as_str())
            .unwrap_or(path);
        let (params, receipt) = rc_inspect::audio_params::load(root, &catalog, leaf)?;
        let playback = params.select(&mut 0).ok_or("Silent work sound")?;
        let index = map.assets.sounds.len() + sounds.len();
        bindings.insert(path.to_string(), index);
        sounds.push(sound);
        parameters.push(serde_json::json!({"sound":index,"playback":playback}));
        receipts.push(serde_json::json!({"group":path,"leaf":leaf,"selection":selection,"params":receipt,"policy":"First original variant and fixed local parameter selection; not native global random selection"}));
    }
    let mut schedules = vec![];
    let mut placements = vec![];
    for (i, job) in g.assets.interactions.iter().enumerate() {
        let p = &g.work_animations.as_ref().ok_or("Missing work clips")?[i];
        let mesh = if job.demolition {
            &map.squad
        } else {
            &g.meshes[2].0
        };
        let mut schedule = vec![];
        let mut place = None;
        for phase in 0..3 {
            let c = &mesh.mesh.clips[p.clips[phase]];
            for n in &c.sequence.notifies {
                let row = notifies
                    .iter()
                    .find(|r| {
                        r["id"] == n.object_index
                            && r["clip"] == c.source
                            && r["time_bits"] == n.time_bits
                    })
                    .ok_or("Unaudited work notify")?;
                let time = f32::from_bits(n.time_bits) as f64;
                let times: Vec<_> = match phase {
                    0 => vec![time * p.seconds[0]],
                    1 => (0..p.cycles)
                        .map(|cycle| {
                            p.seconds[0]
                                + (cycle as f64 + time) * (p.work - p.seconds[0] - p.seconds[2])
                                    / p.cycles as f64
                        })
                        .collect(),
                    _ => vec![p.work - p.seconds[2] + time * p.seconds[2]],
                };
                if row["class"] == "Engine.AnimNotify_Sound" {
                    let path = row["properties"]["values"]
                        .as_array()
                        .unwrap()
                        .iter()
                        .find(|p| p["name"] == "Sound")
                        .unwrap()["value"]["value"]["path"]
                        .as_str()
                        .unwrap();
                    for at in times {
                        schedule.push(serde_json::json!({"original":n.object_index,"at":at,"sound":bindings[path]}));
                    }
                } else if row["class"] == "Engine.AnimNotify_Trigger"
                    && job.demolition
                    && phase == 0
                    && row["properties"]["values"][0]["value"]["value"] == "MDplaceCharge"
                {
                    place = Some(times[0]);
                } else {
                    return Err("Unsupported work notify dispatch".into());
                }
            }
        }
        schedule.sort_by(|a, b| {
            a["at"]
                .as_f64()
                .unwrap()
                .total_cmp(&b["at"].as_f64().unwrap())
        });
        schedules.push(schedule);
        placements.push(place);
    }
    let identity = ActorTransform {
        location: [0.; 3],
        rotation: [0; 3],
        scale: [1.; 3],
        pivot: [0.; 3],
    };
    let mut mesh_receipts = vec![];
    let inactive = rc_inspect::mission_assets::mesh_scene_skinned(
        root,
        "MarkerIcons.Demolition.DemolitionCharge",
        &identity,
        &catalog,
        &mut mesh_receipts,
        Some("MarkerIconTextures.Demolition.DemolitionCharge_Inactive"),
    )?;
    let active = rc_inspect::mission_assets::mesh_scene_skinned(
        root,
        "MarkerIcons.Demolition.DemolitionCharge",
        &identity,
        &catalog,
        &mut mesh_receipts,
        Some("MarkerIconTextures.Demolition.DemolitionCharge_Active"),
    )?;
    let mut lookup: BTreeMap<_, Vec<usize>> = BTreeMap::new();
    for (i, t) in game.scene.triangles.iter().enumerate() {
        lookup.entry(key(t)).or_default().push(i);
    }
    let mut used = BTreeSet::new();
    let mut charges = vec![];
    for (i, job) in g
        .assets
        .interactions
        .iter()
        .enumerate()
        .filter(|(_, j)| j.demolition)
    {
        let row = rows
            .iter()
            .find(|r| r["original"] == job.original)
            .ok_or("Missing charge actor")?;
        let get = |name: &str| {
            row["properties"]["values"]
                .as_array()
                .unwrap()
                .iter()
                .find(|p| p["name"] == name)
                .map(|p| &p["value"]["value"])
        };
        let pos: [f64; 3] =
            serde_json::from_value(get("Location").ok_or("Missing charge location")?.clone())?;
        let rot: [i32; 3] = get("Rotation")
            .map(|p| serde_json::from_value(p.clone()))
            .transpose()?
            .unwrap_or([0; 3]);
        let transform = ActorTransform {
            location: pos.map(|v| v as f32),
            rotation: rot,
            scale: [1.; 3],
            pivot: [0.; 3],
        };
        let world = rc_inspect::mission_assets::mesh_scene(
            root,
            "MarkerIcons.Demolition.DemolitionCharge",
            &transform,
            &catalog,
            &mut mesh_receipts,
        )?;
        let mut indices = vec![];
        for t in &world.triangles {
            let k = key(t);
            let index = lookup
                .get(&k)
                .and_then(|v| v.iter().find(|i| !used.contains(*i)))
                .copied()
                .ok_or("Original charge triangle not found in current base")?;
            used.insert(index);
            indices.push(index);
        }
        charges.push(
            serde_json::json!({"job":i,"position":pos,"rotation":rot,"static_triangles":indices}),
        );
    }
    let assets = serde_json::json!({"schedules":schedules,"placed_at":placements,"sounds":parameters,"charges":charges,"models":[inactive.snapshot()?,active.snapshot()?]});
    fs::write(out.join("equipment.json"), serde_json::to_vec(&assets)?)?;
    fs::write(out.join("sounds.json"), serde_json::to_vec(&sounds)?)?;
    let report = serde_json::json!({"passed":true,"sound_paths":bindings,"sound_receipts":receipts,"mesh_receipts":mesh_receipts,"charge_triangles":used.len(),"events_per_job":schedules.iter().map(Vec::len).collect::<Vec<_>>(),"placements":placements,"scope":"Original work notifies; first original sound variant; original inactive/active charge skins and exact matched world triangles; no invented hack tool or particle effects"});
    fs::write(
        out.join("validation.json"),
        serde_json::to_vec_pretty(&report)?,
    )?;
    println!(
        "{}",
        serde_json::to_string(
            &serde_json::json!({"passed":true,"events_per_job":report["events_per_job"],"charge_triangles":used.len(),"sounds":sounds.len()})
        )?
    );
    Ok(())
}
