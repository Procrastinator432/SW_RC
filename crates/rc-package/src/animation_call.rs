//! Reviewed Actor native animation wrappers; virtual playback remains a host boundary.
use crate::{
    event_lookup::EventNameSnapshot,
    prop_state::{AnimationKind, AnimationRequest},
    script::{Function, Operand},
};
use serde::Serialize;

#[derive(Clone, Copy, Default)]
pub struct AnimationArguments {
    pub bone: Option<EventNameSnapshot>,
    pub rate: Option<f32>,
    pub start_frame: Option<f32>,
    pub channel: Option<i32>,
}

/// Semantic fields of the 32-byte FPlayAnim passed at vtable +0x1b0.
/// +0x10 is initialized to 1.0; its meaning is unresolved. +0x14 and the
/// padding after the loop byte are not initialized here and are not modeled.
/// This is not a Rust representation of the native memory layout.
#[derive(Clone, Copy, Debug, Serialize)]
pub struct PlayAnimParameters {
    pub sequence: EventNameSnapshot,
    pub looping: bool,
    pub bone: EventNameSnapshot,
    pub channel: i32,
    pub float_10: f32,
    pub rate: f32,
    pub start_frame: f32,
}
#[derive(Clone, Copy)]
pub struct NativeAnimationWrapper {
    kind: AnimationKind,
}
impl NativeAnimationWrapper {
    pub fn verify(path: &str, function: &Function) -> Result<Self, String> {
        let (kind, name, native, first) = match path {
            "Actor.PlayAnim" => (AnimationKind::PlayAnim, "PlayAnim", 259, 563),
            "Actor.LoopAnim" => (AnimationKind::LoopAnim, "LoopAnim", 260, 569),
            _ => return Err("animation wrapper identity unresolved".into()),
        };
        if function.friendly_name != name
            || function.native_index != native
            || function.flags != 0x20401
            || function.precedence != 0
            || function.replication_offset.is_some()
            || function.logical_script_bytes != 26
            || function.serialized_script_bytes != 16
            || function.expressions.len() != 6
        {
            return Err("animation wrapper metadata differs from original".into());
        }
        for (i, parameter) in ["Sequence", "Bone", "Rate", "StartFrame", "Channel"]
            .iter()
            .enumerate()
        {
            let e = &function.expressions[i];
            if e.opcode != 0x29
                || e.logical_offset != i as u32 * 5
                || e.logical_end != i as u32 * 5 + 5
                || e.serialized_offset != i * 3
                || !e.children.is_empty()
                || !matches!(&e.operand, Operand::Object{index, path} if *index == first+i as i32 && path == &format!("Actor.{name}.{parameter}"))
            {
                return Err("animation wrapper parameter differs from original".into());
            }
        }
        let e = &function.expressions[5];
        if e.opcode != 0x0b
            || e.logical_offset != 25
            || e.logical_end != 26
            || e.serialized_offset != 15
            || !e.children.is_empty()
            || !matches!(e.operand, Operand::None)
        {
            return Err("animation wrapper terminator differs from original".into());
        }
        Ok(Self { kind })
    }
    /// Arguments are already evaluated. None represents an omitted argument:
    /// execNothing leaves its local unchanged; EndFunctionParms additionally
    /// rewinds Code so repeated optional reads see the same end token.
    pub fn prepare(
        self,
        request: AnimationRequest,
        args: AnimationArguments,
    ) -> Result<PlayAnimParameters, String> {
        let native = match self.kind {
            AnimationKind::PlayAnim => 259,
            AnimationKind::LoopAnim => 260,
        };
        if request.kind != self.kind || request.native_index != native {
            return Err("animation request does not match verified wrapper".into());
        }
        Ok(PlayAnimParameters {
            sequence: request.name,
            looping: self.kind == AnimationKind::LoopAnim,
            bone: args.bone.unwrap_or(EventNameSnapshot {
                handle: 0,
                resolved_index: 0,
            }),
            channel: args.channel.unwrap_or(0),
            float_10: 1.0,
            rate: args.rate.unwrap_or(1.0),
            start_frame: args.start_frame.unwrap_or(-1.0),
        })
    }
}

pub trait BaseAnimationHost {
    fn log_missing_mesh(&mut self) -> Result<(), String>;
    /// Native ignores MeshGetInstance's return value. The subsequent operation
    /// reads Actor.MeshInstance (+0xd4) after this call.
    fn mesh_get_instance(&mut self) -> Result<(), String>;
    fn actor_mesh_instance_play(&mut self, parameters: &PlayAnimParameters)
        -> Result<bool, String>;
}
#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize)]
pub enum BaseAnimationResult {
    NoMesh,
    Dispatched,
}

/// Only valid when the caller has established the AActor base virtual target.
/// No mesh/clip/instance existence or derived-class override is inferred here.
pub fn play_anim_base(
    mesh_present: bool,
    object_flags: u32,
    parameters: &PlayAnimParameters,
    host: &mut impl BaseAnimationHost,
) -> Result<BaseAnimationResult, String> {
    if !mesh_present {
        if object_flags & 0x4000 == 0 {
            host.log_missing_mesh()?;
        }
        return Ok(BaseAnimationResult::NoMesh);
    }
    host.mesh_get_instance()?;
    let _ = host.actor_mesh_instance_play(parameters)?;
    Ok(BaseAnimationResult::Dispatched)
}

