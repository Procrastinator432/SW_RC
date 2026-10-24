//! Original pause/stop/queue command decisions over owned portable sources.
//! Driver and music actions are returned as ordered events, not executed here.
use crate::{
    audio_curve::CurveMath,
    audio_source::{Init, Source},
    audio_source_lookup::{self, Found, Query},
};
use serde::{Deserialize, Serialize};
#[derive(Clone, Debug, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Sound {
    pub handle: u32,
    pub default_fade: f32,
    pub streamed: bool,
    pub curve: u8,
    pub category: u8,
}
#[derive(Clone, Debug, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Sources {
    pub active: Vec<Source>,
    pub waiting: Vec<Source>,
    pub counter: u32,
    pub cue_id: u32,
}
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub enum Event {
    MusicPause(bool),
    MusicStop,
    Pause {
        target: Found,
        request: i32,
    },
    Stop {
        target: Found,
        voice_exempt: bool,
    },
    Fade {
        target: Found,
        duration_bits: u32,
        kind: u8,
    },
    Delete {
        target: Found,
        unique_id: u32,
    },
}
#[derive(Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
struct Snapshot<T> {
    schema: u32,
    state: T,
}
fn sound(bank: &[Sound], handle: u32) -> Result<&Sound, String> {
    bank.iter()
        .find(|s| s.handle == handle)
        .ok_or_else(|| "Missing command sound metadata".into())
}
fn validate_bank(bank: &[Sound]) -> Result<(), String> {
    if bank.len() > 4096 {
        return Err("Too many command sound profiles".into());
    }
    let mut seen = std::collections::BTreeSet::new();
    for s in bank {
        if s.handle == 0
            || !seen.insert(s.handle)
            || !s.default_fade.is_finite()
            || !(-60. ..=60.).contains(&s.default_fade)
            || s.curve > 3
        {
            return Err("Invalid command sound metadata".into());
        }
    }
    Ok(())
}
fn stop(s: &mut Source, target: Found, events: &mut Vec<Event>) {
    events.push(Event::Stop {
        target,
        voice_exempt: s.voice_exempt,
    });
    s.stop();
}
impl Sources {
    pub fn validate(&self) -> Result<(), String> {
        if self.active.len() > 4096 || self.waiting.len() > 4096 {
            return Err("Too many managed audio sources".into());
        }
        for s in self.active.iter().chain(&self.waiting) {
            s.validate()?;
        }
        Ok(())
    }
    pub fn checkpoint(&self) -> Result<Vec<u8>, String> {
        self.validate()?;
        serde_json::to_vec(&Snapshot {
            schema: 1,
            state: self,
        })
        .map_err(|e| e.to_string())
    }
    pub fn load(&mut self, bytes: &[u8]) -> Result<(), String> {
        if bytes.len() > 16 * 1024 * 1024 {
            return Err("Audio command snapshot too large".into());
        }
        let next: Snapshot<Self> = serde_json::from_slice(bytes).map_err(|e| e.to_string())?;
        if next.schema != 1 {
            return Err("Unsupported audio command snapshot".into());
        }
        next.state.validate()?;
        *self = next.state;
        Ok(())
    }
    /// Own the previously external constructor counter. Allocation/storage here
    /// is Rust policy; this does not emulate GMalloc or native queue activation.
    pub fn enqueue(&mut self, input: &Init, math: &mut impl CurveMath) -> Result<usize, String> {
        self.validate()?;
        if self.waiting.len() == 4096 {
            return Err("Audio command queue full".into());
        }
        let mut counter = self.counter;
        let mut s = Source::new(&mut counter);
        if !s.initialize(input, math)? {
            return Err("No engine for queued source".into());
        }
        self.counter = counter;
        self.waiting.push(s);
        Ok(self.waiting.len() - 1)
    }
    pub fn pause_sound(
        &mut self,
        actor: u32,
        sound: u32,
        request: i32,
    ) -> Result<Vec<Event>, String> {
        self.validate()?;
        let mut events = vec![];
        for (i, s) in self.active.iter_mut().enumerate() {
            if s.identity != 0 && s.matches(actor, sound) {
                events.push(Event::Pause {
                    target: Found::Active(i),
                    request,
                });
                s.pause(request);
            }
        }
        Ok(events)
    }
    /// Replies belong to virtual IsPaused (+1c), not IsQueued (+20) and not the
    /// stored request. Native returns the driver's i32 verbatim.
    pub fn is_sound_paused(
        &self,
        query: Query,
        active_replies: &[i32],
        waiting_replies: &[i32],
    ) -> Result<i32, String> {
        self.validate()?;
        if active_replies.len() != self.active.len() || waiting_replies.len() != self.waiting.len()
        {
            return Err("Pause reply/source mismatch".into());
        }
        Ok(
            match audio_source_lookup::source(&self.active, &self.waiting, query)? {
                Some(Found::Active(i)) => active_replies[i],
                Some(Found::Queued(i)) => waiting_replies[i],
                None => 0,
            },
        )
    }
    pub fn pause_all(&mut self, request: i32, exclude: u32) -> Result<Vec<Event>, String> {
        self.validate()?;
        let mut events = vec![Event::MusicPause(request != 0)];
        for (queued, list) in [(false, &mut self.active), (true, &mut self.waiting)] {
            for (i, s) in list.iter_mut().enumerate() {
                if request == 0 || s.flags & exclude == 0 {
                    let target = if queued {
                        Found::Queued(i)
                    } else {
                        Found::Active(i)
                    };
                    events.push(Event::Pause { target, request });
                    s.pause(request);
                }
            }
        }
        Ok(events)
    }
    pub fn stop_sound(
        &mut self,
        actor: u32,
        handle: u32,
        fade: f32,
        bank: &[Sound],
        math: &mut impl CurveMath,
    ) -> Result<(bool, Vec<Event>), String> {
        self.validate()?;
        validate_bank(bank)?;
        if !fade.is_finite() || !(-60. ..=60.).contains(&fade) {
            return Err("Invalid stop fade".into());
        }
        let mut next = self.clone();
        let mut events = vec![];
        if actor == 0 && handle == 0 {
            events.push(Event::MusicStop);
            for (i, s) in next.active.iter_mut().enumerate() {
                stop(s, Found::Active(i), &mut events);
                s.voice_exempt = false;
            }
            *self = next;
            return Ok((true, events));
        }
        let mut found = false;
        for (i, s) in next.active.iter_mut().enumerate() {
            if !s.matches(actor, handle) {
                continue;
            }
            let duration = if fade == -1. {
                if handle == 0 {
                    0.
                } else {
                    sound(bank, handle)?.default_fade
                }
            } else {
                fade
            };
            if duration > 0. {
                let (streamed, kind) = if handle == 0 {
                    (false, 0)
                } else {
                    let p = sound(bank, handle)?;
                    (p.streamed, if p.streamed { p.curve } else { 0 })
                };
                s.fade.stop(duration, streamed, kind, math)?;
                events.push(Event::Fade {
                    target: Found::Active(i),
                    duration_bits: duration.to_bits(),
                    kind,
                });
            } else {
                s.voice_exempt = false;
                stop(s, Found::Active(i), &mut events);
            }
            found = true;
        }
        for (i, s) in next.waiting.iter_mut().enumerate() {
            if s.matches(actor, handle) {
                stop(s, Found::Queued(i), &mut events);
                found = true;
            }
        }
        next.validate()?;
        *self = next;
        Ok((found, events))
    }
    pub fn stop_all(&mut self, exclude: u32) -> Result<Vec<Event>, String> {
        self.validate()?;
        let mut events = vec![];
        if exclude & 0x10 == 0 {
            events.push(Event::MusicStop);
        }
        let preserved = if exclude & 0x80 != 0 && self.cue_id != 0 {
            audio_source_lookup::by_id(&self.active, &self.waiting, self.cue_id)?
        } else {
            None
        };
        for (queued, list) in [(false, &mut self.active), (true, &mut self.waiting)] {
            for (i, s) in list.iter_mut().enumerate() {
                let target = if queued {
                    Found::Queued(i)
                } else {
                    Found::Active(i)
                };
                if s.flags & exclude == 0 && Some(target) != preserved && s.sound != 0 {
                    stop(s, target, &mut events);
                    if !queued {
                        s.voice_exempt = false;
                    }
                }
            }
        }
        Ok(events)
    }
    pub fn stop_voices(&mut self, actor: u32, bank: &[Sound]) -> Result<Vec<Event>, String> {
        self.validate()?;
        validate_bank(bank)?;
        let mut next = self.clone();
        let mut events = vec![];
        for (i, s) in next.active.iter_mut().enumerate() {
            if s.actor == actor && sound(bank, s.sound)?.category == 1 {
                s.voice_exempt = false;
                stop(s, Found::Active(i), &mut events);
            }
        }
        *self = next;
        Ok(events)
    }
    pub fn empty_waiting(&mut self) -> Result<Vec<Event>, String> {
        self.validate()?;
        let events = self
            .waiting
            .iter()
            .enumerate()
            .map(|(i, s)| Event::Delete {
                target: Found::Queued(i),
                unique_id: s.unique_id,
            })
            .collect();
        self.waiting.clear();
        Ok(events)
    }
}
