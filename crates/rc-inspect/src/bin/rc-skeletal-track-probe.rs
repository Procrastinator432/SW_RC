//! Synthetic compressed tracks; rotation results are supplied diagnostic values.
use rc_package::{
    skeletal_root_pose::RootTransform,
    skeletal_track::{sample_track, AnimationTrack, TrackRotationHost},
};
use std::{env, fs};
#[derive(Default)]
struct Host {
    events: Vec<serde_json::Value>,
    mode: usize,
}
impl TrackRotationHost for Host {
    fn decode(&mut self, key: [u16; 3]) -> Result<[u32; 4], String> {
        self.events.push(serde_json::json!({"decode":key}));
        if self.mode == 8 {
            Err("unresolved rotation decode".into())
        } else {
            Ok([key[0] as u32, key[1] as u32, key[2] as u32, 42])
        }
    }
    fn slerp(&mut self, a: [u32; 4], b: [u32; 4], t: f32) -> Result<[u32; 4], String> {
        self.events
            .push(serde_json::json!({"slerp":[a,b],"alpha_bits":t.to_bits()}));
        if self.mode == 9 {
            Err("unresolved Slerp".into())
        } else {
            Ok([a[0], b[0], t.to_bits(), 43])
        }
    }
}
fn main() -> Result<(), Box<dyn std::error::Error>> {
    let path = env::args()
        .nth(1)
        .ok_or("usage: rc-skeletal-track-probe OUTPUT.json")?;
    let times = [
        -5.,
        -0.,
        0.,
        0.25,
        1.,
        2.,
        2.25,
        4.,
        5.,
        7.5,
        9.,
        10.,
        10.25,
        12.,
        15.,
        25.,
        100.,
        f32::NAN,
        f32::INFINITY,
        f32::NEG_INFINITY,
    ];
    let variants = [
        "Timed",
        "SingletonRotation",
        "SingletonPosition",
        "SingletonBoth",
        "NoDurations",
        "NegativeDurationCount",
        "ZeroDurations",
        "MissingPosition",
        "DecodeBoundary",
        "SlerpBoundary",
        "TruncatedDurations",
        "TruncatedRotations",
    ];
    let mut probes = vec![];
    for (mode, variant) in variants.iter().enumerate() {
        for time in times {
            let mut track = AnimationTrack {
                rotations: vec![[10, 11, 12], [20, 21, 22], [30, 31, 32]],
                rotation_count_word: 0xa0000003,
                positions: vec![[0, -32767, 32767], [32767, 0, -32767], [-32767, 32767, 0]],
                position_count_word: 0xe0000003,
                position_scale_bits: 2f32.to_bits(),
                durations: vec![2, 3, 5],
                duration_count_word: 0xa0000003,
            };
            if [1, 3].contains(&mode) {
                track.rotation_count_word = 0xe0000001;
                track.rotations.truncate(1);
            }
            if [2, 3].contains(&mode) {
                track.position_count_word = 0xa0000001;
                track.positions.truncate(1);
            }
            if mode == 4 {
                track.duration_count_word = 0;
                track.durations.clear();
            }
            if mode == 5 {
                track.duration_count_word = 0x1fffffff;
                track.durations.clear();
            }
            if mode == 6 {
                track.durations = vec![0, 0, 0];
            }
            if mode == 7 {
                track.positions.clear();
            }
            if mode == 10 {
                track.durations.truncate(1);
            }
            if mode == 11 {
                track.rotations.truncate(1);
            }
            let before = RootTransform {
                rotation: [7; 4],
                position: [8; 3],
            };
            let mut after = before;
            let mut host = Host {
                mode,
                ..Default::default()
            };
            let result = sample_track(&track, time, &mut after, &mut host);
            probes.push(serde_json::json!({"variant":variant,"time_bits":time.to_bits(),"track":track,"before":before,"after":after,"events":host.events,"result":result}));
        }
    }
    let report = serde_json::json!({"scope":"240 synthetic compressed-track snapshots; native key timing and position math; supplied diagnostic quaternion decode/Slerp. No original package tracks, x87 frame multiplication or visible animation.","probes":probes});
    fs::write(path, serde_json::to_string_pretty(&report)? + "\n")?;
    Ok(())
}
