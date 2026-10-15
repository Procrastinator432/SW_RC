//! Selected Scorch encounter. Physical movement and original animations; the
//! companion controller and route guidance are bounded port adapters.
use crate::{
    rendezvous::{self, Effect},
    visual, Game, Phase, Point, STEP,
};
use serde::{Deserialize, Serialize};
#[derive(Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Assets {
    pub flow: rendezvous::Assets,
    pub mesh: Option<visual::Mesh>,
    pub sounds: Vec<crate::combat::Sound>,
    pub spawn: Point,
    pub shoot_route: Vec<Point>,
    pub greet_route: Vec<Point>,
    pub player_route: Vec<Point>,
    pub intro: crate::crash::Trigger,
    pub behind: crate::crash::Trigger,
    pub ground_speed: f64,
    pub provenance: serde_json::Value,
}
pub struct Prepared {
    pub assets: Assets,
    pub mesh: visual::PreparedMesh,
    pub texture_base: usize,
    pub sound_base: usize,
}
#[derive(Clone, Debug, Serialize, serde::Deserialize)]
pub struct State {
    pub flow: rendezvous::State,
    pub pawn: rc_package::controller::ControlledBody,
    pub yaw: f64,
    pub motion: u8,
    pub waypoint: usize,
    pub guide: usize,
    pub behind_seen: bool,
    pub gesture: bool,
    pub frame: f32,
    pub intro_seen: bool,
    pub activated: bool,
    pub radio_started: bool,
    pub completed_tick: Option<u64>,
    pub effects: Vec<(u64, Effect)>,
}
impl Game {
    pub fn attach_rendezvous(&mut self, mut a: Assets) -> Result<(), String> {
        if self.rendezvous.is_some()
            || self.bunker.is_none()
            || a.mesh.is_none()
            || !a.intro.validate()
            || !a.behind.validate()
            || a.intro.original != 532
            || a.behind.original != 1133
            || !a.ground_speed.is_finite()
            || !(1. ..=1000.).contains(&a.ground_speed)
            || a.sounds.is_empty()
            || a.sounds.len() > 32
        {
            return Err("Invalid Scorch encounter".into());
        }
        for route in [&a.shoot_route, &a.greet_route, &a.player_route] {
            if route.is_empty()
                || route.len() > 512
                || route
                    .iter()
                    .flatten()
                    .any(|v| !v.is_finite() || v.abs() > 1e8)
            {
                return Err("Invalid rendezvous route".into());
            }
        }
        if a.spawn.iter().any(|v| !v.is_finite() || v.abs() > 1e8) {
            return Err("Invalid Scorch spawn".into());
        }
        let mut flow = rendezvous::State::new(&a.flow)?;
        flow.event(&a.flow, "levelstart")?;
        for sound in &a.sounds {
            crate::combat::validate_wave(&sound.wave)?;
        }
        let sources = a
            .sounds
            .iter()
            .map(|s| s.source.as_str())
            .collect::<std::collections::BTreeSet<_>>();
        if sources.len()!=a.sounds.len() || a.flow.programs.iter().flat_map(|p|&p.actions).any(|a|matches!(&a.operation,rendezvous::Operation::Sound{source,..} if !sources.contains(source.as_str()))) {return Err("Incomplete Scorch sound bank".into());}
        let mesh = a.mesh.take().unwrap().prepare()?;
        for name in ["ActionBreathe", "RunForward", "GestureSignalOK"] {
            mesh.clip(name)?;
        }
        let texture_base = self.scene.textures.len();
        self.scene
            .textures
            .extend(mesh.mesh.materials.iter().map(|m| rc_render::Texture {
                width: m.width,
                height: m.height,
                pixels: m.pixels.clone(),
            }));
        let sound_base = self.mission.as_ref().unwrap().sounds.len();
        self.mission
            .as_mut()
            .unwrap()
            .sounds
            .extend(a.sounds.iter().map(|s| crate::combat::Sound {
                source: s.source.clone(),
                wave: s.wave.clone(),
            }));
        self.state.rendezvous = Some(State {
            flow,
            pawn: rc_package::controller::ControlledBody {
                body: rc_package::physics::BodyState {
                    position: a.spawn,
                    velocity: [0.; 3],
                    grounded: true,
                },
                jump_held: false,
            },
            yaw: -32735. * std::f64::consts::TAU / 65536.,
            motion: 0,
            waypoint: 0,
            guide: 0,
            behind_seen: false,
            gesture: false,
            frame: 0.,
            intro_seen: false,
            activated: false,
            radio_started: false,
            completed_tick: None,
            effects: vec![],
        });
        self.rendezvous = Some(Prepared {
            assets: a,
            mesh,
            texture_base,
            sound_base,
        });
        self.initial = self.state.clone();
        Ok(())
    }
    fn apply_rendezvous_effects(&mut self, effects: Vec<Effect>) -> Result<(), String> {
        let prepared = self.rendezvous.as_ref().unwrap();
        let tick = self.state.statistics.ticks;
        let s = self.state.rendezvous.as_mut().unwrap();
        for e in effects {
            match &e {
                Effect::PlaySound { source, .. } => {
                    let role = prepared
                        .assets
                        .sounds
                        .iter()
                        .position(|s| &s.source == source)
                        .ok_or("Unknown Scorch sound")?;
                    let m = self.state.mission.as_mut().unwrap();
                    m.audio_sequence += 1;
                    m.events.push(crate::combat::Event {
                        sequence: m.audio_sequence,
                        tick,
                        sound: prepared.sound_base + role,
                        owner: -1,
                    });
                }
                Effect::Event { tag } if tag == "62WalkToIntro" => {
                    s.motion = 1;
                    s.waypoint = 0;
                }
                Effect::ObjectiveCompleted { objective: 4 } => {
                    s.completed_tick = Some(tick);
                    if self.demolition.is_none() {
                        self.state.phase = Phase::Won;
                    }
                }
                _ => {}
            }
            s.effects.push((tick, e));
        }
        Ok(())
    }
    pub(crate) fn rendezvous_tick(&mut self) -> Result<(), String> {
        let Some(prepared) = &self.rendezvous else {
            return Ok(());
        };
        let a = &prepared.assets;
        let s = self.state.rendezvous.as_mut().unwrap();
        let mut effects = vec![];
        if !s.activated
            && self
                .state
                .bunker
                .as_ref()
                .unwrap()
                .destruction_tick
                .is_some()
        {
            s.activated = true;
            effects.extend(s.flow.event(&a.flow, "SpawnSmallBunker")?);
        }
        if !s.radio_started && self.state.bunker.as_ref().unwrap().complete_tick.is_some() {
            s.radio_started = true;
            effects.extend(s.flow.event(&a.flow, "Conversation04")?);
        }
        if s.radio_started {
            let position = self.state.pawn.body.position;
            let mut extent = self.profile.physics.extent;
            if self.state.crouched {
                extent[2] = 56.;
            }
            if !s.intro_seen && a.intro.contains(position, extent) {
                s.intro_seen = true;
                effects.extend(s.flow.event(&a.flow, "62Intro")?);
            }
            if !s.behind_seen && a.behind.contains(position, extent) {
                s.behind_seen = true;
                effects.extend(s.flow.event(&a.flow, "DialogBehindBunker")?);
            }
            let before = s.flow.lanes.iter().filter(|l| l.started).count();
            effects.extend(s.flow.observe(&a.flow, position, extent)?);
            if s.flow.lanes.iter().filter(|l| l.started).count() > before
                && a.flow.join.contains(position, extent)
            {
                s.gesture = true;
                s.frame = 0.;
            }
            while s.guide + 1 < a.player_route.len()
                && crate::distance(position, a.player_route[s.guide]) < 45.
            {
                s.guide += 1;
            }
        }
        effects.extend(s.flow.advance(&a.flow, STEP)?);
        // Script454 moves to the shoot point, then the greeting point; retain pending
        // behind-bunker activation when the player reaches the trigger ahead of Scorch.
        if s.motion == 3 && s.behind_seen {
            s.motion = 2;
            s.waypoint = 0;
        }
        if s.motion == 1 || s.motion == 2 {
            let route = if s.motion == 1 {
                &a.shoot_route
            } else {
                &a.greet_route
            };
            let target = route[s.waypoint];
            let p = s.pawn.body.position;
            if crate::distance(p, target) < 35. && s.pawn.body.grounded {
                if s.waypoint + 1 < route.len() {
                    s.waypoint += 1;
                } else {
                    s.motion = if s.motion == 1 { 3 } else { 4 };
                    s.pawn.body.velocity = [0.; 3];
                }
            } else {
                s.yaw = (target[1] - p[1]).atan2(target[0] - p[0]);
                s.pawn = self
                    .volumes
                    .pawn_diagnostic_tick(
                        &self.world,
                        &s.pawn,
                        rc_package::controller::ViewInput {
                            forward: 1.,
                            strafe: 0.,
                            yaw: (s.yaw * 65536. / std::f64::consts::TAU) as i32,
                            walking: false,
                            jump: false,
                        },
                        STEP,
                        &self.profile,
                        rc_package::pawn_motion::PawnRuntimeOptions {
                            maximum_desired_speed: a.ground_speed,
                            weapon_modifier: 1.,
                            crouch_ratio: 1.,
                            wounded_ratio: 1.,
                            crouched: false,
                            wounded: false,
                            wants_to_crouch: false,
                            current_jump_z: self.profile.controller.jump_speed,
                        },
                    )?
                    .state;
            }
        }
        if s.motion == 4
            && self.state.demolition.as_ref().is_none_or(|s| {
                matches!(
                    s.flow.phase,
                    crate::demolition::Phase::Disabled | crate::demolition::Phase::Available
                )
            })
        {
            let p = self.state.pawn.body.position;
            s.yaw = (p[1] - s.pawn.body.position[1]).atan2(p[0] - s.pawn.body.position[0]);
        }
        let name = if s.gesture {
            "GestureSignalOK"
        } else if s.motion == 1 || s.motion == 2 {
            "RunForward"
        } else {
            "ActionBreathe"
        };
        let clip = prepared.mesh.clip(name)?;
        let sequence = &prepared.mesh.mesh.clips[clip].sequence;
        s.frame += f32::from_bits(sequence.rate_bits) * STEP as f32 / sequence.frames as f32;
        if s.frame >= 1. {
            if s.gesture {
                s.gesture = false;
                s.frame = 0.;
            } else {
                s.frame = s.frame.rem_euclid(1.);
            }
        }
        self.apply_rendezvous_effects(effects)
    }
    pub fn rendezvous_target(&self) -> Option<Point> {
        let a = self.rendezvous.as_ref()?;
        let s = self.state.rendezvous.as_ref()?;
        s.radio_started.then(|| a.assets.player_route[s.guide])
    }
    pub(crate) fn scorch_geometry(
        &self,
    ) -> Result<
        (
            Vec<rc_package::geometry::Triangle>,
            Vec<Option<rc_render::FaceTexture>>,
        ),
        String,
    > {
        let (Some(a), Some(s)) = (&self.rendezvous, &self.state.rendezvous) else {
            return Ok((vec![], vec![]));
        };
        let name = if s.gesture {
            "GestureSignalOK"
        } else if s.motion == 1 || s.motion == 2 {
            "RunForward"
        } else {
            "ActionBreathe"
        };
        let pose = self
            .state
            .squad
            .as_ref()
            .filter(|q| {
                q.joined
                    && (q.down
                        || self
                            .state
                            .demolition
                            .as_ref()
                            .is_some_and(|d| d.flow.objective_completed))
            })
            .map_or((name, s.frame), |q| (q.clip.as_str(), q.frame));
        let (name, frame) = self
            .demolition_clip()
            .filter(|_| {
                !self.state.squad.as_ref().is_some_and(|q| {
                    q.down
                        || self
                            .state
                            .demolition
                            .as_ref()
                            .is_some_and(|d| d.flow.objective_completed)
                })
            })
            .unwrap_or(pose);
        let mut mesh = a
            .mesh
            .geometry_with_root(&a.mesh.channel(a.mesh.clip(name)?, frame)?, true)?;
        let (sin, cos) = (s.yaw as f32).sin_cos();
        for t in &mut mesh.triangles {
            t.points = t.points.map(|p| {
                [
                    s.pawn.body.position[0] as f32 + p[0] * cos - p[1] * sin,
                    s.pawn.body.position[1] as f32 + p[0] * sin + p[1] * cos,
                    s.pawn.body.position[2] as f32
                        - if self.state.squad.as_ref().is_some_and(|q| q.crouched) {
                            56.
                        } else {
                            84.
                        }
                        + p[2]
                        - a.mesh.rest_bounds[0][2],
                ]
            });
        }
        for f in mesh.appearance.iter_mut().flatten() {
            f.texture += a.texture_base;
        }
        Ok((mesh.triangles, mesh.appearance))
    }
}
