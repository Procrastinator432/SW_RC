//! Bounded soundtrack transport and incremental stereo renderer for the mission adapter.
//! This is a port playback policy, not the original iMUSE battle scheduler.
use crate::{
    audio_curve::{Curve, PortableCurveMath},
    audio_vorbis::Provider,
};
use serde::{Deserialize, Serialize};
use std::{collections::VecDeque, sync::Arc};
pub const MAX_BANK: usize = 24 * 1024 * 1024;
pub const COMMAND_BYTES: usize = 80;
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Command {
    pub looped: bool,
    pub track: i32,
    pub activation: u64,
    pub elapsed: u64,
    pub fade_in: f32,
    pub fade_out: f32,
    pub in_kind: u8,
    pub out_kind: u8,
}
impl Default for Command {
    fn default() -> Self {
        Self {
            looped: false,
            track: -1,
            activation: 0,
            elapsed: 0,
            fade_in: 0.,
            fade_out: 0.,
            in_kind: 0,
            out_kind: 0,
        }
    }
}
impl Command {
    pub fn validate(&self, count: usize) -> Result<(), String> {
        if self.track < -1
            || self.track >= count as i32
            || self.activation > i64::MAX as u64
            || self.elapsed > 48000 * 60 * 60 * 24
            || !self.fade_in.is_finite()
            || !self.fade_out.is_finite()
            || !(0. ..=60.).contains(&self.fade_in)
            || !(0. ..=60.).contains(&self.fade_out)
            || self.in_kind > 3
            || self.out_kind > 3
        {
            return Err("Invalid soundtrack command".into());
        }
        Ok(())
    }
}
pub fn commands(bytes: &[u8], count: usize) -> Result<[Command; 2], String> {
    if bytes.len() != COMMAND_BYTES || &bytes[..8] != b"RCMUS001" {
        return Err("Invalid soundtrack packet".into());
    }
    let mut out = [Command::default(), Command::default()];
    for (i, c) in out.iter_mut().enumerate() {
        let p = 8 + i * 36;
        c.track = i32::from_le_bytes(bytes[p..p + 4].try_into().unwrap());
        c.activation = u64::from_le_bytes(bytes[p + 4..p + 12].try_into().unwrap());
        c.elapsed = u64::from_le_bytes(bytes[p + 12..p + 20].try_into().unwrap());
        c.fade_in = f32::from_le_bytes(bytes[p + 20..p + 24].try_into().unwrap());
        c.fade_out = f32::from_le_bytes(bytes[p + 24..p + 28].try_into().unwrap());
        let a = u32::from_le_bytes(bytes[p + 28..p + 32].try_into().unwrap());
        let b = u32::from_le_bytes(bytes[p + 32..p + 36].try_into().unwrap());
        if a & !0x103 != 0 || b > 3 {
            return Err("Invalid soundtrack curve".into());
        }
        c.looped = a & 0x100 != 0;
        c.in_kind = (a & 3) as u8;
        c.out_kind = b as u8;
        c.validate(count)?;
    }
    Ok(out)
}
pub fn packet(commands: &[Command; 2]) -> Vec<u8> {
    let mut out = b"RCMUS001".to_vec();
    for c in commands {
        out.extend(c.track.to_le_bytes());
        out.extend(c.activation.to_le_bytes());
        out.extend(c.elapsed.to_le_bytes());
        out.extend(c.fade_in.to_le_bytes());
        out.extend(c.fade_out.to_le_bytes());
        out.extend((u32::from(c.in_kind) | if c.looped { 0x100 } else { 0 }).to_le_bytes());
        out.extend(u32::from(c.out_kind).to_le_bytes());
    }
    out
}
/// Full parsing is done before opening any physical device.
pub fn bank(bytes: &[u8]) -> Result<Vec<Arc<[u8]>>, String> {
    if bytes.len() < 12 || bytes.len() > MAX_BANK || &bytes[..8] != b"RCMUB001" {
        return Err("Invalid soundtrack bank".into());
    }
    let count = u32::from_le_bytes(bytes[8..12].try_into().unwrap()) as usize;
    if count > 24 {
        return Err("Soundtrack count".into());
    }
    let mut at = 12;
    let mut tracks = vec![];
    for _ in 0..count {
        let n = u32::from_le_bytes(
            bytes
                .get(at..at + 4)
                .ok_or("Truncated soundtrack")?
                .try_into()
                .unwrap(),
        ) as usize;
        at += 4;
        if n == 0 || n > 4 * 1024 * 1024 {
            return Err("Soundtrack resource budget".into());
        }
        let raw = bytes.get(at..at + n).ok_or("Truncated soundtrack")?;
        at += n;
        let p = Provider::new(raw.to_vec())?;
        if p.metadata().frequency > 96000 || p.metadata().frames == 0 {
            return Err("Soundtrack format".into());
        }
        tracks.push(Arc::from(raw));
    }
    if at != bytes.len() {
        return Err("Trailing soundtrack bytes".into());
    }
    Ok(tracks)
}
#[derive(Clone, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct SavedVoice {
    command: Command,
    position: u64,
    looped: bool,
    curve: Curve,
    retiring: bool,
    channel: usize,
}
#[derive(Clone, Default, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Saved {
    pub desired: [Command; 2],
    voices: Vec<SavedVoice>,
    pub duck: f32,
}
struct Voice {
    saved: SavedVoice,
    provider: Provider,
    samples: VecDeque<[f32; 2]>,
    first: u64,
}
impl Voice {
    fn new(raw: &[u8], saved: SavedVoice) -> Result<Self, String> {
        let mut provider = Provider::new(raw.to_vec())?;
        let meta = provider.metadata();
        let frame = saved.position * u64::from(meta.frequency) / 48000;
        let frame = if saved.looped {
            frame % meta.frames
        } else {
            frame.min(meta.frames)
        };
        provider.seek_byte(frame * u64::from(meta.channels) * 2)?;
        Ok(Self {
            saved,
            provider,
            samples: VecDeque::new(),
            first: frame,
        })
    }
    fn sample(&mut self) -> Result<Option<[f32; 2]>, String> {
        let meta = self.provider.metadata().clone();
        let pos = self.saved.position * u64::from(meta.frequency);
        let frame = pos / 48000;
        let frame = if self.saved.looped {
            frame % meta.frames
        } else {
            frame
        };
        if frame >= meta.frames {
            return Ok(None);
        }
        if frame < self.first {
            self.provider.seek_byte(0)?;
            self.samples.clear();
            self.first = 0;
        }
        loop {
            while self.samples.len() < 2 || self.first < frame {
                if self.samples.len() > 1 && self.first < frame {
                    self.samples.pop_front();
                    self.first += 1;
                    continue;
                }
                let bytes = self.provider.read_bytes(8192)?;
                if bytes.is_empty() {
                    if self.saved.looped {
                        self.provider.seek_byte(0)?;
                    } else {
                        let last = self.samples.back().copied().unwrap_or([0.; 2]);
                        self.samples.push_back(last);
                        break;
                    }
                } else {
                    for f in bytes.chunks_exact(usize::from(meta.channels) * 2) {
                        let l = f32::from(i16::from_le_bytes([f[0], f[1]]));
                        let r = if meta.channels == 2 {
                            f32::from(i16::from_le_bytes([f[2], f[3]]))
                        } else {
                            l
                        };
                        self.samples.push_back([l, r]);
                    }
                }
            }
            if self.first == frame {
                break;
            }
        }
        let a = self.samples[0];
        let b = self.samples.get(1).copied().unwrap_or(a);
        let t = (pos % 48000) as f32 / 48000.;
        self.saved.position += 1;
        Ok(Some([a[0] + (b[0] - a[0]) * t, a[1] + (b[1] - a[1]) * t]))
    }
}
pub struct Player {
    tracks: Vec<Arc<[u8]>>,
    voices: Vec<Voice>,
    desired: [Command; 2],
    duck: f32,
}
impl Player {
    pub fn new(tracks: Vec<Arc<[u8]>>) -> Self {
        Self {
            tracks,
            voices: vec![],
            desired: [Command::default(), Command::default()],
            duck: 1.,
        }
    }
    pub fn count(&self) -> usize {
        self.tracks.len()
    }
    pub fn update(&mut self, next: [Command; 2]) -> Result<(), String> {
        for c in &next {
            c.validate(self.tracks.len())?;
        }
        for (channel, c) in next.iter().enumerate() {
            if self.desired[channel].track == c.track
                && self.desired[channel].activation == c.activation
            {
                continue;
            }
            for v in &mut self.voices {
                if v.saved.channel == channel && !v.saved.retiring {
                    v.saved.retiring = true;
                    v.saved
                        .curve
                        .set(0., c.fade_out, c.out_kind, &mut PortableCurveMath)?;
                }
            }
            if c.track >= 0 {
                let mut curve = Curve::new(if c.fade_in == 0. { 1. } else { 0. })?;
                curve.set(1., c.fade_in, c.in_kind, &mut PortableCurveMath)?;
                let saved = SavedVoice {
                    command: c.clone(),
                    position: c.elapsed,
                    looped: c.looped,
                    curve,
                    retiring: false,
                    channel,
                };
                self.voices
                    .push(Voice::new(&self.tracks[c.track as usize], saved)?);
            }
            self.desired[channel] = c.clone();
        }
        // Latest outgoing voice per lane suffices for rapid user/map changes.
        for ch in 0..2 {
            while self.voices.iter().filter(|v| v.saved.channel == ch).count() > 2 {
                let i = self
                    .voices
                    .iter()
                    .position(|v| v.saved.channel == ch && v.saved.retiring)
                    .ok_or("Soundtrack voice budget")?;
                self.voices.remove(i);
            }
        }
        Ok(())
    }
    /// Music stays on its timeline even when muted. Smooth speech ducking is a
    /// documented mission policy, independent of unrecovered original ducker flags.
    pub fn mix(&mut self, samples: &mut [i16], gain: f32, speech: bool) -> Result<(), String> {
        if samples.is_empty()
            || !samples.len().is_multiple_of(2)
            || samples.len() > 960
            || !gain.is_finite()
            || !(0. ..=1.).contains(&gain)
        {
            return Err("Invalid music render block".into());
        }
        let target = if speech { 0.5 } else { 1. };
        let step = 0.5 / 24000.;
        let mut ended = vec![false; self.voices.len()];
        let count = samples.len() / 2;
        let mut fades = vec![];
        for v in &mut self.voices {
            let start = v.saved.curve.value(&mut PortableCurveMath)?;
            let end = v
                .saved
                .curve
                .step(count as f32 / 48000., &mut PortableCurveMath)?;
            fades.push((start, end));
        }
        for (at, frame) in samples.chunks_exact_mut(2).enumerate() {
            self.duck += (target - self.duck).clamp(-step, step);
            let mut music = [0.; 2];
            for (i, v) in self.voices.iter_mut().enumerate() {
                let (start, end) = fades[i];
                let f = start + (end - start) * (at + 1) as f32 / count as f32;
                if v.saved.retiring && f == 0. {
                    ended[i] = true;
                    continue;
                }
                if let Some(s) = v.sample()? {
                    for k in 0..2 {
                        music[k] += s[k] * f;
                    }
                } else {
                    ended[i] = true;
                }
            }
            for k in 0..2 {
                frame[k] = (f32::from(frame[k]) + music[k] * gain * self.duck)
                    .round()
                    .clamp(-32768., 32767.) as i16;
            }
        }
        let mut i = 0;
        self.voices.retain(|_| {
            let keep = !ended[i];
            i += 1;
            keep
        });
        Ok(())
    }
    pub fn saved(&self) -> Saved {
        Saved {
            desired: self.desired.clone(),
            voices: self.voices.iter().map(|v| v.saved.clone()).collect(),
            duck: self.duck,
        }
    }
    pub fn restore(&mut self, s: Saved) -> Result<(), String> {
        if s.voices.len() > 4 || !s.duck.is_finite() || !(0.5..=1.).contains(&s.duck) {
            return Err("Invalid saved soundtrack".into());
        }
        for ch in 0..2 {
            if s.voices.iter().filter(|v| v.channel == ch).count() > 2
                || s.voices
                    .iter()
                    .filter(|v| v.channel == ch && !v.retiring)
                    .count()
                    > 1
            {
                return Err("Saved music channel budget".into());
            }
        }
        for c in &s.desired {
            c.validate(self.tracks.len())?;
        }
        let mut voices = vec![];
        for v in &s.voices {
            v.command.validate(self.tracks.len())?;
            v.curve.validate()?;
            if v.command.track < 0
                || v.channel > 1
                || v.position > 48000 * 60 * 60 * 24
                || v.looped != v.command.looped
            {
                return Err("Invalid saved music voice".into());
            }
            voices.push(Voice::new(
                &self.tracks[v.command.track as usize],
                v.clone(),
            )?);
        }
        self.voices = voices;
        self.desired = s.desired;
        self.duck = s.duck;
        Ok(())
    }
}
#[cfg(test)]
mod tests {
    use super::*;
    const OGG: &[u8] = include_bytes!("../../../fixtures/audio-vorbis/stereo.ogg");
    fn player() -> Player {
        Player::new(vec![Arc::from(OGG)])
    }
    fn start() -> [Command; 2] {
        [
            Command {
                looped: true,
                track: 0,
                activation: 1,
                ..Default::default()
            },
            Command::default(),
        ]
    }
    #[test]
    fn bounded_bank_checks_ogg_and_trailing_bytes() {
        let raw = [
            b"RCMUB001".as_slice(),
            &1u32.to_le_bytes(),
            &(OGG.len() as u32).to_le_bytes(),
            OGG,
        ]
        .concat();
        assert_eq!(bank(&raw).unwrap().len(), 1);
        for n in [0, 8, 11, 16, raw.len() - 1] {
            assert!(bank(&raw[..n]).is_err());
        }
        let mut bad = raw.clone();
        bad.push(0);
        assert!(bank(&bad).is_err());
        let mut bad = raw;
        *bad.last_mut().unwrap() ^= 1;
        assert!(bank(&bad).is_err());
    }
    #[test]
    fn command_validation_is_complete() {
        let c = start();
        let p = packet(&c);
        assert_eq!(commands(&p, 1).unwrap(), c);
        assert!(commands(&p, 0).is_err());
        for at in [8, 28, 32, 36, 40] {
            let mut bad = p.clone();
            bad[at..at + 4]
                .copy_from_slice(&(if at == 8 { i32::MAX as u32 } else { u32::MAX }).to_le_bytes());
            assert!(commands(&bad, 1).is_err());
        }
        assert!(commands(&p[..79], 1).is_err());
    }
    #[test]
    fn streamed_pcm_mute_clock_and_cold_suffix() {
        let mut p = player();
        p.update(start()).unwrap();
        let mut silent = vec![0; 960];
        p.mix(&mut silent, 0., false).unwrap();
        assert!(silent.iter().all(|v| *v == 0));
        let saved = p.saved();
        let mut q = player();
        q.restore(saved).unwrap();
        for _ in 0..30 {
            let mut a = vec![0; 960];
            let mut b = a.clone();
            p.mix(&mut a, 0.5, false).unwrap();
            q.mix(&mut b, 0.5, false).unwrap();
            assert_eq!(a, b);
            assert!(a.iter().any(|v| *v != 0));
        }
    }
    #[test]
    fn duplicate_pose_does_not_restart_and_crossfades_are_bounded() {
        let mut p = player();
        p.update(start()).unwrap();
        p.mix(&mut vec![0; 960], 0.5, false).unwrap();
        let before = p.saved().voices[0].position;
        let mut c = start();
        c[0].elapsed = 0;
        p.update(c.clone()).unwrap();
        assert_eq!(p.saved().voices[0].position, before);
        for id in 2..20 {
            c[0].activation = id;
            c[0].fade_in = 1.;
            c[0].fade_out = 1.;
            p.update(c.clone()).unwrap();
        }
        assert_eq!(p.voices.len(), 2);
        let mut q = player();
        q.restore(p.saved()).unwrap();
        assert_eq!(q.voices.len(), 2);
    }
    #[test]
    fn speech_ducking_attacks_and_recovers_even_when_muted() {
        let mut p = player();
        p.update(start()).unwrap();
        for _ in 0..60 {
            p.mix(&mut vec![0; 960], 0., true).unwrap();
        }
        assert_eq!(p.duck, 0.5);
        for _ in 0..60 {
            p.mix(&mut vec![0; 960], 0., false).unwrap();
        }
        assert_eq!(p.duck, 1.);
    }
    #[test]
    fn invalid_restore_is_transactional_and_never_indexes_bad_track() {
        let mut p = player();
        p.update(start()).unwrap();
        let before = serde_json::to_vec(&p.saved()).unwrap();
        let mut s = p.saved();
        s.voices[0].command.track = 999;
        assert!(p.restore(s).is_err());
        assert_eq!(before, serde_json::to_vec(&p.saved()).unwrap());
        let mut s = p.saved();
        s.duck = f32::NAN;
        assert!(p.restore(s).is_err());
    }
}
