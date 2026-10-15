//! Selected GEO_01C planning-room actors; bounded combat, not native AI/Prop parity.
use crate::{
    visual::{Mesh, PreparedMesh},
    Enemy, Game, Point, STEP,
};
use rc_package::{geometry::Triangle, hull::HullSet, world_collision::BodyShape};
use serde::{Deserialize, Serialize};
use std::sync::Arc;

#[derive(Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Factory {
    pub original: u32,
    pub position: Point,
    pub health: u32,
    pub extent: Point,
    pub warrior: bool,
}
#[derive(Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Assets {
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub guard: Option<Box<crate::map3_guard::Assets>>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub escape: Option<Box<crate::map3_escape::Assets>>,
    pub factories: Vec<Factory>,
    pub droid: Option<Mesh>,
    pub warrior: Option<Mesh>,
    pub glass_actor: String,
    pub glass_health: u32,
    pub glass_snapshot: Vec<u8>,
    pub squad_sound: usize,
    pub provenance: serde_json::Value,
}
pub struct Prepared {
    pub guard: Option<Box<crate::map3_guard::Prepared>>,
    pub escape: Option<Box<crate::map3_escape::Prepared>>,
    pub assets: Assets,
    pub meshes: Vec<(PreparedMesh, usize)>,
    pub glass: rc_render::Scene,
    pub texture: usize,
    pub collider: (usize, Arc<HullSet>),
}
#[derive(Clone, Debug, Serialize, serde::Deserialize)]
pub struct State {
    #[serde(skip_serializing_if = "Option::is_none")]
    pub guard: Option<Box<crate::map3_guard::State>>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub escape: Option<Box<crate::map3_escape::State>>,
    pub spawned: bool,
    pub glass_health: u32,
    pub glass_hits: u32,
    pub destroyed_tick: Option<u64>,
    pub assist_remaining: f64,
    pub assists: u32,
}
impl Game {
    pub(crate) fn prepare_map3_planning(&mut self) -> Result<(), String> {
        let Some(mut a) = self
            .map
            .as_mut()
            .and_then(|m| m.opening.as_mut())
            .and_then(|p| p.assets.planning.take())
        else {
            return Ok(());
        };
        if a.factories.iter().map(|f| f.original).collect::<Vec<_>>() != [164, 165, 190, 191, 192]
            || a.glass_actor != "WindowShatter_Test1"
            || a.glass_health != 150
            || a.glass_snapshot.len() > 2 * 1024 * 1024
            || a.squad_sound >= self.map.as_ref().unwrap().assets.sounds.len()
            || a.factories.iter().any(|f| {
                f.health == 0
                    || f.health > 10000
                    || f.position.iter().any(|v| !v.is_finite() || v.abs() > 1e7)
                    || f.extent
                        .iter()
                        .any(|v| !v.is_finite() || *v <= 0. || *v > 250.)
            })
        {
            return Err("Invalid GEO_01C planning bindings".into());
        }
        let collider = self
            .world
            .actors
            .iter()
            .enumerate()
            .find_map(|(i, x)| {
                if x.name == a.glass_actor {
                    if let BodyShape::Hulls(h) = &x.shape {
                        return Some((i, h.clone()));
                    }
                }
                None
            })
            .ok_or("Missing original planning-room glass collider")?;
        let glass = rc_render::Scene::from_snapshot(&a.glass_snapshot)?;
        if glass.triangles.is_empty() {
            return Err("Missing original glass geometry".into());
        }
        let texture = self.scene.textures.len();
        self.scene.textures.extend(glass.textures.iter().cloned());
        let mut meshes = vec![];
        for mesh in [&mut a.droid, &mut a.warrior] {
            let mesh = mesh
                .take()
                .ok_or("Missing planning-room enemy mesh")?
                .prepare_reference()?;
            mesh.clip("ActionBreathe")?;
            mesh.clip("RunForward")?;
            let base = self.scene.textures.len();
            self.scene
                .textures
                .extend(mesh.mesh.materials.iter().map(|m| rc_render::Texture {
                    width: m.width,
                    height: m.height,
                    pixels: m.pixels.clone(),
                }));
            meshes.push((mesh, base));
        }
        for f in &a.factories {
            if self.world.body_placement(f.position, f.extent)?.state
                == rc_package::movement::PlacementState::Penetrating
            {
                return Err(format!("Planning factory {} penetrates world", f.original));
            }
        }
        let state = State {
            guard: None,
            escape: None,
            spawned: false,
            glass_health: a.glass_health,
            glass_hits: 0,
            destroyed_tick: None,
            assist_remaining: 0.,
            assists: 0,
        };
        self.state
            .map
            .as_mut()
            .unwrap()
            .opening
            .as_mut()
            .unwrap()
            .planning = Some(Box::new(state));
        self.map
            .as_mut()
            .unwrap()
            .opening
            .as_mut()
            .unwrap()
            .planning = Some(Box::new(Prepared {
            escape: None,
            guard: None,
            assets: *a,
            meshes,
            glass,
            texture,
            collider,
        }));
        Ok(())
    }
    pub(crate) fn map3_planning_tick(&mut self) -> Result<(), String> {
        let Some(p) = self
            .map
            .as_ref()
            .and_then(|m| m.opening.as_ref())
            .and_then(|p| p.planning.as_ref())
        else {
            return Ok(());
        };
        let o = self.state.map.as_mut().unwrap().opening.as_mut().unwrap();
        let s = o.planning.as_mut().unwrap();
        if !s.spawned && o.events.contains("SpawnPlanningRoom") {
            self.state.enemies = p
                .assets
                .factories
                .iter()
                .map(|f| Enemy {
                    position: f.position,
                    health: f.health,
                    cooldown: 1.4,
                    flash: 0.,
                })
                .collect();
            s.spawned = true;
        }
        self.sync_map3_glass();
        Ok(())
    }
    pub(crate) fn map3_enemy_extent(&self, i: usize) -> Option<Point> {
        self.map3_combat_spec(i).map(|s| s.extent)
    }
    pub(crate) fn map3_glass_fraction(
        &self,
        from: Point,
        to: Point,
    ) -> Result<Option<f64>, String> {
        let Some(p) = self
            .map
            .as_ref()
            .and_then(|m| m.opening.as_ref())
            .and_then(|p| p.planning.as_ref())
        else {
            return Ok(None);
        };
        if self
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
            .glass_health
            == 0
        {
            return Ok(None);
        }
        let sweep = p.collider.1.sweep(from, to, [0.; 3])?;
        Ok(sweep.map(|h| h.fraction))
    }
    pub(crate) fn map3_damage_glass(&mut self, damage: u32) {
        let o = self.state.map.as_mut().unwrap().opening.as_mut().unwrap();
        let s = o.planning.as_mut().unwrap();
        if s.glass_health == 0 {
            return;
        }
        s.glass_hits += 1;
        s.glass_health = s.glass_health.saturating_sub(damage);
        if s.glass_health == 0 {
            let tick = self.state.statistics.ticks;
            s.destroyed_tick = Some(tick);
            o.events.insert("WindowDestroyed".into());
            o.event_ticks
                .entry("WindowDestroyed".into())
                .or_insert(tick);
        }
        self.sync_map3_glass();
    }
    pub(crate) fn sync_map3_glass(&mut self) {
        let Some(p) = self
            .map
            .as_ref()
            .and_then(|m| m.opening.as_ref())
            .and_then(|p| p.planning.as_ref())
        else {
            return;
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
            .unwrap();
        self.world.actors[p.collider.0].shape = if s.glass_health == 0 {
            BodyShape::Clear
        } else {
            BodyShape::Hulls(p.collider.1.clone())
        };
    }
    pub(crate) fn map3_enemy_tick(&mut self) -> Result<bool, String> {
        let Some(p) = self
            .map
            .as_ref()
            .and_then(|m| m.opening.as_ref())
            .and_then(|p| p.planning.as_ref())
        else {
            return Ok(false);
        };
        let o = self.state.map.as_ref().unwrap().opening.as_ref().unwrap();
        if o.locked {
            return Ok(true);
        }
        let members: Vec<_> = self
            .state
            .map
            .as_ref()
            .unwrap()
            .companion
            .iter()
            .chain(self.state.map.as_ref().unwrap().fixer.iter())
            .chain(std::iter::once(&o.sev))
            .filter(|c| c.joined && !c.down && c.health > 0)
            .map(|c| c.position)
            .collect();
        let can_assist = o.planning.as_ref().unwrap().assist_remaining <= 0.;
        let mut assist = None;
        let mut enemy_sounds = vec![];
        let player = self.state.pawn.body.position;
        let eye = self.eye();
        let mut order: Vec<_> = (0..self.state.enemies.len()).collect();
        if let Some(focus) = o
            .planning
            .as_ref()
            .and_then(|p| p.guard.as_ref())
            .and_then(|g| g.focus)
        {
            order.sort_by_key(|i| usize::from(*i != focus));
        }
        for i in order {
            let mut e = self.state.enemies[i].clone();
            let f = self.map3_combat_spec(i).ok_or("Missing C enemy binding")?;
            e.flash = (e.flash - STEP).max(0.);
            e.cooldown = (e.cooldown - STEP).max(0.);
            if e.health > 0 && f.active {
                let from = [e.position[0], e.position[1], e.position[2] + 35.];
                let d = crate::distance(e.position, player);
                let visible = d < 1800. && self.wall_fraction(from, eye)? > 0.999999;
                if visible && d > if f.warrior { 130. } else { 600. } {
                    let speed = if f.warrior {
                        220.
                    } else if f.sbd {
                        100.
                    } else {
                        65.
                    };
                    let to = std::array::from_fn(|k| {
                        e.position[k] + (player[k] - e.position[k]) * (speed * STEP / d).min(1.)
                    });
                    if let Some(path) =
                        crate::squad::corridor(&self.world, e.position, to, f.extent)?
                    {
                        if let Some(at) = path.last() {
                            e.position = *at;
                        }
                    }
                }
                if visible && e.cooldown == 0. && (!f.warrior || d < 180.) {
                    self.state.health =
                        self.state
                            .health
                            .saturating_sub(if f.warrior { 10 } else { 5 });
                    self.state.statistics.enemy_shots += 1;
                    if let Some(sound) = f.fire_sound {
                        enemy_sounds.push((i, sound));
                    }
                    self.state.damage_flash = 0.2;
                    e.cooldown = if f.warrior { 1.2 } else { 1.4 };
                }
                if can_assist && assist.is_none() {
                    for member in &members {
                        let from = [member[0], member[1], member[2] + 40.];
                        if crate::distance(from, e.position) < 1600.
                            && self.wall_fraction(from, e.position)? > 0.999999
                        {
                            assist = Some(i);
                            break;
                        }
                    }
                }
            }
            self.state.enemies[i] = e;
        }
        let m = self.state.map.as_mut().unwrap();
        for (owner, sound) in enemy_sounds {
            m.audio_sequence += 1;
            if m.events.len() >= 256 {
                m.events.remove(0);
            }
            m.events.push(crate::combat::Event {
                sequence: m.audio_sequence,
                tick: self.state.statistics.ticks,
                sound,
                owner: owner as i32,
            });
        }
        let s = m.opening.as_mut().unwrap().planning.as_mut().unwrap();
        s.assist_remaining = (s.assist_remaining - STEP).max(0.);
        if let Some(i) = assist {
            let e = &mut self.state.enemies[i];
            e.health = e.health.saturating_sub(15);
            e.flash = 0.12;
            self.state.statistics.kills += u32::from(e.health == 0);
            s.assist_remaining = 0.6;
            s.assists += 1;
            m.audio_sequence += 1;
            if m.events.len() >= 256 {
                m.events.remove(0);
            }
            m.events.push(crate::combat::Event {
                sequence: m.audio_sequence,
                tick: self.state.statistics.ticks,
                sound: p.assets.squad_sound,
                owner: -2,
            });
        }
        Ok(true)
    }
    pub(crate) fn map3_planning_geometry(
        &self,
    ) -> Result<(Vec<Triangle>, Vec<Option<rc_render::FaceTexture>>), String> {
        let Some(p) = self
            .map
            .as_ref()
            .and_then(|m| m.opening.as_ref())
            .and_then(|p| p.planning.as_ref())
        else {
            return Ok((vec![], vec![]));
        };
        let o = self.state.map.as_ref().unwrap().opening.as_ref().unwrap();
        let s = o.planning.as_ref().unwrap();
        let mut ts = vec![];
        let mut fs = vec![];
        if s.glass_health > 0 {
            ts.extend(p.glass.triangles.iter().cloned());
            fs.extend(p.glass.appearance.iter().map(|f| {
                f.map(|mut f| {
                    f.texture += p.texture;
                    f
                })
            }));
        }
        for (i, e) in self
            .state
            .enemies
            .iter()
            .enumerate()
            .filter(|(_, e)| e.health > 0)
        {
            let f = self
                .map3_combat_spec(i)
                .ok_or("Missing C enemy visual binding")?;
            let (mesh, base) = if f.sbd {
                let g = p.guard.as_ref().ok_or("Missing SBD mesh")?;
                (&g.mesh, g.texture)
            } else {
                let (m, b) = &p.meshes[usize::from(f.warrior)];
                (m, *b)
            };
            let clip = mesh.clip(if e.cooldown < 1. {
                if f.sbd {
                    "WalkForward"
                } else {
                    "RunForward"
                }
            } else {
                "ActionBreathe"
            })?;
            let seq = &mesh.mesh.clips[clip].sequence;
            let frame = (o.clock * f32::from_bits(seq.rate_bits) as f64 / seq.frames as f64)
                .rem_euclid(1.) as f32;
            let channel = mesh.channel(clip, frame)?;
            let mut g = mesh.geometry_with_root(&channel, true)?;
            let height = 2. * f.extent[2] as f32;
            let scale = height / (mesh.rest_bounds[1][2] - mesh.rest_bounds[0][2]);
            let yaw = (self.state.pawn.body.position[1] - e.position[1])
                .atan2(self.state.pawn.body.position[0] - e.position[0])
                as f32;
            let (sin, cos) = yaw.sin_cos();
            for t in &mut g.triangles {
                t.points = t.points.map(|v| {
                    [
                        e.position[0] as f32 + (v[0] * cos - v[1] * sin) * scale,
                        e.position[1] as f32 + (v[0] * sin + v[1] * cos) * scale,
                        e.position[2] as f32 - height / 2.
                            + (v[2] - mesh.rest_bounds[0][2]) * scale,
                    ]
                });
            }
            ts.extend(g.triangles);
            fs.extend(g.appearance.into_iter().map(|f| {
                f.map(|mut f| {
                    f.texture += base;
                    f
                })
            }));
        }
        Ok((ts, fs))
    }
}
