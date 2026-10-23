//! Selected USoundMultiple::SelectSound (engine.dll 103d1f80).
//! Flat groups, prepared member properties and an explicit random host. No
//! recursive sound rendering, battle music, RefreshGroup or audio parameter host.
use serde::{Deserialize, Serialize};
#[derive(Clone, Debug, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Member {
    pub sound: usize,
    pub battle: u8,
    pub minimum: u8,
    pub maximum: u8,
    pub likelihood: f32,
}
#[derive(Clone, Debug, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Group {
    pub source: String,
    pub in_order: bool,
    pub first: Option<usize>,
    pub members: Vec<Option<Member>>,
}
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct State {
    pub flags: u8,
    pub total: f32,
    pub which: usize,
    pub rendered: Option<usize>,
}
impl Default for State {
    fn default() -> Self {
        Self {
            flags: 1,
            total: 0.,
            which: 0,
            rendered: None,
        }
    }
}
impl Group {
    pub fn validate(&self, sounds: usize) -> Result<(), String> {
        if self.members.len() > 64
            || self.first.is_some_and(|i| i >= sounds)
            || self.members.iter().flatten().any(|m| {
                m.sound >= sounds
                    || m.battle > 2
                    || m.minimum > m.maximum
                    || !m.likelihood.is_finite()
                    || !(0. ..=1e6).contains(&m.likelihood)
            })
        {
            return Err("Invalid prepared sound group".into());
        }
        Ok(())
    }
}
impl State {
    pub fn validate(&self, g: &Group) -> Result<(), String> {
        if self.flags & !7 != 0
            || !self.total.is_finite()
            || self.total < 0.
            || self.which >= g.members.len().max(1)
            || self.rendered.is_some_and(|s| {
                Some(s) != g.first && !g.members.iter().flatten().any(|m| m.sound == s)
            })
        {
            return Err("Invalid sound group checkpoint".into());
        }
        Ok(())
    }
    pub fn select(
        &mut self,
        g: &Group,
        campaign: u8,
        flags: u32,
        mut random: impl FnMut() -> f32,
    ) -> Option<usize> {
        if g.members.is_empty() {
            return None;
        }
        if g.in_order {
            self.rendered = g.members[self.which].as_ref().map(|m| m.sound);
            self.which += 1;
            if self.which == g.members.len() {
                self.which = 0;
            }
            return self.rendered;
        }
        if let Some(first) = g.first.filter(|_| self.flags & 1 != 0) {
            self.flags &= !1;
            self.rendered = Some(first);
            return self.rendered;
        }
        if self.flags & 2 == 0 {
            self.total = 0.;
            self.flags |= 6;
            for m in g.members.iter().flatten() {
                self.total += m.likelihood;
                if m.battle != 0 || m.minimum != 0 || m.maximum != 255 {
                    self.flags &= !4;
                }
            }
        }
        let eligible = |m: &Member| {
            m.battle != if flags & 0x200 != 0 { 2 } else { 1 }
                && m.minimum <= campaign
                && campaign <= m.maximum
        };
        if self.flags & 4 == 0 {
            self.total = 0.;
            for m in g.members.iter().flatten().filter(|m| eligible(m)) {
                self.total += m.likelihood;
            }
        }
        // Original appSRand returns an f32 in ST0. FMUL/FSTP stores one f32;
        // the product of two f32 inputs fits exactly in an x87 significand.
        let threshold = random() * self.total;
        self.rendered = g.members[0].as_ref().map(|m| m.sound);
        let mut accumulated = 0f32;
        for m in g.members.iter().flatten().filter(|m| eligible(m)) {
            accumulated += m.likelihood;
            if accumulated > threshold {
                self.rendered = Some(m.sound);
                break;
            }
        }
        self.rendered
    }
}
/// Original core.dll appSRand 10110250; explicit local seed, no global RNG claim.
pub fn random(seed: &mut u32) -> f32 {
    *seed = seed.wrapping_mul(0x0bb38435).wrapping_add(0x3619636b);
    f32::from_bits(0x3f800000 | (*seed & 0x007fffff)) - 1.
}
/// Native serialized TArray<USound*> followed by one SelectionStyle byte.
pub fn read_members(payload: &[u8], native_offset: usize) -> Result<(Vec<i32>, u8), String> {
    let mut r = super::Reader::at(payload, native_offset)?;
    let count = r.index()?;
    if !(0..=64).contains(&count) {
        return Err("Invalid sound group count".into());
    }
    let mut members = Vec::new();
    for _ in 0..count {
        members.push(r.index()?);
    }
    let style = r.byte()?;
    if style > 1 || r.pos != payload.len() {
        return Err("Unsupported sound group layout/style".into());
    }
    Ok((members, style))
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn matches_960_original_instruction_cases_including_saved_cache_and_rng() {
        #[derive(Deserialize)]
        struct Expected {
            selected: Option<usize>,
            seed: u32,
            draws: u32,
            state: State,
        }
        #[derive(Deserialize)]
        struct Case {
            group: Group,
            state: State,
            seed: u32,
            campaign: u8,
            flags: u32,
            expected: Expected,
        }
        #[derive(Deserialize)]
        struct Fixture {
            cases: Vec<Case>,
        }
        let fixture: Fixture =
            serde_json::from_str(include_str!("../../../fixtures/sound-multiple-native.json"))
                .unwrap();
        assert_eq!(fixture.cases.len(), 960);
        for (i, c) in fixture.cases.into_iter().enumerate() {
            let mut state = c.state;
            let mut seed = c.seed;
            let mut draws = 0;
            let selected = state.select(&c.group, c.campaign, c.flags, || {
                draws += 1;
                random(&mut seed)
            });
            assert_eq!(
                (selected, seed, draws),
                (c.expected.selected, c.expected.seed, c.expected.draws),
                "case {i}"
            );
            assert_eq!(state.flags, c.expected.state.flags, "case {i}");
            assert_eq!(state.which, c.expected.state.which, "case {i}");
            assert_eq!(state.rendered, c.expected.state.rendered, "case {i}");
            assert_eq!(
                state.total.to_bits(),
                c.expected.state.total.to_bits(),
                "case {i}"
            );
        }
    }
    fn group() -> Group {
        Group {
            source: "fixture".into(),
            in_order: false,
            first: None,
            members: vec![
                Some(Member {
                    sound: 0,
                    battle: 0,
                    minimum: 0,
                    maximum: 255,
                    likelihood: 1.,
                }),
                Some(Member {
                    sound: 1,
                    battle: 0,
                    minimum: 0,
                    maximum: 255,
                    likelihood: 3.,
                }),
            ],
        }
    }
    #[test]
    fn strict_weight_boundary_and_first_sound_do_not_consume_random() {
        let mut g = group();
        let mut s = State::default();
        assert_eq!(s.select(&g, 0, 0, || 0.249), Some(0));
        assert_eq!(s.select(&g, 0, 0, || 0.25), Some(1));
        g.first = Some(9);
        s = State::default();
        assert_eq!(
            s.select(&g, 0, 0, || panic!("FirstSound must not draw")),
            Some(9)
        );
        assert_eq!(s.select(&g, 0, 0, || 0.5), Some(1));
    }
    #[test]
    fn in_order_nulls_wrap_and_empty_without_random() {
        let mut g = group();
        g.in_order = true;
        g.members[1] = None;
        let mut s = State::default();
        for expected in [Some(0), None, Some(0)] {
            assert_eq!(
                s.select(&g, 0, 0, || panic!("order must not draw")),
                expected
            );
        }
        g.members.clear();
        assert_eq!(s.select(&g, 0, 0, || panic!("empty must not draw")), None);
    }
    #[test]
    fn campaign_and_battle_filter_recompute_and_native_zero_total_fallback() {
        let mut g = group();
        g.members[0].as_mut().unwrap().battle = 1;
        g.members[1].as_mut().unwrap().minimum = 5;
        let mut s = State::default();
        assert_eq!(s.select(&g, 5, 0, || 0.5), Some(1));
        assert_eq!(s.total, 3.);
        assert_eq!(s.select(&g, 0, 0, || 0.5), Some(0));
        assert_eq!(s.total, 0.);
        assert_eq!(s.select(&g, 0, 0x200, || 0.5), Some(0));
        assert_eq!(s.total, 1.);
    }
}
