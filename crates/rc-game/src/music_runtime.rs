//! Stateful mission music host over the original selection and battle kernels.
//! A private saved SRand stream isolates music from gameplay randomness.
use crate::music::{Cue, Score};
use rc_package::{
    mission_music::Command,
    music_decisions::{Group, Member, Random},
};
use serde::{Deserialize, Serialize};
use std::collections::{BTreeMap, BTreeSet};
#[derive(Clone, Debug, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Playback {
    pub looped: bool,
    pub fade_in: f32,
    pub fade_out: f32,
    pub in_kind: u8,
    pub out_kind: u8,
}
#[derive(Clone, Debug, Serialize, Deserialize)]
#[serde(tag = "kind", content = "index")]
pub enum Target {
    Track(usize),
    Group(usize),
}
#[derive(Clone, Debug, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Child {
    pub target: Option<Target>,
    pub context: u8,
    pub min: u8,
    pub max: u8,
    pub weight: f32,
}
#[derive(Clone, Debug, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Definition {
    pub source: String,
    pub style: u8,
    pub first: Option<Target>,
    pub children: Vec<Child>,
    pub victory: Option<Target>,
    pub lost: Option<Target>,
    pub fallback: usize,
}
#[derive(Clone, Debug, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct BattleRules {
    pub criteria: u8,
    pub count: i32,
    pub inner: f32,
    pub outer: f32,
}
#[derive(Clone, Debug, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Voice {
    pub cue: Cue,
    pub tick: u64,
    pub id: u64,
}
#[derive(Clone, Debug, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Runtime {
    pub attackers: BTreeSet<usize>,
    pub random: Random,
    pub groups: Vec<Group>,
    pub applied: BTreeSet<u64>,
    pub room: Option<Voice>,
    pub room_request: Option<Voice>,
    pub episode: Option<Voice>,
    pub battle_voice: Option<Voice>,
    pub battle_root: Cue,
    pub victory: Option<Target>,
    pub enabled: bool,
    pub rules: BattleRules,
    pub next_check: u64,
    pub end_pending: Option<u64>,
    pub tracked: BTreeSet<usize>,
    pub transitions: u64,
}
pub(crate) fn set_attack(state: &mut Option<crate::music::MusicState>, index: usize, active: bool) {
    if let Some(runtime) = state.as_mut().and_then(|s| s.runtime.as_mut()) {
        if active {
            runtime.attackers.insert(index);
        } else {
            runtime.attackers.remove(&index);
        }
    }
}
#[derive(Clone, Debug, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Zone {
    pub original: u32,
    pub name: String,
    pub nodes: Vec<crate::bundle::VolumeNode>,
    pub root_outside: bool,
    pub transform: crate::bundle::Transform,
    pub factories: Vec<u32>,
    #[serde(default)]
    pub forever: Vec<u32>,
}
impl Zone {
    fn validate(&self) -> Result<(), String> {
        if self.nodes.len() > 4096
            || self.name.len() > 256
            || self.factories.len() > 128
            || self.transform.scale.contains(&0.)
            || self
                .transform
                .location
                .iter()
                .chain(&self.transform.scale)
                .chain(&self.transform.pivot)
                .any(|v| !v.is_finite())
        {
            return Err("Music zone budget/transform".into());
        }
        let solid = rc_package::collision::SolidBsp {
            nodes: self
                .nodes
                .iter()
                .map(|n| rc_package::collision::SolidNode {
                    plane: n.plane,
                    front: n.front,
                    back: n.back,
                    flags: n.flags,
                })
                .collect(),
            root_outside: self.root_outside,
            linked: false,
            zones: 0,
            bounds: 0,
            hull_indices: 0,
            hull_words: vec![],
            leaves: 0,
            lights: 0,
            decoded_bytes: 0,
            remaining_bytes: 0,
        };
        solid.validate()
    }
    fn contains(&self, point: [f32; 3]) -> Result<bool, String> {
        let t = &self.transform;
        let local = rc_package::mesh_query::ActorTransform {
            location: t.location,
            rotation: t.rotation,
            scale: t.scale,
            pivot: t.pivot,
        }
        .local_point(point)?;
        let mut outside = self.root_outside;
        let mut index = if self.nodes.is_empty() { -1 } else { 0 };
        for _ in 0..=self.nodes.len() {
            if index == -1 {
                return Ok(!outside);
            }
            let n = self
                .nodes
                .get(usize::try_from(index).map_err(|_| "Music zone index")?)
                .ok_or("Music zone index")?;
            let distance =
                local[0] * n.plane[0] + local[1] * n.plane[1] + local[2] * n.plane[2] - n.plane[3];
            if !distance.is_finite() {
                return Err("Music zone arithmetic".into());
            }
            let front = distance > 0.;
            if n.vertex_count != 0 && (n.flags & 0x1f) & 0x21 == 0 {
                outside = front;
            }
            index = if front { n.front } else { n.back };
        }
        Err("Music zone cycle".into())
    }
}
impl crate::Game {
    fn music_factory_exhausted(&self, id: u32) -> bool {
        if let (Some(p), Some(s)) = (
            self.map.as_ref().and_then(|m| m.gameplay.as_ref()),
            self.state.map.as_ref().and_then(|m| m.gameplay.as_ref()),
        ) {
            if let Some((i, f)) = p
                .assets
                .factories
                .iter()
                .enumerate()
                .find(|(_, f)| f.original == id)
            {
                return s.factories.get(i).is_some_and(|n| *n >= f.capacity);
            }
        }
        if let (Some(p), Some(s)) = (
            self.map
                .as_ref()
                .and_then(|m| m.opening.as_ref())
                .and_then(|o| o.planning.as_ref()),
            self.state
                .map
                .as_ref()
                .and_then(|m| m.opening.as_ref())
                .and_then(|o| o.planning.as_ref()),
        ) {
            if p.assets.factories.iter().any(|f| f.original == id) {
                return s.spawned;
            }
            if let (Some(g), Some(gs)) = (&p.guard, &s.guard) {
                if let Some(i) = g
                    .assets
                    .factories
                    .iter()
                    .position(|f| f.factory.original == id)
                {
                    return gs.bindings.iter().any(|(_, slot)| *slot == i);
                }
            }
        }
        if let (Some(p), Some(s)) = (self.balcony_prepared(), self.balcony_state()) {
            if let Some(i) = p.assets.factories.iter().position(|f| f.original == id) {
                return s.bindings.iter().any(|(_, slot)| *slot == i);
            }
        }
        if let (Some(p), Some(s)) = (self.finale_prepared(), self.finale_state()) {
            if p.assets.climber.original == id {
                return s.climber.is_some();
            }
        }
        false
    }