#[cfg(test)]
mod tests {
    use super::*;
    fn wrapper(looping: bool) -> NativeAnimationWrapper {
        let f: Function = serde_json::from_str(if looping {
            include_str!("../tests/fixtures/loop_anim.json")
        } else {
            include_str!("../tests/fixtures/play_anim.json")
        })
        .unwrap();
        NativeAnimationWrapper::verify(
            if looping {
                "Actor.LoopAnim"
            } else {
                "Actor.PlayAnim"
            },
            &f,
        )
        .unwrap()
    }
    fn request(looping: bool) -> AnimationRequest {
        AnimationRequest {
            kind: if looping {
                AnimationKind::LoopAnim
            } else {
                AnimationKind::PlayAnim
            },
            name: EventNameSnapshot {
                handle: 337,
                resolved_index: 336,
            },
            native_index: if looping { 260 } else { 259 },
        }
    }
    #[test]
    fn omitted_defaults_and_request_identity() {
        for looping in [false, true] {
            let p = wrapper(looping)
                .prepare(request(looping), AnimationArguments::default())
                .unwrap();
            assert_eq!(
                (
                    p.sequence.handle,
                    p.looping,
                    p.bone.handle,
                    p.channel,
                    p.float_10.to_bits()
                ),
                (337, looping, 0, 0, 0x3f800000)
            );
            assert_eq!((p.float_10, p.rate, p.start_frame), (1.0, 1.0, -1.0));
            assert!(wrapper(looping)
                .prepare(request(!looping), AnimationArguments::default())
                .is_err());
        }
    }
    #[test]
    fn explicit_values_preserve_float_bits_and_signed_channel() {
        let p = wrapper(false)
            .prepare(
                request(false),
                AnimationArguments {
                    bone: Some(EventNameSnapshot {
                        handle: 71,
                        resolved_index: 9,
                    }),
                    rate: Some(f32::from_bits(0x7fc01234)),
                    start_frame: Some(-0.0),
                    channel: Some(i32::MIN),
                },
            )
            .unwrap();
        assert_eq!(p.rate.to_bits(), 0x7fc01234);
        assert_eq!(p.start_frame.to_bits(), 0x80000000);
        assert_eq!((p.bone.handle, p.channel), (71, i32::MIN));
    }
    #[test]
    fn reject_modified_native_signature() {
        let original = include_str!("../tests/fixtures/play_anim.json");
        for change in 0..4 {
            let mut f: Function = serde_json::from_str(original).unwrap();
            match change {
                0 => f.native_index = 260,
                1 => f.expressions[2].operand = Operand::None,
                2 => f.expressions[4].logical_end += 1,
                _ => f.flags ^= 1,
            };
            assert!(NativeAnimationWrapper::verify("Actor.PlayAnim", &f).is_err());
        }
    }
    #[derive(Default)]
    struct Host {
        events: Vec<&'static str>,
        fail: Option<&'static str>,
        result: bool,
    }
    impl Host {
        fn event(&mut self, e: &'static str) -> Result<(), String> {
            self.events.push(e);
            if self.fail == Some(e) {
                Err(e.into())
            } else {
                Ok(())
            }
        }
    }
    impl BaseAnimationHost for Host {
        fn log_missing_mesh(&mut self) -> Result<(), String> {
            self.event("log")
        }
        fn mesh_get_instance(&mut self) -> Result<(), String> {
            self.event("instance")
        }
        fn actor_mesh_instance_play(&mut self, _: &PlayAnimParameters) -> Result<bool, String> {
            self.event("play")?;
            Ok(self.result)
        }
    }
    #[test]
    fn no_mesh_log_flag_and_base_order() {
        let p = wrapper(false)
            .prepare(request(false), AnimationArguments::default())
            .unwrap();
        for (mesh, flags, events, result) in [
            (false, 0, vec!["log"], BaseAnimationResult::NoMesh),
            (false, 0x4000, vec![], BaseAnimationResult::NoMesh),
            (
                true,
                0x4000,
                vec!["instance", "play"],
                BaseAnimationResult::Dispatched,
            ),
        ] {
            for value in [false, true] {
                let mut h = Host {
                    result: value,
                    ..Host::default()
                };
                assert_eq!(play_anim_base(mesh, flags, &p, &mut h).unwrap(), result);
                assert_eq!(h.events, events);
            }
        }
    }
    #[test]
    fn host_errors_stop_at_boundary() {
        let p = wrapper(true)
            .prepare(request(true), AnimationArguments::default())
            .unwrap();
        for (mesh, fail, events) in [
            (false, "log", vec!["log"]),
            (true, "instance", vec!["instance"]),
            (true, "play", vec!["instance", "play"]),
        ] {
            let mut h = Host {
                fail: Some(fail),
                ..Host::default()
            };
            assert_eq!(play_anim_base(mesh, 0, &p, &mut h), Err(fail.into()));
            assert_eq!(h.events, events);
        }
    }
}
