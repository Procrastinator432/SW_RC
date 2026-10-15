//! Immutable presentation snapshots shared by the CPU reference and graphics adapters.
use crate::{Game, Phase, Point};
use rc_package::geometry::Triangle;
use rc_package::performance::{Category, Span};
fn cube(out: &mut Vec<Triangle>, center: Point, extent: Point, color: u32) {
    let p: [[f32; 3]; 8] = std::array::from_fn(|n| {
        std::array::from_fn(|i| {
            (center[i]
                + if n & (1 << i) == 0 {
                    -extent[i]
                } else {
                    extent[i]
                }) as f32
        })
    });
    for [a, b, c, d] in [
        [0, 2, 3, 1],
        [4, 5, 7, 6],
        [0, 1, 5, 4],
        [2, 6, 7, 3],
        [0, 4, 6, 2],
        [1, 3, 7, 5],
    ] {
        out.push(Triangle {
            points: [p[a], p[b], p[c]],
            surface: color as usize,
        });
        out.push(Triangle {
            points: [p[a], p[c], p[d]],
            surface: color as usize,
        });
    }
}
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct HudRect {
    pub x: i32,
    pub y: i32,
    pub width: i32,
    pub height: i32,
    pub color: u32,
}
/// Owned data; no Game borrow, graphics handles, or simulation mutation.
pub struct RenderFrame {
    pub view: rc_render::View,
    pub triangles: Vec<Triangle>,
    pub appearance: Option<Vec<Option<rc_render::FaceTexture>>>,
    pub weapon_start: usize,
    pub hud: Vec<HudRect>,
    pub width: usize,
    pub height: usize,
}
impl RenderFrame {
    /// RCFRA001, dimensions, camera basis (eye/right/up/forward), triangle/rect counts,
    /// interleaved transient vertices, then x/y/w/h/ARGB rectangles. No world copy.
    pub fn gpu_packet(&self, scene: &rc_render::Scene) -> Vec<u8> {
        if let Some(appearance) = &self.appearance {
            return self.animated_packet(scene, appearance);
        }
        let mut out = b"RCFRA001".to_vec();
        out.extend_from_slice(&(self.width as u32).to_le_bytes());
        out.extend_from_slice(&(self.height as u32).to_le_bytes());
        for vector in rc_render::gpu::basis(scene, self.view) {
            for value in vector {
                out.extend_from_slice(&value.to_le_bytes());
            }
        }
        out.extend_from_slice(&(self.triangles.len() as u32).to_le_bytes());
        out.extend_from_slice(&(self.hud.len() as u32).to_le_bytes());
        for triangle in &self.triangles {
            rc_render::gpu::vertices(&mut out, triangle, None, true);
        }
        for rect in &self.hud {
            for value in [rect.x, rect.y, rect.width, rect.height, rect.color as i32] {
                out.extend_from_slice(&value.to_le_bytes());
            }
        }
        out
    }
    fn animated_packet(
        &self,
        scene: &rc_render::Scene,
        appearance: &[Option<rc_render::FaceTexture>],
    ) -> Vec<u8> {
        let mut out = b"RCFRA002".to_vec();
        out.extend_from_slice(&(self.width as u32).to_le_bytes());
        out.extend_from_slice(&(self.height as u32).to_le_bytes());
        for vector in rc_render::gpu::basis(scene, self.view) {
            for value in vector {
                out.extend_from_slice(&value.to_le_bytes());
            }
        }
        let mut batches = std::collections::BTreeMap::<(u32, i32), Vec<u8>>::new();
        for (i, t) in self.triangles.iter().enumerate() {
            let face = appearance[i];
            let key = (
                u32::from(i >= self.weapon_start),
                face.map_or(-1, |f| f.texture as i32),
            );
            rc_render::gpu::vertices(batches.entry(key).or_default(), t, face.map(|f| f.uv), true);
        }
        out.extend_from_slice(&(batches.len() as u32).to_le_bytes());
        out.extend_from_slice(&(self.hud.len() as u32).to_le_bytes());
        for ((pass, texture), vertices) in batches {
            out.extend_from_slice(&pass.to_le_bytes());
            out.extend_from_slice(&texture.to_le_bytes());
            out.extend_from_slice(&((vertices.len() / 36) as u32).to_le_bytes());
            out.extend(vertices);
        }
        for r in &self.hud {
            for value in [r.x, r.y, r.width, r.height, r.color as i32] {
                out.extend_from_slice(&value.to_le_bytes());
            }
        }
        out
    }
    pub fn render_cpu(&self, scene: &rc_render::Scene) -> Result<Vec<u32>, String> {
        let mut pixels = if let Some(faces) = &self.appearance {
            scene.render_animated_view(
                self.width,
                self.height,
                self.view,
                &self.triangles,
                faces,
                self.weapon_start,
            )?
        } else {
            scene.render_game_view(self.width, self.height, self.view, &self.triangles)?
        };
        let _timing = Span::new(Category::Hud);
        for r in &self.hud {
            for y in r.y.max(0)..(r.y + r.height).min(self.height as i32) {
                for x in r.x.max(0)..(r.x + r.width).min(self.width as i32) {
                    pixels[y as usize * self.width + x as usize] = r.color;
                }
            }
        }
        Ok(pixels)
    }
}
impl Game {
    pub fn render(&self, width: usize, height: usize) -> Result<Vec<u32>, String> {
        self.render_frame(width, height)?.render_cpu(&self.scene)
    }
    pub fn render_frame(&self, width: usize, height: usize) -> Result<RenderFrame, String> {
        if width == 0 || height == 0 || width > 2048 || height > 2048 {
            return Err("Invalid presentation dimensions".into());
        }
        let geometry_timing = Span::new(Category::ActorGeometry);
        let mut extra = vec![];
        for e in self.state.enemies.iter().filter(|_| self.visuals.is_none()) {
            let p = e.position;
            if e.health == 0 {
                cube(
                    &mut extra,
                    [p[0], p[1], p[2] - 53.],
                    [30., 22., 7.],
                    0x545158,
                );
                continue;
            }
            let color = if e.flash > 0. { 0xff6040 } else { 0xceb986 };
            cube(&mut extra, [p[0], p[1], p[2] + 5.], [17., 12., 27.], color);
            cube(&mut extra, [p[0], p[1], p[2] + 45.], [13., 11., 15.], color);
            for side in [-1., 1.] {
                cube(
                    &mut extra,
                    [p[0], p[1] + side * 12., p[2] - 38.],
                    [8., 6., 22.],
                    0x8c826b,
                );
                cube(
                    &mut extra,
                    [p[0], p[1] + side * 22., p[2] + 2.],
                    [5., 5., 25.],
                    color,
                );
            }
            cube(
                &mut extra,
                [p[0], p[1], p[2] + 46.],
                [14., 12., 3.],
                0xcf3020,
            );
        }
        if self.map.is_some() || self.state.enemies.iter().all(|e| e.health == 0) {
            let p = self.mission_target().unwrap_or(self.exit);
            cube(
                &mut extra,
                [p[0], p[1], p[2] - self.profile.physics.extent[2] + 2.],
                [60., 60., 2.],
                0x30eda1,
            );
            cube(
                &mut extra,
                [p[0], p[1], p[2] + 110.],
                [8., 8., 14.],
                0x30eda1,
            );
        }
        if self.state.muzzle_flash > 0. {
            if let Some(p) = self.state.impact {
                cube(&mut extra, p, [3.; 3], 0xffd760);
            }
        }
        if let Some(c) = &self.state.combat {
            for p in &c.projectiles {
                if crate::distance(p.position, self.eye()) < 80. {
                    continue;
                }
                cube(&mut extra, p.position, [1.; 3], 0x70dfff);
                let tail = std::array::from_fn(|i| p.position[i] - p.direction[i] * 12.);
                cube(&mut extra, tail, [0.6; 3], 0x306faa);
            }
        }
        let rappel_eye = self.rappel_eye();
        let eye = if let Some(eye) = rappel_eye {
            eye
        } else if self.state.opening.as_ref().is_some_and(|s| s.locked) {
            self.opening.as_ref().unwrap().assets.camera_origin
        } else {
            self.eye()
        };
        let (camera_yaw, camera_pitch) = if rappel_eye.is_some() {
            (self.state.yaw, self.state.pitch)
        } else if self.state.opening.as_ref().is_some_and(|s| s.locked) {
            let target = self.opening.as_ref().unwrap().assets.camera_target;
            let d: [f64; 3] = std::array::from_fn(|i| target[i] - eye[i]);
            (d[1].atan2(d[0]), d[2].atan2(d[0].hypot(d[1])))
        } else {
            (self.state.yaw, self.state.pitch)
        };
        let view = rc_render::View {
            yaw: (camera_yaw + std::f64::consts::PI) as f32,
            pitch: -camera_pitch as f32,
            zoom: 1.,
            position: Some(std::array::from_fn(|i| {
                (eye[i] as f32 - self.scene.center[i]) / self.scene.radius
            })),
        };
        let (dynamic, dynamic_faces) = self.dynamic_geometry();
        let dynamic_start = extra.len();
        extra.extend(dynamic);
        let mut appearance = if self.dynamic.is_some() {
            let mut faces = vec![None; dynamic_start];
            faces.extend(&dynamic_faces);
            Some(faces)
        } else {
            None
        };
        let mut weapon_start = extra.len();
        if let (Some(assets), Some(state)) = (&self.visuals, &self.state.visuals) {
            let mut faces = vec![None; extra.len()];
            faces[dynamic_start..].copy_from_slice(&dynamic_faces);
            let enemy_height = assets.enemy.rest_bounds[1][2] - assets.enemy.rest_bounds[0][2];
            if enemy_height <= 0. {
                return Err("Invalid enemy presentation bounds".into());
            }
            let scale = 120. / enemy_height;
            for (owner, (enemy, player)) in self
                .state
                .enemies
                .iter()
                .zip(&state.enemies)
                .enumerate()
                .filter(|_| self.map.as_ref().is_none_or(|m| m.gameplay.is_none()))
            {
                let mut mesh = assets.enemy.geometry_with_root(
                    &assets.enemy.channel(player.clip, player.frame)?,
                    (self.combat.as_ref().is_some_and(|a| a.jump.is_some())
                        && ["TakeOff", "Land", "FallBreathe"].contains(
                            &assets.enemy.mesh.clips[player.clip]
                                .sequence
                                .name
                                .name
                                .as_str(),
                        ))
                        || self.combat.as_ref().is_some_and(|a| a.lunge.is_some())
                            && ["Lunge", "Lunge2"].contains(
                                &assets.enemy.mesh.clips[player.clip]
                                    .sequence
                                    .name
                                    .name
                                    .as_str(),
                            ),
                )?;
                let yaw = self
                    .state
                    .combat
                    .as_ref()
                    .and_then(|c| c.melee.as_ref())
                    .map_or_else(
                        || (eye[1] - enemy.position[1]).atan2(eye[0] - enemy.position[0]) as f32,
                        |m| m.attacks[owner].yaw as f32,
                    );
                let (sin, cos) = yaw.sin_cos();
                for t in &mut mesh.triangles {
                    t.points = t.points.map(|p| {
                        [
                            enemy.position[0] as f32 + (p[0] * cos - p[1] * sin) * scale,
                            enemy.position[1] as f32 + (p[0] * sin + p[1] * cos) * scale,
                            enemy.position[2] as f32 - 60.
                                + (p[2] - assets.enemy.rest_bounds[0][2]) * scale,
                        ]
                    });
                }
                faces.extend(mesh.appearance.into_iter().map(|f| {
                    f.map(|mut f| {
                        f.texture += assets.texture_base;
                        f
                    })
                }));
                extra.extend(mesh.triangles);
            }
            if let (Some(t), Some(prepared)) = (&self.state.thermal, &self.thermal) {
                for projectile in &t.projectiles {
                    extra.extend(prepared.triangles.iter().map(|triangle| Triangle {
                        points:
                            triangle.points.map(|p| {
                                std::array::from_fn(|i| p[i] + projectile.position[i] as f32)
                            }),
                        surface: triangle.surface,
                    }));
                    faces.extend(prepared.appearance.iter().copied());
                }
            }
            let basis = rc_render::gpu::basis(&self.scene, view);
            let (fx, fx_faces) = self.explosion_geometry(basis[1], basis[2]);
            extra.extend(fx);
            faces.extend(fx_faces);
            let (artillery, artillery_faces) = self.artillery_geometry()?;
            extra.extend(artillery);
            faces.extend(artillery_faces);
            let (opening, opening_faces) = self.opening_geometry()?;
            extra.extend(opening);
            faces.extend(opening_faces);
            let (encounters, encounter_faces) = self.map_encounter_geometry()?;
            extra.extend(encounters);
            faces.extend(encounter_faces);
            let (elevator, elevator_faces) = self.map3_elevator_geometry();
            extra.extend(elevator);
            faces.extend(elevator_faces);
            let (finale, finale_faces) = self.map3_finale_geometry()?;
            extra.extend(finale);
            faces.extend(finale_faces);
            let (balcony, balcony_faces) = self.map3_balcony_geometry()?;
            extra.extend(balcony);
            faces.extend(balcony_faces);
            let (breach, breach_faces) = self.map3_breach_geometry()?;
            extra.extend(breach);
            faces.extend(breach_faces);
            let (guard, guard_faces) = self.map3_guard_geometry()?;
            extra.extend(guard);
            faces.extend(guard_faces);
            let (escape, escape_faces) = self.map3_escape_geometry();
            extra.extend(escape);
            faces.extend(escape_faces);
            let (planning, planning_faces) = self.map3_planning_geometry()?;
            extra.extend(planning);
            faces.extend(planning_faces);
            let (entry, entry_faces) = self.map3_geometry()?;
            extra.extend(entry);
            faces.extend(entry_faces);
            let (map, map_faces) = self.map_geometry()?;
            extra.extend(map);
            faces.extend(map_faces);
            let (intro, intro_faces) = self.intro_geometry()?;
            extra.extend(intro);
            faces.extend(intro_faces);
            let (rope, rope_faces) = self.rappel_geometry()?;
            extra.extend(rope);
            faces.extend(rope_faces);
            let (scorch, scorch_faces) = self.scorch_geometry()?;
            extra.extend(scorch);
            faces.extend(scorch_faces);
            let (rifle, rifle_faces) = self.squad_weapon_geometry()?;
            extra.extend(rifle);
            faces.extend(rifle_faces);
            let (charge, charge_faces) = self.demolition_geometry(false, view)?;
            extra.extend(charge);
            faces.extend(charge_faces);
            weapon_start = extra.len();
            let (detonator, detonator_faces) = self.demolition_geometry(true, view)?;
            if !detonator.is_empty() {
                extra.extend(detonator);
                faces.extend(detonator_faces);
            } else {
                let mut mesh = assets.weapon.geometry(
                    &assets
                        .weapon
                        .channel(state.weapon.clip, state.weapon.frame)?,
                )?;
                let [eye, right, up, forward] = rc_render::gpu::basis(&self.scene, view);
                for t in &mut mesh.triangles {
                    t.points = t.points.map(|p| {
                        std::array::from_fn(|i| {
                            eye[i] + forward[i] * p[0] + right[i] * p[1] + up[i] * p[2]
                        })
                    });
                }
                let base = assets.texture_base + assets.enemy.mesh.materials.len();
                faces.extend(mesh.appearance.into_iter().map(|f| {
                    f.map(|mut f| {
                        f.texture += base;
                        f
                    })
                }));
                extra.extend(mesh.triangles);
            }
            appearance = Some(faces);
        }
        if self.map3_locked() || self.state.opening.as_ref().is_some_and(|s| s.locked) {
            extra.truncate(weapon_start);
            if let Some(faces) = &mut appearance {
                faces.truncate(weapon_start);
            }
        }
        drop(geometry_timing);
        let _hud_timing = Span::new(Category::Hud);
        let mut hud = Canvas { rectangles: vec![] };
        let w = width as i32;
        let h = height as i32;
        if self.state.damage_flash > 0. {
            hud.rect(0, 0, w, 4, 0xffd34040);
            hud.rect(0, h - 4, w, 4, 0xffd34040);
            hud.rect(0, 0, 4, h, 0xffd34040);
            hud.rect(w - 4, 0, 4, h, 0xffd34040);
        }
        // Provisional first-person blaster silhouette; follows the same framebuffer on all hosts.
        if self.visuals.is_none() {
            hud.rect(w * 3 / 4 - 18, h - 65, 40, 65, 0xff354a59);
            hud.rect(w * 3 / 4 - 8, h - 92, 17, 48, 0xff596e7a);
            hud.rect(w * 3 / 4 - 5, h - 99, 11, 12, 0xff27323d);
            if self.state.muzzle_flash > 0. {
                hud.rect(w * 3 / 4 - 10, h - 112, 22, 16, 0xffffdc72);
            }
        }
        let color = if self.state.hit_flash > 0. {
            0xfffc7555
        } else {
            0xffc2f4ef
        };
        hud.rect(w / 2 - 9, h / 2, 6, 1, color);
        hud.rect(w / 2 + 4, h / 2, 6, 1, color);
        hud.rect(w / 2, h / 2 - 9, 1, 6, color);
        hud.rect(w / 2, h / 2 + 4, 1, 6, color);
        hud.rect(0, 0, w, 30, 0xff14222d);
        let remaining = self.state.enemies.iter().filter(|e| e.health > 0).count();
        let objective = if let Some(status) = self.map_gameplay_status() {
            status.to_uppercase().replace('·', "|")
        } else if self.map.is_some() {
            self.mission_status().to_uppercase().replace('·', "|")
        } else if self.mission.is_some() {
            if self.state.opening.as_ref().is_some_and(|s| !s.done) {
                "GEO_01A | LANDUNGSBRIEFING".into()
            } else if self.state.spire.as_ref().is_some_and(|s| s.flow.enabled) {
                let s = &self.state.spire.as_ref().unwrap().flow;
                if !s.completed
                    && self
                        .state
                        .artillery
                        .as_ref()
                        .is_some_and(|a| a.attack_tick.is_some() && a.impacts.len() < 4)
                {
                    "GEO_01A | BOMBARDIERUNG - ZURUECK IN DECKUNG"
                } else if s.completed {
                    "GEO_01A | TURMEINGANG ERREICHT - ZIELE 0-6"
                } else if s.entered && !s.barrier_open {
                    "GEO_01A | ARTILLERIE - BARRIKADENFREIGABE ABWARTEN"
                } else if s.door_open {
                    "GEO_01A | TURMTUER OFFEN - AUSGANG ERREICHEN"
                } else {
                    "GEO_01A | IN DEN GEONOSIANERTURM EINDRINGEN"
                }
                .into()
            } else if self
                .state
                .demolition
                .as_ref()
                .is_some_and(|s| s.flow.phase != crate::demolition::Phase::Disabled)
            {
                let s = self.state.demolition.as_ref().unwrap();
                match s.flow.phase {
                    crate::demolition::Phase::Available => {
                        "GEO_01A | LADUNG ANVISIEREN - BENUTZEN: BEFEHL"
                    }
                    crate::demolition::Phase::Moving => "GEO_01A | SCORCH LAEUFT ZUM SPRENGPUNKT",
                    crate::demolition::Phase::Starting | crate::demolition::Phase::Setting => {
                        "GEO_01A | SCORCH SETZT DIE LADUNG"
                    }
                    crate::demolition::Phase::Stopping | crate::demolition::Phase::Armed
                        if s.flow.plunger_selected =>
                    {
                        "GEO_01A | ZUENDER - ZURUECKTRETEN, DANN FEUER"
                    }
                    crate::demolition::Phase::Stopping | crate::demolition::Phase::Armed => {
                        "GEO_01A | LADUNG SCHARF - BENUTZEN: ZUENDER"
                    }
                    _ => "GEO_01A | BUNKER GESPRENGT - ZIELE 0-5",
                }
                .into()
            } else if self
                .state
                .rendezvous
                .as_ref()
                .is_some_and(|s| s.radio_started)
            {
                let s = self.state.rendezvous.as_ref().unwrap();
                if s.flow.objective_completed {
                    "GEO_01A | SCORCH IM SQUAD - ABSCHNITT GESCHAFFT".into()
                } else if s.flow.squad_joined {
                    "GEO_01A | DELTA 62 - HUD VERBINDEN".into()
                } else {
                    "GEO_01A | SIX-TWO TREFFEN - DUCKEN IM TUNNEL".into()
                }
            } else if self.throw_target().is_some() {
                let t = self.state.thermal.as_ref().unwrap();
                format!(
                    "GEO_01A  |  {}",
                    if self.state.bunker.is_some() && self.state.phase == Phase::Won {
                        "GESCHUETZSTELLUNG GESICHERT"
                    } else if t.gun.objective_completed {
                        "GESCHUETZ GETROFFEN"
                    } else {
                        "GESCHUETZ - GRANATE WERFEN"
                    }
                )
            } else if self.grenade_target().is_some() {
                let s = self
                    .state
                    .mission
                    .as_ref()
                    .unwrap()
                    .grenades
                    .as_ref()
                    .unwrap();
                format!(
                    "GEO_01A  |  GRANATEN {} / 5  |  {}",
                    s.flow.ammunition,
                    if self.state.phase == Phase::Won {
                        "ABSCHNITT ABGESCHLOSSEN"
                    } else if s.barrier_started && !s.barrier_open {
                        "BARRIKADE - WARTEN"
                    } else {
                        "GRANATEN AUFNEHMEN"
                    }
                )
            } else if self.state.mission.as_ref().unwrap().crash.is_some()
                && self.state.mission.as_ref().unwrap().completed
            {
                if self.state.phase == Phase::Won {
                    "GEO_01A  |  ABSTURZSTELLE GESICHERT".into()
                } else {
                    format!("GEO_01A  |  ABSTURZSTELLE SICHERN  |  {remaining} DROIDEN")
                }
            } else if self.state.mission.as_ref().unwrap().completed {
                "GEO_01A  |  BACTA-ZIEL ABGESCHLOSSEN".into()
            } else {
                "GEO_01A  |  BACTA ERREICHEN UND BENUTZEN HALTEN".into()
            }
        } else if remaining > 0 {
            format!(
                "GEO_01A  |  ZIEL: {remaining} {} AUSSCHALTEN",
                if self.visuals.is_some() {
                    "GEONOSIANER"
                } else {
                    "DROHNEN"
                }
            )
        } else {
            "GEO_01A  |  ZUR GRUENEN EXFIL-MARKIERUNG".into()
        };
        hud.text(8, 7, &objective, 1, 0xffbdeade);
        if let Some(s) = &self.state.opening {
            if s.captions.iter().any(|(_, until)| s.clock < *until) {
                let columns = ((w - 16) / 6).max(1) as usize;
                let text = s
                    .captions
                    .iter()
                    .filter(|(_, until)| s.clock < *until)
                    .map(|(text, _)| text.as_str())
                    .collect::<Vec<_>>()
                    .join(" | ");
                let chars: Vec<_> = text.to_uppercase().chars().collect();
                for (i, line) in chars.chunks(columns).take(3).enumerate() {
                    hud.text(
                        8,
                        h - 65 + i as i32 * 10,
                        &line.iter().collect::<String>(),
                        1,
                        0xffffdf40,
                    );
                }
            }
            if s.clock < s.prompt_until {
                hud.text(8, 38, &s.prompt, 1, 0xffbdeade);
            }
            if s.clock < s.blink_until && ((s.clock * 2.).floor() as u64).is_multiple_of(2) {
                hud.rect(0, h - 28, 90, 2, 0xffff4444);
            }
        }

        if let Some(s) = self
            .state
            .map
            .as_ref()
            .and_then(|m| m.opening.as_ref())
            .filter(|s| s.clock < s.subtitle_until)
        {
            let chars: Vec<_> = s.subtitle.to_uppercase().chars().collect();
            let columns = ((w - 16) / 6).max(1) as usize;
            for (i, line) in chars.chunks(columns).take(3).enumerate() {
                hud.text(
                    8,
                    h - 65 + i as i32 * 10,
                    &line.iter().collect::<String>(),
                    1,
                    0xffffdf40,
                );
            }
        }

        if self
            .state
            .rendezvous
            .as_ref()
            .is_some_and(|s| s.flow.squad_joined)
        {
            hud.text(w - 100, 38, "62 SCORCH", 1, 0xffffdf40);
            if let Some(s) = &self.state.squad {
                let maximum = self.tactics.as_ref().unwrap().assets.squad.max_health;
                hud.rect(w - 100, 49, 85, 4, 0xff263934);
                hud.rect(
                    w - 100,
                    49,
                    (85. * s.health as f64 / maximum as f64).clamp(0., 85.) as i32,
                    4,
                    if s.down { 0xffff5555 } else { 0xff3be796 },
                );
                let label = if s.player_down {
                    "HILFE: BENUTZEN"
                } else if s.down {
                    "62 BELEBEN: HALTEN"
                } else {
                    match s.order {
                        crate::squad::Order::Follow => "62 FOLGT",
                        crate::squad::Order::Hold => "62 HAELT",
                        crate::squad::Order::Focus(_) => "62 ZIEL BESCHIESSEN",
                    }
                };
                hud.text(8, 64, label, 1, 0xffffdf40);
                if s.progress > 0. {
                    let duration = self.tactics.as_ref().unwrap().assets.squad.revive_duration;
                    hud.rect(
                        8,
                        75,
                        (120. * s.progress / duration).clamp(0., 120.) as i32,
                        4,
                        0xff3be796,
                    );
                }
            } else {
                hud.rect(w - 100, 49, 85, 4, 0xff3be796);
            }
        }
        hud.rect(0, h - 26, w, 26, 0xff14222d);
        hud.text(
            8,
            h - 18,
            &self
                .state
                .combat
                .as_ref()
                .and_then(|c| c.melee.as_ref())
                .map_or_else(
                    || {
                        format!(
                            "HP {}   MUNITION {}/{}",
                            self.state.health, self.state.ammo, self.state.reserve
                        )
                    },
                    |m| {
                        format!(
                            "HP {} SCHILD {}   MUNITION {}/{}",
                            self.state.health,
                            crate::melee::display_health(m.player.shields),
                            self.state.ammo,
                            self.state.reserve
                        )
                    },
                ),
            1,
            0xffe4f3f3,
        );
        if self.state.reload_remaining > 0. {
            hud.text(w / 2 - 36, h / 2 + 25, "NACHLADEN", 1, 0xffffdb79);
        }
        if self.state.phase != Phase::Playing {
            hud.rect(w / 2 - 145, h / 2 - 35, 290, 70, 0xff182c39);
            let text = if self.state.phase == Phase::Won {
                if self
                    .state
                    .spire
                    .as_ref()
                    .is_some_and(|s| s.handoff.is_some())
                {
                    "GEO_01A ABGESCHLOSSEN"
                } else {
                    "ABSCHNITT GESCHAFFT"
                }
            } else {
                "GEFALLEN"
            };
            hud.text(
                w / 2 - text.len() as i32 * 6,
                h / 2 - 22,
                text,
                2,
                0xffbdeade,
            );
            hud.text(w / 2 - 78, h / 2 + 12, "ENTER / NEUSTART", 1, 0xffe3e9e9);
        }
        if let Some(g) = self
            .state
            .mission
            .as_ref()
            .and_then(|m| m.grenades.as_ref())
        {
            if g.blink_remaining <= 0. || self.state.statistics.ticks % 60 < 30 {
                hud.text(
                    w.saturating_sub(145),
                    h - 43,
                    &format!("GRANATEN {} / 5", g.flow.ammunition),
                    1,
                    0xffbdeade,
                );
            }
        }
        Ok(RenderFrame {
            view,
            triangles: extra,
            appearance,
            weapon_start,
            hud: hud.rectangles,
            width,
            height,
        })
    }
}
struct Canvas {
    rectangles: Vec<HudRect>,
}
impl Canvas {
    fn rect(&mut self, x: i32, y: i32, width: i32, height: i32, color: u32) {
        self.rectangles.push(HudRect {
            x,
            y,
            width,
            height,
            color,
        });
    }
    fn text(&mut self, mut x: i32, y: i32, text: &str, scale: i32, color: u32) {
        for c in text.chars() {
            let columns: [u8; 5] = match c {
                'A' => [126, 9, 9, 9, 126],
                'B' => [127, 73, 73, 73, 54],
                'C' => [62, 65, 65, 65, 34],
                'D' => [127, 65, 65, 34, 28],
                'E' => [127, 73, 73, 73, 65],
                'F' => [127, 9, 9, 9, 1],
                'G' => [62, 65, 73, 73, 122],
                'H' => [127, 8, 8, 8, 127],
                'I' => [65, 65, 127, 65, 65],
                'J' => [32, 64, 65, 63, 1],
                'K' => [127, 8, 20, 34, 65],
                'L' => [127, 64, 64, 64, 64],
                'M' => [127, 2, 12, 2, 127],
                'N' => [127, 4, 8, 16, 127],
                'O' => [62, 65, 65, 65, 62],
                'P' => [127, 9, 9, 9, 6],
                'Q' => [62, 65, 81, 33, 94],
                'R' => [127, 9, 25, 41, 70],
                'S' => [70, 73, 73, 73, 49],
                'T' => [1, 1, 127, 1, 1],
                'U' => [63, 64, 64, 64, 63],
                'V' => [31, 32, 64, 32, 31],
                'W' => [63, 64, 56, 64, 63],
                'X' => [99, 20, 8, 20, 99],
                'Y' => [7, 8, 112, 8, 7],
                'Z' => [97, 81, 73, 69, 67],
                '0' => [62, 81, 73, 69, 62],
                '1' => [0, 66, 127, 64, 0],
                '2' => [98, 81, 73, 73, 70],
                '3' => [34, 65, 73, 73, 54],
                '4' => [24, 20, 18, 127, 16],
                '5' => [39, 69, 69, 69, 57],
                '6' => [60, 74, 73, 73, 48],
                '7' => [1, 113, 9, 5, 3],
                '8' => [54, 73, 73, 73, 54],
                '9' => [6, 73, 73, 41, 30],
                '/' => [32, 16, 8, 4, 2],
                ':' => [0, 54, 54, 0, 0],
                '-' => [8; 5],
                '_' => [64; 5],
                '|' => [0, 0, 127, 0, 0],
                _ => [0; 5],
            };
            for (i, column) in columns.into_iter().enumerate() {
                for j in 0..7 {
                    if column & (1 << j) != 0 {
                        self.rect(x + i as i32 * scale, y + j * scale, scale, scale, color);
                    }
                }
            }
            x += 6 * scale;
        }
    }
}