    /// Explicit selected-controller attack state, written by the actual AI host.
    /// It is not inferred by the music renderer from cooldown, health or distance.
    pub(crate) fn music_attack(&mut self, index: usize, active: bool) {
        if let Some(runtime) = self.state.music.as_mut().and_then(|s| s.runtime.as_mut()) {
            if active {
                runtime.attackers.insert(index);
            } else {
                runtime.attackers.remove(&index);
            }
        }
    }
    pub(crate) fn music_battle_observation(
        &self,
        runtime: &Runtime,
    ) -> Result<
        (
            rc_package::music_decisions::Battle,
            Vec<rc_package::music_decisions::Enemy>,
        ),
        String,
    > {
        let mut zone = None;
        if let Some(a) = &self.music {
            for z in &a.score.zones {
                if z.contains(self.state.pawn.body.position.map(|v| v as f32))? {
                    if zone.is_some() {
                        return Err("Overlapping music zones require original touch order".into());
                    }
                    zone = Some(z);
                }
            }
        }
        let factories_exhausted = zone.is_none_or(|z| {
            z.factories
                .iter()
                .all(|id| self.music_factory_exhausted(*id))
        });
        let input = rc_package::music_decisions::Battle {
            ready: true,
            player: true,
            player_dead: self.state.health == 0,
            player_location: self.state.pawn.body.position.map(|v| v as f32),
            current: runtime.battle_voice.is_some(),
            force_context: false,
            zone: zone.is_some(),
            player_in_zone: zone.is_some(),
            factories_exhausted,
            criteria: runtime.rules.criteria,
            threshold: runtime.rules.count,
            inner: runtime.rules.inner,
            outer: runtime.rules.outer,
        };
        let enemies = self
            .state
            .enemies
            .iter()
            .enumerate()
            .map(|(i, e)| {
                Ok(rc_package::music_decisions::Enemy {
                    // The bounded port retires a dead pawn controller immediately.
                    ai: e.health > 0,
                    pawn: true,
                    hostile: true,
                    dead: e.health == 0,
                    attack_flag: runtime.attackers.contains(&i),
                    in_zone: match zone {
                        Some(z) => z.contains(e.position.map(|v| v as f32))?,
                        None => false,
                    },
                    location: e.position.map(|v| v as f32),
                })
            })
            .collect::<Result<Vec<_>, String>>()?;
        Ok((input, enemies))
    }
}
impl Score {
    pub fn target_cue(&self, target: &Target) -> Result<Cue, String> {
        let (track, group) = match target {
            Target::Track(i) => (*i, None),
            Target::Group(i) => (
                self.groups.get(*i).ok_or("Music group target")?.fallback,
                Some(*i),
            ),
        };
        let p = self
            .tracks
            .get(track)
            .and_then(|t| t.playback.as_ref())
            .ok_or("Missing music track playback")?;
        Ok(Cue {
            track: track as i32,
            group,
            use_defaults: true,
            looped: p.looped,
            fade_in: p.fade_in,
            fade_out: p.fade_out,
            in_kind: p.in_kind,
            out_kind: p.out_kind,
        })
    }
    pub fn validate_groups(&self) -> Result<(), String> {
        if self.zones.len() > 16 {
            return Err("Music zone budget".into());
        }
        for z in &self.zones {
            z.validate()?;
        }
        if self.groups.len() > 64 {
            return Err("Music group budget".into());
        }
        fn visit(
            score: &Score,
            target: &Target,
            stack: &mut BTreeSet<usize>,
            done: &mut BTreeSet<usize>,
            depth: usize,
        ) -> Result<(), String> {
            if depth > 8 {
                return Err("Music group depth".into());
            }
            let Target::Group(i) = target else {
                score.target_cue(target)?;
                return Ok(());
            };
            if stack.contains(i) {
                return Err("Cyclic music group".into());
            }
            if done.contains(i) {
                return Ok(());
            }
            let g = score.groups.get(*i).ok_or("Music group target")?;
            if g.source.len() > 256
                || g.style > 1
                || g.children.is_empty()
                || g.children.len() > 64
                || g.fallback >= score.tracks.len()
            {
                return Err("Invalid music group definition".into());
            }
            stack.insert(*i);
            for c in &g.children {
                if c.context > 2 || !c.weight.is_finite() || !(0. ..=1e6).contains(&c.weight) {
                    return Err("Invalid music group weight/filter".into());
                }
                if let Some(t) = &c.target {
                    visit(score, t, stack, done, depth + 1)?;
                }
            }
            for t in [&g.first, &g.victory, &g.lost].into_iter().flatten() {
                visit(score, t, stack, done, depth + 1)?;
            }
            stack.remove(i);
            done.insert(*i);
            Ok(())
        }
        let mut done = BTreeSet::new();
        for i in 0..self.groups.len() {
            visit(self, &Target::Group(i), &mut BTreeSet::new(), &mut done, 0)?;
        }
        if let Some(r) = &self.battle_rules {
            if r.criteria > 2
                || !(0..=1024).contains(&r.count)
                || !r.inner.is_finite()
                || !r.outer.is_finite()
                || r.inner < 0.
                || r.outer < r.inner
                || r.outer > 1e7
            {
                return Err("Invalid battle music rules".into());
            }
        }
        Ok(())
    }
}
impl Runtime {
    pub fn new(score: &Score) -> Self {
        Self {
            attackers: BTreeSet::new(),
            random: Random {
                seed: score.seed,
                draws: 0,
            },
            groups: score
                .groups
                .iter()
                .map(|g| Group {
                    style: g.style,
                    first: if g.first.is_some() {
                        g.children.len() as u32 + 1
                    } else {
                        0
                    },
                    flags: 1,
                    rendered: 0,
                    total_bits: 0,
                    which: 0,
                })
                .collect(),
            applied: BTreeSet::new(),
            room: None,
            room_request: None,
            episode: None,
            battle_voice: None,
            battle_root: score.battle.clone(),
            victory: None,
            enabled: true,
            rules: score.battle_rules.clone().unwrap_or(BattleRules {
                criteria: 1,
                count: score.threshold as i32,
                inner: score.inner as f32,
                outer: score.outer as f32,
            }),
            next_check: 0,
            end_pending: None,
            tracked: BTreeSet::new(),
            transitions: 0,
        }
    }
    fn select(
        &mut self,
        score: &Score,
        target: &Target,
        depth: usize,
    ) -> Result<Option<usize>, String> {
        if depth > 8 {
            return Err("Music selection depth".into());
        }
        let Target::Group(index) = target else {
            return match target {
                Target::Track(i) => Ok(Some(*i)),
                _ => unreachable!(),
            };
        };
        let def = score.groups.get(*index).ok_or("Music group missing")?;
        let members: Vec<_> = def
            .children
            .iter()
            .enumerate()
            .map(|(i, c)| Member {
                sound: if c.target.is_some() { i as u32 + 1 } else { 0 },
                context: c.context,
                min: c.min,
                max: c.max,
                weight: c.weight,
            })
            .collect();
        let state = self
            .groups
            .get_mut(*index)
            .ok_or("Music group state missing")?;
        let draw = if state.requires_draw(members.len()) {
            self.random.sample()?
        } else {
            0.
        };
        // Original music start 103acc50 invokes SelectSound(0,0), twice.
        let id = state.select(&members, 0, 0, draw)?;
        let selected = if id == 0 {
            None
        } else if id as usize == def.children.len() + 1 {
            def.first.as_ref()
        } else {
            def.children
                .get(id as usize - 1)
                .and_then(|c| c.target.as_ref())
        };
        selected
            .map(|t| self.select(score, t, depth + 1))
            .transpose()
            .map(Option::flatten)
    }
    fn resolve(&mut self, score: &Score, cue: &Cue) -> Result<Cue, String> {
        let Some(root) = cue.group else {
            return Ok(cue.clone());
        };
        let mut out = cue.clone();
        out.group = None;
        let Some(track) = self.select(score, &Target::Group(root), 0)? else {
            out.track = -1;
            return Ok(out);
        };
        let actual = score.target_cue(&Target::Track(track))?;
        out.track = track as i32;
        out.looped = actual.looped;
        if cue.use_defaults {
            out.fade_in = actual.fade_in;
            out.fade_out = actual.fade_out;
            out.in_kind = actual.in_kind;
            out.out_kind = actual.out_kind;
        }
        out.use_defaults = false;
        Ok(out)
    }
    fn expire(&mut self, score: &Score, now: u64) {
        if self.episode.as_ref().is_some_and(|v| {
            v.cue.track < 0
                || now.saturating_sub(v.tick) * u64::from(score.tracks[v.cue.track as usize].rate)
                    >= score.tracks[v.cue.track as usize].frames * 60
        }) {
            self.episode = None;
        }
    }
    fn episode(&mut self, score: &Score, cue: &Cue, tick: u64, id: u64) -> Result<(), String> {
        if self.episode.is_some() || cue.track < 0 {
            return Ok(());
        }
        let cue = self.resolve(score, cue)?;
        if cue.track < 0 {
            return Ok(());
        }
        self.episode = Some(Voice { cue, tick, id });
        self.room = None;
        self.battle_voice = None;
        self.tracked.clear();
        self.end_pending = None;
        self.transitions += 1;
        Ok(())
    }
    fn stop_battle(&mut self) {
        if self.battle_voice.take().is_some() {
            self.room = None;
        }
        self.tracked.clear();
        self.end_pending = None;
    }
    pub fn packet(&self, now: u64) -> Vec<u8> {
        let mut c = [Command::default(), Command::default()];
        if let Some(v) = &self.episode {
            c[1] = v.cue.command(v.tick, now, v.id);
            c[0].fade_out = v.cue.fade_out;
            c[0].out_kind = v.cue.out_kind;
        } else if let Some(v) = self.battle_voice.as_ref().or(self.room.as_ref()) {
            c[0] = v.cue.command(v.tick, now, v.id);
        }
        rc_package::mission_music::packet(&c)
    }
    pub fn advance(
        &mut self,
        score: &Score,
        events: &BTreeMap<String, u64>,
        now: u64,
        observation: &rc_package::music_decisions::Battle,
        enemies: &[rc_package::music_decisions::Enemy],
    ) -> Result<(), String> {
        let mut actions = vec![];
        for (lane_id, lane) in score.lanes.iter().enumerate() {
            let mut at = 0u64;
            for (index, op) in lane.iter().enumerate() {
                match op {
                    crate::music::Op::Wait { ticks } => at = at.saturating_add(*ticks),
                    crate::music::Op::Event { tag } => {
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
                        let id = ((lane_id as u64 + 1) << 32) | (index as u64 + 1);
                        if !self.applied.contains(&id) {
                            actions.push((at, id, op));
                        }
                    }
                }
            }
        }
        actions.sort_by_key(|(at, id, _)| (*at, *id));
        for (at, id, op) in actions {
            self.expire(score, at);
            match op {
                crate::music::Op::State { cue } => {
                    self.room = None;
                    self.room_request = Some(Voice {
                        cue: cue.clone(),
                        tick: at,
                        id,
                    });
                }
                crate::music::Op::Episode { cue } => self.episode(score, cue, at, id)?,
                crate::music::Op::Battle { enabled } => {
                    if !enabled && self.episode.is_none() {
                        self.stop_battle();
                    }
                    self.enabled = *enabled;
                }
                crate::music::Op::Radius { inner, outer } => {
                    self.rules.inner = *inner as f32;
                    self.rules.outer = *outer as f32;
                }
                crate::music::Op::Params {
                    criteria,
                    count,
                    inner,
                    outer,
                } => {
                    self.rules = BattleRules {
                        criteria: *criteria,
                        count: *count,
                        inner: *inner,
                        outer: *outer,
                    };
                }
                crate::music::Op::Multi { cue, .. } => self.battle_root = cue.clone(),
                crate::music::Op::Victory => {
                    if self.battle_voice.is_some() && self.episode.is_none() {
                        if let Some(t) = self.victory.clone() {
                            self.episode(score, &score.target_cue(&t)?, at, id)?;
                        }
                        self.stop_battle();
                    }
                }
                _ => {}
            }
            self.applied.insert(id);
        }
        self.expire(score, now);
        if self.enabled && self.episode.is_none() && now >= self.next_check {
            self.next_check = (now / 60 + 1) * 60;
            let mut input = observation.clone();
            input.current = self.battle_voice.is_some();
            input.criteria = self.rules.criteria;
            input.threshold = self.rules.count;
            input.inner = self.rules.inner;
            input.outer = self.rules.outer;
            let decision = rc_package::music_decisions::battle(&input, enemies)?;
            self.tracked.extend(decision.added);
            if decision.active {
                self.end_pending = None;
                if self.battle_voice.is_none() {
                    self.victory = None;
                    let root = self.battle_root.clone();
                    // Preserve the actual first selected subgroup's victory, not
                    // the first variant's victory from the old scene builder.
                    let cue = self.resolve(score, &root)?;
                    if let Some(g) = root.group {
                        let def = &score.groups[g];
                        let id = self.groups[g].rendered;
                        let t = if id as usize == def.children.len() + 1 {
                            def.first.as_ref()
                        } else if id == 0 {
                            None
                        } else {
                            def.children
                                .get(id as usize - 1)
                                .and_then(|c| c.target.as_ref())
                        };
                        if let Some(Target::Group(i)) = t {
                            self.victory = score.groups[*i].victory.clone();
                        }
                    }

                    if cue.track >= 0 {
                        self.battle_voice = Some(Voice {
                            cue,
                            tick: now,
                            id: (1u64 << 62) | now,
                        });
                        self.transitions += 1;
                    }
                }
            } else if self.battle_voice.is_some() {
                let all_dead = self
                    .tracked
                    .iter()
                    .all(|i| enemies.get(*i).is_none_or(|e| e.dead))
                    && (!input.zone || input.factories_exhausted);
                if all_dead {
                    if let Some(t) = self.victory.clone() {
                        self.episode(score, &score.target_cue(&t)?, now, (1u64 << 61) | now)?;
                    }
                    self.stop_battle();
                } else {
                    let deadline = *self.end_pending.get_or_insert(now + 120);
                    if now > deadline {
                        self.stop_battle();
                    }
                }
            }
        }
        if self.episode.is_none() && self.battle_voice.is_none() && self.room.is_none() {
            if let Some(room) = self.room_request.clone() {
                self.room = Some(Voice {
                    cue: self.resolve(score, &room.cue)?,
                    tick: now,
                    ..room
                });
            }
        }
        Ok(())
    }
    pub fn validate(&self, score: &Score, now: u64, enemies: usize) -> Result<(), String> {
        if now > 24 * 60 * 60 * 60 {
            return Err("Saved music clock budget".into());
        }
        if self.groups.len() != score.groups.len()
            || self.applied.len() > 2048
            || self
                .tracked
                .iter()
                .chain(&self.attackers)
                .any(|i| *i >= enemies)
            || self.random.draws > i64::MAX as u64
            || self.next_check > now + 60
            || self.end_pending.is_some_and(|t| t > now + 120)
            || self.transitions > i64::MAX as u64
        {
            return Err("Invalid saved music runtime".into());
        }
        let valid_rules = |r: &BattleRules| {
            r.criteria <= 2
                && (0..=1024).contains(&r.count)
                && r.inner.is_finite()
                && r.outer.is_finite()
                && r.inner >= 0.
                && r.outer >= r.inner
                && r.outer <= 1e7
        };
        if !valid_rules(&self.rules) {
            return Err("Invalid saved battle rules".into());
        }
        self.battle_root
            .command(0, 0, 0)
            .validate(score.tracks.len())?;
        if self
            .battle_root
            .group
            .is_some_and(|i| i >= score.groups.len())
        {
            return Err("Saved battle group".into());
        }
        if let Some(t) = &self.victory {
            score.target_cue(t)?;
        }
        for id in &self.applied {
            let lane = (id >> 32).checked_sub(1).ok_or("Saved music action lane")? as usize;
            let index = (*id as u32)
                .checked_sub(1)
                .ok_or("Saved music action index")? as usize;
            if !score.lanes.get(lane).is_some_and(|l| {
                l.get(index).is_some_and(|op| {
                    !matches!(
                        op,
                        crate::music::Op::Event { .. } | crate::music::Op::Wait { .. }
                    )
                })
            }) {
                return Err("Saved music action".into());
            }
        }
        for (g, d) in self.groups.iter().zip(&score.groups) {
            if g.style != d.style
                || g.which >= d.children.len()
                || g.first
                    != if d.first.is_some() {
                        d.children.len() as u32 + 1
                    } else {
                        0
                    }
                || g.rendered > d.children.len() as u32 + 1
                || g.flags & !7 != 0
                || !f32::from_bits(g.total_bits).is_finite()
                || f32::from_bits(g.total_bits) < 0.
            {
                return Err("Invalid saved music group".into());
            }
        }
        for v in [
            &self.room,
            &self.room_request,
            &self.episode,
            &self.battle_voice,
        ]
        .into_iter()
        .flatten()
        {
            if v.tick > now
                || v.cue.track < -1
                || v.cue.track >= score.tracks.len() as i32
                || v.cue.group.is_some_and(|g| g >= score.groups.len())
            {
                return Err("Invalid saved music voice".into());
            }
            v.cue
                .command(v.tick, now, v.id)
                .validate(score.tracks.len())?;
        }
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::music::{Op, Track};
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
        let mut s = Score {
            zones: vec![],
            groups: vec![],
            battle_rules: Some(BattleRules {
                criteria: 0,
                count: 1,
                inner: 100.,
                outer: 200.,
            }),
            seed: 17,
            tracks: (0..3)
                .map(|i| Track {
                    source: format!("test.{i}"),
                    frames: 48000,
                    rate: 48000,
                    playback: Some(Playback {
                        looped: true,
                        fade_in: i as f32,
                        fade_out: 0.,
                        in_kind: 0,
                        out_kind: 0,
                    }),
                })
                .collect(),
            lanes: vec![],
            battle: cue(0),
            victory: cue(-1),
            inner: 100.,
            outer: 200.,
            threshold: 1,
            provenance: serde_json::Value::Null,
        };
        s.groups.push(Definition {
            source: "root".into(),
            style: 1,
            first: None,
            children: vec![
                Child {
                    target: Some(Target::Group(1)),
                    context: 0,
                    min: 0,
                    max: 255,
                    weight: 1.,
                },
                Child {
                    target: Some(Target::Group(2)),
                    context: 0,
                    min: 0,
                    max: 255,
                    weight: 1.,
                },
            ],
            victory: None,
            lost: None,
            fallback: 0,
        });
        for i in 0..2 {
            s.groups.push(Definition {
                source: format!("sub{i}"),
                style: 0,
                first: None,
                children: vec![Child {
                    target: Some(Target::Track(i)),
                    context: 0,
                    min: 0,
                    max: 255,
                    weight: 1.,
                }],
                victory: Some(Target::Track(i + 1)),
                lost: None,
                fallback: i,
            });
        }
        s.battle = s.target_cue(&Target::Group(0)).unwrap();
        s
    }
    fn input() -> rc_package::music_decisions::Battle {
        rc_package::music_decisions::Battle {
            ready: true,
            player: true,
            player_dead: false,
            player_location: [0.; 3],
            current: false,
            force_context: false,
            zone: false,
            player_in_zone: false,
            factories_exhausted: true,
            criteria: 0,
            threshold: 1,
            inner: 100.,
            outer: 200.,
        }
    }
    fn enemy(attack: bool, dead: bool) -> rc_package::music_decisions::Enemy {
        rc_package::music_decisions::Enemy {
            ai: true,
            pawn: true,
            hostile: true,
            dead,
            attack_flag: attack,
            in_zone: false,
            location: [10., 0., 0.],
        }
    }
    #[test]
    fn selected_subgroup_controls_victory_and_next_battle_after_cold_restore() {
        let s = score();
        s.validate().unwrap();
        let mut r = Runtime::new(&s);
        let events = BTreeMap::new();
        r.advance(&s, &events, 0, &input(), &[enemy(true, false)])
            .unwrap();
        assert_eq!(r.battle_voice.as_ref().unwrap().cue.track, 0);
        assert!(matches!(r.victory, Some(Target::Track(1))));
        r.advance(&s, &events, 60, &input(), &[enemy(false, true)])
            .unwrap();
        assert_eq!(r.episode.as_ref().unwrap().cue.track, 1);
        let bytes = serde_json::to_vec(&r).unwrap();
        let mut cold: Runtime = serde_json::from_slice(&bytes).unwrap();
        cold.validate(&s, 60, 1).unwrap();
        for t in 61..=180 {
            r.advance(&s, &events, t, &input(), &[enemy(true, false)])
                .unwrap();
            cold.advance(&s, &events, t, &input(), &[enemy(true, false)])
                .unwrap();
            assert_eq!(r.packet(t), cold.packet(t));
        }
        assert_eq!(r.battle_voice.as_ref().unwrap().cue.track, 1);
        assert!(matches!(r.victory, Some(Target::Track(2))));
        assert_eq!(r.random.draws, 2);
    }
    #[test]
    fn render_snapshots_never_select_or_advance_random() {
        let s = score();
        let mut r = Runtime::new(&s);
        r.advance(&s, &BTreeMap::new(), 0, &input(), &[enemy(true, false)])
            .unwrap();
        let before = serde_json::to_vec(&r).unwrap();
        for _ in 0..100 {
            r.packet(0);
        }
        assert_eq!(before, serde_json::to_vec(&r).unwrap());
    }
    #[test]
    fn room_selection_waits_for_episode_and_starts_at_release() {
        let mut s = score();
        s.lanes = vec![vec![
            Op::Episode { cue: cue(2) },
            Op::State {
                cue: s.target_cue(&Target::Group(0)).unwrap(),
            },
            Op::Battle { enabled: false },
        ]];
        let mut r = Runtime::new(&s);
        r.advance(&s, &BTreeMap::new(), 0, &input(), &[]).unwrap();
        assert_eq!(r.random.draws, 0);
        r.advance(&s, &BTreeMap::new(), 60, &input(), &[]).unwrap();
        assert_eq!(r.random.draws, 1);
        assert_eq!(r.room.as_ref().unwrap().tick, 60);
        assert_eq!(r.room.as_ref().unwrap().cue.track, 0);
    }
    #[test]
    fn battle_end_delay_is_strict_and_factories_keep_volume_active() {
        let s = score();
        let mut r = Runtime::new(&s);
        let events = BTreeMap::new();
        r.advance(&s, &events, 0, &input(), &[enemy(true, false)])
            .unwrap();
        for t in [60, 120, 180] {
            r.advance(&s, &events, t, &input(), &[enemy(false, false)])
                .unwrap();
            assert!(r.battle_voice.is_some());
        }
        r.advance(&s, &events, 240, &input(), &[enemy(false, false)])
            .unwrap();
        assert!(r.battle_voice.is_none());
        let mut r = Runtime::new(&s);
        r.advance(&s, &events, 0, &input(), &[enemy(true, false)])
            .unwrap();
        let mut v = input();
        v.zone = true;
        v.player_in_zone = true;
        v.factories_exhausted = false;
        r.advance(&s, &events, 60, &v, &[enemy(false, true)])
            .unwrap();
        assert!(r.battle_voice.is_some());
        v.factories_exhausted = true;
        r.advance(&s, &events, 120, &v, &[enemy(false, true)])
            .unwrap();
        assert!(r.battle_voice.is_none());
        assert!(r.episode.is_some());
    }
    #[test]
    fn invalid_graph_parameters_and_saved_action_are_rejected() {
        let mut s = score();
        s.groups[1].children[0].target = Some(Target::Group(0));
        assert!(s.validate().is_err());
        let mut s = score();
        s.lanes = vec![vec![Op::Params {
            criteria: 3,
            count: 1,
            inner: 0.,
            outer: 1.,
        }]];
        assert!(s.validate().is_err());
        let s = score();
        let mut r = Runtime::new(&s);
        r.applied.insert(9);
        assert!(r.validate(&s, 0, 0).is_err());
        r.applied.clear();
        r.rules.outer = f32::NAN;
        assert!(r.validate(&s, 0, 0).is_err());
    }
    #[test]
    fn room_is_selected_again_after_battle_and_cold_restore() {
        let mut s = score();
        let room = s.target_cue(&Target::Group(0)).unwrap();
        s.lanes = vec![vec![Op::State { cue: room }, Op::Battle { enabled: false }]];
        let mut r = Runtime::new(&s);
        r.advance(&s, &BTreeMap::new(), 0, &input(), &[]).unwrap();
        assert_eq!(r.room.as_ref().unwrap().cue.track, 0);
        r.room = None;
        r.battle_voice = Some(Voice {
            cue: cue(2),
            tick: 1,
            id: 1,
        });
        let mut cold: Runtime = serde_json::from_slice(&serde_json::to_vec(&r).unwrap()).unwrap();
        cold.stop_battle();
        cold.advance(&s, &BTreeMap::new(), 30, &input(), &[])
            .unwrap();
        assert_eq!(cold.room.as_ref().unwrap().cue.track, 1);
        assert_eq!(cold.room.as_ref().unwrap().tick, 30);
    }
    #[test]
    fn disabling_idle_battle_does_not_restart_current_room() {
        let mut s = score();
        s.lanes = vec![vec![
            Op::State { cue: cue(0) },
            Op::Battle { enabled: false },
            Op::Wait { ticks: 30 },
            Op::Battle { enabled: false },
        ]];
        let mut r = Runtime::new(&s);
        r.advance(&s, &BTreeMap::new(), 0, &input(), &[]).unwrap();
        let before = r.room.as_ref().unwrap().tick;
        r.advance(&s, &BTreeMap::new(), 30, &input(), &[]).unwrap();
        assert_eq!(r.room.as_ref().unwrap().tick, before);
    }
    #[test]
    fn zone_uses_original_back_side_at_plane_equality() {
        let z = Zone {
            original: 1,
            name: "test".into(),
            nodes: vec![crate::bundle::VolumeNode {
                plane: [1., 0., 0., 0.],
                front: -1,
                back: -1,
                flags: 0,
                vertex_count: 3,
            }],
            root_outside: true,
            transform: crate::bundle::Transform {
                location: [10., 0., 0.],
                rotation: [0; 3],
                scale: [1.; 3],
                pivot: [0.; 3],
            },
            factories: vec![],
            forever: vec![],
        };
        z.validate().unwrap();
        assert!(z.contains([10., 0., 0.]).unwrap());
        assert!(!z.contains([11., 0., 0.]).unwrap());
    }
}
