//! Selected original guard-room intro and bounded encounter/focus host.
use crate::{
    map3_opening::{self, Animation, Notify},
    map3_planning::Factory,
    visual::{Mesh, PreparedMesh},
    Enemy, Game, Input, Point,
};
use serde::{Deserialize, Serialize};
#[derive(Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Actor {
    pub original: u32,
    pub sbd: bool,
    pub position: Point,
    pub rotation: [i32; 3],
    pub clip: String,
    pub height: f64,
    pub sounds: Vec<Notify>,
}
#[derive(Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Spawn {
    pub factory: Factory,
    pub sbd: bool,
}
#[derive(Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Assets {
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub breach: Option<Box<crate::map3_breach::Assets>>,
    pub fire_sound: usize,
    pub mesh: Option<Mesh>,
    pub actors: Vec<Actor>,
    pub factories: Vec<Spawn>,
    pub next_target: Point,
    pub provenance: serde_json::Value,
}
pub struct Prepared {
    pub breach: Option<Box<crate::map3_breach::Prepared>>,
    pub assets: Assets,
    pub mesh: PreparedMesh,
    pub texture: usize,
}
#[derive(Clone, Debug, Serialize, serde::Deserialize)]
pub struct State {
    #[serde(skip_serializing_if = "Option::is_none")]
    pub breach: Option<Box<crate::map3_breach::State>>,
    pub spawned: bool,
    pub intro_tick: Option<u64>,
    pub animations: Vec<Animation>,
    pub bindings: Vec<(usize, usize)>,
    pub sbd: Option<usize>,
    pub focus: Option<usize>,
    pub focus_commands: u32,
    pub destroyed_tick: Option<u64>,
}
#[derive(Clone, Copy)]
pub(crate) struct Spec {
    pub extent: Point,
    pub warrior: bool,
    pub sbd: bool,
    pub active: bool,
    pub fire_sound: Option<usize>,
}
impl Game {
    pub(crate) fn prepare_map3_guard(&mut self) -> Result<(), String> {
        let Some(mut a) = self
            .map
            .as_mut()
            .and_then(|m| m.opening.as_mut())
            .and_then(|p| p.planning.as_mut())
            .and_then(|p| p.assets.guard.take())
        else {
            return Ok(());
        };
        let valid_point = |p: Point| p.iter().all(|v| v.is_finite() && v.abs() < 1e7);
        if a.fire_sound >= self.map.as_ref().unwrap().assets.sounds.len()
            || a.actors.iter().map(|x| x.original).ne([321, 322, 323])
            || a.factories
                .iter()
                .map(|x| x.factory.original)
                .ne([196, 197, 228, 229, 230, 231, 232, 233])
            || !valid_point(a.next_target)
            || a.factories.iter().any(|f| {
                !valid_point(f.factory.position)
                    || f.factory.health == 0
                    || f.factory.health > 10000
                    || f.factory
                        .extent
                        .iter()
                        .any(|v| !v.is_finite() || *v <= 0. || *v > 250.)
            })
        {
            return Err("Invalid guard-room bindings".into());
        }
        let mesh = a
            .mesh
            .take()
            .ok_or("Missing SBD mesh")?
            .prepare_reference()?;
        for clip in ["IntroSBD", "ActionBreathe", "WalkForward"] {
            mesh.clip(clip)?;
        }
        let texture = self.scene.textures.len();
        self.scene
            .textures
            .extend(mesh.mesh.materials.iter().map(|m| rc_render::Texture {
                width: m.width,
                height: m.height,
                pixels: m.pixels.clone(),
            }));
        let p = self
            .map
            .as_ref()
            .unwrap()
            .opening
            .as_ref()
            .unwrap()
            .planning
            .as_ref()
            .unwrap();
        let mut animations = vec![];
        for actor in &a.actors {
            if !valid_point(actor.position)
                || !actor.height.is_finite()
                || !(1. ..=200.).contains(&actor.height)
                || actor
                    .sounds
                    .iter()
                    .any(|n| n.sound >= self.map.as_ref().unwrap().assets.sounds.len())
            {
                return Err("Invalid intro actor binding".into());
            }
            let m = if actor.sbd { &mesh } else { &p.meshes[0].0 };
            m.clip(&actor.clip)?;
            animations.push(Animation {
                random: None,
                clip: actor.clip.clone(),
                playback: map3_opening::empty_playback(),
                finished: true,
                visible: false,
                blocked: 0,
                position: actor.position,
                notifies: vec![],
                root: false,
            });
        }
        let s = State {
            breach: None,
            spawned: false,
            intro_tick: None,
            animations,
            bindings: vec![],
            sbd: None,
            focus: None,
            focus_commands: 0,
            destroyed_tick: None,
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
            .guard = Some(Box::new(s));
        self.map
            .as_mut()
            .unwrap()
            .opening
            .as_mut()
            .unwrap()
            .planning
            .as_mut()
            .unwrap()
            .guard = Some(Box::new(Prepared {
            breach: None,
            assets: *a,
            mesh,
            texture,
        }));
        Ok(())
    }
    pub(crate) fn map3_combat_spec(&self, i: usize) -> Option<Spec> {
        if let Some(spec) = self.map3_finale_spec(i) {
            return Some(spec);
        }
        if let Some(spec) = self.map3_balcony_spec(i) {
            return Some(spec);
        }
        let p = self.map.as_ref()?.opening.as_ref()?.planning.as_ref()?;
        if let Some(f) = p.assets.factories.get(i) {
            return Some(Spec {
                extent: f.extent,
                warrior: f.warrior,
                sbd: false,
                active: true,
                fire_sound: None,
            });
        }
        let o = self.state.map.as_ref()?.opening.as_ref()?;
        let g = p.guard.as_ref()?;
        let s = o.planning.as_ref()?.guard.as_ref()?;
        let slot = s.bindings.iter().find(|(global, _)| *global == i)?.1;
        if slot == 8 {
            return Some(Spec {
                extent: [65., 65., 115.],
                warrior: false,
                sbd: true,
                active: true,
                fire_sound: Some(g.assets.fire_sound),
            });
        }
        let f = &g.assets.factories[slot];
        Some(Spec {
            extent: f.factory.extent,
            warrior: f.factory.warrior,
            sbd: f.sbd,
            active: o.events.contains("GuardRoomDroids"),
            fire_sound: (!f.factory.warrior).then_some(g.assets.fire_sound),
        })
    }
    pub(crate) fn map3_guard_tick(&mut self, input: Input) -> Result<(), String> {
        let Some(p) = self
            .map
            .as_ref()
            .and_then(|m| m.opening.as_ref())
            .and_then(|p| p.planning.as_ref())
        else {
            return Ok(());
        };
        let Some(g) = p.guard.as_ref() else {
            return Ok(());
        };
        let eye = self.eye();
        let direction = crate::direction(self.state.yaw, self.state.pitch);
        let end = std::array::from_fn(|i| eye[i] + direction[i] * 2500.);
        let mut focus = None;
        if input.use_action {
            let mut nearest = self.wall_fraction(eye, end)?;
            for (i, e) in self
                .state
                .enemies
                .iter()
                .enumerate()
                .filter(|(_, e)| e.health > 0)
            {
                if let Some(spec) = self.map3_combat_spec(i) {
                    if let Some(f) = crate::ray_box(eye, end, e.position, spec.extent) {
                        if f < nearest {
                            nearest = f;
                            focus = Some(i);
                        }
                    }
                }
            }
        }
        let tick = self.state.statistics.ticks + 1;
        let m = self.state.map.as_mut().unwrap();
        let o = m.opening.as_mut().unwrap();
        if o.locked {
            return Ok(());
        }
        let s = o.planning.as_mut().unwrap().guard.as_mut().unwrap();
        if let Some(target) = focus {
            if s.focus != Some(target) {
                s.focus_commands += 1;
            }
            s.focus = Some(target);
        }
        if s.focus.is_some_and(|i| self.state.enemies[i].health == 0) {
            s.focus = None;
        }
        if !s.spawned && o.events.contains("SpawnGuardRoom") {
            for (slot, f) in g.assets.factories.iter().enumerate() {
                let i = self.state.enemies.len();
                self.state.enemies.push(Enemy {
                    position: f.factory.position,
                    health: f.factory.health,
                    cooldown: 1.4,
                    flash: 0.,
                });
                s.bindings.push((i, slot));
            }
            s.spawned = true;
        }
        if s.intro_tick.is_none() && o.events.contains("SBDIntro") {
            s.intro_tick = Some(tick);
            for (a, state) in g.assets.actors.iter().zip(&mut s.animations) {
                let mesh = if a.sbd { &g.mesh } else { &p.meshes[0].0 };
                map3_opening::begin(mesh, state, &a.clip)?;
                state.visible = true;
            }
        }
        if s.intro_tick.is_some() {
            for (index, (a, state)) in g.assets.actors.iter().zip(&mut s.animations).enumerate() {
                if state.finished {
                    continue;
                }
                let mesh = if a.sbd { &g.mesh } else { &p.meshes[0].0 };
                for id in map3_opening::tick_animation(mesh, state, tick)? {
                    if let Some(sound) = a.sounds.iter().find(|n| n.original == id).map(|n| n.sound)
                    {
                        m.audio_sequence += 1;
                        map3_opening::queue_audio(m.audio_sequence, tick, sound, &mut m.events);
                    }
                }
                if state.root {
                    state.position = map3_opening::authored(mesh, state, a.position, a.rotation)?;
                }
                if state.finished && index != 1 {
                    state.visible = false;
                }
            }
            if s.animations[1].finished && s.sbd.is_none() {
                let at = s.animations[1].position;
                if let Some(position) = crate::mission_motion::ground(
                    &self.world,
                    [at[0], at[1], at[2] + 200.],
                    [65., 65., 115.],
                    500.,
                )? {
                    let i = self.state.enemies.len();
                    self.state.enemies.push(Enemy {
                        position,
                        health: 400,
                        cooldown: 1.4,
                        flash: 0.,
                    });
                    s.bindings.push((i, 8));
                    s.sbd = Some(i);
                    s.animations[1].visible = false;
                    o.events.insert("SBDCombatReady".into());
                    o.event_ticks.insert("SBDCombatReady".into(), tick);
                }
            }
        }
        if s.sbd.is_some_and(|i| self.state.enemies[i].health == 0) && s.destroyed_tick.is_none() {
            s.destroyed_tick = Some(tick);
            o.events.insert("SBDDestroyed".into());
            o.event_ticks.insert("SBDDestroyed".into(), tick);
        }
        Ok(())
    }
    pub(crate) fn map3_guard_target(&self) -> Option<Point> {
        let o = self.state.map.as_ref()?.opening.as_ref()?;
        let s = o.planning.as_ref()?.guard.as_ref()?;
        s.intro_tick?;
        let g = self
            .map
            .as_ref()?
            .opening
            .as_ref()?
            .planning
            .as_ref()?
            .guard
            .as_ref()?;
        Some(if o.objectives[3] {
            g.assets.next_target
        } else if let Some(i) = s.sbd {
            self.state.enemies[i].position
        } else {
            s.animations[1].position
        })
    }
    pub(crate) fn map3_guard_status(&self) -> Option<String> {
        let o = self.state.map.as_ref()?.opening.as_ref()?;
        let s = o.planning.as_ref()?.guard.as_ref()?;
        s.intro_tick?;
        Some(if o.objectives[3] {
            "GEO_01C · SBD zerstoert · Sun Facs Tuer erreichen".into()
        } else if s.sbd.is_none() {
            "GEO_01C · Superkampfdroiden-Auftritt".into()
        } else if s.focus == s.sbd {
            "GEO_01C · Squad konzentriert Feuer auf den SBD".into()
        } else {
            "GEO_01C · SBD ausschalten · BENUTZEN: Squad-Ziel".into()
        })
    }
    pub(crate) fn map3_guard_geometry(
        &self,
    ) -> Result<
        (
            Vec<rc_package::geometry::Triangle>,
            Vec<Option<rc_render::FaceTexture>>,
        ),
        String,
    > {
        let Some(p) = self
            .map
            .as_ref()
            .and_then(|m| m.opening.as_ref())
            .and_then(|p| p.planning.as_ref())
        else {
            return Ok((vec![], vec![]));
        };
        let Some(g) = p.guard.as_ref() else {
            return Ok((vec![], vec![]));
        };
        let s = self
            .state
            .map
            .as_ref()
            .unwrap()
            .opening
            .as_ref()
            .unwrap()
            .planning
            .as_ref()
            .unwrap()
            .guard
            .as_ref()
            .unwrap();
        let mut ts = vec![];
        let mut fs = vec![];
        for (a, state) in g.assets.actors.iter().zip(&s.animations) {
            if !state.visible {
                continue;
            }
            let (mesh, base) = if a.sbd {
                (&g.mesh, g.texture)
            } else {
                (&p.meshes[0].0, p.meshes[0].1)
            };
            let clip = mesh.clip(&state.clip)?;
            let frame = map3_opening::anim_frame(state, mesh)?;
            let channel = mesh.channel(clip, frame)?;
            let mut geometry = mesh.geometry_with_root(&channel, state.root)?;
            let mut at = state.position;
            at[2] -= a.height + mesh.rest_bounds[0][2] as f64;
            for t in &mut geometry.triangles {
                t.points = t.points.map(|v| {
                    rc_package::mesh::transform_point(
                        v,
                        at.map(|v| v as f32),
                        a.rotation,
                        [1.; 3],
                        [0.; 3],
                    )
                });
            }
            ts.extend(geometry.triangles);
            fs.extend(geometry.appearance.into_iter().map(|f| {
                f.map(|mut f| {
                    f.texture += base;
                    f
                })
            }));
        }
        Ok((ts, fs))
    }
}
