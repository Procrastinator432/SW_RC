//! Campaign map/entry integration for GEO_01B and GEO_01C.
//! Mission scripts and encounters are not inferred from carried player state.
use crate::{
    visual::{Mesh, PreparedMesh},
    Game, Point, STEP,
};
use serde::{Deserialize, Serialize};
#[derive(Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Assets {
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub gameplay: Option<Box<crate::map_gameplay::Assets>>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub opening: Option<Box<crate::map3_opening::Assets>>,
    pub destination: String,
    pub start: String,
    pub pitch: f64,
    pub squad_mesh: Option<Mesh>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub fixer_mesh: Option<Mesh>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub sev_mesh: Option<Mesh>,
    pub weapon_snapshot: Vec<u8>,
    pub sounds: Vec<crate::combat::Sound>,
}
pub struct Prepared {
    pub gameplay: Option<Box<crate::map_gameplay::Prepared>>,
    pub opening: Option<Box<crate::map3_opening::Prepared>>,
    pub assets: Assets,
    pub squad: PreparedMesh,
    pub fixer: Option<(PreparedMesh, usize)>,
    pub sev: Option<(PreparedMesh, usize)>,
    pub texture_base: usize,
    pub weapon: rc_render::Scene,
    pub weapon_texture: usize,
}
#[derive(Clone, Debug, Serialize, serde::Deserialize)]
pub struct Companion {
    pub joined: bool,
    pub health: u32,
    pub shields: f32,
    pub down: bool,
    pub position: Point,
    pub yaw: f64,
    pub moving: bool,
}
#[derive(Clone, Debug, Serialize, serde::Deserialize)]
pub struct State {
    #[serde(skip_serializing_if = "Option::is_none")]
    pub gameplay: Option<Box<crate::map_gameplay::State>>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub opening: Option<Box<crate::map3_opening::State>>,
    pub destination: String,
    pub restored: bool,
    pub grenades: u32,
    pub companion: Option<Companion>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub fixer: Option<Companion>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub sev: Option<Companion>,
    pub events: Vec<crate::combat::Event>,
    pub audio_sequence: u64,
}
impl Game {
    pub fn attach_map2(&mut self, mut assets: Assets) -> Result<(), String> {
        if self.map.is_some()
            || self.mission.is_some()
            || ![
                "GEO_01B.ctm#playerstart",
                "GEO_01C.ctm#playerstart",
                "GEO_03A.ctm#playerstart",
            ]
            .contains(&assets.destination.as_str())
            || (assets.destination == "GEO_01C.ctm#playerstart"
                && (assets.gameplay.is_some() || assets.fixer_mesh.is_none()))
            || (assets.destination == "GEO_01B.ctm#playerstart"
                && (assets.fixer_mesh.is_some() || assets.opening.is_some()))
            || (assets.destination == "GEO_03A.ctm#playerstart"
                && (assets.gameplay.is_some()
                    || assets.opening.is_some()
                    || assets.fixer_mesh.is_none()
                    || assets.sev_mesh.is_none()))
            || (assets.destination != "GEO_03A.ctm#playerstart" && assets.sev_mesh.is_some())
            || assets.start.is_empty()
            || !assets.pitch.is_finite()
            || assets.pitch.abs() > 1.4
            || assets.sounds.is_empty()
            || assets.sounds.len() > 128
        {
            return Err("Invalid campaign-map bindings".into());
        }
        for sound in &assets.sounds {
            crate::combat::validate_wave(&sound.wave)?;
        }
        let gameplay = assets.gameplay.take();
        let opening = assets.opening.take();
        let squad = assets
            .squad_mesh
            .take()
            .ok_or("Missing campaign companion mesh")?
            .prepare_reference()?;
        for clip in ["ActionBreathe", "RunForward"] {
            squad.clip(clip)?;
        }
        let texture_base = self.scene.textures.len();
        self.scene
            .textures
            .extend(squad.mesh.materials.iter().map(|m| rc_render::Texture {
                width: m.width,
                height: m.height,
                pixels: m.pixels.clone(),
            }));
        let fixer = if let Some(mesh) = assets.fixer_mesh.take() {
            let mesh = mesh.prepare_reference()?;
            for clip in ["ActionBreathe", "RunForward"] {
                mesh.clip(clip)?;
            }
            let base = self.scene.textures.len();
            self.scene
                .textures
                .extend(mesh.mesh.materials.iter().map(|m| rc_render::Texture {
                    width: m.width,
                    height: m.height,
                    pixels: m.pixels.clone(),
                }));
            Some((mesh, base))
        } else {
            None
        };
        let sev = if let Some(mesh) = assets.sev_mesh.take() {
            let mesh = mesh.prepare_reference()?;
            for clip in ["ActionBreathe", "RunForward"] {
                mesh.clip(clip)?;
            }
            let base = self.scene.textures.len();
            self.scene
                .textures
                .extend(mesh.mesh.materials.iter().map(|m| rc_render::Texture {
                    width: m.width,
                    height: m.height,
                    pixels: m.pixels.clone(),
                }));
            Some((mesh, base))
        } else {
            None
        };
        let weapon = rc_render::Scene::from_snapshot(&assets.weapon_snapshot)?;
        let weapon_texture = self.scene.textures.len();
        self.scene.textures.extend(weapon.textures.iter().cloned());
        self.state.pitch = assets.pitch;
        self.state.map = Some(State {
            gameplay: None,
            opening: None,
            destination: assets.destination.clone(),
            restored: false,
            grenades: 0,
            companion: None,
            fixer: None,
            sev: None,
            events: vec![],
            audio_sequence: 0,
        });
        self.map = Some(Prepared {
            gameplay: None,
            opening: None,
            assets,
            squad,
            fixer,
            sev,
            texture_base,
            weapon,
            weapon_texture,
        });
        if let Some(a) = gameplay {
            let p = self.prepare_map_gameplay(*a)?;
            self.map.as_mut().unwrap().gameplay = Some(Box::new(p));
        }
        if let Some(a) = opening {
            let p = a.prepare(
                &mut self.scene,
                self.map.as_ref().unwrap().assets.sounds.len(),
            )?;
            let state = p.state(self.state.pawn.body.position);
            let m = self.state.map.as_mut().unwrap();
            for (target, actor) in [
                (&mut m.companion, &p.assets.actors[0]),
                (&mut m.fixer, &p.assets.actors[1]),
            ] {
                *target = Some(Companion {
                    joined: true,
                    health: 100,
                    shields: 100.,
                    down: false,
                    position: actor.position,
                    yaw: actor.rotation[1] as f64 * std::f64::consts::TAU / 65536.,
                    moving: false,
                });
            }
            m.opening = Some(Box::new(state));
            self.map.as_mut().unwrap().opening = Some(Box::new(p));
        }
        self.prepare_map3_planning()?;
        self.prepare_map3_escape()?;
        self.prepare_map3_guard()?;
        self.prepare_map3_breach()?;
        self.prepare_map3_balcony()?;
        self.prepare_map3_finale()?;
        self.prepare_map3_elevator()?;
        self.initial = self.state.clone();
        Ok(())
    }
    pub fn restore_handoff(&mut self, h: &crate::transition::Handoff) -> Result<(), String> {
        let map = self
            .map
            .as_ref()
            .ok_or("Destination is not a campaign map")?;
        if self.state.statistics.ticks != 0
            || self.state.phase != crate::Phase::Playing
            || !crate::transition::valid_route(h)
            || h.destination != map.assets.destination
            || self.state.map.as_ref().unwrap().restored
        {
            return Err("Handoff does not match destination".into());
        }
        let health = h.player["health"]
            .as_u64()
            .filter(|v| *v > 0 && *v <= 1000)
            .ok_or("Invalid handoff health")? as u32;
        let ammo = h.player["ammo"]
            .as_u64()
            .filter(|v| *v <= crate::MAGAZINE as u64)
            .ok_or("Invalid handoff magazine")? as u32;
        let reserve = h.player["reserve"]
            .as_u64()
            .filter(|v| *v <= 100000)
            .ok_or("Invalid handoff reserve")? as u32;
        let grenades = h.player["grenades"]["flow"]["ammunition"]
            .as_u64()
            .filter(|v| *v <= 100)
            .ok_or("Invalid handoff grenades")? as u32;
        let mut companion = self.carried_member(&h.squad, &[])?;
        let avoid: Vec<_> = companion.iter().map(|c| c.position).collect();
        let mut fixer = if let Some(carry) = &h.map2 {
            self.carried_member(&carry.fixer, &avoid)?
        } else if let Some(carry) = &h.map3 {
            self.carried_member(&carry.fixer, &avoid)?
        } else {
            None
        };
        let avoid: Vec<_> = companion
            .iter()
            .chain(fixer.iter())
            .map(|c| c.position)
            .collect();
        let sev = if let Some(carry) = &h.map3 {
            self.carried_member(&carry.sev, &avoid)?
        } else {
            None
        };
        if let Some(p) = self.map.as_ref().and_then(|m| m.opening.as_ref()) {
            for (member, actor) in [
                (&mut companion, &p.assets.actors[0]),
                (&mut fixer, &p.assets.actors[1]),
            ] {
                if let Some(c) = member {
                    if self
                        .world
                        .body_placement(actor.position, crate::squad::EXTENT)?
                        .state
                        == rc_package::movement::PlacementState::Penetrating
                    {
                        return Err("Original rappel member entry penetrates geometry".into());
                    }
                    c.position = actor.position;
                    c.yaw = actor.rotation[1] as f64 * std::f64::consts::TAU / 65536.;
                }
            }
        }
        // Commit only after all data and destination placements validate.
        self.state.health = health;
        self.state.ammo = ammo;
        self.state.reserve = reserve;
        let s = self.state.map.as_mut().unwrap();
        s.restored = true;
        s.grenades = grenades;
        s.companion = companion;
        s.fixer = fixer;
        s.sev = sev;
        self.initial = self.state.clone();
        Ok(())
    }
    fn carried_member(
        &self,
        value: &serde_json::Value,
        avoid: &[Point],
    ) -> Result<Option<Companion>, String> {
        if value.is_null() {
            return Ok(None);
        }
        Ok({
            let joined = value["joined"]
                .as_bool()
                .ok_or("Invalid squad membership")?;
            let hp = value["health"]
                .as_u64()
                .filter(|v| *v <= 1000)
                .ok_or("Invalid companion health")? as u32;
            let shields = value["shields"]
                .as_f64()
                .filter(|v| v.is_finite() && (0. ..=1000.).contains(v))
                .ok_or("Invalid companion shields")? as f32;
            let down = value["down"]
                .as_bool()
                .ok_or("Invalid companion incapacitation")?;
            // New spatial host: never copy old-map route/trail/coordinates.
            let spawn = self.state.pawn.body.position;
            let mut position = None;
            for radius in [140., 220., 300.] {
                for offset in [
                    std::f64::consts::PI,
                    -std::f64::consts::FRAC_PI_2,
                    std::f64::consts::FRAC_PI_2,
                    0.,
                ] {
                    let yaw = self.state.yaw + offset;
                    let candidate = [
                        spawn[0] + yaw.cos() * radius,
                        spawn[1] + yaw.sin() * radius,
                        spawn[2] + 100.,
                    ];
                    if let Some(p) = crate::mission_motion::ground(
                        &self.world,
                        candidate,
                        crate::squad::EXTENT,
                        512.,
                    )? {
                        if avoid.iter().all(|other| crate::distance(*other, p) >= 100.)
                            && self
                                .world
                                .sweep_motion(spawn, p, crate::squad::EXTENT)?
                                .contacts
                                .is_empty()
                        {
                            position = Some(p);
                            break;
                        }
                    }
                }
                if position.is_some() {
                    break;
                }
            }
            Some(Companion {
                joined,
                health: hp,
                shields,
                down,
                position: position.ok_or("No safe companion entry placement")?,
                yaw: self.state.yaw,
                moving: false,
            })
        })
    }
    pub(crate) fn map_tick(&mut self) -> Result<(), String> {
        if self.state.map.as_ref().is_some_and(|s| {
            ["GEO_01C.ctm#playerstart", "GEO_03A.ctm#playerstart"].contains(&s.destination.as_str())
        }) {
            if self.map3_breach_follow()? {
                return Ok(());
            }
            let target = self.state.pawn.body.position;
            let s = self.state.map.as_mut().unwrap();
            let mut occupied = None;
            for c in s
                .companion
                .iter_mut()
                .chain(s.fixer.iter_mut())
                .chain(s.sev.iter_mut())
                .chain(s.opening.iter_mut().map(|o| &mut o.sev))
            {
                c.moving = false;
                if c.joined && !c.down && c.health > 0 && crate::distance(c.position, target) > 180.
                {
                    if let Some(next) =
                        crate::map_gameplay::follow_step(&self.world, c.position, target, 450.)?
                    {
                        if occupied.is_none_or(|p| crate::distance(p, next) >= 100.) {
                            c.yaw = (target[1] - c.position[1]).atan2(target[0] - c.position[0]);
                            c.position = next;
                            c.moving = true;
                        }
                    }
                }
                occupied = Some(c.position);
            }
            return Ok(());
        }

        if self.map_follow_tick()? {
            return Ok(());
        }
        let Some(s) = &mut self.state.map else {
            return Ok(());
        };
        let Some(c) = &mut s.companion else {
            return Ok(());
        };
        c.moving = false;
        if !c.joined || c.down || c.health == 0 {
            return Ok(());
        }
        let target = self.state.pawn.body.position;
        let from = c.position;
        let distance = crate::distance(from, target);
        if distance > 180. && distance < 1200. {
            let fraction = (300. * STEP / distance).min(1.);
            let next = std::array::from_fn(|i| from[i] + (target[i] - from[i]) * fraction);
            if let Some(path) =
                crate::squad::corridor(&self.world, from, next, crate::squad::EXTENT)?
            {
                if let Some(next) = path.last() {
                    c.position = *next;
                    c.yaw = (target[1] - from[1]).atan2(target[0] - from[0]);
                    c.moving = true;
                }
            }
        }
        Ok(())
    }
    pub(crate) fn map_fire_sound(&mut self) {
        if let Some(s) = &mut self.state.map {
            s.audio_sequence += 1;
            if s.events.len() >= 256 {
                s.events.remove(0);
            }
            s.events.push(crate::combat::Event {
                sequence: s.audio_sequence,
                tick: self.state.statistics.ticks,
                sound: 0,
                owner: -1,
            });
        }
    }
    pub(crate) fn map_geometry(
        &self,
    ) -> Result<
        (
            Vec<rc_package::geometry::Triangle>,
            Vec<Option<rc_render::FaceTexture>>,
        ),
        String,
    > {
        let Some(p) = &self.map else {
            return Ok((vec![], vec![]));
        };
        let state = self.state.map.as_ref().unwrap();
        if self.map3_locked() || self.map3_breach_playing() {
            return Ok((vec![], vec![]));
        }
        let mut triangles = vec![];
        let mut faces = vec![];
        for (c, mesh, base) in state
            .companion
            .iter()
            .map(|c| (c, &p.squad, p.texture_base))
            .chain(
                state
                    .fixer
                    .iter()
                    .filter_map(|c| p.fixer.as_ref().map(|(mesh, base)| (c, mesh, *base))),
            )
            .chain(
                state
                    .sev
                    .iter()
                    .filter_map(|c| p.sev.as_ref().map(|(mesh, base)| (c, mesh, *base))),
            )
        {
            let (t, f) = self.map_member_geometry(c, mesh, base)?;
            triangles.extend(t);
            faces.extend(f);
        }
        Ok((triangles, faces))
    }
    fn map_member_geometry(
        &self,
        c: &Companion,
        mesh: &PreparedMesh,
        texture_base: usize,
    ) -> Result<
        (
            Vec<rc_package::geometry::Triangle>,
            Vec<Option<rc_render::FaceTexture>>,
        ),
        String,
    > {
        let p = self.map.as_ref().unwrap();
        if !c.joined || (c.health == 0 && !c.down) {
            return Ok((vec![], vec![]));
        }
        let clip = mesh.clip(if c.moving {
            "RunForward"
        } else {
            "ActionBreathe"
        })?;
        let seq = &mesh.mesh.clips[clip].sequence;
        let frame =
            (self.state.statistics.ticks as f64 * STEP * f32::from_bits(seq.rate_bits) as f64
                / seq.frames as f64)
                .rem_euclid(1.) as f32;
        let channel = mesh.channel(clip, frame)?;
        let mut g = mesh.geometry_with_root(&channel, true)?;
        for t in &mut g.triangles {
            t.points = t.points.map(|v| {
                rc_package::mesh::transform_point(
                    v,
                    [
                        c.position[0] as f32,
                        c.position[1] as f32,
                        c.position[2] as f32 - 84. - mesh.rest_bounds[0][2],
                    ],
                    [0, (c.yaw * 65536. / std::f64::consts::TAU) as i32, 0],
                    [1.; 3],
                    [0.; 3],
                )
            });
        }
        for f in g.appearance.iter_mut().flatten() {
            f.texture += texture_base;
        }
        let transform = mesh.attachment_transform(&channel, "weaponAttach_R", true)?;
        let (sin, cos) = (c.yaw as f32).sin_cos();
        for t in &p.weapon.triangles {
            let mut t = t.clone();
            t.points = t.points.map(|v| {
                let v: [f32; 3] = std::array::from_fn(|i| {
                    v[0] * transform[0][i]
                        + v[1] * transform[1][i]
                        + v[2] * transform[2][i]
                        + transform[3][i]
                });
                [
                    c.position[0] as f32 + v[0] * cos - v[1] * sin,
                    c.position[1] as f32 + v[0] * sin + v[1] * cos,
                    c.position[2] as f32 - 84. + v[2] - mesh.rest_bounds[0][2],
                ]
            });
            g.triangles.push(t);
        }
        g.appearance.extend(p.weapon.appearance.iter().map(|f| {
            (*f).map(|mut f| {
                f.texture += p.weapon_texture;
                f
            })
        }));
        Ok((g.triangles, g.appearance))
    }
}
