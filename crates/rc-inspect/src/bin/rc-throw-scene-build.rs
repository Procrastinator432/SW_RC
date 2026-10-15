//! Extend the repaired pickup scene with an original DC15s throw channel and live flight.
use rc_game::{bundle, throw_runtime, visual};
use rc_package::read_package;
use std::{env, fs, path::Path};
fn wave(root: &Path, path: &str) -> Result<rc_game::combat::Sound, Box<dyn std::error::Error>> {
    let (package, object) = path.split_once('.').ok_or("Invalid sound path")?;
    let data = fs::read(root.join("Sounds").join(format!("{package}.uax")))?;
    let pkg = read_package(&data)?;
    let e = pkg
        .exports
        .iter()
        .enumerate()
        .find_map(|(i, e)| {
            pkg.object_path(i as i32 + 1)
                .is_ok_and(|p| p.eq_ignore_ascii_case(object))
                .then_some(e)
        })
        .ok_or("Missing original sound")?;
    let payload = pkg.payload(&data, e)?;
    let at = payload
        .windows(4)
        .position(|v| v == b"RIFF")
        .ok_or("No wave")?;
    let size = u32::from_le_bytes(
        payload
            .get(at + 4..at + 8)
            .ok_or("Short wave")?
            .try_into()?,
    ) as usize
        + 8;
    let wave = payload.get(at..at + size).ok_or("Truncated wave")?.to_vec();
    rc_game::combat::validate_wave(&wave)?;
    Ok(rc_game::combat::Sound {
        source: path.into(),
        wave,
    })
}
fn main() -> Result<(), Box<dyn std::error::Error>> {
    let args: Vec<_> = env::args_os().collect();
    if args.len() != 4 {
        return Err("usage: rc-throw-scene-build GAME_DATA REPAIRED_SCENE OUTPUT".into());
    }
    let root = Path::new(&args[1]);
    let out = Path::new(&args[3]);
    fs::create_dir_all(out)?;
    let source = fs::read(&args[2])?;
    // Preserve all original mission metadata and the corrected static snapshot.
    if !source.starts_with(b"RCPLAY04") {
        return Err("Expected pickup mission".into());
    }
    let n = u32::from_le_bytes(source[8..12].try_into()?) as usize;
    let animated = &source[12 + n..];
    if !animated.starts_with(b"RCPLAY02") {
        return Err("Expected animated base".into());
    }
    let count = u32::from_le_bytes(animated[8..12].try_into()?) as usize;
    let mut visuals: visual::Assets = serde_json::from_slice(&animated[12..12 + count])?;
    visuals.weapon = serde_json::from_slice(&fs::read(
        "analysis/mission4-flight/throw-visual-assets.json",
    )?)?;
    let base = bundle::encode_animated(&animated[12 + count..], &visuals)?;
    let mut mission = source[..12 + n].to_vec();
    mission.extend(base);
    let trace: serde_json::Value = serde_json::from_slice(&fs::read(
        "analysis/mission4-world-flight/world-flight-trace.json",
    )?)?;
    let a = throw_runtime::Assets {
        animation: serde_json::from_slice(&fs::read(
            "analysis/mission4-flight/throw-assets.json",
        )?)?,
        projectile: serde_json::from_slice(&fs::read(
            "analysis/mission4-detonator/detonator-assets.json",
        )?)?,
        flight: serde_json::from_slice(&fs::read(
            "analysis/mission4-world-flight/flight-assets.json",
        )?)?,
        gun: serde_json::from_slice(&fs::read("analysis/mission4-detonator/gunner-assets.json")?)?,
        offset: serde_json::from_value(
            serde_json::from_slice::<serde_json::Value>(&fs::read(
                "analysis/mission4-world-flight/world-validation.json",
            )?)?["original_release_offset"]
                .clone(),
        )?,
        route: serde_json::from_value(trace["route"].clone())?,
        snapshot: fs::read("analysis/mission4-detonator/detonator.rcscene")?,
        foley_frames: [f32::from_bits(940614037), f32::from_bits(1064039965)],
        sounds: [
            "GEN_Sound.Foley_Movement.fol_GEN_grenadeThrow_01",
            "GEN_Sound.Foley_Movement.fol_GEN_wepGrab_02",
            "GEN_Sound.Impacts_Explos.imp_GEN_grenadeBounce_01",
            "GEN_Sound.Interface.int_GEN_selGrnEmpty_01",
            "GEO_Voice.GEO_01.CA1G1716",
        ]
        .iter()
        .map(|s| wave(root, s))
        .collect::<Result<_, _>>()?,
    };
    let bytes = bundle::encode_throw(&mission, &a)?;
    let game = bundle::decode(&bytes)?;
    let file = out.join("geo_01a-throw.rcplay");
    fs::write(&file, &bytes)?;
    fs::write(
        out.join("build-validation.json"),
        serde_json::to_vec_pretty(&serde_json::json!({
            "passed":true,"bytes":bytes.len(),"format":"RCPLAY05","sound_roles":game.mission.as_ref().unwrap().sounds.iter().map(|s| &s.source).collect::<Vec<_>>(),
            "original_mission_metadata_unchanged":true,"initial_state":game.state,
            "scope":"Live throw preparation; original bunker destruction not integrated, no Android acceptance"
        }))?,
    )?;
    println!("Built {} bytes: {}", bytes.len(), file.display());
    Ok(())
}
