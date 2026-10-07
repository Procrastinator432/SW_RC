//! GetMoveCoords(false) wired into PostInit's root boundary on diagnostic snapshots.
use rc_package::{
    animation_replication::{skeletal_post_init, SkeletalPostInitHost},
    mesh_animation::AnimationChannel,
    move_coords::{get_move_coords_absolute, MoveCoordsHost},
};
use std::{env, fs};
struct Host {
    channels: Vec<AnimationChannel>,
    move_bone: i32,
    mode: &'static str,
    events: Vec<String>,
}
impl MoveCoordsHost for Host {
    fn sequence_flags(
        &mut self,
        i: usize,
        c: &mut [AnimationChannel],
    ) -> Result<Option<u8>, String> {
        self.events.push(format!("GetSequence({i})"));
        c[i].words[17] = 900 + i as u32;
        Ok(if self.mode == "SequenceMissing" {
            None
        } else {
            Some(if self.mode == "NoMoveFlag" { 2 } else { 1 })
        })
    }
    fn evaluate_move_matrix(
        &mut self,
        i: usize,
        _: &mut [AnimationChannel],
    ) -> Result<[u32; 16], String> {
        self.events.push(format!("EvaluateMoveMatrix({i})"));
        if self.mode == "FrameBoundary" {
            Err("unresolved frame/pose evaluation".into())
        } else {
            Ok([100 + i as u32; 16])
        }
    }
}
impl SkeletalPostInitHost for Host {
    fn root_location(&mut self, m: &mut [u32; 16]) -> Result<bool, String> {
        self.events.push("GetMoveCoords(relative=false)".into());
        let mut channels = std::mem::take(&mut self.channels);
        let result = get_move_coords_absolute(&mut channels, self.move_bone, Some(m), self);
        self.channels = channels;
        result
    }
    fn inverse_actor_compose(&mut self, _: &[u32; 16]) -> Result<[u32; 16], String> {
        self.events.push("InverseActorCompose".into());
        if self.mode == "SuppliedCompose" {
            Ok([9; 16])
        } else {
            Err("unresolved inverse/actor matrix composition".into())
        }
    }
}
fn main() -> Result<(), Box<dyn std::error::Error>> {
    let args: Vec<_> = env::args().collect();
    if args.len() != 3 {
        return Err("Usage: rc-move-coords-probe <channel-playback.json> <report.json>".into());
    }
    let previous: serde_json::Value = serde_json::from_slice(&fs::read(&args[1])?)?;
    let mut probes = Vec::new();
    for (source_index, source) in previous["probes"]
        .as_array()
        .ok_or("missing channel probes")?
        .iter()
        .enumerate()
    {
        if source["variant"] != "NewSequence" {
            continue;
        }
        let index: usize = source["channel_index"]
            .as_u64()
            .ok_or("missing channel index")?
            .try_into()?;
        let base: Vec<AnimationChannel> =
            serde_json::from_value(source["after"]["channels"].clone())?;
        for scenario in [
            "LoopSkipped",
            "BoneMismatch",
            "Stopped",
            "Occluded",
            "SequenceMissing",
            "NoMoveFlag",
            "NoOutput",
            "MatrixCopy",
            "FrameBoundary",
            "SuppliedCompose",
        ] {
            let mut channels = base.clone();
            for c in &mut channels {
                c.words[1] = (c.words[1] & 0xffffff00) | 1;
                c.words[14] = 0;
            }
            let c = &mut channels[index];
            c.words[1] &= 0xffffff00;
            c.words[6] = 1.0f32.to_bits();
            c.words[14] = 1.0f32.to_bits();
            c.words[11] = 0.5f32.to_bits();
            let move_bone = c.words[15] as i32;
            if scenario == "LoopSkipped" {
                c.words[1] |= 1;
            }
            if scenario == "Stopped" {
                c.words[6] = 0;
            }
            if scenario == "Occluded" {
                let mut blocker = c.clone();
                blocker.words[1] |= 1;
                blocker.words[11] = 1.0f32.to_bits();
                channels.push(blocker);
            }
            let move_bone = if scenario == "BoneMismatch" {
                move_bone + 100
            } else {
                move_bone
            };
            let before = channels.clone();
            let mut host = Host {
                channels,
                move_bone,
                mode: scenario,
                events: vec![],
            };
            let mut matrix = [7; 16];
            let result = if scenario == "NoOutput" {
                let mut channels = std::mem::take(&mut host.channels);
                let r = get_move_coords_absolute(&mut channels, move_bone, None, &mut host);
                host.channels = channels;
                serde_json::to_value(r)?
            } else {
                serde_json::to_value(skeletal_post_init(&mut matrix, &mut host))?
            };
            probes.push(serde_json::json!({"source_index":source_index,"class":source["class"],"source_case":source["source_case"],"channel_index":index,"scenario":scenario,"move_bone":move_bone,"before_channels":before,"after_channels":host.channels,"matrix":matrix,"events":host.events,"result":result}));
        }
    }
    let report = serde_json::json!({"scope":"768 earlier supplied NewSequence channel snapshots, ten GetMoveCoords(false)/PostInit scenarios each. Move bone, loop/rate/weight words, sequence move flags, editor-refresh tokens and evaluated matrices are deliberately supplied diagnostics. IsChannelActive and GetMoveCoords selection execute in Rust. Frame/pose evaluation and inverse/actor matrix composition remain host boundaries; one scenario explicitly supplies composed matrix. NoOutput directly calls GetMoveCoords with null output. No original skeleton/pose/Android playback proven.","probes":probes});
    fs::write(&args[2], serde_json::to_string_pretty(&report)? + "\n")?;
    println!("{} supplied move-coordinates scenarios", probes.len());
    Ok(())
}
