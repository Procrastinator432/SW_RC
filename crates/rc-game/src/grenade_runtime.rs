//! Original pickup and selected connecting scripts, with a bounded wall-pose adapter.
use crate::{crash, dynamic, grenade_pickup, Game, Phase, Point, STEP};
use serde::{Deserialize, Serialize};
#[derive(Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Assets {
    pub flow: grenade_pickup::Assets,
    pub route: Vec<Point>,
    pub barrier_waypoint: usize,
    pub spawn_trigger: crash::Trigger,
    pub dialog_trigger: crash::Trigger,
    pub wall_delay: f64,
    pub barrier_waits: Vec<f64>,
    pub sounds: Vec<String>,
    pub objects: Vec<dynamic::Object>,
    pub pickup_snapshot: Vec<u8>,
}
#[derive(Clone, Debug, Serialize)]
pub struct State {
    pub flow: grenade_pickup::State,
    pub waypoint: usize,
    pub secured_seconds: f64,
    pub walls_released: bool,
    pub barrier_armed: bool,
    pub barrier_started: bool,
    pub barrier_open: bool,
    pub barrier_step: usize,
    pub barrier_remaining: f64,
    pub blink_remaining: f64,
}
pub struct Prepared {
    pub triangles: Vec<rc_package::geometry::Triangle>,
    pub appearance: Vec<Option<rc_render::FaceTexture>>,
}
impl Assets {
    pub fn validate(&self) -> Result<(), String> {
        self.flow.validate()?;
        let ids: std::collections::BTreeSet<_> = self.objects.iter().map(|o| o.original).collect();
        if self.route.is_empty()
            || self.route.len() > 256
            || self.barrier_waypoint >= self.route.len()
            || self
                .route
                .iter()
                .flatten()
                .any(|v| !v.is_finite() || v.abs() > 1e8)
            || !self.spawn_trigger.validate()
            || !self.dialog_trigger.validate()
            || !self.wall_delay.is_finite()
            || !(0. ..=10.).contains(&self.wall_delay)
            || self.barrier_waits.len() != 6
            || self
                .barrier_waits
                .iter()
                .any(|v| !v.is_finite() || !(0. ..=120.).contains(v))
            || self.sounds.len() != 8
            || self.sounds.iter().any(|v| v.is_empty() || v.len() > 256)
            || self.objects.len() != 5
            || ids != [909, 910, 912, 913, 964].into_iter().collect()
        {
            return Err("Invalid grenade scene assets".into());
        }
        if self
            .sounds
            .iter()
            .collect::<std::collections::BTreeSet<_>>()
            .len()
            != 8
            || self
                .flow
                .cues
                .iter()
                .enumerate()
                .any(|(i, c)| self.sounds[i] != c.source)
        {
            return Err("Invalid grenade sound roles".into());
        }
        let scene = rc_render::Scene::from_snapshot(&self.pickup_snapshot)?;
        if scene.triangles.len() != 402 || scene.appearance.iter().any(Option::is_none) {
            return Err("Incomplete pickup appearance".into());
        }
        Ok(())
    }
}
impl State {
    pub fn new(a: &Assets) -> Result<Self, String> {
        Ok(Self {
            flow: grenade_pickup::State::new(&a.flow)?,
            waypoint: 0,
            secured_seconds: 0.,
            walls_released: false,
            barrier_armed: false,
            barrier_started: false,
            barrier_open: false,
            barrier_step: 0,
            barrier_remaining: 0.,
            blink_remaining: 0.,
        })
    }
}
impl Game {
    pub(crate) fn prepare_pickup(&mut self, snapshot: &[u8]) -> Result<(), String> {
        let scene = rc_render::Scene::from_snapshot(snapshot)?;
        let base = self.scene.textures.len();
        let appearance = scene
            .appearance
            .into_iter()
            .map(|f| {
                f.map(|mut f| {
                    f.texture += base;
                    f
                })
            })
            .collect();
        self.scene.textures.extend(scene.textures);
        self.pickup = Some(Prepared {
            triangles: scene.triangles,
            appearance,
        });
        Ok(())
    }
    pub fn grenade_target(&self) -> Option<Point> {
        let m = self.state.mission.as_ref()?;
        if m.crash.as_ref()?.flow.combat != crash::CombatStage::Secured {
            return None;
        }
        let a = self.mission.as_ref()?.grenades.as_ref()?;
        let s = m.grenades.as_ref()?;
        a.route
            .get(s.waypoint)
            .copied()
            .or_else(|| a.route.last().copied())
    }
    pub fn grenade_status(&self) -> Option<String> {
        let target = self.grenade_target()?;
        let s = self.state.mission.as_ref()?.grenades.as_ref()?;
        if self.state.phase == Phase::Won {
            return Some("Originalziele abgeschlossen: Bacta, Absturzstelle und Granatenaufnahme · 5 Thermaldetonatoren".into());
        }
        if s.barrier_started && !s.barrier_open {
            return Some("Barrikade · Durchgang wird freigesprengt · kurz warten".into());
        }
        let p = self.state.pawn.body.position;
        let turn = ((target[1] - p[1]).atan2(target[0] - p[0]) - self.state.yaw
            + std::f64::consts::PI)
            .rem_euclid(std::f64::consts::TAU)
            - std::f64::consts::PI;
        Some(format!(
            "Granaten aufnehmen · Vorrat {}/5 · {:.0} m · {} {:.0}°",
            s.flow.ammunition,
            crate::distance(p, target) / 100.,
            if turn >= 0. { "links" } else { "rechts" },
            turn.abs().to_degrees()
        ))
    }
    pub(crate) fn grenade_tick(&mut self) -> Result<(), String> {
        let Some(a) = self.mission.as_ref().and_then(|m| m.grenades.as_ref()) else {
            return Ok(());
        };
        if self
            .state
            .mission
            .as_ref()
            .unwrap()
            .crash
            .as_ref()
            .unwrap()
            .flow
            .combat
            != crash::CombatStage::Secured
        {
            return Ok(());
        }
        let p = self.state.pawn.body.position;
        let extent = self.profile.physics.extent;
        let s = self
            .state
            .mission
            .as_mut()
            .unwrap()
            .grenades
            .as_mut()
            .unwrap();
        let mut release = vec![];
        let mut sounds = vec![];
        s.secured_seconds += STEP;
        s.blink_remaining = (s.blink_remaining - STEP).max(0.);
        if !s.walls_released && s.secured_seconds + 1e-9 >= a.wall_delay {
            s.walls_released = true;
            release.extend([909, 910, 912, 913]);
        }
        if a.spawn_trigger.contains(p, extent) {
            s.barrier_armed = true;
        }
        if s.barrier_armed && !s.barrier_started && a.dialog_trigger.contains(p, extent) {
            s.barrier_started = true;
            s.barrier_remaining = a.barrier_waits[0];
        }
        let mut delta = STEP;
        while s.barrier_started && !s.barrier_open {
            if delta + 1e-9 < s.barrier_remaining {
                s.barrier_remaining -= delta;
                break;
            }
            delta = (delta - s.barrier_remaining).max(0.);
            match s.barrier_step {
                0 => sounds.push(a.sounds[4].clone()),
                3 => sounds.push(a.sounds[5].clone()),
                5 => {
                    s.barrier_open = true;
                    release.push(964);
                    sounds.extend(a.sounds[6..8].iter().cloned());
                }
                _ => {}
            }
            s.barrier_step += 1;
            s.barrier_remaining = a.barrier_waits.get(s.barrier_step).copied().unwrap_or(0.);
        }
        while s.waypoint + 1 < a.route.len() && crate::distance(p, a.route[s.waypoint]) < 65. {
            if s.waypoint == a.barrier_waypoint && !s.barrier_open {
                break;
            }
            s.waypoint += 1;
        }
        let effects = s
            .flow
            .advance(&a.flow, STEP, p, extent, self.state.health > 0, true)?;
        for effect in effects {
            match effect {
                grenade_pickup::Effect::PlaySound(source) => sounds.push(source),
                grenade_pickup::Effect::BlinkHud(seconds) => s.blink_remaining = seconds,
                _ => {}
            }
        }
        // A playable section finishes after actual inventory grant and both blocking
        // advisor scripts. The separate original objective contact alone cannot win.
        let won = s.flow.objective_completed
            && s.flow.ammunition > 0
            && s.flow.intro == grenade_pickup::IntroStage::Done
            && s.flow.pickup_script == grenade_pickup::PickupStage::Done;
        for id in release {
            self.disable_dynamic(id)?;
        }
        for source in sounds {
            let sound = self
                .mission
                .as_ref()
                .unwrap()
                .sounds
                .iter()
                .position(|s| s.source == source)
                .ok_or("Missing connecting/pickup sound")?;
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
        }
        if won {
            self.state.phase = Phase::Won;
        }
        Ok(())
    }
}
