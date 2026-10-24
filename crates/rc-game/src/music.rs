//! Selected original music-script lanes; battle proximity is an explicit port policy.
use crate::{Game, State};
use rc_package::mission_music::Command;
use serde::{Deserialize, Serialize};
use std::collections::BTreeMap;
#[derive(Clone, Debug, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Track {
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub playback: Option<crate::music_runtime::Playback>,
    pub source: String,
    pub frames: u64,
    pub rate: u32,
}
#[derive(Clone, Debug, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Cue {
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub group: Option<usize>,
    #[serde(default, skip_serializing_if = "std::ops::Not::not")]
    pub use_defaults: bool,
    pub looped: bool,
    pub track: i32,
    pub fade_in: f32,
    pub fade_out: f32,
    pub in_kind: u8,
    pub out_kind: u8,
}
impl Cue {
    pub(crate) fn command(&self, tick: u64, now: u64, id: u64) -> Command {
        Command {
            looped: self.looped,
            track: self.track,
            activation: id,
            elapsed: (now - tick) * 800,
            fade_in: self.fade_in,
            fade_out: self.fade_out,
            in_kind: self.in_kind,
            out_kind: self.out_kind,
        }
    }
}
#[derive(Clone, Serialize, Deserialize)]
#[serde(tag = "op", deny_unknown_fields)]
pub enum Op {
    Wait {
        ticks: u64,
    },
    Event {
        tag: String,
    },
    State {
        cue: Cue,
    },
    Episode {
        cue: Cue,
    },
    Battle {
        enabled: bool,
    },
    Radius {
        inner: f64,
        outer: f64,
    },
    Params {
        criteria: u8,
        count: i32,
        inner: f32,
        outer: f32,
    },
    Multi {
        cue: Cue,
        victory: Cue,
    },
    Victory,
}
#[derive(Clone, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Score {
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub zones: Vec<crate::music_runtime::Zone>,
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub groups: Vec<crate::music_runtime::Definition>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub battle_rules: Option<crate::music_runtime::BattleRules>,
    #[serde(default)]
    pub seed: u32,
    pub tracks: Vec<Track>,
    pub lanes: Vec<Vec<Op>>,
    pub battle: Cue,
    pub victory: Cue,
    pub inner: f64,
    pub outer: f64,
    pub threshold: usize,
    pub provenance: serde_json::Value,
}
pub struct Assets {
    pub score: Score,
    pub bank: Vec<u8>,
}
#[derive(Clone, Debug, Default, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct MusicState {
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub runtime: Option<crate::music_runtime::Runtime>,
    pub events: BTreeMap<String, u64>,
    pub battle: bool,
    pub battle_tick: u64,
    pub victory_tick: Option<u64>,
}
struct Timeline {
    state: Option<(u64, u64, Cue)>,
    episode: Option<(u64, u64, Cue)>,
    enabled: bool,
    battle: Cue,
    victory: Cue,
    inner: f64,
    outer: f64,
}
impl Score {
    pub fn validate(&self) -> Result<(), String> {
        self.validate_groups()?;
        if self.tracks.len() > 24
            || self.lanes.len() > 16
            || self.threshold > 1024
            || !self.inner.is_finite()
            || !self.outer.is_finite()
            || self.inner < 0.
            || self.outer < self.inner
            || self.outer > 1e7
        {
            return Err("Invalid mission score".into());
        }
        for t in &self.tracks {
            if let Some(p) = &t.playback {
                Command {
                    fade_in: p.fade_in,
                    fade_out: p.fade_out,
                    in_kind: p.in_kind,
                    out_kind: p.out_kind,
                    ..Default::default()
                }
                .validate(self.tracks.len())?;
            }
            if t.source.len() > 256 || t.frames == 0 || t.rate == 0 || t.rate > 96000 {
                return Err("Invalid score track".into());
            }
        }
        let cue = |c: &Cue| {
            if c.group.is_some_and(|g| g >= self.groups.len()) {
                return Err("Music cue group".into());
            }
            c.command(0, 0, 0).validate(self.tracks.len())
        };
        cue(&self.battle)?;
        cue(&self.victory)?;
        for lane in &self.lanes {
            if lane.len() > 128 {
                return Err("Music lane budget".into());
            }
            for op in lane {
                match op {
                    Op::Wait { ticks } if *ticks > 3600 * 60 => {
                        return Err("Music wait budget".into())
                    }
                    Op::Event { tag } if tag.len() > 256 => return Err("Music event budget".into()),
                    Op::State { cue: c } | Op::Episode { cue: c } => cue(c)?,
                    Op::Multi { cue: c, victory: v } => {
                        cue(c)?;
                        cue(v)?;
                    }
                    Op::Params {
                        criteria,
                        count,
                        inner,
                        outer,
                    } => {
                        if *criteria > 2
                            || !(0..=1024).contains(count)
                            || !inner.is_finite()
                            || !outer.is_finite()
                            || *inner < 0.
                            || outer < inner
                            || *outer > 1e7
                        {
                            return Err("Music battle parameters".into());
                        }
                    }
                    Op::Radius { inner, outer }
                        if !inner.is_finite()
                            || !outer.is_finite()
                            || *inner < 0.
                            || outer < inner
                            || *outer > 1e7 =>
                    {
                        return Err("Music battle radius".into())
                    }
                    _ => {}
                }
            }
        }
        Ok(())
    }
    fn timeline(&self, events: &BTreeMap<String, u64>, now: u64) -> Timeline {
        let mut actions = vec![];
        for (lane_id, lane) in self.lanes.iter().enumerate() {
            let mut at = 0u64;
            for (index, op) in lane.iter().enumerate() {
                match op {
                    Op::Wait { ticks } => at = at.saturating_add(*ticks),
                    Op::Event { tag } => {
                        let Some(t) = events
                            .iter()
                            .find(|(key, _)| key.eq_ignore_ascii_case(tag))
                            .map(|(_, t)| *t)
                        else {
                            break;
                        };
                        at = at.max(t);
                    }
                    _ => {
                        if at > now {
                            break;
                        }
                        actions.push((at, lane_id, index, op));
                    }
                }
            }
        }
        actions.sort_by_key(|(at, lane, i, _)| (*at, *lane, *i));
        let mut t = Timeline {
            state: None,
            episode: None,
            enabled: true,
            battle: self.battle.clone(),
            victory: self.victory.clone(),
            inner: self.inner,
            outer: self.outer,
        };
        for (at, lane, index, op) in actions {
            let id = ((lane as u64 + 1) << 32) | ((index + 1) as u64);
            match op {
                Op::State { cue } => t.state = Some((at, id, cue.clone())),
                Op::Episode { cue } => {
                    // 103d0ee0/103ab9b0: null requests and a second episode
                    // during an active one are ignored. The selected adapter
                    // observes completion on the gameplay clock from asset duration.
                    let busy = t.episode.as_ref().is_some_and(|(start, _, old)| {
                        if old.track < 0 {
                            return false;
                        }
                        let track = &self.tracks[old.track as usize];
                        (at - start) * u64::from(track.rate) < track.frames * 60
                    });
                    if rc_package::music_decisions::episode_allowed(
                        true,
                        u32::from(busy),
                        u32::from(cue.track >= 0),
                    ) {
                        t.episode = Some((at, id, cue.clone()));
                    }
                }
                Op::Battle { enabled } => t.enabled = *enabled,
                Op::Radius { inner, outer } => {
                    t.inner = *inner;
                    t.outer = *outer;
                }
                Op::Multi { cue, victory } => {
                    t.battle = cue.clone();
                    t.victory = victory.clone();
                }
                Op::Victory => t.episode = Some((at, id, t.victory.clone())),
                _ => {}
            }
        }
        t
    }
    pub fn commands(&self, state: &MusicState, now: u64) -> [Command; 2] {
        let t = self.timeline(&state.events, now);
        let mut out = [Command::default(), Command::default()];
        if let Some((at, id, cue)) = t.state {
            out[0] = cue.command(at, now, id);
        }
        if state.battle && t.enabled {
            out[0] = t
                .battle
                .command(state.battle_tick, now, (1u64 << 62) | state.battle_tick);
        }
        let mut episode = t.episode;
        if let Some(at) = state.victory_tick {
            if episode.as_ref().is_none_or(|(old, _, _)| *old < at) {
                episode = Some((at, (1u64 << 61) | at, t.victory));
            }
        }
        if let Some((at, id, cue)) = episode {
            if cue.track >= 0 {
                let track = &self.tracks[cue.track as usize];
                if (now - at) * 800 * u64::from(track.rate) < track.frames * 48000 {
                    out[1] = cue.command(at, now, id);
                    out[0].track = -1;
                    out[0].fade_out = cue.fade_out;
                    out[0].out_kind = cue.out_kind;
                }
            }
        }
        out
    }
}
impl Game {
    pub(crate) fn tick_music(&mut self) -> Result<(), String> {
        let Some(assets) = &self.music else {
            return Ok(());
        };
        let now = self.state.statistics.ticks;
        let mut s = self
            .state
            .music
            .clone()
            .ok_or("Missing mission music state")?;
        if let Some(m) = &self.state.map {
            if let Some(g) = &m.gameplay {
                for e in &g.events {
                    s.events.entry(e.clone()).or_insert(now);
                }
            }
            if let Some(o) = &m.opening {
                for (e, t) in &o.event_ticks {
                    s.events.entry(e.clone()).or_insert(*t);
                }
            }
        }
        if !assets.score.groups.is_empty() {
            let mut runtime = s
                .runtime
                .take()
                .unwrap_or_else(|| crate::music_runtime::Runtime::new(&assets.score));
            let (input, enemies) = self.music_battle_observation(&runtime)?;
            runtime.advance(&assets.score, &s.events, now, &input, &enemies)?;
            s.battle = runtime.battle_voice.is_some();
            s.battle_tick = runtime.battle_voice.as_ref().map_or(now, |v| v.tick);
            s.runtime = Some(runtime);
            self.state.music = Some(s);
            return Ok(());
        }
        let t = assets.score.timeline(&s.events, now);
        let radius = if s.battle { t.outer } else { t.inner };
        let alive = self
            .state
            .enemies
            .iter()
            .filter(|e| {
                e.health > 0 && crate::distance(e.position, self.state.pawn.body.position) < radius
            })
            .count();
        let battle = t.enabled && alive >= assets.score.threshold.max(1);
        if battle != s.battle {
            if !battle && t.enabled {
                s.victory_tick = Some(now);
            }
            s.battle = battle;
            s.battle_tick = now;
        }
        self.state.music = Some(s);
        Ok(())
    }
    pub fn music_packet(&self) -> Vec<u8> {
        if let Some(runtime) = self.state.music.as_ref().and_then(|s| s.runtime.as_ref()) {
            return runtime.packet(self.state.statistics.ticks);
        }
        let c = self
            .music
            .as_ref()
            .zip(self.state.music.as_ref())
            .map(|(a, s)| a.score.commands(s, self.state.statistics.ticks))
            .unwrap_or_default();
        rc_package::mission_music::packet(&c)
    }
}
pub fn validate_state(game: &Game, state: &State) -> Result<(), String> {
    if game.music.is_some() != state.music.is_some() {
        return Err("Checkpoint soundtrack module mismatch".into());
    }
    if let Some(s) = &state.music {
        if let Some(runtime) = &s.runtime {
            runtime.validate(
                &game.music.as_ref().ok_or("Missing runtime assets")?.score,
                state.statistics.ticks,
                state.enemies.len(),
            )?;
        } else if game
            .music
            .as_ref()
            .is_some_and(|a| !a.score.groups.is_empty())
        {
            return Err("Missing saved music runtime".into());
        }
        if state.statistics.ticks > 24 * 60 * 60 * 60
            || s.events.len() > 4096
            || s.events
                .iter()
                .any(|(k, t)| k.len() > 256 || *t > state.statistics.ticks)
            || s.battle_tick > state.statistics.ticks
            || s.victory_tick.is_some_and(|t| t > state.statistics.ticks)
        {
            return Err("Invalid soundtrack checkpoint".into());
        }
    }
    Ok(())
}
#[cfg(test)]
mod tests {
    use super::*;
    fn cue(track: i32) -> Cue {
        Cue {
            group: None,
            use_defaults: false,
            looped: true,
            track,
            fade_in: 0.,
            fade_out: 0.,
            in_kind: 0,
            out_kind: 0,
        }
    }
    fn score() -> Score {
        Score {
            zones: vec![],
            groups: vec![],
            battle_rules: None,
            seed: 0,
            tracks: vec![
                Track {
                    playback: None,
                    source: "original.loop".into(),
                    frames: 48000,
                    rate: 48000,
                },
                Track {
                    playback: None,
                    source: "original.episode".into(),
                    frames: 48000,
                    rate: 48000,
                },
            ],
            lanes: vec![vec![
                Op::Event {
                    tag: "levelstart".into(),
                },
                Op::State { cue: cue(0) },
                Op::Event {
                    tag: "finale".into(),
                },
                Op::Wait { ticks: 30 },
                Op::Episode { cue: cue(1) },
            ]],
            battle: cue(0),
            victory: cue(1),
            inner: 1250.,
            outer: 2500.,
            threshold: 1,
            provenance: serde_json::json!({}),
        }
    }
    fn state() -> MusicState {
        MusicState {
            events: BTreeMap::from([("levelstart".into(), 0)]),
            ..Default::default()
        }
    }
    #[test]
    fn missing_event_never_guesses_a_music_start() {
        let a = score();
        a.validate().unwrap();
        assert_eq!(a.commands(&state(), 200)[0].track, 0);
        assert_eq!(a.commands(&state(), 200)[1].track, -1);
    }
    #[test]
    fn event_delay_episode_end_and_underlying_loop_time() {
        let a = score();
        let mut s = state();
        s.events.insert("FiNaLe".into(), 100);
        assert_eq!(a.commands(&s, 129)[1].track, -1);
        let c = a.commands(&s, 130);
        assert_eq!(c[0].track, -1);
        assert_eq!(c[1].track, 1);
        assert_eq!(c[1].elapsed, 0);
        assert_eq!(a.commands(&s, 159)[1].elapsed, 29 * 800);
        let c = a.commands(&s, 190);
        assert_eq!(c[1].track, -1);
        assert_eq!(c[0].track, 0);
        assert_eq!(c[0].elapsed, 190 * 800);
    }
    #[test]
    fn cold_music_state_keeps_events_and_activation_exact() {
        let a = score();
        let mut s = state();
        s.events.insert("finale".into(), 100);
        s.battle = true;
        s.battle_tick = 50;
        let restored: MusicState =
            serde_json::from_slice(&serde_json::to_vec(&s).unwrap()).unwrap();
        for tick in [120, 130, 160, 300] {
            assert_eq!(a.commands(&s, tick), a.commands(&restored, tick));
        }
    }
    #[test]
    fn corrupted_score_and_track_indices_rejected() {
        let mut a = score();
        a.battle.track = 2;
        assert!(a.validate().is_err());
        let mut a = score();
        a.lanes[0].push(Op::Radius {
            inner: f64::NAN,
            outer: 1.,
        });
        assert!(a.validate().is_err());
        let mut a = score();
        a.tracks[0].rate = 0;
        assert!(a.validate().is_err());
    }
    #[test]
    fn overlapping_episode_requests_keep_the_first_until_its_end() {
        let mut a = score();
        a.lanes = vec![vec![
            Op::Episode { cue: cue(1) },
            Op::Wait { ticks: 30 },
            Op::Episode { cue: cue(0) },
        ]];
        let c = a.commands(&state(), 40);
        assert_eq!(c[1].track, 1);
        assert_eq!(c[1].elapsed, 40 * 800);
        assert_eq!(a.commands(&state(), 60)[1].track, -1);
    }
    #[test]
    fn episode_at_exact_completion_is_accepted() {
        let mut a = score();
        a.lanes = vec![vec![
            Op::Episode { cue: cue(1) },
            Op::Wait { ticks: 60 },
            Op::Episode { cue: cue(0) },
        ]];
        let c = a.commands(&state(), 60);
        assert_eq!(c[1].track, 0);
        assert_eq!(c[1].elapsed, 0);
    }
    #[test]
    fn null_episode_does_not_cancel_music_and_room_changes_are_deferred() {
        let mut a = score();
        a.lanes = vec![vec![
            Op::State { cue: cue(0) },
            Op::Episode { cue: cue(1) },
            Op::Wait { ticks: 30 },
            Op::State { cue: cue(1) },
            Op::Episode { cue: cue(-1) },
        ]];
        assert_eq!(a.commands(&state(), 40)[1].track, 1);
        let c = a.commands(&state(), 60);
        assert_eq!(c[1].track, -1);
        assert_eq!(c[0].track, 1);
        assert_eq!(c[0].elapsed, 30 * 800);
    }
}
