use rc_package::{
    quaternion_animation::{PortableQuaternionMath, QuaternionTrackHost},
    read_package,
    skeletal_animation::read_mesh_animation,
    skeletal_root_pose::RootTransform,
    skeletal_track::{sample_track, AnimationTrack},
};
use std::{env, fs};
fn fingerprint(t: &AnimationTrack) -> String {
    let mut bytes = vec![];
    bytes.extend(t.rotation_count_word.to_le_bytes());
    for key in &t.rotations {
        for v in key {
            bytes.extend(v.to_le_bytes());
        }
    }
    bytes.extend(t.position_count_word.to_le_bytes());
    for key in &t.positions {
        for v in key {
            bytes.extend(v.to_le_bytes());
        }
    }
    bytes.extend(t.position_scale_bits.to_le_bytes());
    bytes.extend(t.duration_count_word.to_le_bytes());
    bytes.extend(&t.durations);
    let hash = bytes.into_iter().fold(0xcbf29ce484222325u64, |h, b| {
        (h ^ u64::from(b)).wrapping_mul(0x100000001b3)
    });
    format!("{hash:016x}")
}
fn main() -> Result<(), Box<dyn std::error::Error>> {
    let args: Vec<_> = env::args_os().collect();
    if args.len() != 3 {
        return Err("usage: rc-animation-track-check ANIMATIONS_DIRECTORY REPORT.json".into());
    }
    let mut files = fs::read_dir(&args[1])?
        .map(|e| e.map(|e| e.path()))
        .collect::<Result<Vec<_>, _>>()?;
    files.sort();
    let mut objects = vec![];
    let (mut animation_count, mut sequence_count, mut track_count, mut errors) = (0, 0, 0, 0);
    for file in files
        .into_iter()
        .filter(|p| p.extension().is_some_and(|e| e.eq_ignore_ascii_case("ukx")))
    {
        let data = fs::read(&file)?;
        let pkg = read_package(&data)?;
        for (index, e) in pkg.exports.iter().enumerate() {
            if pkg.object_path(e.class)? != "Engine.MeshAnimation" {
                continue;
            }
            let object = pkg.object_path(index as i32 + 1)?;
            if pkg.summary.version < 151 {
                objects.push(serde_json::json!({"file":file,"object":object,"export_index":index+1,"version":pkg.summary.version,"status":"UnsupportedLegacyRotation"}));
                continue;
            }
            animation_count += 1;
            match read_mesh_animation(&pkg, &data, e) {
                Err(error) => {
                    errors += 1;
                    objects.push(serde_json::json!({"file":file,"object":object,"export_index":index+1,"version":pkg.summary.version,"error":error}));
                }
                Ok(mut a) => {
                    let mut summaries = vec![];
                    for s in &mut a.sequences {
                        sequence_count += 1;
                        track_count += s.tracks.len();
                        let fingerprints: Vec<_> = s.tracks.iter().map(fingerprint).collect();
                        let counts: Vec<_> = s
                            .tracks
                            .iter()
                            .map(|t| {
                                serde_json::json!([
                                    t.rotations.len(),
                                    t.positions.len(),
                                    t.durations.len()
                                ])
                            })
                            .collect();
                        let mut samples = vec![];
                        if let Some(track) = s.tracks.first() {
                            for time in [0., s.frames as f32 * 0.5, s.frames as f32] {
                                let mut root = RootTransform {
                                    rotation: [7; 4],
                                    position: [8; 3],
                                };
                                let result = sample_track(
                                    track,
                                    time,
                                    &mut root,
                                    &mut QuaternionTrackHost {
                                        math: PortableQuaternionMath,
                                    },
                                );
                                samples.push(serde_json::json!({"time_bits":time.to_bits(),"root":root,"result":result}));
                            }
                        }
                        s.tracks.clear();
                        summaries.push(serde_json::json!({"metadata":s,"track_fingerprints":fingerprints,"track_counts":counts,"first_track_samples":samples}));
                    }
                    objects.push(serde_json::json!({"file":file,"object":object,"export_index":index+1,"version":pkg.summary.version,
                        "native_offset":a.native_offset,"end_offset":a.end_offset,"tail_bytes":a.unparsed_tail_bytes,"word_28":a.word_28,"reference_bones":a.reference_bones,"sequences":summaries}));
                }
            }
        }
    }
    let report = serde_json::json!({"scope":"Original MeshAnimation exports, SWRC 151–159/licensee1; decoded archive tracks and three portable samples of each sequence's first stored track. Stored track zero is not yet mapped to mesh root. Legacy rotation conversion, skeletal mesh linkage, full pose and x86 math parity remain open.","animations":animation_count,"sequences":sequence_count,"tracks":track_count,"errors":errors,"objects":objects});
    fs::write(&args[2], serde_json::to_vec_pretty(&report)?)?;
    println!("{animation_count} animations, {sequence_count} sequences, {track_count} tracks, {errors} errors");
    if errors != 0 {
        return Err("original animation parse failures; see report".into());
    }
    Ok(())
}
