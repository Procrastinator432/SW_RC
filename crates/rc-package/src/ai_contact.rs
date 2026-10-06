//! Own post-step static AI contact routing; not native within-physics callback timing.
use crate::{
    crouch_motion::{
        CrouchMotionOptions, CrouchPhysicsInput, CrouchingPhysicsFrame, CrouchingScriptBody,
    },
    hit_wall::{HitWallContext, HitWallRecoveryFrame},
    movement_profile::MovementProfile,
    pawn_jump::JumpPhysics,
    pawn_motion::{MotionMode, PawnMotion},
    volumes::VolumeWorld,
    world_collision::StaticBodyWorld,
};
use serde::Serialize;
use std::collections::BTreeMap;

#[derive(Debug, Clone, Copy)]
pub struct WallEventSnapshot {
    pub excluded_wall_class: bool,
    pub notify_handled: bool,
}
impl WallEventSnapshot {
    /// Native processHitWall returns early for APawn or any Pawn subclass.
    /// Notify result remains an explicit event snapshot; no callback execution.
    pub fn from_actor_class(
        catalog: &crate::defaults::Catalog,
        actor_class: &str,
        notify_handled: bool,
    ) -> Result<Self, String> {
        if !catalog.derives_from(actor_class, "Engine.Actor")? {
            return Err("wall class is not an Actor".into());
        }
        Ok(Self {
            excluded_wall_class: catalog.derives_from(actor_class, "Engine.Pawn")?,
            notify_handled,
        })
    }
}
#[cfg(test)]
mod class_tests {
    use super::*;
    #[test]
    fn native_wall_exclusion_follows_pawn_ancestry_and_keeps_notify_snapshot() {
        let mut catalog = crate::defaults::Catalog::default();
        for (name, parent) in [
            ("Engine.Actor", None),
            ("Engine.Pawn", Some("Engine.Actor")),
            ("CTCharacters.PlayerCommando", Some("Engine.Pawn")),
            ("Engine.StaticMeshActor", Some("Engine.Actor")),
        ] {
            catalog
                .insert(crate::defaults::Class {
                    name: name.into(),
                    parent: parent.map(str::to_string),
                    package: "fixture".into(),
                    source: "fixture".into(),
                    properties: crate::properties::Properties {
                        values: vec![],
                        native_offset: 0,
                    },
                    fields: vec![],
                })
                .unwrap();
        }
        for class in ["Engine.Pawn", "CTCharacters.PlayerCommando"] {
            assert!(
                WallEventSnapshot::from_actor_class(&catalog, class, false)
                    .unwrap()
                    .excluded_wall_class
            );
        }
        let mesh =
            WallEventSnapshot::from_actor_class(&catalog, "Engine.StaticMeshActor", true).unwrap();
        assert!(!mesh.excluded_wall_class && mesh.notify_handled);
        assert!(WallEventSnapshot::from_actor_class(&catalog, "Missing.Class", false).is_err());
    }
}
pub struct AiControllerSnapshot {
    pub controller_present: bool,
    pub direct_hit_wall: bool,
    pub human_controlled: bool,
    pub destination: [f64; 3],
    pub minimum_hit_wall: f64,
    /// Explicit entries for contacted objects, including World.BSP when applicable.
    pub walls: BTreeMap<String, WallEventSnapshot>,
}
pub struct AiContactOptions<'a> {
    pub motion: CrouchMotionOptions<'a>,
    pub controller: &'a AiControllerSnapshot,
}
#[derive(Debug, Serialize)]
pub struct AiContactFrame {
    pub state: CrouchingScriptBody,
    pub physics: CrouchingPhysicsFrame,
    pub wall_object: Option<String>,
    pub incoming_velocity: Option<[f64; 3]>,
    pub dispatch: Option<HitWallRecoveryFrame>,
}
impl VolumeWorld {
    /// Own policy: completed Walking step, no successful step-up, first approaching
    /// non-floor contact only. Errors discard physics and dispatch candidates together.
    pub fn ai_crouching_contact_tick(
        &self,
        world: &StaticBodyWorld,
        state: &CrouchingScriptBody,
        input: CrouchPhysicsInput,
        dt: f64,
        profile: &MovementProfile,
        options: AiContactOptions<'_>,
    ) -> Result<AiContactFrame, String> {
        let physics = self.crouching_physics_tick(
            world,
            state,
            input,
            dt,
            profile,
            CrouchMotionOptions {
                runtime: options.motion.runtime,
                shape: options.motion.shape,
            },
        )?;
        let mut result = AiContactFrame {
            state: physics.state,
            physics,
            wall_object: None,
            incoming_velocity: None,
            dispatch: None,
        };
        let PawnMotion::Walking(walking) = &result.physics.movement.motion else {
            return Ok(result);
        };
        if !walking.walking_applied
            || result.physics.movement.next_mode != MotionMode::Walking
            || walking.motion.step.is_some()
        {
            return Ok(result);
        }
        let incoming = walking.planned_velocity;
        let Some(contact) = walking.motion.contacts.iter().find(|c| {
            c.hit.normal[2].abs() < profile.physics.minimum_up
                && (0..3).map(|i| c.hit.normal[i] * incoming[i]).sum::<f64>() < 0.0
        }) else {
            return Ok(result);
        };
        let wall = options
            .controller
            .walls
            .get(&contact.object)
            .ok_or_else(|| format!("missing AI wall event snapshot: {}", contact.object))?;
        let context = HitWallContext {
            wall_present: true,
            excluded_wall_class: wall.excluded_wall_class,
            direct_hit_wall: options.controller.direct_hit_wall,
            controller_present: options.controller.controller_present,
            destination: options.controller.destination,
            wall_normal: contact.hit.normal,
            minimum_hit_wall: options.controller.minimum_hit_wall,
            notify_handled: wall.notify_handled,
            physics: JumpPhysics::Walking,
            human_controlled: options.controller.human_controlled,
            can_crouch: input.can_crouch,
        };
        let mut dispatch_state = result.state;
        dispatch_state.script.body.velocity = incoming;
        let dispatch = world.hit_wall_auto_crouch_recovery(
            &dispatch_state,
            context,
            options.motion.shape,
            profile.physics.skin,
        )?;
        result.wall_object = Some(contact.object.clone());
        result.incoming_velocity = Some(incoming);
        result.state = dispatch.state;
        // The incoming snapshot is solely for the event's stationary gate.
        result.state.script.body.velocity = result.physics.state.script.body.velocity;
        result.dispatch = Some(dispatch);
        Ok(result)
    }
}
