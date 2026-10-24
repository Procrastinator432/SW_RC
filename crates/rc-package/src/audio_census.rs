//! Voice-cue refresh and the selected late Update census. Driver Update outputs
//! and Params flags are explicit inputs; no streaming driver is synthesized.
use crate::audio_source::Source;
use serde::{Deserialize, Serialize};
#[derive(Clone, Debug, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Status {
    pub pending: u32,
    pub current: u32,
    pub ducking_voices: u32,
    pub any_playing: bool,
}
#[derive(Clone, Debug, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Frame {
    pub playing: i32,
    pub seconds_bits: u32,
    pub after: Source,
}
#[derive(Clone, Copy, Debug, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Params {
    pub sound: u32,
    pub flags: u32,
}
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub enum Event {
    IsPlaying(usize),
    Update { index: usize, seconds_bits: u32 },
    Params { index: usize, sound: u32 },
}
impl Status {
    pub fn validate(&self) -> Result<(), String> {
        if self.ducking_voices > 4096 {
            return Err("Too many counted ducking voices".into());
        }
        Ok(())
    }
    /// Whole UpdateVoiceCueStatus probes IsPlaying twice for an initially
    /// playing source; retain changes between those replies and pending bits.
    pub fn refresh(
        &mut self,
        sources: &[Source],
        replies: &[[i32; 2]],
    ) -> Result<Vec<Event>, String> {
        self.validate()?;
        if sources.len() > 4096 || sources.len() != replies.len() {
            return Err("Cue reply/source mismatch".into());
        }
        for s in sources {
            s.validate()?;
        }
        let mut current = self.pending;
        let mut events = vec![];
        for (i, (s, r)) in sources.iter().zip(replies).enumerate() {
            events.push(Event::IsPlaying(i));
            if r[0] != 0 {
                events.push(Event::IsPlaying(i));
                if r[1] != 0 && s.voice_exempt {
                    current |= 1;
                }
            }
        }
        self.current = current;
        Ok(events)
    }
    /// Original 103d99cc..103d9a60, excluding surrounding Update orchestration.
    /// IsPlaying is sampled before Update; Params and cursor are read after it.
    pub fn recount(
        &mut self,
        sources: &mut [Source],
        frames: &[Frame],
        params: &[Params],
    ) -> Result<Vec<Event>, String> {
        self.validate()?;
        if sources.len() > 4096 || sources.len() != frames.len() || params.len() > 4096 {
            return Err("Census frame/source mismatch".into());
        }
        for s in sources.iter() {
            s.validate()?;
        }
        let mut handles = std::collections::BTreeSet::new();
        for p in params {
            if p.sound == 0 || !handles.insert(p.sound) {
                return Err("Invalid census sound params".into());
            }
        }
        for f in frames {
            f.after.validate()?;
            let seconds = f32::from_bits(f.seconds_bits);
            if !seconds.is_finite() || !(0. ..=60.).contains(&seconds) {
                return Err("Invalid census listener delta".into());
            }
        }
        let mut next = sources.to_vec();
        let mut current = self.pending;
        let mut voices = 0;
        let mut any = false;
        let mut events = vec![];
        for (i, (s, f)) in next.iter_mut().zip(frames).enumerate() {
            events.push(Event::IsPlaying(i));
            if f.playing == 0 {
                continue;
            }
            any = true;
            events.push(Event::Update {
                index: i,
                seconds_bits: f.seconds_bits,
            });
            *s = f.after.clone();
            if s.cursor != 0 && s.voice_exempt {
                current |= 1;
            }
            events.push(Event::Params {
                index: i,
                sound: s.sound,
            });
            let p = params
                .iter()
                .find(|p| p.sound == s.sound)
                .ok_or("Missing post-update sound params")?;
            if p.flags & 1 != 0 {
                voices += 1;
            }
        }
        sources.clone_from_slice(&next);
        self.current = current;
        self.ducking_voices = voices;
        self.any_playing = any;
        Ok(events)
    }
}
