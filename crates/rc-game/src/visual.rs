//! Original compressed animation, hierarchy and skinning; encounter clip selection is port policy.
//! Immutable asset caches are shared. Channels, scratch and skin outputs never cross instances.
use rc_package::{
    mesh_animation::AnimationChannel,
    quaternion_animation::PortableQuaternionMath,
    skeletal_animation::StoredSequence,
    skeletal_channel::PortableChannelAngularMath,
    skeletal_channel_stack::{apply_channels_prepared, PreparedChannelPose},
    skeletal_draw::build_skeletal_draw,
    skeletal_hierarchy::{build_pose_matrices, prepare_hierarchy, BoneHierarchy},
    skeletal_lod::{inspect_skin_program, StoredLod},
    skeletal_mesh::StoredBone,
    skeletal_reference_cache::ensure_inverse_reference_cache,
    skeletal_root_pose::RootTransform,
    skeletal_runtime_hosts::{
        ResolvedLinkup, ResolvedSequence, RuntimeAnimationBindings, RuntimePoseHost,
    },
    skeletal_skin::{build_skin_palette, skin_lod_stream, SkinStreamVertex},
};
use serde::{Deserialize, Serialize};
use std::{cell::RefCell, rc::Rc};

#[derive(Serialize, Deserialize)]
pub struct Assets {
    pub enemy: Mesh,
    pub weapon: Mesh,
}
pub struct PreparedAssets {
    pub enemy: PreparedMesh,
    pub weapon: PreparedMesh,
    pub texture_base: usize,
}
#[derive(Debug, Clone, Serialize, serde::Deserialize)]
pub struct Player {
    pub clip: usize,
    pub frame: f32,
    pub finished: bool,
    pub started: bool,
}
#[derive(Debug, Clone, Serialize, serde::Deserialize)]
pub struct State {
    pub enemies: Vec<Player>,
    pub weapon: Player,
}
impl Player {
    fn new(mesh: &PreparedMesh, name: &str) -> Result<Self, String> {
        Ok(Self {
            clip: mesh.clip(name)?,
            frame: 0.,
            finished: false,
            started: false,
        })
    }
    pub(crate) fn select(
        &mut self,
        mesh: &PreparedMesh,
        name: &str,
        restart: bool,
    ) -> Result<(), String> {
        let clip = mesh.clip(name)?;
        if self.clip != clip || restart {
            self.clip = clip;
            self.frame = 0.;
            self.finished = false;
            self.started = false;
        }
        Ok(())
    }
    fn advance(
        &mut self,
        mesh: &PreparedMesh,
        looping: bool,
        duration: Option<f32>,
    ) -> Result<(f32, f32), String> {
        if self.finished {
            return Ok((self.frame, self.frame));
        }
        let sequence = &mesh.mesh.clips[self.clip].sequence;
        let mut channel = mesh.channel(self.clip, self.frame)?;
        let rate = duration.map_or(f32::from_bits(sequence.rate_bits), |d| {
            sequence.frames as f32 / d
        });
        rc_package::skeletal_animation_tick::advance_channel_frame(
            &mut channel,
            sequence.frames,
            rate,
            crate::STEP as f32,
        );
        let next = f32::from_bits(channel.words[7]);
        let old = if self.started {
            self.frame
        } else {
            -f32::EPSILON
        };
        self.started = true;
        if looping {
            self.frame = next.rem_euclid(1.);
        } else {
            let last = 1. - 1. / sequence.frames as f32;
            self.frame = next.min(last);
            self.finished = next >= last;
        }
        Ok((old, if looping { next } else { self.frame }))
    }
}
impl PreparedAssets {
    pub fn state(&self, count: usize) -> Result<State, String> {
        Ok(State {
            enemies: (0..count)
                .map(|_| Player::new(&self.enemy, "ActionBreathe"))
                .collect::<Result<_, _>>()?,
            weapon: Player::new(&self.weapon, "ActionBreathe")?,
        })
    }
    pub fn tick(
        &self,
        now: &crate::State,
        old: &crate::State,
        visual: &mut State,
        combat: Option<&crate::combat::Assets>,
    ) -> Result<Vec<(usize, i32)>, String> {
        let mut events = vec![];
        for (owner, ((player, e), previous)) in visual
            .enemies
            .iter_mut()
            .zip(&now.enemies)
            .zip(&old.enemies)
            .enumerate()
        {
            let name = &self.enemy.mesh.clips[player.clip].sequence.name.name;
            let (clip, looping) = if e.health == 0 {
                ("Death", false)
            } else if e.health < previous.health {
                (
                    if self
                        .enemy
                        .mesh
                        .source
                        .eq_ignore_ascii_case("droid.BattleDroid")
                    {
                        "Impact"
                    } else {
                        "hit"
                    },
                    false,
                )
            } else if let Some(clip) = now
                .combat
                .as_ref()
                .and_then(|c| c.jump.as_ref())
                .and_then(|s| s.motions[owner].as_ref())
                .and_then(|m| m.clip())
            {
                (clip, clip == "FallBreathe")
            } else if let Some(index) = now
                .combat
                .as_ref()
                .and_then(|c| c.melee.as_ref())
                .and_then(|m| m.attacks[owner].active)
            {
                (
                    combat
                        .and_then(|c| c.melee.as_ref())
                        .ok_or("Missing melee rules")?
                        .clips[index]
                        .name
                        .as_str(),
                    false,
                )
            } else if combat.is_none_or(|c| c.melee.is_none()) && e.cooldown > previous.cooldown {
                ("Fire", false)
            } else if !player.finished && ["hit", "Impact", "Fire"].contains(&name.as_str()) {
                (name.as_str(), false)
            } else if e.position != previous.position {
                ("RunForward", true)
            } else {
                ("ActionBreathe", true)
            };
            // Copy the name before selecting to keep asset data immutable while state changes.
            let clip = clip.to_owned();
            player.select(
                &self.enemy,
                &clip,
                e.health < previous.health && e.health > 0 || e.cooldown > previous.cooldown,
            )?;
            let interval = player.advance(&self.enemy, looping, None)?;
            notify_events(combat, false, &clip, interval, owner as i32, &mut events);
        }
        let shot = now.statistics.shots > old.statistics.shots;
        let reload = now.reload_remaining > 0.;
        let current = &self.weapon.mesh.clips[visual.weapon.clip]
            .sequence
            .name
            .name;
        let moving = now.pawn.body.position != old.pawn.body.position;
        let throwing = now.thermal.as_ref().is_some_and(|t| t.gate.active);
        let name = if throwing {
            "ThrowGrenade"
        } else if reload {
            "Reload"
        } else if shot || current == "Fire" && !visual.weapon.finished {
            "Fire"
        } else if moving {
            "Walk"
        } else {
            "ActionBreathe"
        };
        visual.weapon.select(
            &self.weapon,
            name,
            shot || now.statistics.reloads > old.statistics.reloads,
        )?;
        let interval = visual.weapon.advance(
            &self.weapon,
            !reload && name != "Fire" && !throwing,
            if combat.is_some() {
                None
            } else {
                reload.then_some(1.2)
            },
        )?;
        notify_events(combat, true, name, interval, -1, &mut events);
        Ok(events)
    }
}

