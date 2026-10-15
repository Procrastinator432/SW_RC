//! Live selected objective5. Original assets/timers with bounded targeting and companion navigation.
use crate::{
    demolition::{self, Effect, Phase as Stage},
    Game, Input, Point, STEP,
};
use serde::{Deserialize, Serialize};
#[derive(Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Notify {
    pub clip: String,
    pub time: f32,
    pub source: String,
}
#[derive(Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Assets {
    pub flow: demolition::Assets,
    pub object: Option<crate::dynamic::Object>,
    pub route: Vec<Point>,
    pub marker: Point,
    pub charge: Point,
    pub yaw: f64,
    pub sounds: Vec<crate::combat::Sound>,
    pub notifies: Vec<Notify>,
    pub hands: Option<crate::visual::Mesh>,
    pub plunger: Vec<u8>,
    pub charges: [Vec<u8>; 2],
    pub fire_fraction: f32,
    pub provenance: serde_json::Value,
}
pub struct Prepared {
    pub assets: Assets,
    pub hands: crate::visual::PreparedMesh,
    pub plunger: rc_render::Scene,
    pub charges: [rc_render::Scene; 2],
    pub sound_base: usize,
    pub hands_base: usize,
}
#[derive(Clone, Debug, Serialize, serde::Deserialize)]
pub struct State {
    pub flow: demolition::State,
    pub waypoint: usize,
    pub guide: usize,
    pub held: bool,
    pub firing: bool,
    pub hand_frame: f32,
    pub hand_started: bool,
    pub fired: bool,
    pub complete_tick: Option<u64>,
    pub explosion_tick: Option<u64>,
    pub navigation_open: bool,
    pub effects: Vec<(u64, Effect)>,
    pub damage: Vec<crate::gunner_runtime::Damage>,
}
impl Game {
    pub fn attach_demolition(&mut self, mut a: Assets) -> Result<(), String> {
        if self.demolition.is_some()
            || self.rendezvous.is_none()
            || a.hands.is_none()
            || a.object
                .as_ref()
                .is_none_or(|o| o.original != 851 || o.released.is_none())
            || a.route.is_empty()
            || a.route.len() > 512
            || a.sounds.is_empty()
            || a.sounds.len() > 32
            || !a.yaw.is_finite()
            || !a.fire_fraction.is_finite()
            || !(0. ..=1.).contains(&a.fire_fraction)
        {
            return Err("Invalid demolition extension".into());
        }
        if a.route
            .iter()
            .chain([&a.marker, &a.charge])
            .flatten()
            .any(|v| !v.is_finite() || v.abs() > 1e8)
        {
            return Err("Invalid demolition route".into());
        }
        let flow = demolition::State::new(&a.flow)?;
        for sound in &a.sounds {
            crate::combat::validate_wave(&sound.wave)?;
        }
        let sources = a
            .sounds
            .iter()
            .map(|s| s.source.as_str())
            .collect::<std::collections::BTreeSet<_>>();
        if sources.len()!=a.sounds.len()||[&a.flow.placed_sound,&a.flow.armed_sound,&a.flow.explosion_sound].into_iter().any(|s|!sources.contains(s.as_str()))||a.flow.programs.iter().flat_map(|p|&p.actions).any(|a|matches!(&a.operation,demolition::Operation::Sound{source,..}if !sources.contains(source.as_str()))){return Err("Incomplete demolition sound bank".into());}
        for n in &a.notifies {
            if !n.time.is_finite()
                || !(0. ..=1.).contains(&n.time)
                || !sources.contains(n.source.as_str())
            {
                return Err("Invalid demolition notify".into());
            }
        }
        let hands = a.hands.take().unwrap().prepare_reference()?;
        hands.clip("Detonate")?;
        hands.attachment_transform(&hands.channel(0, 0.)?, "weaponAttach_L", false)?;
        for clip in [
            "DemolitionSetChargeStart",
            "DemolitionSetChargeLoop",
            "DemolitionSetChargeStop",
        ] {
            self.rendezvous.as_ref().unwrap().mesh.clip(clip)?;
        }
        let mut plunger = rc_render::Scene::from_snapshot(&a.plunger)?;
        let mut charges = [
            rc_render::Scene::from_snapshot(&a.charges[0])?,
            rc_render::Scene::from_snapshot(&a.charges[1])?,
        ];
        if plunger.triangles.len() > 10000 || charges.iter().any(|s| s.triangles.len() > 10000) {
            return Err("Demolition equipment exceeds budget".into());
        }
        for n in &a.notifies {
            if n.clip != "Detonate" {
                self.rendezvous.as_ref().unwrap().mesh.clip(&n.clip)?;
            }
        }
        self.append_dynamic(vec![a.object.take().unwrap()])?;
        let hands_base = self.scene.textures.len();
        self.scene
            .textures
            .extend(hands.mesh.materials.iter().map(|m| rc_render::Texture {
                width: m.width,
                height: m.height,
                pixels: m.pixels.clone(),
            }));
        for scene in std::iter::once(&mut plunger).chain(charges.iter_mut()) {
            let base = self.scene.textures.len();
            self.scene.textures.append(&mut scene.textures);
            for f in scene.appearance.iter_mut().flatten() {
                f.texture += base;
            }
        }
        let sound_base = self.mission.as_ref().unwrap().sounds.len();
        self.mission
            .as_mut()
            .unwrap()
            .sounds
            .extend(a.sounds.iter().map(|s| crate::combat::Sound {
                source: s.source.clone(),
                wave: s.wave.clone(),
            }));
        self.state.demolition = Some(State {
            flow,
            waypoint: 0,
            guide: 0,
            held: false,
            firing: false,
            hand_frame: 0.,
            hand_started: false,
            fired: false,
            complete_tick: None,
            explosion_tick: None,
            navigation_open: false,
            effects: vec![],
            damage: vec![],
        });
        self.demolition = Some(Prepared {
            assets: a,
            hands,
            plunger,
            charges,
            sound_base,
            hands_base,
        });
        self.initial = self.state.clone();
        Ok(())
    }
    fn demolition_sound(&mut self, source: &str) -> Result<(), String> {
        let p = self.demolition.as_ref().unwrap();
        let sound = p.sound_base
            + p.assets
                .sounds
                .iter()
                .position(|s| s.source == source)
                .ok_or("Unknown demolition sound")?;
        let m = self.state.mission.as_mut().unwrap();
        m.audio_sequence += 1;
        if m.events.len() >= 256 {
            m.events.remove(0);
        }
        m.events.push(crate::combat::Event {
            sequence: m.audio_sequence,
            tick: self.state.statistics.ticks,
            sound,
            owner: -1,
        });
        Ok(())
    }
    fn demolition_effects(&mut self, effects: Vec<Effect>) -> Result<(), String> {
        let tick = self.state.statistics.ticks;
        for effect in effects {
            match &effect {
                Effect::Sound { source } => self.demolition_sound(source)?,
                Effect::DestroyProp { original } => self.disable_dynamic(*original)?,
                Effect::UnblockNavigation { original } if *original == 1094 => {
                    self.state.demolition.as_mut().unwrap().navigation_open = true
                }
                Effect::Blast { damage, radius } => {
                    let p = self.demolition.as_ref().unwrap().assets.charge;
                    let hits = self.blast_damage(p, *damage as f32, *radius as f32)?;
                    for hit in &hits {
                        if hit.victim < 0 {
                            self.state.health = self.state.health.saturating_sub(hit.amount);
                            self.state.damage_flash = 0.2;
                        } else {
                            let e = &mut self.state.enemies[hit.victim as usize];
                            let old = e.health;
                            e.health = e.health.saturating_sub(hit.amount);
                            self.state.statistics.kills += u32::from(old > 0 && e.health == 0);
                        }
                    }
                    let s = self.state.demolition.as_mut().unwrap();
                    s.damage.extend(hits);
                    s.explosion_tick = Some(tick);
                    self.state.bunker.as_mut().unwrap().explosions.push(
                        crate::gunner_runtime::Explosion {
                            position: p,
                            age: 0.,
                            bunker: true,
                        },
                    );
                }
                Effect::Event { tag } if tag == "Conversation05" => {
                    self.state.demolition.as_mut().unwrap().complete_tick = Some(tick)
                }
                _ => {}
            }
            self.state
                .demolition
                .as_mut()
                .unwrap()
                .effects
                .push((tick, effect));
        }
        Ok(())
    }
    pub fn demolition_target(&self) -> Option<Point> {
        let p = self.demolition.as_ref()?;
        let s = self.state.demolition.as_ref()?;
        if s.flow.phase == Stage::Disabled {
            return None;
        }
        Some(if s.flow.phase == Stage::Available {
            p.assets.route[s.guide]
        } else {
            p.assets.charge
        })
    }
    fn demolition_aim(&self, point: Point, range: f64) -> Result<bool, String> {
        let eye = self.eye();
        let d = crate::distance(eye, point);
        if d > range || d < 1. {
            return Ok(false);
        }
        let forward = crate::direction(self.state.yaw, self.state.pitch);
        let dot = (0..3)
            .map(|i| forward[i] * (point[i] - eye[i]) / d)
            .sum::<f64>();
        Ok(dot > 0.85 && self.wall_fraction(eye, point)? >= 0.99)
    }
    pub(crate) fn demolition_tick(&mut self, input: Input) -> Result<(), String> {
        if self.demolition.is_none() {
            return Ok(());
        }
        let enabled = self.state.rendezvous.as_ref().unwrap().flow.charge_enabled
            && self.state.rendezvous.as_ref().unwrap().flow.squad_joined;
        let a = &self.demolition.as_ref().unwrap().assets;
        let s = self.state.demolition.as_mut().unwrap();
        s.flow.enable(&a.flow, enabled)?;
        while s.guide + 1 < a.route.len()
            && crate::distance(self.state.pawn.body.position, a.route[s.guide]) < 45.
        {
            s.guide += 1;
        }
        let pressed = input.use_action && !s.held;
        s.held = input.use_action;
        let mut effects = vec![];
        if pressed {
            let target = self.demolition.as_ref().unwrap().assets.charge;
            let phase = self.state.demolition.as_ref().unwrap().flow.phase;
            let range = if phase == Stage::Available {
                2500.
            } else {
                self.demolition.as_ref().unwrap().assets.flow.player_range
            };
            if self.demolition_aim(target, range)? {
                let eye = self.eye();
                let a = &self.demolition.as_ref().unwrap().assets.flow;
                let s = self.state.demolition.as_mut().unwrap();
                if phase == Stage::Available {
                    effects.extend(s.flow.command(a, 62, true)?);
                    self.state.rendezvous.as_mut().unwrap().gesture = false;
                } else if matches!(phase, Stage::Stopping | Stage::Armed) {
                    s.flow.select_plunger(a, crate::distance(eye, target))?;
                }
            }
        }
        let p = self.demolition.as_ref().unwrap();
        let a = &p.assets;
        let s = self.state.demolition.as_mut().unwrap();
        if s.flow.phase == Stage::Moving {
            let companion = self.state.rendezvous.as_mut().unwrap();
            let target = a.route[s.waypoint];
            let at = companion.pawn.body.position;
            if crate::distance(at, target) < 25. && companion.pawn.body.grounded {
                if s.waypoint + 1 < a.route.len() {
                    s.waypoint += 1;
                } else {
                    s.flow.arrived(&a.flow, 62)?;
                    companion.pawn.body.velocity = [0.; 3];
                    companion.yaw = a.yaw;
                }
            } else {
                companion.yaw = (target[1] - at[1]).atan2(target[0] - at[0]);
                companion.pawn = self
                    .volumes
                    .pawn_diagnostic_tick(
                        &self.world,
                        &companion.pawn,
                        rc_package::controller::ViewInput {
                            forward: 1.,
                            strafe: 0.,
                            yaw: (companion.yaw * 65536. / std::f64::consts::TAU) as i32,
                            walking: false,
                            jump: false,
                        },
                        STEP,
                        &self.profile,
                        rc_package::pawn_motion::PawnRuntimeOptions {
                            maximum_desired_speed: 450.,
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
        let old_phase = s.flow.phase;
        let old_time = s.flow.elapsed;
        let old_setup = s.flow.setup_elapsed;
        effects.extend(s.flow.advance(&a.flow, STEP)?);
        let loop_sequence = &self.rendezvous.as_ref().unwrap().mesh.mesh.clips[self
            .rendezvous
            .as_ref()
            .unwrap()
            .mesh
            .clip("DemolitionSetChargeLoop")?]
        .sequence;
        let loop_duration =
            loop_sequence.frames as f64 / f32::from_bits(loop_sequence.rate_bits) as f64;
        let loop_start = a.flow.start_seconds * (1. - a.flow.place_fraction);
        let loop_at = |setup: f64| ((setup - loop_start).max(0.) / loop_duration) as f32;
        let (clip, old, next) = match old_phase {
            Stage::Starting => (
                "DemolitionSetChargeStart",
                (old_time / a.flow.start_seconds) as f32,
                if s.flow.phase == old_phase {
                    (s.flow.elapsed / a.flow.start_seconds) as f32
                } else {
                    1.
                },
            ),
            Stage::Setting => (
                "DemolitionSetChargeLoop",
                loop_at(old_setup),
                loop_at(s.flow.setup_elapsed),
            ),
            Stage::Stopping => (
                "DemolitionSetChargeStop",
                (old_time / a.flow.stop_seconds) as f32,
                if s.flow.phase == old_phase {
                    (s.flow.elapsed / a.flow.stop_seconds) as f32
                } else {
                    1.
                },
            ),
            _ => ("", 0., 0.),
        };
        let mut sounds = a
            .notifies
            .iter()
            .filter(|n| n.clip == clip && crate::visual::crossed_notify(old, next, n.time))
            .map(|n| n.source.clone())
            .collect::<Vec<_>>();
        // The host can consume a partial next clip in the same simulation step.
        // Include its leading notifies, especially the Stop button at 0.0000435.
        if old_phase != s.flow.phase {
            let incoming = match s.flow.phase {
                Stage::Setting => Some(("DemolitionSetChargeLoop", loop_at(s.flow.setup_elapsed))),
                Stage::Stopping => Some((
                    "DemolitionSetChargeStop",
                    (s.flow.elapsed / a.flow.stop_seconds) as f32,
                )),
                _ => None,
            };
            if let Some((clip, next)) = incoming {
                sounds.extend(
                    a.notifies
                        .iter()
                        .filter(|n| {
                            n.clip == clip
                                && crate::visual::crossed_notify(-f32::EPSILON, next, n.time)
                        })
                        .map(|n| n.source.clone()),
                );
            }
        }
        if s.flow.plunger_selected && input.fire && !s.firing {
            s.firing = true;
            s.hand_frame = 0.;
        }
        if s.firing {
            let seq = &p.hands.mesh.clips[0].sequence;
            let old = if s.hand_started {
                s.hand_frame
            } else {
                -f32::EPSILON
            };
            s.hand_started = true;
            s.hand_frame = (s.hand_frame
                + f32::from_bits(seq.rate_bits) * STEP as f32 / seq.frames as f32)
                .min(1.);
            sounds.extend(
                a.notifies
                    .iter()
                    .filter(|n| {
                        n.clip == "Detonate"
                            && crate::visual::crossed_notify(old, s.hand_frame, n.time)
                    })
                    .map(|n| n.source.clone()),
            );
            if !s.fired && crate::visual::crossed_notify(old, s.hand_frame, a.fire_fraction) {
                s.fired = true;
                effects.extend(s.flow.detonate(&a.flow, true)?);
            }
            if s.hand_frame >= 1. {
                s.firing = false;
            }
        }
        for source in sounds {
            self.demolition_sound(&source)?;
        }
        self.demolition_effects(effects)?;
        let s = self.state.demolition.as_ref().unwrap();
        if s.complete_tick.is_some() && !s.firing && self.state.health > 0 && self.spire.is_none() {
            self.state.phase = crate::Phase::Won;
        }
        Ok(())
    }
    pub(crate) fn demolition_clip(&self) -> Option<(&'static str, f32)> {
        let s = self.state.demolition.as_ref()?;
        let a = &self.demolition.as_ref()?.assets.flow;
        Some(match s.flow.phase {
            Stage::Moving => ("RunForward", self.state.rendezvous.as_ref()?.frame),
            Stage::Starting => (
                "DemolitionSetChargeStart",
                (s.flow.elapsed / a.start_seconds).min(0.999) as f32,
            ),
            Stage::Setting => {
                let mesh = &self.rendezvous.as_ref()?.mesh;
                let seq = &mesh.mesh.clips[mesh.clip("DemolitionSetChargeLoop").ok()?].sequence;
                let duration = seq.frames as f64 / f32::from_bits(seq.rate_bits) as f64;
                (
                    "DemolitionSetChargeLoop",
                    (((s.flow.setup_elapsed - a.start_seconds * (1. - a.place_fraction)).max(0.)
                        / duration)
                        % 1.) as f32,
                )
            }
            Stage::Stopping => (
                "DemolitionSetChargeStop",
                (s.flow.elapsed / a.stop_seconds).min(0.999) as f32,
            ),
            Stage::Armed | Stage::Exploded => {
                ("ActionBreathe", self.state.rendezvous.as_ref()?.frame)
            }
            _ => return None,
        })
    }
    pub(crate) fn demolition_geometry(
        &self,
        first_person: bool,
        view: rc_render::View,
    ) -> Result<
        (
            Vec<rc_package::geometry::Triangle>,
            Vec<Option<rc_render::FaceTexture>>,
        ),
        String,
    > {
        let (Some(p), Some(s)) = (&self.demolition, &self.state.demolition) else {
            return Ok((vec![], vec![]));
        };
        let mut tris = vec![];
        let mut faces = vec![];
        if first_person {
            if !s.flow.plunger_selected && !s.firing {
                return Ok((tris, faces));
            }
            // Detonate starts with withdrawn hands. Hold its raised preparation pose until firing.
            let channel = p.hands.channel(
                0,
                if s.firing {
                    s.hand_frame.min(0.999)
                } else {
                    0.4
                },
            )?;
            let hands = p.hands.geometry(&channel)?;
            tris.extend(hands.triangles);
            faces.extend(hands.appearance.into_iter().map(|f| {
                f.map(|mut f| {
                    f.texture += p.hands_base;
                    f
                })
            }));
            let transform = p
                .hands
                .attachment_transform(&channel, "weaponAttach_L", false)?;
            for t in &p.plunger.triangles {
                let mut t = t.clone();
                t.points = t.points.map(|v| {
                    std::array::from_fn(|i| {
                        v[0] * transform[0][i]
                            + v[1] * transform[1][i]
                            + v[2] * transform[2][i]
                            + transform[3][i]
                    })
                });
                tris.push(t);
            }
            faces.extend(p.plunger.appearance.iter().copied());
            let [eye, right, up, forward] = rc_render::gpu::basis(&self.scene, view);
            for t in &mut tris {
                t.points = t.points.map(|v| {
                    std::array::from_fn(|i| {
                        eye[i] + forward[i] * v[0] + right[i] * v[1] + up[i] * v[2]
                    })
                });
            }
        } else if s.flow.phase == Stage::Starting && !s.flow.placed {
            let scorch = self.rendezvous.as_ref().unwrap();
            let pawn = self.state.rendezvous.as_ref().unwrap();
            let (clip, frame) = self.demolition_clip().unwrap();
            let channel = scorch.mesh.channel(scorch.mesh.clip(clip)?, frame)?;
            let transform = scorch
                .mesh
                .attachment_transform(&channel, "weaponAttach_L", true)?;
            let (sin, cos) = (pawn.yaw as f32).sin_cos();
            for triangle in &p.charges[0].triangles {
                let mut triangle = triangle.clone();
                triangle.points = triangle.points.map(|v| {
                    let local: [f32; 3] = std::array::from_fn(|i| {
                        v[0] * transform[0][i]
                            + v[1] * transform[1][i]
                            + v[2] * transform[2][i]
                            + transform[3][i]
                    });
                    [
                        pawn.pawn.body.position[0] as f32 + local[0] * cos - local[1] * sin,
                        pawn.pawn.body.position[1] as f32 + local[0] * sin + local[1] * cos,
                        pawn.pawn.body.position[2] as f32 - 84. + local[2]
                            - scorch.mesh.rest_bounds[0][2],
                    ]
                });
                tris.push(triangle);
            }
            faces.extend(p.charges[0].appearance.iter().copied());
        } else if s.flow.placed && s.flow.phase != Stage::Exploded {
            let charge =
                &p.charges[usize::from(matches!(s.flow.phase, Stage::Stopping | Stage::Armed))];
            let (sin, cos) = p.assets.yaw.sin_cos();
            for t in &charge.triangles {
                let mut t = t.clone();
                t.points = t.points.map(|v| {
                    [
                        p.assets.charge[0] as f32 + v[0] * cos as f32 - v[1] * sin as f32,
                        p.assets.charge[1] as f32 + v[0] * sin as f32 + v[1] * cos as f32,
                        p.assets.charge[2] as f32 + v[2],
                    ]
                });
                tris.push(t);
            }
            faces.extend(charge.appearance.iter().copied());
        }
        Ok((tris, faces))
    }
}
