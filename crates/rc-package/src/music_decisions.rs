//! Original 103ac860 battle predicate and 103d1f80 SoundMultiple selection.
//! Observations, relationships, volume membership and random draws are explicit
//! host inputs. This module does not fabricate AI perception or play music.
use serde::{Deserialize, Serialize};
/// Original Core.appSRand 10110250; the mission owns a separate saved seed.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Random {
    pub seed: u32,
    pub draws: u64,
}
impl Random {
    pub fn sample(&mut self) -> Result<f32, String> {
        if self.draws >= i64::MAX as u64 {
            return Err("Music random counter exhausted".into());
        }
        self.seed = self.seed.wrapping_mul(0x0bb38435).wrapping_add(0x3619636b);
        self.draws += 1;
        Ok(f32::from_bits(0x3f800000 | (self.seed & 0x007fffff)) - 1.)
    }
}
/// Public PlayMusicEpisode 103d0ee0 and its acceptance gate 103ab9b0.
pub fn episode_allowed(ready: bool, current: u32, incoming: u32) -> bool {
    ready && incoming != 0 && current == 0
}
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Episode {
    pub episode: u32,
    pub battle: u32,
    pub overlay: u32,
    pub overlay_queued: u32,
}
impl Episode {
    /// Accepted public episodes flush pending playback and clear the overlay via
    /// 103ab330; the caller executes the returned acceptance's physical flush.
    pub fn submit(&mut self, ready: bool, incoming: u32) -> bool {
        if !episode_allowed(ready, self.episode, incoming) {
            return false;
        }
        self.episode = incoming;
        self.battle = 0;
        if self.overlay != 0 {
            self.overlay_queued = 0;
            self.overlay = 0;
        }
        true
    }
}
#[derive(Clone, Debug, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Enemy {
    pub ai: bool,
    pub pawn: bool,
    pub hostile: bool,
    pub dead: bool,
    pub attack_flag: bool,
    pub in_zone: bool,
    pub location: [f32; 3],
}
#[derive(Clone, Debug, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Battle {
    pub ready: bool,
    pub player: bool,
    pub player_dead: bool,
    pub player_location: [f32; 3],
    pub current: bool,
    pub force_context: bool,
    pub zone: bool,
    pub player_in_zone: bool,
    pub factories_exhausted: bool,
    pub criteria: u8,
    pub threshold: i32,
    pub inner: f32,
    pub outer: f32,
}
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct Decision {
    pub active: bool,
    pub added: Vec<usize>,
}
pub fn battle(input: &Battle, enemies: &[Enemy]) -> Result<Decision, String> {
    if input.criteria > 2
        || enemies.len() > 4096
        || !input.inner.is_finite()
        || !input.outer.is_finite()
        || input.inner.abs() > 1e7
        || input.outer.abs() > 1e7
        || input
            .player_location
            .iter()
            .chain(enemies.iter().flat_map(|e| &e.location))
            .any(|v| !v.is_finite() || v.abs() > 1e7)
    {
        return Err("Invalid battle observation".into());
    }
    let mut out = Decision {
        active: false,
        added: vec![],
    };
    if !input.ready {
        return Ok(out);
    }
    let context = input.current || input.force_context;
    let radius = if input.current {
        input.outer
    } else {
        input.inner
    };
    let mut attackers = 0;
    let mut near = 0;
    for (i, e) in enemies.iter().enumerate() {
        if !e.ai || !e.pawn || !input.player || !e.hostile {
            continue;
        }
        if input.zone && context && input.player_in_zone {
            if e.in_zone {
                attackers += 1;
                near += 1;
                out.added.push(i);
            }
            continue;
        }
        if input.zone && !context && !e.in_zone {
            continue;
        }
        let a = (input.criteria == 0 || input.criteria == 2) && e.attack_flag;
        if a {
            attackers += 1;
        }
        let delta = std::array::from_fn::<_, 3, _>(|k| e.location[k] - input.player_location[k]);
        let distance = (delta[2] * delta[2] + delta[1] * delta[1]) + delta[0] * delta[0];
        let n = (input.criteria == 1 || input.criteria == 2 && a)
            && !e.dead
            && distance < radius * radius;
        if n {
            near += 1;
        }
        if input.criteria == 0 && a || input.criteria == 1 && n || a && n {
            out.added.push(i);
        }
    }
    if !input.player || input.player_dead {
        return Ok(out);
    }
    if input.zone && context && input.player_in_zone {
        out.active = attackers > 0 || !input.factories_exhausted;
    } else {
        let threshold = if context { 1 } else { input.threshold };
        out.active = match input.criteria {
            0 => attackers >= threshold,
            1 => near >= threshold,
            _ => attackers >= threshold && near >= threshold,
        };
    }
    Ok(out)
}
#[derive(Clone, Debug, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Member {
    pub sound: u32,
    pub context: u8,
    pub min: u8,
    pub max: u8,
    pub weight: f32,
}
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Group {
    pub style: u8,
    pub first: u32,
    pub flags: u32,
    pub rendered: u32,
    pub total_bits: u32,
    pub which: usize,
}
impl Group {
    pub fn requires_draw(&self, count: usize) -> bool {
        count > 0 && self.style != 1 && !(self.first != 0 && self.flags & 1 != 0)
    }
    pub fn select(
        &mut self,
        members: &[Member],
        campaign: u8,
        flags: u32,
        draw: f32,
    ) -> Result<u32, String> {
        if members.len() > 64
            || self.style > 1
            || !draw.is_finite()
            || !(0. ..=1.).contains(&draw)
            || !f32::from_bits(self.total_bits).is_finite()
            || members
                .iter()
                .any(|m| m.context > 2 || !m.weight.is_finite() || !(0. ..=1e6).contains(&m.weight))
            || self.style == 1 && !members.is_empty() && self.which >= members.len()
        {
            return Err("Invalid music group selection".into());
        }
        if members.is_empty() {
            return Ok(0);
        }
        if self.style == 1 {
            self.rendered = members[self.which].sound;
            self.which = (self.which + 1) % members.len();
            return Ok(self.rendered);
        }
        if self.first != 0 && self.flags & 1 != 0 {
            self.flags &= !1;
            self.rendered = self.first;
            return Ok(self.rendered);
        }
        let eligible = |m: &Member| {
            m.sound != 0
                && m.context != if flags & 0x200 != 0 { 2 } else { 1 }
                && m.min <= campaign
                && campaign <= m.max
        };
        if self.flags & 2 == 0 {
            self.total_bits = 0;
            self.flags |= 6;
            let mut total = 0f32;
            for m in members.iter().filter(|m| m.sound != 0) {
                total += m.weight;
                if m.context != 0 || m.min != 0 || m.max != 255 {
                    self.flags &= !4;
                }
            }
            self.total_bits = total.to_bits();
        }
        if self.flags & 4 == 0 {
            let mut total = 0f32;
            for m in members.iter().filter(|m| eligible(m)) {
                total += m.weight;
            }
            self.total_bits = total.to_bits();
        }
        self.rendered = members[0].sound;
        let mut accumulated = 0f32;
        for m in members.iter().filter(|m| eligible(m)) {
            accumulated += m.weight;
            // Original FMUL is followed by FSTP binary32 before COMISS.
            if draw * f32::from_bits(self.total_bits) < accumulated {
                self.rendered = m.sound;
                break;
            }
        }
        Ok(self.rendered)
    }
}
#[cfg(test)]
#[path = "music_decisions_tests.rs"]
mod tests;
