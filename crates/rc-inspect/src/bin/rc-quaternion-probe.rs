use rc_package::{quaternion_animation::*, skeletal_root_pose::RootTransform, skeletal_track::*};
use std::{env, fs};
#[derive(Default)]
struct Math {
    fail: bool,
    events: Vec<serde_json::Value>,
}
impl QuaternionMath for Math {
    fn reciprocal_sqrt_seed(&mut self, n: f32) -> Result<f32, String> {
        self.events
            .push(serde_json::json!({"rsqrt_bits":n.to_bits()}));
        if self.fail {
            Err("unresolved RSQRTSS seed".into())
        } else {
            Ok(0.875)
        }
    }
    fn spherical_weights(&mut self, d: f32, t: f32) -> Result<[f32; 2], String> {
        self.events
            .push(serde_json::json!({"dot_bits":d.to_bits(),"alpha_bits":t.to_bits()}));
        if self.fail {
            Err("unresolved x87 weights".into())
        } else {
            Ok([0.25, 0.75])
        }
    }
}
fn main() -> Result<(), Box<dyn std::error::Error>> {
    let path = env::args()
        .nth(1)
        .ok_or("usage: rc-quaternion-probe OUTPUT.json")?;
    let mut decodes = vec![];
    let mut slerps = vec![];
    let mut tracks = vec![];
    for base in [
        [0u16; 3],
        [12000, 57536, 4000],
        [32766, 0, 0],
        [32768, 32768, 32768],
        [1000, 2000, 3000],
    ] {
        for flags in 0..8 {
            let key = [
                base[0] | (flags & 1),
                base[1] | ((flags >> 1) & 1),
                base[2] | ((flags >> 2) & 1),
            ];
            for fail in [false, true] {
                let mut m = Math {
                    fail,
                    ..Default::default()
                };
                let result = decode_rotation(key, &mut m);
                decodes.push(
                    serde_json::json!({"key":key,"fail":fail,"events":m.events,"result":result}),
                );
            }
        }
    }
    for (a, b) in [
        ([0., 0., 0., 1.], [0., 0., 0., 1.]),
        ([0., 0., 0., 1.], [0., 0., 0., -1.]),
        ([1., 0., 0., 0.], [0., 0., 1., 0.]),
        ([0., 0., 0., 1.], [0., 0., 0., SLERP_LINEAR_THRESHOLD]),
        ([0., 0., 0., 1.], [0., 0., 0., 0.54]),
        ([0.1, -0.2, 0.3, 0.9], [-0.7, 0.4, 0.2, -0.1]),
    ] {
        for time in [-1., -0., 0., 0.25, 0.5, 1., 2.5] {
            for fail in [false, true] {
                let a = a.map(f32::to_bits);
                let b = b.map(f32::to_bits);
                let mut m = Math {
                    fail,
                    ..Default::default()
                };
                let result = slerp_rotation(a, b, time, &mut m);
                slerps.push(serde_json::json!({"current":a,"next":b,"alpha_bits":time.to_bits(),"fail":fail,"events":m.events,"result":result}));
            }
        }
    }
    for rotations in [
        vec![[0, 0, 0], [0, 1, 0]],
        vec![[0, 0, 0], [0, 0, 1]],
        vec![[12000, 57536, 4000], [1000, 2000, 3000]],
    ] {
        for time in [0., 0.5, 1., 2.5] {
            let track = AnimationTrack {
                rotations: rotations.clone(),
                rotation_count_word: 2,
                positions: vec![[0, -32767, 32767], [32767, 0, -32767]],
                position_count_word: 2,
                position_scale_bits: 2f32.to_bits(),
                durations: vec![2, 2],
                duration_count_word: 2,
            };
            let mut out = RootTransform {
                rotation: [7; 4],
                position: [8; 3],
            };
            let result = sample_track(
                &track,
                time,
                &mut out,
                &mut QuaternionTrackHost {
                    math: PortableQuaternionMath,
                },
            );
            tracks.push(serde_json::json!({"track":track,"time_bits":time.to_bits(),"result":result,"root":out}));
        }
    }
    fs::write(
        path,
        serde_json::to_string_pretty(
            &serde_json::json!({"scope":"80 decode and 84 Slerp cases with supplied primitive math or explicit boundaries; 12 synthetic compressed-track samples using portable sqrt/f64 trigonometry. No original clips or x86 bit-parity claim.","decodes":decodes,"slerps":slerps,"portable_tracks":tracks}),
        )? + "\n",
    )?;
    Ok(())
}
