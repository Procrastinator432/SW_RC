//! Static PhysicsVolume classification, original brush containment and priority audit.
use rc_package::{level, read_package};
use std::{env, fs, path::Path};
#[path = "../defaults.rs"]
mod defaults;
#[path = "../volumes.rs"]
mod volumes_load;
fn main() -> Result<(), Box<dyn std::error::Error>> {
    let args: Vec<_> = env::args_os().collect();
    if args.len() != 3 {
        return Err("Usage: rc-volume-check <GameData> <report.json>".into());
    }
    let game = Path::new(&args[1]);
    let catalog = defaults::load(game)?;
    let profile = rc_package::movement_profile::MovementProfile::read(&catalog)?;
    let mut maps = Vec::new();
    let mut default = None;
    let mut failures = 0;
    let mut total = 0;
    let mut starts_total = 0;
    let mut files: Vec<_> = fs::read_dir(game.join("Maps"))?
        .map(|e| e.map(|e| e.path()))
        .collect::<Result<_, _>>()?;
    files.sort();
    for file in files
        .into_iter()
        .filter(|f| f.extension().is_some_and(|e| e.eq_ignore_ascii_case("ctm")))
    {
        let data = fs::read(&file)?;
        let pkg = read_package(&data)?;
        let source = file.to_string_lossy();
        let loaded = volumes_load::load(&pkg, &data, &catalog, &source, profile.physics)?;
        failures += loaded.errors;
        total += loaded.actors.len();
        default.get_or_insert_with(|| loaded.world.default.clone());
        let world = loaded.world;
        let actors = loaded.actors;
        let unidentified = loaded.unidentified;
        let starts = level::player_starts_with_defaults(&pkg, &data, &catalog, &source)?;
        starts_total += starts.len();
        let mut selections = Vec::new();
        for start in starts {
            let result = world.select(start.location)?;
            failures += usize::from(!result.complete);
            let physics = if result.complete {
                Some(result.settings.apply(profile.physics))
            } else {
                None
            };
            selections.push(
                serde_json::json!({"start":start.actor,"position":start.location,"result":result,"pc_physics_policy":physics}),
            );
        }
        maps.push(serde_json::json!({"map":source,"volumes":actors,"unidentified_volume_classes":unidentified,"selections":selections}));
    }
    if let Some(parent) = Path::new(&args[2]).parent() {
        fs::create_dir_all(parent)?;
    }
    fs::write(
        &args[2],
        serde_json::to_vec_pretty(
            &serde_json::json!({"maps":maps,"volume_actors":total,"starts":starts_total,"incomplete":failures,"default":default,"scope":"Static classified PhysicsVolume-derived Actors at serialized pose; center containment plus priority. No runtime hash/touch ordering, moving volumes, callbacks, skeletal bases, global unknown class guarantee or native float parity. Tied priorities reported incomplete; water/zone effects only recorded."}),
        )?,
    )?;
    println!("{total} volume actors; {starts_total} centers; {failures} incomplete results");
    if failures > 0 {
        return Err("Volume audit incomplete; see report".into());
    }
    Ok(())
}
