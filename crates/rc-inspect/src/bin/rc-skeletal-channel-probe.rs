use rc_package::{
    mesh_animation::AnimationChannel,
    skeletal_channel::{apply_channel, ChannelPoseInput, SkeletalChannelHost},
    skeletal_root_pose::RootTransform,
};
use serde_json::{json, Value};
#[derive(Default)]
struct Host {
    events: Vec<Value>,
    failure: usize,
    angular: bool,
}
impl Host {
    fn event(&mut self, event: Value) -> Result<(), String> {
        self.events.push(event);
        if self.events.len() == self.failure {
            Err("host boundary".into())
        } else {
            Ok(())
        }
    }
}
fn pose(x: f32) -> RootTransform {
    RootTransform {
        rotation: [0, 0, 0, 1f32.to_bits()],
        position: [x.to_bits(), 0, 0],
    }
}
impl SkeletalChannelHost for Host {
    fn sample(&mut self, track: usize, time: f32) -> Result<RootTransform, String> {
        self.event(json!(["sample", track, time.to_bits()]))?;
        Ok(pose(10.0))
    }
    fn slerp(&mut self, a: [u32; 4], b: [u32; 4], alpha: f32) -> Result<[u32; 4], String> {
        self.event(json!(["slerp", a, b, alpha.to_bits()]))?;
        Ok([11, 22, 33, 44])
    }
    fn angular_limit(
        &mut self,
        a: [u32; 4],
        b: [u32; 4],
        p: [u32; 4],
        progress: f32,
    ) -> Result<Option<f32>, String> {
        self.event(json!(["angular", a, b, p, progress.to_bits()]))?;
        Ok(self.angular.then_some(0.25))
    }
    fn reciprocal_sqrt_seed(&mut self, n: f32) -> Result<f32, String> {
        self.event(json!(["seed", n.to_bits()]))?;
        // Supplied deterministic seed, independent of platform RSQRTSS.
        Ok(0.125)
    }
}
fn main() -> Result<(), Box<dyn std::error::Error>> {
    let mut cases = vec![];
    for index in [0usize, 1] {
        for blend in [0f32, 0.25, 0.5, 1., 2., f32::NAN] {
            for weight in [0f32, 0.5, 1., f32::NAN] {
                for move_bone in [-1, 0] {
                    for mapping in [-1, 0] {
                        for frames in [None, Some(8)] {
                            for failure in [0usize, 1, 3, 5] {
                                for angular in [false, true] {
                                    let mut c = AnimationChannel::default();
                                    c.words[7] = 0.75f32.to_bits();
                                    c.words[12] = 0.25f32.to_bits();
                                    c.words[11] = blend.to_bits();
                                    c.words[14] = weight.to_bits();
                                    c.words[16] = 2;
                                    let before = c.clone();
                                    let map = [-1, mapping];
                                    let reference = [pose(20.); 2];
                                    let previous = [pose(0.); 2];
                                    let mut scratch = [pose(2.); 2];
                                    let mut host = Host {
                                        failure,
                                        angular,
                                        ..Host::default()
                                    };
                                    let result = apply_channel(
                                        &mut c,
                                        index,
                                        ChannelPoseInput {
                                            frames,
                                            mapping: &map,
                                            reference: &reference,
                                            previous: &previous,
                                            move_bone,
                                        },
                                        &mut scratch,
                                        &mut host,
                                    );
                                    cases.push(json!({"index":index,"frames":frames,"move_bone":move_bone,"mapping":map,"failure":failure,"angular":angular,"before":before,"after":c,"scratch":scratch,"result":result,"events":host.events}));
                                }
                            }
                        }
                    }
                }
            }
        }
    }
    let report = json!({"scope":"Prepared ApplyAnimChannel orchestration. Supplied sample, Slerp, angular-budget and reciprocal-sqrt host values; not original game playback or x87 emulation.","cases":cases});
    let path = std::env::args().nth(1).ok_or("output path required")?;
    std::fs::write(path, serde_json::to_vec_pretty(&report)?)?;
    println!(
        "{} prepared channel cases",
        report["cases"].as_array().unwrap().len()
    );
    Ok(())
}
