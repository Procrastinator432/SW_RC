//! Selected Sun Fac run, original fighter success/failure clips and bounded sniper host.
use crate::{
    map3_opening::{self, Animation},
    map3_planning::Factory,
    visual::{Mesh, PreparedMesh},
    Enemy, Game, Phase, Point, STEP,
};
use serde::{Deserialize, Serialize};
#[derive(Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Cue {
    pub original: u32,
    pub sound: usize,
    pub seconds: f64,
    pub text: String,
}
#[derive(Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Assets {
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub elevator: Option<Box<crate::map3_elevator::Assets>>,
    pub ship: Option<Mesh>,
    pub ship_anchor: Point,
    pub ship_rotation: [i32; 3],
    pub wreck_snapshot: Vec<u8>,
    pub sniper_snapshot: Vec<u8>,
    pub run: Vec<Point>,
    pub board: Vec<Point>,
    pub climber: Factory,
    pub reminders: Vec<Cue>,
    pub start: Cue,
    pub takeoff: Cue,
    pub failures: Vec<Cue>,
    pub sniper_sound: usize,
    pub shot_delay: f64,
    pub elevator_target: Point,
    pub provenance: serde_json::Value,
}
pub struct Prepared {
    pub elevator: Option<Box<crate::map3_elevator::Prepared>>,
    pub assets: Assets,
    pub ship: PreparedMesh,
    pub ship_texture: usize,
    pub wreck: rc_render::Scene,
    pub wreck_texture: usize,
    pub sniper: rc_render::Scene,
    pub sniper_texture: usize,
}
#[derive(Clone, Debug, Serialize, serde::Deserialize)]
pub struct State {
    #[serde(skip_serializing_if = "Option::is_none")]
    pub elevator: Option<Box<crate::map3_elevator::State>>,
    pub sun_stage: u8,
    pub waypoint: usize,
    pub sun_animation: Animation,
    pub sun_anchor: Point,
    pub sun_rotation: [i32; 3],
    pub blocked: u64,
    pub reminder_started: bool,
    pub reminder_index: usize,
    pub reminder_wait: f64,
    pub start_tick: Option<u64>,
    pub start_voice: f64,
    pub takeoff_tick: Option<u64>,
    pub ship_stage: u8,
    pub ship_animation: Animation,
    pub ship_position: Point,
    pub ship_visible: bool,
    pub wreck_visible: bool,
    pub success: Option<bool>,
    pub sev_animation: Animation,
    pub shooting: f64,
    pub shot_cooldown: f64,
    pub shots: u32,
    pub climber: Option<usize>,
    pub terminated_tick: Option<u64>,
    pub failed_tick: Option<u64>,
    pub failure_voice: usize,
    pub failure_wait: f64,
}
fn animation(clip: &str, position: Point) -> Animation {
    Animation {
        random: None,
        clip: clip.into(),
        playback: map3_opening::empty_playback(),
        finished: true,
        visible: true,
        blocked: 0,
        position,
        notifies: vec![],
        root: false,
    }
}
fn cue(
    cue: &Cue,
    sequence: &mut u64,
    events: &mut Vec<crate::combat::Event>,
    tick: u64,
    subtitle: &mut String,
    until: &mut f64,
    clock: f64,
) {
    *sequence += 1;
    map3_opening::queue_audio(*sequence, tick, cue.sound, events);
    *subtitle = cue.text.clone();
    *until = clock + cue.seconds;
}
impl Game {
    pub(crate) fn finale_prepared(&self) -> Option<&Prepared> {
        self.balcony_prepared()?.finale.as_deref()
    }
    pub(crate) fn finale_state(&self) -> Option<&State> {
        self.balcony_state()?.finale.as_deref()
    }
    pub(crate) fn prepare_map3_finale(&mut self) -> Result<(), String> {
        let Some(p) = self
            .map
            .as_mut()
            .and_then(|m| m.opening.as_mut())
            .and_then(|p| p.planning.as_mut())
            .and_then(|p| p.guard.as_mut())
            .and_then(|p| p.breach.as_mut())
            .and_then(|p| p.balcony.as_mut())
        else {
            return Ok(());
        };
        let Some(mut a) = p.assets.finale.take() else {
            return Ok(());
        };
        let sounds = self.map.as_ref().unwrap().assets.sounds.len();
        if a.reminders.len() != 4
            || a.failures.len() != 2
            || a.run.is_empty()
            || a.run.len() > 4096
            || a.board.is_empty()
            || a.board.len() > 2048
            || a.climber.original != 141
            || !a.climber.warrior
            || a.climber.health == 0
            || a.climber.health > 10000
            || a.climber
                .position
                .iter()
                .chain(a.climber.extent.iter())
                .any(|v| !v.is_finite() || v.abs() > 1e7)
            || a.climber.extent.iter().any(|v| *v <= 0. || *v > 250.)
            || a.run
                .iter()
                .flatten()
                .chain(a.board.iter().flatten())
                .chain(a.ship_anchor.iter())
                .chain(a.elevator_target.iter())
                .any(|v| !v.is_finite() || v.abs() > 1e7)
            || a.sniper_sound >= sounds
            || !a.shot_delay.is_finite()
            || !(0.1..=5.).contains(&a.shot_delay)
            || a.reminders
                .iter()
                .chain(a.failures.iter())
                .chain([&a.start, &a.takeoff])
                .any(|c| {
                    c.sound >= sounds
                        || !c.seconds.is_finite()
                        || !(0. ..=120.).contains(&c.seconds)
                })
            || a.wreck_snapshot.len() > 4 * 1024 * 1024
            || a.sniper_snapshot.len() > 4 * 1024 * 1024
        {
            return Err("Invalid C finale bindings".into());
        }
        let ship = a
            .ship
            .take()
            .ok_or("Missing original fighter")?
            .prepare_reference()?;
        for clip in [
            "SevKillSunFacIntro",
            "SevKillSunFacIdle",
            "SevKillSunFacDeath",
            "SevKillSunFacFailMission",
        ] {
            ship.clip(clip)?;
        }
        let ship_texture = self.scene.textures.len();
        self.scene
            .textures
            .extend(ship.mesh.materials.iter().map(|m| rc_render::Texture {
                width: m.width,
                height: m.height,
                pixels: m.pixels.clone(),
            }));
        let wreck = rc_render::Scene::from_snapshot(&a.wreck_snapshot)?;
        let wreck_texture = self.scene.textures.len();
        self.scene.textures.extend(wreck.textures.iter().cloned());
        let sniper = rc_render::Scene::from_snapshot(&a.sniper_snapshot)?;
        let sniper_texture = self.scene.textures.len();
        self.scene.textures.extend(sniper.textures.iter().cloned());
        let opening = self.map.as_ref().unwrap().opening.as_ref().unwrap();
        for clip in ["Stunned", "EmoteHearEnemy", "WalkForward"] {
            opening.actors[5].0.clip(clip)?;
        }
        opening.actors[2].0.clip("FireSniper")?;
        let ship_animation = animation("SevKillSunFacIntro", a.ship_anchor);
        let ship_position =
            map3_opening::authored(&ship, &ship_animation, a.ship_anchor, a.ship_rotation)?;
        let state = State {
            elevator: None,
            sun_stage: 0,
            waypoint: 0,
            sun_animation: animation("Stunned", a.run[0]),
            sun_anchor: a.run[0],
            sun_rotation: [0; 3],
            blocked: 0,
            reminder_started: false,
            reminder_index: 0,
            reminder_wait: 0.,
            start_tick: None,
            start_voice: 0.,
            takeoff_tick: None,
            ship_stage: 0,
            ship_animation,
            ship_position,
            ship_visible: true,
            wreck_visible: false,
            success: None,
            sev_animation: animation("FireSniper", [0.; 3]),
            shooting: 0.,
            shot_cooldown: 0.,
            shots: 0,
            climber: None,
            terminated_tick: None,
            failed_tick: None,
            failure_voice: 0,
            failure_wait: 0.,
        };
        self.state
            .map
            .as_mut()
            .unwrap()
            .opening
            .as_mut()
            .unwrap()
            .planning
            .as_mut()
            .unwrap()
            .guard
            .as_mut()
            .unwrap()
            .breach
            .as_mut()
            .unwrap()
            .balcony
            .as_mut()
            .unwrap()
            .finale = Some(Box::new(state));
        self.map
            .as_mut()
            .unwrap()
            .opening
            .as_mut()
            .unwrap()
            .planning
            .as_mut()
            .unwrap()
            .guard
            .as_mut()
            .unwrap()
            .breach
            .as_mut()
            .unwrap()
            .balcony
            .as_mut()
            .unwrap()
            .finale = Some(Box::new(Prepared {
            elevator: None,
            assets: *a,
            ship,
            ship_texture,
            wreck,
            wreck_texture,
            sniper,
            sniper_texture,
        }));
        Ok(())
    }
    pub(crate) fn map3_finale_spec(&self, i: usize) -> Option<crate::map3_guard::Spec> {
        if self.finale_state()?.climber != Some(i) {
            return None;
        }
        Some(crate::map3_guard::Spec {
            extent: self.finale_prepared()?.assets.climber.extent,
            warrior: true,
            sbd: false,
            active: true,
            fire_sound: None,
        })
    }
    pub(crate) fn map3_finale_tick(&mut self) -> Result<(), String> {
        let Some(p) = self
            .map
            .as_ref()
            .and_then(|m| m.opening.as_ref())
            .and_then(|p| p.planning.as_ref())
            .and_then(|p| p.guard.as_ref())
            .and_then(|p| p.breach.as_ref())
            .and_then(|p| p.balcony.as_ref())
            .and_then(|p| p.finale.as_ref())
        else {
            return Ok(());
        };
        if self.map3_locked() {
            return Ok(());
        }
        let a = &p.assets;
        let ship_mesh = &p.ship;
        let opening = self.map.as_ref().unwrap().opening.as_ref().unwrap();
        let sun_mesh = &opening.actors[5].0;
        let sev_mesh = &opening.actors[2].0;
        let m = self.state.map.as_mut().unwrap();
        let o = m.opening.as_mut().unwrap();
        let planning = o.planning.as_mut().unwrap();
        let escape = planning.escape.as_mut().unwrap();
        let balcony = planning
            .guard
            .as_mut()
            .unwrap()
            .breach
            .as_mut()
            .unwrap()
            .balcony
            .as_mut()
            .unwrap();
        let s = balcony.finale.as_mut().unwrap();
        let tick = self.state.statistics.ticks + 1;
        if !s.reminder_started && o.events.contains("SnipeReminder") {
            s.reminder_started = true;
            s.reminder_wait = 2.;
        }
        if s.reminder_started && s.start_tick.is_none() {
            s.reminder_wait = (s.reminder_wait - STEP).max(0.);
            if s.reminder_wait < 1e-8 {
                if o.events.contains("InSnipePosition") || s.reminder_index == a.reminders.len() {
                    s.start_tick = Some(tick);
                    s.start_voice = a.start.seconds;
                    cue(
                        &a.start,
                        &mut m.audio_sequence,
                        &mut m.events,
                        tick,
                        &mut o.subtitle,
                        &mut o.subtitle_until,
                        o.clock,
                    );
                    for event in [
                        "StartSunFacRunning",
                        "SunFacRunToShip",
                        "SpawnClimbOverWall",
                    ] {
                        o.events.insert(event.into());
                        o.event_ticks.insert(event.into(), tick);
                    }
                    let i = self.state.enemies.len();
                    self.state.enemies.push(Enemy {
                        position: a.climber.position,
                        health: a.climber.health,
                        cooldown: 1.4,
                        flash: 0.,
                    });
                    s.climber = Some(i);
                } else {
                    let c = &a.reminders[s.reminder_index];
                    cue(
                        c,
                        &mut m.audio_sequence,
                        &mut m.events,
                        tick,
                        &mut o.subtitle,
                        &mut o.subtitle_until,
                        o.clock,
                    );
                    s.reminder_wait = c.seconds + 2.;
                    s.reminder_index += 1;
                }
            }
        }
        if s.start_tick.is_some() && s.start_voice > 0. {
            s.start_voice = (s.start_voice - STEP).max(0.);
            if s.start_voice < 1e-8 {
                o.objectives[5] = true;
                o.active = 6;
            }
        }
        if s.sun_stage == 0 && balcony.stage == 6 {
            s.sun_stage = 1;
            s.waypoint = 0;
        }
        if s.sun_stage == 1 || s.sun_stage == 5 {
            let route = if s.sun_stage == 1 { &a.run } else { &a.board };
            escape.moving = false;
            if let Some(target) = route.get(s.waypoint) {
                let from = escape.position;
                if crate::distance(from, *target) < 5. {
                    s.waypoint += 1;
                } else {
                    let d = crate::distance(from, *target);
                    let speed = if s.sun_stage == 5 { 180. } else { 450. };
                    let wanted = std::array::from_fn(|i| {
                        from[i] + (target[i] - from[i]) * (speed * STEP / d).min(1.)
                    });
                    if let Some(to) = crate::map3_balcony::sun_step(&self.world, from, wanted)? {
                        escape.position = to;
                        escape.yaw = (target[1] - from[1]).atan2(target[0] - from[0]);
                        escape.moving = crate::distance(from, to) > 0.01;
                    } else {
                        s.blocked += 1;
                    }
                }
            } else if s.sun_stage == 1 {
                s.sun_stage = 2;
                escape.moving = false;
            } else {
                s.sun_stage = 6;
                s.takeoff_tick = Some(tick);
                o.animations[6].visible = false;
                escape.moving = false;
                o.events.insert("TakeOff".into());
                o.event_ticks.insert("TakeOff".into(), tick);
                cue(
                    &a.takeoff,
                    &mut m.audio_sequence,
                    &mut m.events,
                    tick,
                    &mut o.subtitle,
                    &mut o.subtitle_until,
                    o.clock,
                );
                s.ship_stage = 1;
                map3_opening::begin(ship_mesh, &mut s.ship_animation, "SevKillSunFacIntro")?;
                if balcony.positioned_tick.is_some() && !o.sev.down {
                    s.shooting = 5.;
                }
            }
        }
        if s.sun_stage == 2 && s.start_tick.is_some() {
            s.sun_stage = 3;
            s.sun_anchor = escape.position;
            s.sun_rotation = [0, (escape.yaw * 65536. / std::f64::consts::TAU) as i32, 0];
            map3_opening::begin(sun_mesh, &mut s.sun_animation, "Stunned")?;
        }
        if s.sun_stage == 3 || s.sun_stage == 4 {
            escape.moving = false;
            map3_opening::tick_animation(sun_mesh, &mut s.sun_animation, tick)?;
            let to =
                map3_opening::authored(sun_mesh, &s.sun_animation, s.sun_anchor, s.sun_rotation)?;
            let wanted = [to[0], to[1], escape.position[2]];
            if let Some(at) = crate::map3_balcony::sun_step(&self.world, escape.position, wanted)? {
                escape.position = at;
            } else {
                s.blocked += 1;
            }
            if s.sun_animation.finished {
                if s.sun_stage == 3 {
                    s.sun_stage = 4;
                    s.sun_anchor = escape.position;
                    map3_opening::begin(sun_mesh, &mut s.sun_animation, "EmoteHearEnemy")?;
                } else {
                    s.sun_stage = 5;
                    s.waypoint = 0;
                }
            }
        }
        if s.sun_stage > 0 {
            o.animations[6].position = escape.position;
        }
        if s.ship_stage > 0 && s.ship_stage < 5 {
            for id in map3_opening::tick_animation(ship_mesh, &mut s.ship_animation, tick)? {
                if id == 130 {
                    s.ship_visible = false;
                    s.wreck_visible = true;
                    s.terminated_tick = Some(tick);
                    for event in ["KillMe", "SunFacTerminated"] {
                        o.events.insert(event.into());
                        o.event_ticks.insert(event.into(), tick);
                    }
                    o.objectives[0] = true;
                }
            }
            s.ship_position = map3_opening::authored(
                ship_mesh,
                &s.ship_animation,
                a.ship_anchor,
                a.ship_rotation,
            )?;
            if s.ship_animation.finished {
                match s.ship_stage {
                    1 => {
                        s.ship_stage = 2;
                        map3_opening::begin(ship_mesh, &mut s.ship_animation, "SevKillSunFacIdle")?;
                    }
                    2 => {
                        o.events.insert("SnipeSunFac".into());
                        o.event_ticks.insert("SnipeSunFac".into(), tick);
                        let success =
                            balcony.positioned_tick.is_some() && o.sev.joined && !o.sev.down;
                        s.success = Some(success);
                        s.ship_stage = if success { 3 } else { 4 };
                        if success {
                            o.events.insert("SunFacDeathMusicQueue".into());
                            o.event_ticks.insert("SunFacDeathMusicQueue".into(), tick);
                        }
                        map3_opening::begin(
                            ship_mesh,
                            &mut s.ship_animation,
                            if success {
                                "SevKillSunFacDeath"
                            } else {
                                "SevKillSunFacFailMission"
                            },
                        )?;
                    }
                    3 => {
                        s.ship_stage = 5;
                    }
                    4 => {
                        s.ship_stage = 6;
                        s.failed_tick = Some(tick);
                        o.events.insert("MissionFailed".into());
                        o.event_ticks.insert("MissionFailed".into(), tick);
                    }
                    _ => {}
                }
            }
        }
        s.shot_cooldown = (s.shot_cooldown - STEP).max(0.);
        if s.shooting > 0. {
            s.shooting = (s.shooting - STEP).max(0.);
            if s.shot_cooldown < 1e-8 && !o.sev.down {
                map3_opening::begin(sev_mesh, &mut s.sev_animation, "FireSniper")?;
                s.shots += 1;
                s.shot_cooldown = a.shot_delay;
                m.audio_sequence += 1;
                map3_opening::queue_audio(m.audio_sequence, tick, a.sniper_sound, &mut m.events);
            }
            o.sev.yaw = (s.ship_position[1] - o.sev.position[1])
                .atan2(s.ship_position[0] - o.sev.position[0]);
        }
        map3_opening::tick_animation(sev_mesh, &mut s.sev_animation, tick)?;
        if s.ship_stage == 6 {
            s.failure_wait = (s.failure_wait - STEP).max(0.);
            if s.failure_wait < 1e-8 {
                if let Some(c) = a.failures.get(s.failure_voice) {
                    cue(
                        c,
                        &mut m.audio_sequence,
                        &mut m.events,
                        tick,
                        &mut o.subtitle,
                        &mut o.subtitle_until,
                        o.clock,
                    );
                    s.failure_wait = c.seconds
                        + if s.failure_voice + 1 == a.failures.len() {
                            0.2
                        } else {
                            0.
                        };
                    s.failure_voice += 1;
                } else {
                    s.ship_stage = 7;
                    self.state.health = 0;
                    self.state.phase = Phase::Lost;
                }
            }
        }
        if s.terminated_tick.is_some() {
            o.active = 6;
        }
        Ok(())
    }
    pub(crate) fn map3_finale_sun_pose(&self) -> Result<Option<(usize, f32)>, String> {
        let Some(s) = self.finale_state() else {
            return Ok(None);
        };
        let opening = self.map.as_ref().unwrap().opening.as_ref().unwrap();
        let mesh = &opening.actors[5].0;
        if s.sun_stage == 3 || s.sun_stage == 4 {
            return Ok(Some((
                mesh.clip(&s.sun_animation.clip)?,
                map3_opening::anim_frame(&s.sun_animation, mesh)?,
            )));
        }
        if s.sun_stage == 5 {
            let clip = mesh.clip("WalkForward")?;
            let seq = &mesh.mesh.clips[clip].sequence;
            let clock = self
                .state
                .map
                .as_ref()
                .unwrap()
                .opening
                .as_ref()
                .unwrap()
                .clock;
            return Ok(Some((
                clip,
                (clock * f32::from_bits(seq.rate_bits) as f64 / seq.frames as f64).rem_euclid(1.)
                    as f32,
            )));
        }
        Ok(None)
    }
    pub(crate) fn map3_finale_sev_pose(&self) -> Result<Option<(usize, f32)>, String> {
        let Some(s) = self.finale_state() else {
            return Ok(None);
        };
        let o = self.state.map.as_ref().unwrap().opening.as_ref().unwrap();
        if !o.events.contains("InSnipePosition") || o.events.contains("DisableBalconySnipe") {
            return Ok(None);
        }
        let mesh = &self.map.as_ref().unwrap().opening.as_ref().unwrap().actors[2].0;
        if !s.sev_animation.finished {
            return Ok(Some((
                mesh.clip("FireSniper")?,
                map3_opening::anim_frame(&s.sev_animation, mesh)?,
            )));
        }
        let clip = mesh.clip("CrouchBreathe")?;
        let seq = &mesh.mesh.clips[clip].sequence;
        Ok(Some((
            clip,
            (o.clock * f32::from_bits(seq.rate_bits) as f64 / seq.frames as f64).rem_euclid(1.)
                as f32,
        )))
    }
    pub(crate) fn map3_finale_weapon(&self) -> Option<(&rc_render::Scene, usize)> {
        let o = self.state.map.as_ref()?.opening.as_ref()?;
        if !o.events.contains("InSnipePosition") || o.events.contains("DisableBalconySnipe") {
            return None;
        }
        let p = self.finale_prepared()?;
        Some((&p.sniper, p.sniper_texture))
    }
    pub(crate) fn map3_finale_status(&self) -> Option<String> {
        let s = self.finale_state()?;
        if self
            .elevator_state()
            .is_some_and(|e| e.completed_tick.is_some())
        {
            return Some("GEO_01C · Mission abgeschlossen · GEO_03A".into());
        }
        if s.terminated_tick.is_some() {
            return Some("GEO_01C · Sun Fac ausgeschaltet · Zum Aufzug".into());
        }
        if s.failed_tick.is_some() {
            return Some("GEO_01C · Sun Fac entkommen · Mission fehlgeschlagen".into());
        }
        if s.takeoff_tick.is_some() {
            return Some("GEO_01C · Sun Facs Jaeger · Sev decken".into());
        }
        s.start_tick
            .map(|_| "GEO_01C · Sun Fac laeuft zum Jaeger · Sev decken".into())
    }
    pub(crate) fn map3_finale_target(&self) -> Option<Point> {
        let s = self.finale_state()?;
        let p = self.finale_prepared()?;
        if s.terminated_tick.is_some() {
            Some(p.assets.elevator_target)
        } else if s.start_tick.is_some() {
            Some(s.ship_position)
        } else {
            None
        }
    }
    pub(crate) fn map3_finale_geometry(
        &self,
    ) -> Result<
        (
            Vec<rc_package::geometry::Triangle>,
            Vec<Option<rc_render::FaceTexture>>,
        ),
        String,
    > {
        let Some(p) = self.finale_prepared() else {
            return Ok((vec![], vec![]));
        };
        let s = self.finale_state().unwrap();
        let mut ts = vec![];
        let mut fs = vec![];
        if s.ship_visible {
            let clip = p.ship.clip(&s.ship_animation.clip)?;
            let channel = p
                .ship
                .channel(clip, map3_opening::anim_frame(&s.ship_animation, &p.ship)?)?;
            let mut g = p.ship.geometry_with_root(&channel, true)?;
            for t in &mut g.triangles {
                t.points = t.points.map(|v| {
                    rc_package::mesh::transform_point(
                        v,
                        s.ship_position.map(|v| v as f32),
                        p.assets.ship_rotation,
                        [1.; 3],
                        [0.; 3],
                    )
                });
            }
            ts.extend(g.triangles);
            fs.extend(g.appearance.into_iter().map(|f| {
                f.map(|mut f| {
                    f.texture += p.ship_texture;
                    f
                })
            }));
        }
        if s.wreck_visible {
            ts.extend(p.wreck.triangles.iter().cloned());
            fs.extend(p.wreck.appearance.iter().map(|f| {
                f.map(|mut f| {
                    f.texture += p.wreck_texture;
                    f
                })
            }));
        }
        Ok((ts, fs))
    }
}
