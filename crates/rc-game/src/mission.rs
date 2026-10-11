//! Selected original geo_01a Bacta objective. Route guidance and section boundary
//! are port adapters; the cinematic, general script VM and later combat are not executed.
use crate::{distance, Game, Phase, Point, STEP};
use serde::{Deserialize, Serialize};

#[derive(Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Assets {
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub weapon_fire_sound: Option<usize>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub grenades: Option<crate::grenade_runtime::Assets>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub crash: Option<crate::crash_runtime::Assets>,
    pub provenance: serde_json::Value,
    pub objective: String,
    pub route: Vec<Point>,
    pub station: Point,
    pub use_radius: f64,
    pub max_health: f32,
    pub start_health: f32,
    pub bacta_rate: f32,
    pub sounds: Vec<crate::combat::Sound>,
}
#[derive(Clone, Debug, Serialize)]
pub struct State {
    #[serde(skip_serializing_if = "Option::is_none")]
    pub grenades: Option<crate::grenade_runtime::State>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub crash: Option<crate::crash_runtime::State>,
    pub waypoint: usize,
    pub health: f32,
    pub using: bool,
    pub completed: bool,
    pub objective_mode: u8,
    pub events: Vec<crate::combat::Event>,
    pub audio_sequence: u64,
}
impl Game {
    pub fn attach_mission(&mut self, mut assets: Assets) -> Result<(), String> {
        if self.mission.is_some()
            || self.combat.is_some()
            || assets.route.is_empty()
            || assets.route.len() > 256
            || assets.objective.is_empty()
            || assets.objective.len() > 256
            || assets
                .route
                .iter()
                .chain([&assets.station])
                .flatten()
                .any(|v| !v.is_finite() || v.abs() > 1e8)
            || !assets.use_radius.is_finite()
            || !(1. ..=250.).contains(&assets.use_radius)
            || !assets.max_health.is_finite()
            || !(1. ..=10000.).contains(&assets.max_health)
            || !assets.start_health.is_finite()
            || !(0. ..assets.max_health).contains(&assets.start_health)
            || assets.start_health == 0.
            || !assets.bacta_rate.is_finite()
            || !(0.1..=120.).contains(&assets.bacta_rate)
        {
            return Err("Invalid mission assets".into());
        }
        let advisor_count = if assets.crash.is_some() { 4 } else { 3 };
        if assets.sounds.len()
            != advisor_count
                + usize::from(assets.weapon_fire_sound.is_some())
                + if assets.grenades.is_some() { 8 } else { 0 }
            || assets.weapon_fire_sound.is_some_and(|i| i != advisor_count)
        {
            return Err("Invalid mission advisor/weapon sound bank".into());
        }
        for sound in &assets.sounds {
            crate::combat::validate_wave(&sound.wave)?;
        }
        for p in &assets.route {
            let s = self.world.body_placement(*p, self.profile.physics.extent)?;
            if s.state == rc_package::movement::PlacementState::Penetrating {
                return Err("Unsupported mission route placement".into());
            }
        }
        let grenades = if let Some(a) = &assets.grenades {
            a.validate()?;
            if assets.crash.is_none() || assets.weapon_fire_sound != Some(4) {
                return Err("Grenade section requires original crash and weapon bank".into());
            }
            for (i, source) in a.sounds.iter().enumerate() {
                if assets.sounds[5 + i].source != *source {
                    return Err("Incorrect grenade sound bank mapping".into());
                }
            }
            for p in &a.route {
                if self
                    .world
                    .body_placement(*p, self.profile.physics.extent)?
                    .state
                    == rc_package::movement::PlacementState::Penetrating
                {
                    return Err("Grenade route penetrates world".into());
                }
            }
            Some(crate::grenade_runtime::State::new(a)?)
        } else {
            None
        };
        let crash = if let Some(a) = &mut assets.crash {
            a.validate()?;
            for (p, extent) in a
                .route
                .iter()
                .map(|p| (p, self.profile.physics.extent))
                .chain(a.spawns.iter().map(|p| (p, [22., 22., 60.])))
            {
                if self.world.body_placement(*p, extent)?.state
                    == rc_package::movement::PlacementState::Penetrating
                {
                    return Err("Crash-site route or spawn penetrates world".into());
                }
            }
            let state = crate::crash_runtime::State {
                flow: crate::crash::State::new(&a.flow)?,
                waypoint: 0,
                tracker: String::new(),
            };
            let mut objects = std::mem::take(&mut a.objects);
            if let Some(g) = &mut assets.grenades {
                objects.extend(std::mem::take(&mut g.objects));
            }
            self.attach_dynamic(objects)?;
            Some(state)
        } else {
            None
        };
        if let Some(g) = &assets.grenades {
            self.prepare_pickup(&g.pickup_snapshot)?;
        }
        // This original section precedes the first free combat objective. Remove
        // the synthetic three-enemy encounter; do not substitute it for scripts.
        self.state.enemies.clear();
        if let Some(v) = &self.visuals {
            self.state.visuals = Some(v.state(0)?);
        }
        if self.combat.is_some() {
            return Err("Bacta section requires a visual-only base".into());
        }
        self.state.health = assets.start_health.ceil() as u32;
        self.state.mission = Some(State {
            grenades,
            crash,
            waypoint: 0,
            health: assets.start_health,
            using: false,
            completed: false,
            objective_mode: 1,
            events: vec![],
            audio_sequence: 0,
        });
        self.exit = *assets.route.last().unwrap();
        self.initial = self.state.clone();
        self.mission = Some(assets);
        Ok(())
    }
    pub fn mission_target(&self) -> Option<Point> {
        if let Some(p) = self.grenade_target() {
            return Some(p);
        }
        if let Some(p) = self.crash_target() {
            return Some(p);
        }
        let a = self.mission.as_ref()?;
        let s = self.state.mission.as_ref()?;
        a.route
            .get(s.waypoint)
            .copied()
            .or_else(|| a.route.last().copied())
    }
    pub fn mission_status(&self) -> String {
        if let Some(s) = self.grenade_status() {
            return s;
        }
        if let Some(s) = self.crash_status() {
            return s;
        }
        let Some(a) = &self.mission else {
            return String::new();
        };
        let s = self.state.mission.as_ref().unwrap();
        if s.completed {
            return "Originalziel abgeschlossen: Bacta benutzt · Abschnitt geschafft".into();
        }
        let target = self.mission_target().unwrap();
        let p = self.state.pawn.body.position;
        let turn = ((target[1] - p[1]).atan2(target[0] - p[0]) - self.state.yaw
            + std::f64::consts::PI)
            .rem_euclid(std::f64::consts::TAU)
            - std::f64::consts::PI;
        if distance(p, *a.route.last().unwrap()) <= a.use_radius {
            return format!(
                "Bacta: BENUTZEN halten · Gesundheit {:.0}/{:.0}",
                s.health, a.max_health
            );
        }
        format!(
            "Bacta erreichen · Wegpunkt {}/{} · {:.0} m · {} {:.0}°",
            s.waypoint + 1,
            a.route.len(),
            distance(p, target) / 100.,
            if turn >= 0. { "links" } else { "rechts" },
            turn.abs().to_degrees()
        )
    }
    pub(crate) fn mission_tick(&mut self, use_action: bool) {
        let Some(a) = &self.mission else {
            return;
        };
        let p = self.state.pawn.body.position;
        let s = self.state.mission.as_mut().unwrap();
        if s.completed {
            return;
        }
        let tick = self.state.statistics.ticks;
        if tick == 1 || tick == 240 {
            s.audio_sequence += 1;
            s.events.push(crate::combat::Event {
                sequence: s.audio_sequence,
                tick,
                sound: usize::from(tick == 240),
                owner: -1,
            });
        }
        while s.waypoint + 1 < a.route.len() && distance(p, a.route[s.waypoint]) < 100. {
            s.waypoint += 1;
        }
        let final_point = *a.route.last().unwrap();
        let toward = [(a.station[0] - p[0]), (a.station[1] - p[1])];
        let len = toward[0].hypot(toward[1]);
        let facing = len < 1.
            || (toward[0] * self.state.yaw.cos() + toward[1] * self.state.yaw.sin()) / len > 0.5;
        s.using = use_action
            && facing
            && distance(p, final_point) <= a.use_radius
            && self.state.pawn.body.grounded
            && self.state.health > 0;
        if s.using {
            // BactaDispenser.BeginActivating: deltaTime * MaxHealth / BactaRate,
            // clamped to missing health. Release/range/facing interrupt the hold.
            let delta = (STEP as f32 * a.max_health / a.bacta_rate).min(a.max_health - s.health);
            s.health += delta;
            self.state.health = s.health.ceil() as u32;
            if s.health >= a.max_health {
                s.health = a.max_health;
                s.completed = true;
                s.using = false;
                s.objective_mode = 2;
                if a.crash.is_none() {
                    s.audio_sequence += 1;
                    s.events.push(crate::combat::Event {
                        sequence: s.audio_sequence,
                        tick,
                        sound: 2,
                        owner: -1,
                    });
                    self.state.phase = Phase::Won;
                }
            }
        }
    }
}
