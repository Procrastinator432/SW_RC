//! Replace the legacy baked station with its original animated instance.
use rc_package::{
    properties::{self, Value},
    read_package,
};
use std::{collections::HashSet, env, fs, path::Path};
fn key(t: &rc_package::geometry::Triangle) -> [u32; 9] {
    std::array::from_fn(|i| t.points[i / 3][i % 3].to_bits())
}
fn main() -> Result<(), Box<dyn std::error::Error>> {
    let args: Vec<_> = env::args().collect();
    if args.len() != 4 {
        return Err("usage: rc-bacta-build GAME_DATA SCENE OUT".into());
    }
    let root = Path::new(&args[1]);
    let out = Path::new(&args[3]);
    let raw = fs::read(&args[2])?;
    if !raw.starts_with(b"RCPLAY13") {
        return Err("Expected original opening scene".into());
    }
    let game = rc_game::bundle::decode(&raw)?;
    let size = u32::from_le_bytes(raw[8..12].try_into()?) as usize;
    let mut opening: rc_game::opening::Assets = serde_json::from_slice(&raw[12..12 + size])?;
    if opening.bacta.is_some() {
        return Err("Station is already animated".into());
    }
    let catalog = rc_inspect::defaults::load(root)?;
    let bytes = fs::read(root.join("Maps/geo_01a.ctm"))?;
    let map = read_package(&bytes)?;
    let props = properties::read(&map, &bytes, &map.exports[735])?;
    let get = |name| {
        catalog
            .instance(
                "MarkerProperties.BactaDispenserGEO",
                &props,
                name,
                0,
                "geo_01a",
            )
            .map(|p| p.value)
    };
    let Value::Rotator(rotation) = get("Rotation")? else {
        return Err("Station rotation".into());
    };
    let Value::Float(scalar) = get("DrawScale")? else {
        return Err("Station scale".into());
    };
    let Value::Vector(scale) = get("DrawScale3D")? else {
        return Err("Station scale3D".into());
    };
    let scale = scale.map(|v| v * scalar);
    let Value::Vector(pivot) = get("PrePivot")? else {
        return Err("Station pivot".into());
    };
    let Value::Float(height) = get("CollisionHeight")? else {
        return Err("Station height".into());
    };
    let Value::Vector(actor_location) = get("Location")? else {
        return Err("Station actor location".into());
    };
    let Value::Float(offset) = get("PlayerOffset")? else {
        return Err("Station player offset".into());
    };
    let Value::Float(player_range) = get("PlayerRange")? else {
        return Err("Station player range".into());
    };
    let Value::Float(marker_z) = get("HUDMarkerIconZOffset")? else {
        return Err("Station marker offset".into());
    };
    let Value::Bool(lock_location) = get("bLockPlayerLocation")? else {
        return Err("Station location lock".into());
    };
    let facing =
        rc_package::mesh::transform_point([offset, 0., 0.], [0.; 3], rotation, [1.; 3], [0.; 3]);
    let controller = rc_game::bacta::Controller {
        actor_location,
        desired_xy: std::array::from_fn(|i| actor_location[i] + facing[i]),
        marker_z,
        player_range,
        lock_location,
    };
    let mut resolver = rc_inspect::assets::Assets::new(&[
        &root.join("Animations"),
        &root.join("Textures"),
        &root.join("System"),
    ])?;
    // Reproduce the old import operation exactly to identify only its baked faces.
    let reference = rc_inspect::visual_assets::load(
        root,
        "bactadispensers.ukx",
        "BactaDispenserGEO",
        &[],
        &mut resolver,
    )?
    .prepare_reference()?;
    let mut channel = reference.channel(0, 0.)?;
    channel.words[14] = 0;
    let mut old = reference.scene(&channel)?;
    for t in &mut old.triangles {
        t.points = t
            .points
            .map(|p| rc_package::mesh::transform_point(p, [0.; 3], rotation, scale, pivot));
    }
    let min_z = old
        .triangles
        .iter()
        .flat_map(|t| t.points.iter().map(|p| p[2]))
        .fold(f32::INFINITY, f32::min);
    let station = game
        .mission
        .as_ref()
        .ok_or("Missing station objective")?
        .station
        .map(|v| v as f32);
    let location = [station[0], station[1], station[2] - height - min_z];
    for t in &mut old.triangles {
        for p in &mut t.points {
            p[0] += station[0];
            p[1] += station[1];
            p[2] += station[2] - height - min_z;
        }
    }
    let keys: HashSet<_> = old.triangles.iter().map(key).collect();
    let mesh = rc_inspect::visual_assets::load(
        root,
        "bactadispensers.ukx",
        "BactaDispenserGEO",
        &["Load", "Holster"],
        &mut resolver,
    )?;
    let anim = fs::read(root.join("Animations/bactadispensers.ukx"))?;
    let package = read_package(&anim)?;
    let mut notifies = vec![];
    let mut receipts = vec![];
    let mut bank: Vec<_> = game
        .mission
        .as_ref()
        .unwrap()
        .sounds
        .iter()
        .map(|s| s.source.clone())
        .collect();
    for clip in &mesh.clips {
        for notify in &clip.sequence.notifies {
            let export = &package.exports[notify.object_index as usize - 1];
            if package.object_path(export.class)? != "Engine.AnimNotify_Sound" {
                return Err("Unsupported Bacta notify".into());
            }
            let props = properties::read(&package, &anim, export)?;
            let Value::Object { path, index } = &props
                .values
                .iter()
                .find(|p| p.name == "Sound")
                .ok_or("Missing notify Sound")?
                .value
            else {
                return Err("Notify sound type".into());
            };
            if *index >= 0 {
                return Err("Expected external station sound".into());
            }
            let role = if let Some(i) = bank.iter().position(|s| s == path) {
                i
            } else {
                let (sound, _) =
                    rc_inspect::mission_assets::sound_variant(root, path, &mut receipts)
                        .map_err(|e| format!("Bacta sound {path}: {e}"))?;
                let role = bank.len();
                opening.sound_bindings.push(rc_game::opening::SoundBinding {
                    role,
                    source: path.clone(),
                    pcm: rc_game::replay::fingerprint(&sound.wave),
                });
                opening.sounds.push(sound);
                bank.push(path.clone());
                role
            };
            notifies.push(rc_game::bacta::Notify {
                clip: clip.sequence.name.name.clone(),
                original: notify.object_index as u32,
                time_bits: notify.time_bits,
                role,
            });
            receipts.push(serde_json::json!({"clip":clip.sequence.name.name,"original":notify.object_index,"time_bits":notify.time_bits,"role":role,"source":path}));
        }
    }
    opening.bacta = Some(rc_game::bacta::Assets {
        controller: Some(controller),
        mesh: Some(mesh),
        location,
        rotation,
        scale,
        pivot,
        notifies,
    });
    opening.provenance["bacta"] = serde_json::json!({"actor":736,"source":"ctgame.BactaDispenser.Active: PlayAnim Load, FinishAnim, bActivated; EndState Holster","notifies":receipts,"transform":"Preserved legacy bottom alignment and box collision adapter","limits":"Original Eye/body particle emitters and their ambient loop are not yet hosted; station recharge/reuse after objective is outside this bounded tutorial host"});
    opening.provenance["bacta"]["using_item"] = serde_json::json!({"source":"ctgame.CTPlayer.UsingItem.PlayerMove / engine.PlayerController.ProcessMove", "bindings":opening.bacta.as_ref().unwrap().controller, "acceleration":"15 * (DesiredLocation - Pawn.Location) - Pawn.Velocity; Z=0", "settle":"Dist <= 450 * dt: SetLocation if Dist>1, then zero velocity/acceleration", "rotator_units_per_second":45000, "boundary":"Original script controller on port collision/volume physics; quantized portable rotator math; target acquisition remains the bounded tutorial adapter; hand-signal weapon switch/network/ViewShake are not hosted"});
    let mut result = vec![];
    let mut at = 0;
    for _ in 0..16 {
        let header = raw.get(at..at + 12).ok_or("Truncated layer")?;
        let n = u32::from_le_bytes(header[8..12].try_into()?) as usize;
        let end = at.checked_add(12 + n).ok_or("Layer overflow")?;
        if at == 0 {
            let meta = serde_json::to_vec(&opening)?;
            result.extend_from_slice(&header[..8]);
            result.extend_from_slice(&(meta.len() as u32).to_le_bytes());
            result.extend(meta);
        } else {
            result.extend_from_slice(raw.get(at..end).ok_or("Truncated metadata")?);
        }
        at = end;
        if &header[..8] == b"RCPLAY01" {
            break;
        }
    }
    let mut scene = rc_render::Scene::from_snapshot(&raw[at..])?;
    let mut removed = 0;
    let mut found = HashSet::new();
    let mut triangles = vec![];
    let mut faces = vec![];
    for (t, f) in scene.triangles.into_iter().zip(scene.appearance) {
        let k = key(&t);
        if keys.contains(&k) {
            removed += 1;
            found.insert(k);
        } else {
            triangles.push(t);
            faces.push(f);
        }
    }
    if removed != old.triangles.len() || found != keys {
        return Err(format!(
            "Baked station face identity differs: {removed}/{}",
            old.triangles.len()
        )
        .into());
    }
    scene.triangles = triangles;
    scene.appearance = faces;
    result.extend(scene.snapshot()?);
    rc_game::bundle::decode(&result)?;
    fs::create_dir_all(out)?;
    fs::write(out.join("geo_01a.rcplay"), result)?;
    fs::write(
        out.join("bacta.json"),
        serde_json::to_vec_pretty(
            &serde_json::json!({"removed_baked_faces":removed,"original":opening.provenance["bacta"]}),
        )?,
    )?;
    println!("Replaced {removed} baked Bacta faces; bound six original sound notifies");
    Ok(())
}
