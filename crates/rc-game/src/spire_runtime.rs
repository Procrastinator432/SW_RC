use crate::{
    spire::{self, Effect},
    Game, Point, STEP,
};
use serde::{Deserialize, Serialize};
#[derive(Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Assets {
    pub flow: spire::Flow,
    pub route: Vec<Point>,
    pub sounds: Vec<crate::combat::Sound>,
    pub objects: Vec<crate::dynamic::Object>,
    pub provenance: serde_json::Value,
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
        let effects = s.flow.advance(
            &p.assets.flow,
            STEP,
            self.state.pawn.body.position,
            self.profile.physics.extent,
            demo.explosion_tick.is_some(),
            demo.complete_tick.is_some(),
        )?;
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
        Ok(())
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
