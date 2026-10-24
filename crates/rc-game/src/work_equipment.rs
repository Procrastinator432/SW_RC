//! Original selected work cues and charge appearance, derived from retained progress.
use crate::{Game, Point};
use serde::{Deserialize, Serialize};
#[derive(Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Cue {
    pub original: i32,
    pub at: f64,
    pub sound: usize,
}
#[derive(Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Sound {
    pub sound: usize,
    pub playback: rc_package::audio_params::Playback,
}
#[derive(Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Charge {
    pub job: usize,
    pub position: Point,
    pub rotation: [i32; 3],
    pub static_triangles: Vec<usize>,
}
#[derive(Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Assets {
    pub schedules: Vec<Vec<Cue>>,
    pub placed_at: Vec<Option<f64>>,
    pub sounds: Vec<Sound>,
    pub charges: Vec<Charge>,
    pub models: [Vec<u8>; 2],
}
pub struct Prepared {
    pub assets: Assets,
    pub models: [rc_render::Scene; 2],
}
pub fn crossed(from: f64, to: f64, at: f64) -> bool {
    to > from && at <= to && (at > from || (from == 0. && at == 0.))
}
pub fn charge_skin(progress: f64, placed_at: f64, armed: bool, used: bool) -> Option<usize> {
    if used || progress < placed_at {
        None
    } else {
        Some(usize::from(armed))
    }
}
pub(crate) fn prepare(
    game: &mut Game,
    a: Assets,
    jobs: &[crate::map_gameplay::Interaction],
) -> Result<Prepared, String> {
    if a.schedules.len() != jobs.len()
        || a.placed_at.len() != jobs.len()
        || a.sounds.is_empty()
        || a.sounds.len() > 16
        || a.charges.len() != jobs.iter().filter(|j| j.demolition).count()
    {
        return Err("Invalid work equipment bindings".into());
    }
    let count = game
        .map
        .as_ref()
        .ok_or("Missing work map")?
        .assets
        .sounds
        .len();
    let mut sound_ids = std::collections::BTreeSet::new();
    for s in &a.sounds {
        s.playback.validate()?;
        if s.sound >= count || !sound_ids.insert(s.sound) {
            return Err("Invalid work sound binding".into());
        }
    }
    for (i, job) in jobs.iter().enumerate() {
        let cues = &a.schedules[i];
        if cues.len() > 128
            || cues.iter().any(|c| {
                c.original <= 0
                    || !c.at.is_finite()
                    || !(0. ..=job.seconds).contains(&c.at)
                    || !sound_ids.contains(&c.sound)
            })
            || cues.windows(2).any(|c| c[0].at > c[1].at)
            || a.placed_at[i].is_some() != job.demolition
            || a.placed_at[i].is_some_and(|p| !p.is_finite() || p <= 0. || p >= job.seconds)
        {
            return Err("Invalid work notify timeline".into());
        }
    }
    let mut models = [
        rc_render::Scene::from_snapshot(&a.models[0])?,
        rc_render::Scene::from_snapshot(&a.models[1])?,
    ];
    if models
        .iter()
        .any(|m| m.triangles.is_empty() || m.triangles.len() > 10000)
    {
        return Err("Work model budget".into());
    }
    let mut remove = std::collections::BTreeSet::new();
    let mut owners = std::collections::BTreeSet::new();
    for c in &a.charges {
        if c.job >= jobs.len()
            || !jobs[c.job].demolition
            || !owners.insert(c.job)
            || c.position.iter().any(|v| !v.is_finite() || v.abs() > 1e7)
            || c.static_triangles.len() != models[0].triangles.len()
        {
            return Err("Invalid charge actor binding".into());
        }
        for (index, local) in c.static_triangles.iter().zip(&models[0].triangles) {
            let original = game
                .scene
                .triangles
                .get(*index)
                .ok_or("Charge static triangle index")?;
            let expected = local.points.map(|p| {
                rc_package::mesh::transform_point(
                    p,
                    c.position.map(|v| v as f32),
                    c.rotation,
                    [1.; 3],
                    [0.; 3],
                )
            });
            let key = |points: [[f32; 3]; 3]| {
                let mut p = points.map(|p| p.map(|v| (v as f64 * 512.).round() as i64));
                p.sort();
                p
            };
            if !remove.insert(*index) || key(original.points) != key(expected) {
                return Err("Charge geometry mismatch".into());
            }
        }
    }
    game.scene.triangles = std::mem::take(&mut game.scene.triangles)
        .into_iter()
        .enumerate()
        .filter(|(i, _)| !remove.contains(i))
        .map(|(_, t)| t)
        .collect();
    game.scene.appearance = std::mem::take(&mut game.scene.appearance)
        .into_iter()
        .enumerate()
        .filter(|(i, _)| !remove.contains(i))
        .map(|(_, f)| f)
        .collect();
    for model in &mut models {
        let base = game.scene.textures.len();
        game.scene.textures.append(&mut model.textures);
        for f in model.appearance.iter_mut().flatten() {
            f.texture += base;
        }
    }
    Ok(Prepared { assets: a, models })
}
impl Game {
    pub fn work_playback(&self, sequence: u64) -> Option<&rc_package::audio_params::Playback> {
        let m = self.state.map.as_ref()?;
        let event = m.events.iter().find(|e| e.sequence == sequence)?;
        let p = self
            .map
            .as_ref()?
            .gameplay
            .as_ref()?
            .work_equipment
            .as_ref()?;
        p.assets
            .sounds
            .iter()
            .find(|s| s.sound == event.sound)
            .map(|s| &s.playback)
    }
    pub(crate) fn work_equipment_geometry(
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
            .and_then(|m| m.gameplay.as_ref())
            .and_then(|p| p.work_equipment.as_ref())
        else {
            return Ok((vec![], vec![]));
        };
        let m = self.state.map.as_ref().unwrap();
        let s = m.gameplay.as_ref().unwrap();
        let mut tris = vec![];
        let mut faces = vec![];
        for c in &p.assets.charges {
            if let Some(skin) = charge_skin(
                s.progress[c.job],
                p.assets.placed_at[c.job].unwrap(),
                s.armed[c.job],
                s.used[c.job],
            ) {
                let model = &p.models[skin];
                for t in &model.triangles {
                    let mut t = t.clone();
                    t.points = t.points.map(|v| {
                        rc_package::mesh::transform_point(
                            v,
                            c.position.map(|v| v as f32),
                            c.rotation,
                            [1.; 3],
                            [0.; 3],
                        )
                    });
                    tris.push(t);
                }
                faces.extend(model.appearance.iter().copied());
            }
        }
        if let Some(pose) = self.map_work_pose(0) {
            let task = s
                .execution
                .as_ref()
                .and_then(|e| e.tasks[0].as_ref())
                .unwrap();
            if s.progress[task.interaction] < p.assets.placed_at[task.interaction].unwrap() {
                let map = self.map.as_ref().unwrap();
                let mesh = &map.squad;
                let actor = m.companion.as_ref().unwrap();
                let channel = mesh.channel(pose.clip, pose.frame)?;
                let transform = mesh.attachment_transform(&channel, "weaponAttach_L", true)?;
                let (sin, cos) = (actor.yaw as f32).sin_cos();
                for t in &p.models[0].triangles {
                    let mut t = t.clone();
                    t.points = t.points.map(|v| {
                        let local: [f32; 3] = std::array::from_fn(|i| {
                            v[0] * transform[0][i]
                                + v[1] * transform[1][i]
                                + v[2] * transform[2][i]
                                + transform[3][i]
                        });
                        [
                            actor.position[0] as f32 + local[0] * cos - local[1] * sin,
                            actor.position[1] as f32 + local[0] * sin + local[1] * cos,
                            actor.position[2] as f32 - 84. + local[2] - mesh.rest_bounds[0][2],
                        ]
                    });
                    tris.push(t);
                }
                faces.extend(p.models[0].appearance.iter().copied());
            }
        }
        Ok((tris, faces))
    }
}
#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn boundaries_are_delivered_once() {
        assert!(crossed(0., 0.1, 0.));
        assert!(!crossed(0.1, 0.2, 0.));
        assert!(crossed(0., 0.1, 0.1));
        assert!(!crossed(0.1, 0.2, 0.1));
    }
    #[test]
    fn pause_or_repeated_sampling_emits_nothing() {
        assert!(!crossed(0.5, 0.5, 0.5));
        assert!(!crossed(0.7, 0.5, 0.6));
    }
    #[test]
    fn skipped_frames_deliver_every_crossed_cue() {
        let cues = [0.1, 0.4, 0.5, 0.9];
        assert_eq!(
            cues.into_iter()
                .filter(|t| crossed(0.2, 1., *t))
                .collect::<Vec<_>>(),
            vec![0.4, 0.5, 0.9]
        );
    }
    #[test]
    fn retained_progress_cannot_replay_earlier_cues() {
        let cues = [0.1, 0.4, 0.5, 0.9];
        assert_eq!(
            cues.into_iter()
                .filter(|t| crossed(0.5, 1., *t))
                .collect::<Vec<_>>(),
            vec![0.9]
        );
    }
    #[test]
    fn charge_appears_only_after_original_placement() {
        assert_eq!(charge_skin(0.5, 0.6, false, false), None);
        assert_eq!(charge_skin(0.6, 0.6, false, false), Some(0));
        assert_eq!(charge_skin(1., 0.6, false, false), Some(0));
    }
    #[test]
    fn arming_changes_skin_and_detonation_hides_model() {
        assert_eq!(charge_skin(3., 0.6, true, false), Some(1));
        assert_eq!(charge_skin(3., 0.6, true, true), None);
    }
}