fn notify_events(
    combat: Option<&crate::combat::Assets>,
    weapon: bool,
    clip: &str,
    (old, next): (f32, f32),
    owner: i32,
    out: &mut Vec<(usize, i32)>,
) {
    if let Some(c) = combat {
        for n in &c.notifies {
            if n.weapon == weapon && n.clip == clip && crossed_notify(old, next, n.time) {
                out.push((n.sound, owner));
            }
        }
    }
}
// Half-open intervals dispatch each crossing once, including a loop wrap and time zero at entry.
pub fn crossed_notify(old: f32, next: f32, time: f32) -> bool {
    if next <= old {
        return false;
    }
    let first = (old - time).floor() + 1.;
    time + first <= next
}

#[derive(Serialize, Deserialize)]
pub struct Material {
    pub source: String,
    pub chain: Vec<String>,
    pub opacity: serde_json::Value,
    pub width: usize,
    pub height: usize,
    #[serde(with = "crate::pixel_words")]
    pub pixels: Vec<u32>,
    pub uv_scale: f32,
    pub uv_transform: Option<rc_package::material_uv::UvTransform>,
}
#[derive(Serialize, Deserialize)]
pub struct Clip {
    pub source: String,
    pub mapping: Vec<i32>,
    pub sequence: StoredSequence,
}
#[derive(Serialize, Deserialize)]
pub struct Mesh {
    pub source: String,
    pub scale: [u32; 3],
    pub origin: [u32; 3],
    pub rotation: [i32; 3],
    pub bones: Vec<StoredBone>,
    pub lod: StoredLod,
    pub materials: Vec<Material>,
    pub clips: Vec<Clip>,
}
pub struct PreparedMesh {
    pub mesh: Mesh,
    pub rest_bounds: [[f32; 3]; 2],
    reference: Vec<RootTransform>,
    hierarchy: BoneHierarchy,
    inverse: Vec<[u32; 16]>,
    output_count: usize,
}
impl Mesh {
    pub fn prepare(self) -> Result<PreparedMesh, String> {
        self.prepare_with_rest(false)
    }
    /// Bake a non-character reference mesh, without requiring an idle clip.
    pub fn prepare_reference(self) -> Result<PreparedMesh, String> {
        self.prepare_with_rest(true)
    }
    fn prepare_with_rest(self, reference_only: bool) -> Result<PreparedMesh, String> {
        if self.bones.is_empty()
            || self.bones.len() > 256
            || self.clips.is_empty()
            || self.clips.len() > 32
            || self.materials.is_empty()
            || self.materials.len() > 32
            || self.lod.commands.len() > 200_000
            || self.lod.bind_vertices.len() > 100_000
            || self.lod.stream_vertices.len() > 100_000
            || self.lod.indices.iter().map(Vec::len).sum::<usize>() > 300_000
        {
            return Err("Animated mesh exceeds runtime budget".into());
        }
        for m in &self.materials {
            if m.width == 0
                || m.height == 0
                || m.width > 1024
                || m.height > 1024
                || m.pixels.len() != m.width * m.height
                || !m.uv_scale.is_finite()
            {
                return Err("Invalid animated material".into());
            }
        }
        for (i, c) in self.clips.iter().enumerate() {
            if self.clips[..i].iter().any(|other| {
                other
                    .sequence
                    .name
                    .name
                    .eq_ignore_ascii_case(&c.sequence.name.name)
            }) {
                return Err("Duplicate animated clip name".into());
            }
            if c.sequence.frames < 1
                || c.sequence.frames > 10000
                || !f32::from_bits(c.sequence.rate_bits).is_finite()
                || f32::from_bits(c.sequence.rate_bits) <= 0.
                || f32::from_bits(c.sequence.rate_bits) > 1000.
                || c.sequence.tracks.iter().any(|t| {
                    !f32::from_bits(t.position_scale_bits).is_finite()
                        || t.positions.len() > 100_000
                        || t.rotations.len() > 100_000
                        || t.durations.len() > 100_000
                })
                || c.mapping.len() != self.bones.len()
                || c.sequence.tracks.len() > 256
                || c.mapping
                    .iter()
                    .any(|&v| v < -1 || v >= c.sequence.tracks.len() as i32)
            {
                return Err("Invalid animated clip or bone mapping".into());
            }
        }
        let hierarchy = prepare_hierarchy(&self.bones)?;
        let reference = self
            .bones
            .iter()
            .map(|b| RootTransform {
                rotation: b.rotation,
                position: b.position,
            })
            .collect();
        let mut inverse = vec![];
        ensure_inverse_reference_cache(&self.bones, &mut inverse, &mut 0)?;
        let output_count =
            inspect_skin_program(&self.lod.commands, self.lod.bind_vertices.len())?.outputs;
        if output_count > 100_000 {
            return Err("Animated skin output exceeds budget".into());
        }
        let mut result = PreparedMesh {
            mesh: self,
            reference,
            hierarchy,
            inverse,
            output_count,
            rest_bounds: [[0.; 3]; 2],
        };
        // Validate original transform and section/index/skin dependencies before publishing the asset.
        let mut channel = result.channel(
            if reference_only {
                0
            } else {
                result.clip("ActionBreathe")?
            },
            0.,
        )?;
        if reference_only {
            channel.words[14] = 0;
        }
        let rest = result.geometry(&channel)?;
        result.rest_bounds = std::array::from_fn(|end| {
            std::array::from_fn(|axis| {
                rest.triangles
                    .iter()
                    .flat_map(|t| t.points)
                    .map(|p| p[axis])
                    .fold(
                        if end == 0 {
                            f32::INFINITY
                        } else {
                            f32::NEG_INFINITY
                        },
                        |a, b| if end == 0 { a.min(b) } else { a.max(b) },
                    )
            })
        });
        for index in 0..result.mesh.clips.len() {
            result.geometry(&result.channel(index, 0.)?)?;
        }
        Ok(result)
    }
}
impl PreparedMesh {
    pub fn clip(&self, name: &str) -> Result<usize, String> {
        self.mesh
            .clips
            .iter()
            .position(|c| c.sequence.name.name.eq_ignore_ascii_case(name))
            .ok_or_else(|| format!("{} lacks required clip {name}", self.mesh.source))
    }
    pub fn channel(&self, index: usize, frame: f32) -> Result<AnimationChannel, String> {
        let clip = self
            .mesh
            .clips
            .get(index)
            .ok_or("Invalid animation clip index")?;
        if !frame.is_finite() || !(0. ..=1.).contains(&frame) {
            return Err("Invalid normalized animation frame".into());
        }
        let mut c = AnimationChannel::default();
        // Identity handles are bundle-local and rebuilt; original package name indices may collide.
        c.words[0] = index as u32 + 1;
        c.words[6] = 1f32.to_bits();
        c.words[7] = frame.to_bits();
        c.words[9] = (1. - 1. / clip.sequence.frames as f32).to_bits();
        c.words[11] = 1f32.to_bits();
        c.words[14] = 1f32.to_bits();
        c.words[16] = self.mesh.bones.len() as u32;
        c.words[17] = index as u32 + 1;
        Ok(c)
    }
    pub fn scene(&self, channel: &AnimationChannel) -> Result<rc_render::Scene, String> {
        let mut scene = self.geometry(channel)?;
        scene.textures = self
            .mesh
            .materials
            .iter()
            .map(|m| rc_render::Texture {
                width: m.width,
                height: m.height,
                pixels: m.pixels.clone(),
            })
            .collect();
        scene.snapshot()?;
        Ok(scene)
    }
    pub fn validate_lunge(&self) -> Result<(), String> {
        if self.mesh.rotation != [0; 3]
            || self.mesh.scale != [1f32.to_bits(); 3]
            || self.mesh.bones[0].name.name != "Move"
        {
            return Err("Unsupported lunge mesh transform/root".into());
        }
        for name in ["Lunge", "Lunge2"] {
            let clip = self.clip(name)?;
            let c = &self.mesh.clips[clip];
            let root = usize::try_from(c.mapping[0]).map_err(|_| "Missing lunge root")?;
            let track = c.sequence.tracks.get(root).ok_or("Missing lunge root")?;
            if track.rotations.iter().any(|&r| r != [0; 3]) || c.sequence.frames > 300 {
                return Err("Lunge root rotation/motion duration is unsupported".into());
            }
            let last = 1. - 1. / c.sequence.frames as f32;
            let first = self.root_position(clip, 0.)?;
            let end = self.root_position(clip, last)?;
            if end[0] <= first[0] {
                return Err("Lunge lacks forward translation".into());
            }
        }
        Ok(())
    }
    pub fn root_position(&self, index: usize, frame: f32) -> Result<[f32; 3], String> {
        let result = self.sample_root_position(index, frame)?;
        if result.iter().any(|v| v.abs() > 10000.) {
            return Err("Invalid root translation".into());
        }
        Ok(result)
    }
    /// Authored cinematic movement relative to clip entry; separate from pawn root motion.
    pub fn cinematic_root_delta(&self, index: usize, frame: f32) -> Result<[f32; 3], String> {
        let now = self.sample_root_position(index, frame)?;
        let first = self.sample_root_position(index, 0.)?;
        Ok(std::array::from_fn(|i| now[i] - first[i]))
    }
    fn sample_root_position(&self, index: usize, frame: f32) -> Result<[f32; 3], String> {
        let result = self
            .sample_root_transform(index, frame)?
            .position
            .map(f32::from_bits);
        if result.iter().any(|v| !v.is_finite() || v.abs() > 1e6) {
            return Err("Invalid root translation".into());
        }
        Ok(result)
    }
    /// Raw animated Move transform for GetMoveCoords; no mesh/actor adjustment.
    pub fn root_matrix(&self, index: usize, frame: f32) -> Result<[u32; 16], String> {
        Ok(
            rc_package::skeletal_root_pose::quaternion_translation_matrix(
                self.sample_root_transform(index, frame)?,
            ),
        )
    }
    fn sample_root_transform(&self, index: usize, frame: f32) -> Result<RootTransform, String> {
        self.channel(index, frame)?;
        let clip = &self.mesh.clips[index];
        let index = *clip.mapping.first().ok_or("Missing root mapping")?;
        let track = clip
            .sequence
            .tracks
            .get(usize::try_from(index).map_err(|_| "Missing root track")?)
            .ok_or("Missing root track")?;
        let mut root = self.reference[0];
        let time = (f64::from(clip.sequence.frames) * f64::from(frame)) as f32;
        rc_package::skeletal_track::sample_track(
            track,
            time,
            &mut root,
            &mut rc_package::quaternion_animation::QuaternionTrackHost {
                math: PortableQuaternionMath,
            },
        )?;
        if root
            .rotation
            .iter()
            .chain(root.position.iter())
            .any(|v| !f32::from_bits(*v).is_finite())
        {
            return Err("Invalid root transform".into());
        }
        Ok(root)
    }
    pub fn geometry(&self, channel: &AnimationChannel) -> Result<rc_render::Scene, String> {
        self.geometry_with_root(channel, false)
    }
    pub fn geometry_with_root(
        &self,
        channel: &AnimationChannel,
        lock_root: bool,
    ) -> Result<rc_render::Scene, String> {
        let matrices = self.pose_matrices(channel, lock_root)?;
        self.geometry_from_matrices(&matrices)
    }
    fn pose_matrices(
        &self,
        channel: &AnimationChannel,
        lock_root: bool,
    ) -> Result<Vec<[u32; 16]>, String> {
        let index = channel.words[17]
            .checked_sub(1)
            .ok_or("Null visual sequence")? as usize;
        let clip = self
            .mesh
            .clips
            .get(index)
            .ok_or("Unknown visual sequence")?;
        let identity = index as u32 + 1;
        let sequences = [ResolvedSequence {
            identity,
            linkup_key: 1,
            sequence: &clip.sequence,
        }];
        let names = [(identity, identity)];
        let mut host = RuntimePoseHost {
            data: RuntimeAnimationBindings {
                sequences: &sequences,
                names: &names,
                linkups: Rc::new(RefCell::new(vec![ResolvedLinkup {
                    key: 1,
                    animation_names: None,
                    mapping: clip.mapping.clone(),
                }])),
                events: Rc::new(RefCell::new(vec![])),
            },
            reference: &self.reference,
            move_bone: self.hierarchy.move_bone,
            editor: false,
            math: PortableQuaternionMath,
            angular: PortableChannelAngularMath,
        };
        let mut pose = PreparedChannelPose {
            local: self.reference.clone(),
            scratch: self.reference.clone(),
            byte_61: 0,
        };
        apply_channels_prepared(
            &mut pose,
            &self.reference,
            &mut [channel.clone()],
            false,
            0,
            &mut host,
        )?;
        if lock_root {
            pose.local[0].position = self.reference[0].position;
        }
        let mut matrices = vec![];
        build_pose_matrices(&pose.local, &self.hierarchy, false, &mut matrices)?;
        Ok(matrices)
    }
    /// Original animated bone followed by the same mesh adjustment as skinned vertices.
    pub fn joint_positions(
        &self,
        channel: &AnimationChannel,
        lock_root: bool,
    ) -> Result<Vec<[f32; 3]>, String> {
        let matrices = self.pose_matrices(channel, lock_root)?;
        let t = self.adjustment()?;
        Ok(matrices
            .into_iter()
            .map(|m| {
                std::array::from_fn(|i| {
                    f32::from_bits(m[12]) * t[0][i]
                        + f32::from_bits(m[13]) * t[1][i]
                        + f32::from_bits(m[14]) * t[2][i]
                        + t[3][i]
                })
            })
            .collect())
    }
    pub fn attachment_transform(
        &self,
        channel: &AnimationChannel,
        bone: &str,
        lock_root: bool,
    ) -> Result<[[f32; 4]; 4], String> {
        let index = self
            .mesh
            .bones
            .iter()
            .position(|b| b.name.name.eq_ignore_ascii_case(bone))
            .ok_or("Missing attachment bone")?;
        let matrices = self.pose_matrices(channel, lock_root)?;
        let b = matrices[index].map(f32::from_bits);
        let t = self.adjustment()?;
        Ok(std::array::from_fn(|row| {
            std::array::from_fn(|column| (0..4).map(|k| b[row * 4 + k] * t[k][column]).sum())
        }))
    }
    fn adjustment(&self) -> Result<[[f32; 4]; 4], String> {
        Ok(rc_package::mesh_transform::Input {
            actor_location: [0; 3],
            actor_rotation: [0; 3],
            actor_draw_scale: 1f32.to_bits(),
            actor_draw_scale_3d: [1f32.to_bits(); 3],
            actor_prepivot: [0; 3],
            actor_flags: 0,
            actor_height: 0,
            mesh_scale: self.mesh.scale,
            mesh_origin: self.mesh.origin,
            mesh_rotation: self.mesh.rotation,
            suppress_origin: 0,
            bypass_mesh_adjustments: 0,
        }
        .build()?
        .matrix
        .map(|row| row.map(f32::from_bits)))
    }
    fn geometry_from_matrices(&self, matrices: &[[u32; 16]]) -> Result<rc_render::Scene, String> {
        let mut palette = vec![];
        build_skin_palette(matrices, &self.inverse, &mut palette)?;
        let mut soft = vec![SkinStreamVertex::default(); self.output_count];
        skin_lod_stream(
            &self.mesh.lod.commands,
            &self.mesh.lod.bind_vertices,
            &palette,
            &mut soft,
            &mut vec![],
        )?;
        let draw = build_skeletal_draw(&self.mesh.lod, &soft, matrices, &self.inverse)?;
        let mut scene = rc_render::Scene::from_skeletal_draw(&draw)?;
        scene.textures.clear();
        for (triangle, face) in scene.triangles.iter().zip(&mut scene.appearance) {
            let m = self
                .mesh
                .materials
                .get(triangle.surface)
                .ok_or("Unknown animated material slot")?;
            let face = face.as_mut().ok_or("Missing animated source UV")?;
            face.texture = triangle.surface;
            face.uv = face.uv.map(|uv| {
                let scaled = uv.map(|v| v * m.uv_scale);
                m.uv_transform.map_or(scaled, |t| t.apply(scaled))
            });
            if face.uv.iter().flatten().any(|v| !v.is_finite()) {
                return Err("Nonfinite animated UV".into());
            }
        }
        let transform = self.adjustment()?;
        for triangle in &mut scene.triangles {
            triangle.points = triangle.points.map(|p| {
                std::array::from_fn(|i| {
                    p[0] * transform[0][i]
                        + p[1] * transform[1][i]
                        + p[2] * transform[2][i]
                        + transform[3][i]
                })
            });
        }
        if scene
            .triangles
            .iter()
            .flat_map(|t| t.points.iter().flatten())
            .any(|v| !v.is_finite() || v.abs() > 1e9)
        {
            return Err("Invalid animated position".into());
        }
        Ok(scene)
    }
}
