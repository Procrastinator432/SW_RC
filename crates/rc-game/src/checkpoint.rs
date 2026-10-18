//! Full, scene/build-bound runtime checkpoints. No input replay on restoration.
use crate::{Game, State, STEP};
use serde::{Deserialize, Serialize};

pub const LIMIT: usize = 8 * 1024 * 1024;
const ABI: &str = "rcport-state-0.46.0-1";
impl Game {
    /// Conservative eligibility for a stable fallback, never a guarantee of
    /// future survival. The host additionally requires five quiet seconds.
    /// Resume snapshots and explicitly requested manual saves bypass this gate.
    pub fn checkpoint_safe(&self) -> bool {
        let s = &self.state;
        s.phase == crate::Phase::Playing
            && s.health >= 50
            && s.pawn.body.grounded
            && s.damage_flash == 0.
            && s.reload_remaining == 0.
            && s.fire_remaining == 0.
            && !s
                .enemies
                .iter()
                .any(|e| e.health > 0 && crate::distance(e.position, s.pawn.body.position) < 2500.)
            && s.combat.as_ref().is_none_or(|c| c.projectiles.is_empty())
            && s.thermal.as_ref().is_none_or(|t| t.projectiles.is_empty())
            && s.bunker
                .as_ref()
                .is_none_or(|b| b.health == 0 || b.contact_tick.is_none())
            && s.artillery.as_ref().is_none_or(|a| {
                a.missiles.is_empty()
                    && !a.warriors.iter().any(|w| w.health > 0)
                    && (a.attack_tick.is_none() || a.destroyed_tick.is_some())
            })
            && s.demolition.as_ref().is_none_or(|d| !d.firing)
            && s.opening.as_ref().is_none_or(|o| !o.locked)
            && s.map
                .as_ref()
                .and_then(|m| m.opening.as_ref())
                .is_none_or(|o| !o.locked)
    }
}
#[derive(Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
struct Snapshot {
    abi: String,
    scene: String,
    state: State,
    initial: State,
    accumulator: f64,
    pending: [bool; 4],
}
fn checksum(bytes: &[u8]) -> u64 {
    bytes.iter().fold(0xcbf29ce484222325u64, |h, b| {
        (h ^ u64::from(*b)).wrapping_mul(0x100000001b3)
    })
}
fn bounded(v: &serde_json::Value, depth: usize, budget: &mut usize) -> Result<(), String> {
    if depth > 80 || *budget == 0 {
        return Err("Checkpoint structure is too large".into());
    }
    *budget -= 1;
    match v {
        serde_json::Value::Array(a) => {
            for v in a {
                bounded(v, depth + 1, budget)?;
            }
        }
        serde_json::Value::Object(a) => {
            for v in a.values() {
                bounded(v, depth + 1, budget)?;
            }
        }
        serde_json::Value::String(s) if s.len() > 16384 => {
            return Err("Checkpoint string too large".into())
        }
        serde_json::Value::Number(n)
            if n.as_f64().is_none_or(|n| !n.is_finite() || n.abs() > 1e12) =>
        {
            return Err("Invalid checkpoint number".into())
        }
        _ => {}
    }
    Ok(())
}
fn topology(base: &serde_json::Value, saved: &serde_json::Value) -> Result<(), String> {
    if let (Some(base), Some(saved)) = (base.as_object(), saved.as_object()) {
        for (key, v) in base {
            let other = saved.get(key).unwrap_or(&serde_json::Value::Null);
            if [
                "visuals",
                "combat",
                "mission",
                "dynamic",
                "thermal",
                "bunker",
                "rendezvous",
                "demolition",
                "spire",
                "artillery",
                "squad",
                "opening",
                "bacta",
                "world_effects",
                "rigid",
                "map",
                "planning",
                "escape",
                "guard",
                "breach",
                "balcony",
                "finale",
                "elevator",
            ]
            .contains(&key.as_str())
                && !v.is_null()
                && other.is_null()
            {
                return Err(format!("Checkpoint missing runtime module {key}"));
            }
            if [
                "dynamic",
                "lanes",
                "rigid",
                "animations",
                "objectives",
                "scripts",
                "triggered",
                "factories",
                "factory_timers",
                "progress",
                "armed",
                "used",
                "pickups",
                "gate_timers",
                "healed",
            ]
            .contains(&key.as_str())
            {
                if let Some(a) = v.as_array() {
                    if other.as_array().is_none_or(|b| a.len() != b.len()) {
                        return Err(format!("Checkpoint {key} topology mismatch"));
                    }
                }
            }
            topology(v, other)?;
        }
    } else if let (Some(a), Some(b)) = (base.as_array(), saved.as_array()) {
        for (v, other) in a.iter().zip(b) {
            topology(v, other)?;
        }
    }
    Ok(())
}
impl Game {
    pub fn checkpoint(&self, scene: &str) -> Result<Vec<u8>, String> {
        let body = serde_json::to_vec(&Snapshot {
            abi: ABI.into(),
            scene: scene.into(),
            state: self.state.clone(),
            initial: self.initial.clone(),
            accumulator: self.accumulator,
            pending: [
                self.pending_jump,
                self.pending_fire,
                self.pending_reload,
                self.pending_throw,
            ],
        })
        .map_err(|e| e.to_string())?;
        if body.len() > LIMIT - 20 {
            return Err("Checkpoint too large".into());
        }
        let mut out = b"RCSAVE01".to_vec();
        out.extend((body.len() as u32).to_le_bytes());
        out.extend(checksum(&body).to_le_bytes());
        out.extend(body);
        Ok(out)
    }
    /// Validation and collision/cache installation commit together; entry restart state is carried too.
    pub fn restore_checkpoint(&mut self, bytes: &[u8], scene: &str) -> Result<(), String> {
        if bytes.len() < 20 || bytes.len() > LIMIT || &bytes[..8] != b"RCSAVE01" {
            return Err("Invalid checkpoint header".into());
        }
        let n = u32::from_le_bytes(bytes[8..12].try_into().unwrap()) as usize;
        if n != bytes.len() - 20
            || checksum(&bytes[20..]) != u64::from_le_bytes(bytes[12..20].try_into().unwrap())
        {
            return Err("Checkpoint damaged or truncated".into());
        }
        let value: serde_json::Value =
            serde_json::from_slice(&bytes[20..]).map_err(|e| e.to_string())?;
        bounded(&value, 0, &mut 250000)?;
        let s: Snapshot = serde_json::from_value(value).map_err(|e| e.to_string())?;
        if s.abi != ABI || s.scene != scene {
            return Err("Checkpoint belongs to another scene or game version".into());
        }
        if !s.accumulator.is_finite()
            || !(-1e-9..STEP).contains(&s.accumulator)
            || s.initial.statistics.ticks != 0
        {
            return Err("Invalid checkpoint clock".into());
        }
        let base = serde_json::to_value(&self.initial).map_err(|e| e.to_string())?;
        for state in [&s.state, &s.initial] {
            if let Some(prepared) = &self.opening {
                let bodies = state.opening.as_ref().and_then(|o| o.rigid.as_ref());
                if bodies.is_some() == prepared.assets.rigid.is_empty() {
                    return Err("Rigid checkpoint module mismatch".into());
                }
                if let Some(bodies) = bodies {
                    if bodies.len() != prepared.assets.rigid.len()
                        || bodies
                            .iter()
                            .zip(&prepared.assets.rigid)
                            .any(|(s, b)| !s.valid(b))
                    {
                        return Err("Invalid rigid checkpoint state".into());
                    }
                    for body in bodies {
                        let i = self
                            .dynamic
                            .as_ref()
                            .and_then(|p| {
                                p.objects.iter().position(|o| o.original == body.original)
                            })
                            .ok_or("Missing rigid checkpoint actor")?;
                        if body.active != state.dynamic.as_ref().is_some_and(|d| d[i]) {
                            return Err("Rigid release disagrees with dynamic state".into());
                        }
                    }
                }
                let fx = state
                    .opening
                    .as_ref()
                    .and_then(|o| o.world_effects.as_ref());
                if fx.is_some() == prepared.assets.world_effects.is_empty() {
                    return Err("World effects checkpoint module mismatch".into());
                }
                if let Some(fx) = fx {
                    fx.validate(&prepared.assets.world_effects)?;
                }
            }
            if let Some(prepared) = self.opening.as_ref().and_then(|o| o.bacta.as_ref()) {
                let particles = state
                    .opening
                    .as_ref()
                    .and_then(|o| o.bacta.as_ref())
                    .and_then(|b| b.particles.as_ref());
                if particles.is_some() == prepared.assets.effects.is_empty() {
                    return Err("Checkpoint particle module does not match scene".into());
                }
                if let Some(particles) = particles {
                    particles.validate(&prepared.assets.effects)?;
                }
            }
            if state
                .opening
                .as_ref()
                .and_then(|o| o.bacta.as_ref())
                .is_some()
                != self
                    .initial
                    .opening
                    .as_ref()
                    .and_then(|o| o.bacta.as_ref())
                    .is_some()
            {
                return Err("Checkpoint Bacta module does not match scene".into());
            }
            if state.health > 1000
                || state.ammo > crate::MAGAZINE
                || state.reserve > 100000
                || state.enemies.len() > 4096
                || state.enemy_traces.len() > 64
                || state.opening.as_ref().is_some_and(|o| {
                    o.owned_voices.len() > 64
                        || o.owned_voices.iter().any(|v| {
                            v.owner < 1
                                || !v.until.is_finite()
                                || v.until < 0.
                                || v.until > o.clock + 120.
                        })
                })
                || state.enemy_traces.iter().any(|t| {
                    t.owner >= state.enemies.len()
                        || t.tick > state.statistics.ticks
                        || t.from.iter().chain(t.to.iter()).any(|v| v.abs() > 1e8)
                })
                || state
                    .opening
                    .as_ref()
                    .and_then(|o| o.bacta.as_ref())
                    .is_some_and(|b| {
                        b.notify_events.len() > 64
                            || b.notify_events
                                .iter()
                                .any(|(t, _)| *t > state.statistics.ticks)
                            || b.activated_tick.is_some_and(|t| t > state.statistics.ticks)
                            || b.playback.as_ref().is_some_and(|p| {
                                p.channels.len() != 1
                                    || p.channels[0].words[0] != 1
                                    || p.channels[0].words[3] != 0
                                    || [4, 5, 6, 7, 8, 9, 10, 11, 14].iter().any(|i| {
                                        !f32::from_bits(p.channels[0].words[*i]).is_finite()
                                    })
                                    || !f32::from_bits(p.channels[0].words[7]).is_finite()
                                    || !(0. ..=1.).contains(&f32::from_bits(p.channels[0].words[7]))
                            })
                            || matches!(
                                b.phase,
                                crate::bacta::Phase::Loading
                                    | crate::bacta::Phase::Active
                                    | crate::bacta::Phase::Holstering
                            ) && b.playback.is_none()
                    })
                || state.statistics.ticks > 20000000
                || state.pawn.body.position.iter().any(|v| v.abs() > 1e8)
            {
                return Err("Invalid checkpoint state limits".into());
            }
            let v = serde_json::to_value(state).map_err(|e| e.to_string())?;
            topology(&base, &v)?;
            for key in [
                "visuals",
                "combat",
                "mission",
                "dynamic",
                "thermal",
                "bunker",
                "rendezvous",
                "demolition",
                "spire",
                "artillery",
                "squad",
                "opening",
                "map",
            ] {
                if base.get(key).is_some() != v.get(key).is_some() {
                    return Err("Checkpoint runtime modules do not match scene".into());
                }
            }
        }
        let old = self.state.clone();
        self.state = s.state;
        let checked =
            std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| -> Result<(), String> {
                self.sync_checkpoint_world();
                self.render_frame(16, 16)?;
                Ok(())
            }))
            .unwrap_or_else(|_| Err("Invalid checkpoint runtime references".into()));
        if let Err(e) = checked {
            self.state = old;
            self.sync_checkpoint_world();
            return Err(e);
        }
        self.initial = s.initial;
        self.accumulator = s.accumulator;
        [
            self.pending_jump,
            self.pending_fire,
            self.pending_reload,
            self.pending_throw,
        ] = s.pending;
        Ok(())
    }
    fn sync_checkpoint_world(&mut self) {
        self.sync_dynamic();
        self.sync_map_gates();
        self.sync_map3_glass();
        self.sync_map3_escape();
        self.sync_map3_breach();
        self.sync_map3_balcony();
        self.sync_map3_elevator();
    }
    /// Stable major progression token, independent of aim, resource counters and distance labels.
    pub fn checkpoint_marker(&self) -> u64 {
        let s = &self.state;
        checksum(&serde_json::to_vec(&serde_json::json!({"phase":s.phase,"mission":s.mission.as_ref().map(|m|(m.completed,m.objective_mode)),"map2":s.map.as_ref().and_then(|m|m.gameplay.as_ref()).map(|g|g.objectives),"map3":s.map.as_ref().and_then(|m|m.opening.as_ref()).map(|o|o.objectives),"rendezvous":s.rendezvous.as_ref().map(|r|r.flow.objective_completed),"demolition":s.demolition.as_ref().map(|d|d.complete_tick),"gun":s.thermal.as_ref().map(|t|t.gun.objective_completed)})).unwrap_or_default())
    }
}
