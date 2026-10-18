use crate::{
    spire::{self, Effect},
    Game, Point, STEP,
};
use serde::{Deserialize, Serialize};
#[derive(Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Assets {
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub door_motion: Vec<DoorMotion>,
    pub flow: spire::Flow,
    pub route: Vec<Point>,
    pub sounds: Vec<crate::combat::Sound>,
    pub objects: Vec<crate::dynamic::Object>,
    pub provenance: serde_json::Value,
}
#[derive(Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct DoorMotion {
    pub original: u32,
    pub delay: f64,
    pub seconds: f64,
    pub glide: bool,
    pub offset: [f32; 3],
}
impl DoorMotion {
    fn begins_between(&self, before: f64, after: f64) -> bool {
        self.delay > 0. && before < self.delay && after >= self.delay
    }
    pub fn displacement(&self, elapsed: f64) -> [f32; 3] {
        let t = ((elapsed - self.delay) / self.seconds).clamp(0., 1.) as f32;
        let fraction = if self.glide {
            3. * t * t - 2. * t * t * t
        } else {
            t
        };
        self.offset.map(|v| v * fraction)
    }
}
pub struct Prepared {
    pub assets: Assets,
    pub sound_base: usize,
}
#[derive(Clone, Debug, Default, Serialize, serde::Deserialize)]
pub struct State {
    #[serde(skip_serializing_if = "Option::is_none")]
    pub handoff: Option<crate::transition::Handoff>,
    pub flow: spire::State,
    pub guide: usize,
    pub effects: Vec<(u64, Effect)>,
    pub completed_tick: Option<u64>,
}
impl Game {
    pub fn attach_spire(&mut self, mut a: Assets) -> Result<(), String> {
        if self.spire.is_some()
            || self.demolition.is_none()
            || a.route.is_empty()
            || a.route.len() > 512
            || a.route
                .iter()
                .flatten()
                .any(|x| !x.is_finite() || x.abs() > 1e8)
            || a.sounds.len() > 24
        {
            return Err("Invalid spire extension".into());
        }
        a.flow.validate(a.sounds.len())?;
        if !a.door_motion.is_empty()
            && (a.door_motion.len() != 2
                || a.door_motion
                    .iter()
                    .map(|m| m.original)
                    .collect::<std::collections::BTreeSet<_>>()
                    != [34, 35].into_iter().collect()
                || a.door_motion.iter().any(|m| {
                    !m.delay.is_finite()
                        || !(0. ..=30.).contains(&m.delay)
                        || !m.seconds.is_finite()
                        || !(0.001..=30.).contains(&m.seconds)
                        || m.offset.iter().any(|v| !v.is_finite() || v.abs() > 10000.)
                        || (m.delay + m.seconds - a.flow.door_seconds).abs() > 0.001
                }))
        {
            return Err("Invalid original door motion".into());
        }
        for s in &a.sounds {
            crate::combat::validate_wave(&s.wave)?;
        }
        let ids = a
            .objects
            .iter()
            .map(|o| o.original)
            .collect::<std::collections::BTreeSet<_>>();
        if ids != [34, 35, 860, 861, 862].into_iter().collect() {
            return Err("Incomplete spire dynamic objects".into());
        }
        for object in &a.objects {
            if matches!(object.original, 34 | 35) && object.released.is_none() {
                return Err("Exit door lacks open pose".into());
            }
        }
        self.append_dynamic(std::mem::take(&mut a.objects))?;
        let sound_base = self.mission.as_ref().unwrap().sounds.len();
        self.mission
            .as_mut()
            .unwrap()
            .sounds
            .extend(a.sounds.iter().map(|s| crate::combat::Sound {
                source: s.source.clone(),
                wave: s.wave.clone(),
            }));
        self.spire = Some(Prepared {
            assets: a,
            sound_base,
        });
        self.state.spire = Some(State::default());
        self.initial = self.state.clone();
        Ok(())
    }
    pub(crate) fn spire_tick(&mut self) -> Result<(), String> {
        let Some(p) = &self.spire else {
            return Ok(());
        };
        let demo = self.state.demolition.as_ref().unwrap();
        let s = self.state.spire.as_mut().unwrap();
        let before_door = s.flow.door_elapsed;
        let mut effects = s.flow.advance(
            &p.assets.flow,
            STEP,
            self.state.pawn.body.position,
            self.profile.physics.extent,
            demo.explosion_tick.is_some(),
            demo.complete_tick.is_some(),
        )?;
        // Original Mover.Open sleeps DelayTime before DoOpen plays OpeningSound.
        if let Some(motion) = p.assets.door_motion.first().filter(|m| m.delay > 0.) {
            effects
                .retain(|e| !matches!(e, Effect::Sound(sound) if *sound == p.assets.flow.opening));
            if s.flow.door_started && motion.begins_between(before_door, s.flow.door_elapsed) {
                effects.insert(0, Effect::Sound(p.assets.flow.opening));
            }
        }
        while s.guide + 1 < p.assets.route.len()
            && crate::distance(self.state.pawn.body.position, p.assets.route[s.guide]) < 55.
        {
            s.guide += 1;
        }
        let sound_base = p.sound_base;
        let tick = self.state.statistics.ticks;
        for e in effects {
            match &e {
                Effect::Sound(sound) => {
                    let m = self.state.mission.as_mut().unwrap();
                    m.audio_sequence += 1;
                    if m.events.len() >= 256 {
                        m.events.remove(0);
                    }
                    m.events.push(crate::combat::Event {
                        sequence: m.audio_sequence,
                        tick,
                        sound: sound_base + sound,
                        owner: -1,
                    });
                }
                Effect::Release(id) => self.disable_dynamic(*id)?,
                Effect::Exit(url) => {
                    self.state.spire.as_mut().unwrap().completed_tick = Some(tick);
                    self.state.phase = crate::Phase::Won;
                    self.capture_map_handoff(url)?;
                }
                _ => {}
            }
            self.state.spire.as_mut().unwrap().effects.push((tick, e));
        }
        self.sync_spire_doors();
        Ok(())
    }
    pub(crate) fn spire_door_offset(&self, original: u32) -> Option<[f32; 3]> {
        let p = self.spire.as_ref()?;
        let s = self.state.spire.as_ref()?;
        let motion = p
            .assets
            .door_motion
            .iter()
            .find(|m| m.original == original)?;
        Some(motion.displacement(if s.flow.door_open {
            motion.delay + motion.seconds
        } else if s.flow.door_started {
            s.flow.door_elapsed
        } else {
            0.
        }))
    }
    pub(crate) fn sync_spire_doors(&mut self) {
        let Some(dynamic) = &self.dynamic else {
            return;
        };
        for object in &dynamic.objects {
            let Some(delta) = self.spire_door_offset(object.original) else {
                continue;
            };
            let delta = delta.map(f64::from);
            let hulls = &object.hulls;
            let moved = rc_package::hull::HullSet {
                hulls: hulls
                    .hulls
                    .iter()
                    .map(|h| rc_package::hull::Hull {
                        offset: h.offset,
                        bounds: h.bounds.map(|b| std::array::from_fn(|i| b[i] + delta[i])),
                        planes: h
                            .planes
                            .iter()
                            .map(|p| {
                                [
                                    p[0],
                                    p[1],
                                    p[2],
                                    p[3] + p[0] * delta[0] + p[1] * delta[1] + p[2] * delta[2],
                                ]
                            })
                            .collect(),
                    })
                    .collect(),
                missing_solid_leaves: hulls.missing_solid_leaves,
                empty_root_solid: hulls.empty_root_solid,
            };
            self.world.actors[object.actor].shape =
                rc_package::world_collision::BodyShape::Hulls(std::sync::Arc::new(moved));
        }
    }
    pub fn spire_target(&self) -> Option<Point> {
        let p = self.spire.as_ref()?;
        let s = self.state.spire.as_ref()?;
        s.flow.enabled.then(|| p.assets.route[s.guide])
    }
    pub fn spire_status(&self) -> Option<String> {
        let s = self.state.spire.as_ref()?;
        if !s.flow.enabled {
            return None;
        }
        Some(
            if s.flow.completed {
                "GEO_01A abgeschlossen · Kartenübergang GEO_01B erreicht · Neustart möglich"
            } else if self
                .state
                .artillery
                .as_ref()
                .is_some_and(|a| a.attack_tick.is_some() && a.impacts.len() < 4)
            {
                "Bombardierung · In Deckung zurückziehen, dann dem Weg folgen"
            } else if s.flow.door_open {
                "Turmtür geöffnet · Ausgang erreichen"
            } else if !s.flow.barrier_open && s.flow.entered {
                "Schwere Artillerie · Funkmeldung und Barrikadenfreigabe abwarten"
            } else {
                "In den Geonosianerturm eindringen · Weganzeige folgen"
            }
            .into(),
        )
    }
}

#[cfg(test)]
mod motion_tests {
    use super::*;
    #[test]
    fn original_delay_and_glide_clamp_before_during_and_after_motion() {
        let mut door = DoorMotion {
            original: 34,
            delay: 0.5,
            seconds: 2.,
            glide: true,
            offset: [160., -80., 0.],
        };
        assert_eq!(door.displacement(0.49), [0.; 3]);
        assert!(!door.begins_between(0.3, 0.49));
        assert!(door.begins_between(0.49, 0.5));
        assert!(!door.begins_between(0.5, 0.51));
        assert_eq!(door.displacement(1.), [25., -12.5, 0.]);
        assert_eq!(door.displacement(1.5), [80., -40., 0.]);
        assert_eq!(door.displacement(9.), door.offset);
        door.glide = false;
        assert_eq!(door.displacement(1.), [40., -20., 0.]);
    }
}
