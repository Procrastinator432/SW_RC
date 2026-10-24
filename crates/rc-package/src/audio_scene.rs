//! Mission WAV adapter on the shared native renderer. No JSON snapshots on its hot path.
//! Scene/save state owns desired loops and event cursors; a new audio session rebuilds those.
use crate::{
    audio_device::{Backend, Executor, Kind, Op},
    audio_pcm::Packet,
    audio_wave::Pcm,
};
use serde::{Deserialize, Serialize};
use std::collections::{BTreeSet, VecDeque};
#[cfg(test)]
#[path = "audio_scene_resume_tests.rs"]
mod resume_tests;
pub const MAX_BANK: usize = 8 * 1024 * 1024 + 1024;
pub const MAX_RESUME: usize = 256 * 1024;
fn unity() -> f32 {
    1.
}
fn fingerprint(bytes: &[u8]) -> u64 {
    bytes.iter().fold(0xcbf29ce484222325, |h, b| {
        (h ^ u64::from(*b)).wrapping_mul(0x100000001b3)
    })
}
#[derive(Clone, Default, Serialize, Deserialize)]
#[serde(default, deny_unknown_fields)]
struct PoolStats {
    admitted: u64,
    protected_rejected: u64,
    capacity_rejected: u64,
    preempted: u64,
    completed: u64,
}
impl PoolStats {
    fn values(&self) -> [u64; 5] {
        [
            self.admitted,
            self.protected_rejected,
            self.capacity_rejected,
            self.preempted,
            self.completed,
        ]
    }
    fn inc(value: &mut u64) {
        *value = value.saturating_add(1).min(i64::MAX as u64);
    }
}
#[derive(Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
struct Resume {
    #[serde(default = "unity")]
    effects_duck: f32,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    music: Option<crate::mission_music::Saved>,
    #[serde(default)]
    stats: PoolStats,
    schema: u32,
    bank: u64,
    frames: u64,
    last: u64,
    last_played: Option<(u64, u64)>,
    age: u64,
    pose: Option<([f32; 3], [f32; 3], [f32; 3])>,
    pending: Vec<Event>,
    dialogs: Vec<(Voice, u32)>,
    loops: Vec<(Loop, u32)>,
}
#[derive(Clone)]
struct Sound {
    frames: u32,
    normal_mono: bool,
    normal: u32,
    spatial: u32,
}
#[derive(Clone, Debug, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
struct Event {
    #[serde(default)]
    no_override: bool,
    sequence: u64,
    tick: u64,
    sound: usize,
    owner: u32,
    category: u32,
    volume: f32,
    pitch: f32,
}
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
struct Loop {
    sound: usize,
    owner: u32,
    location: [f32; 3],
    volume: f32,
    pitch: f32,
    radius: f32,
    flags: u32,
    activation: u64,
}
#[derive(Clone, Debug, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
struct Voice {
    #[serde(default)]
    no_override: bool,
    volume: f32,
    pitch: f32,
    sound: usize,
    owner: u32,
    category: u32,
    activation: Option<u64>,
    spatial: bool,
    age: u64,
}
struct Reader<'a> {
    bytes: &'a [u8],
    at: usize,
}
impl<'a> Reader<'a> {
    fn new(bytes: &'a [u8], magic: &[u8]) -> Result<Self, String> {
        if bytes.len() < magic.len() || &bytes[..magic.len()] != magic {
            return Err("Audio packet magic".into());
        }
        Ok(Self {
            bytes,
            at: magic.len(),
        })
    }
    fn take<const N: usize>(&mut self) -> Result<[u8; N], String> {
        let end = self.at.checked_add(N).ok_or("Audio length")?;
        let v = self
            .bytes
            .get(self.at..end)
            .ok_or("Truncated audio packet")?
            .try_into()
            .unwrap();
        self.at = end;
        Ok(v)
    }
    fn u32(&mut self) -> Result<u32, String> {
        Ok(u32::from_le_bytes(self.take()?))
    }
    fn u64(&mut self) -> Result<u64, String> {
        let n = u64::from_le_bytes(self.take()?);
        if n > i64::MAX as u64 {
            return Err("Audio counter outside Java range".into());
        }
        Ok(n)
    }
    fn float(&mut self, low: f32, high: f32) -> Result<f32, String> {
        let f = f32::from_bits(self.u32()?);
        if !f.is_finite() || f < low || f > high {
            return Err("Audio scalar out of range".into());
        }
        Ok(f)
    }
    fn point(&mut self) -> Result<[f32; 3], String> {
        Ok([
            self.float(-1e7, 1e7)?,
            self.float(-1e7, 1e7)?,
            self.float(-1e7, 1e7)?,
        ])
    }
    fn end(&self) -> Result<(), String> {
        if self.at != self.bytes.len() {
            Err("Trailing audio bytes".into())
        } else {
            Ok(())
        }
    }
}
/// Validate the complete immutable bank before any device IO.
pub fn bank(bytes: &[u8]) -> Result<Vec<Pcm>, String> {
    let (bytes, _) = split_bank(bytes)?;
    if bytes.len() > MAX_BANK {
        return Err("Mission sound bank too large".into());
    }
    let mut r = Reader::new(bytes, b"RCSND001")?;
    let count = r.u32()? as usize;
    if count > 128 {
        return Err("Mission sound count".into());
    }
    let mut result = Vec::with_capacity(count);
    for _ in 0..count {
        let size = r.u32()? as usize;
        if !(44..=2 * 1024 * 1024).contains(&size) {
            return Err("Mission WAV size".into());
        }
        let end = r.at.checked_add(size).ok_or("WAV size")?;
        let pcm = Pcm::from_wave(r.bytes.get(r.at..end).ok_or("Truncated mission WAV")?)?;
        if pcm.bytes.is_empty() {
            return Err("Empty mission WAV".into());
        }
        result.push(pcm);
        r.at = end;
    }
    r.end()?;
    Ok(result)
}
/// RCSND002 embeds an unchanged WAV bank followed by a compressed soundtrack bank.
pub fn split_bank(bytes: &[u8]) -> Result<(&[u8], Option<&[u8]>), String> {
    if bytes.starts_with(b"RCSND002") {
        if bytes.len() < 24 || bytes.len() > MAX_BANK + crate::mission_music::MAX_BANK + 12 {
            return Err("Combined mission bank size".into());
        }
        let n = u32::from_le_bytes(bytes[8..12].try_into().unwrap()) as usize;
        let wav = bytes.get(12..12 + n).ok_or("Truncated WAV bank")?;
        if !wav.starts_with(b"RCSND001") {
            return Err("Invalid nested WAV bank".into());
        }
        let music = bytes.get(12 + n..).ok_or("Missing soundtrack bank")?;
        Ok((wav, Some(music)))
    } else {
        Ok((bytes, None))
    }
}
/// A read-only desired music state accompanies the authoritative listener/loops.
fn split_loops(bytes: &[u8]) -> Result<(&[u8], Option<&[u8]>), String> {
    if bytes.starts_with(b"RCLOOP05") {
        if bytes.len() < 12 {
            return Err("Truncated music pose".into());
        }
        let n = u32::from_le_bytes(bytes[8..12].try_into().unwrap()) as usize;
        let pose = bytes.get(12..12 + n).ok_or("Truncated music pose")?;
        if !pose.starts_with(b"RCLOOP04") {
            return Err("Invalid music pose".into());
        }
        Ok((pose, Some(&bytes[12 + n..])))
    } else {
        Ok((bytes, None))
    }
}
fn events(bytes: &[u8], count: usize, last: u64) -> Result<Vec<Event>, String> {
    let mut r = Reader::new(bytes, b"RCAUD003")?;
    let n = r.u32()?;
    if n > 256 {
        return Err("Mission event count".into());
    }
    let mut result = vec![];
    let mut sequence = last;
    for _ in 0..n {
        let next = r.u64()?;
        let tick = r.u64()?;
        let sound = r.u32()? as usize;
        let owner = r.u32()?;
        let category = r.u32()?;
        let flags = r.u32()?;
        let volume = r.float(0., 1.)?;
        let pitch = r.float(0.5, 2.)?;
        let radius = r.float(0., 1e6)?;
        let original = r.u32()?;
        if next <= sequence
            || sound >= count
            || category > 2
            || flags > 1
            || original > 1
            || (original == 0 && (volume != 1. || pitch != 1. || radius != 0.))
            || (original == 1 && volume <= 0.0001)
        {
            return Err("Invalid mission event".into());
        }
        result.push(Event {
            no_override: flags != 0,
            sequence: next,
            tick,
            sound,
            owner,
            category,
            volume,
            pitch,
        });
        sequence = next;
    }
    r.end()?;
    Ok(result)
}
struct Loops {
    listener: [f32; 3],
    yaw: f32,
    pitch: f32,
    voices: Vec<Loop>,
}
fn loops(bytes: &[u8], count: usize) -> Result<Loops, String> {
    let pitched = bytes.starts_with(b"RCLOOP04");
    let mut r = Reader::new(bytes, if pitched { b"RCLOOP04" } else { b"RCLOOP03" })?;
    let n = r.u32()?;
    if n > 16 {
        return Err("Mission loop count".into());
    }
    let listener = r.point()?;
    let yaw = r.float(-1e7, 1e7)?;
    let pitch = if pitched { r.float(-1.571, 1.571)? } else { 0. };
    let mut voices = vec![];
    let mut owners = BTreeSet::new();
    for _ in 0..n {
        let sound = r.u32()? as usize;
        let owner = r.u32()?;
        let location = r.point()?;
        let volume = r.float(0., 1.)?;
        let pitch = r.float(0.5, 2.)?;
        let radius = r.float(0., 1e6)?;
        let flags = r.u32()?;
        let activation = r.u64()?;
        if sound >= count
            || owner == 0
            || owner > i32::MAX as u32
            || !owners.insert(owner)
            || flags & !7 != 0
            || (flags & 1 == 0 && (flags != 0 || volume != 1. || pitch != 1. || radius != 0.))
            || (flags != 0 && volume <= 0.0001)
        {
            return Err("Invalid mission loop".into());
        }
        voices.push(Loop {
            sound,
            owner,
            location,
            volume,
            pitch,
            radius,
            flags,
            activation,
        });
    }
    r.end()?;
    Ok(Loops {
        listener,
        yaw,
        pitch,
        voices,
    })
}
fn position(v: [f32; 3]) -> [u32; 3] {
    [v[0] * 0.01, v[2] * 0.01, -v[1] * 0.01].map(f32::to_bits)
}
pub struct Scene<B: Backend> {
    effects_duck: f32,
    music: crate::mission_music::Player,
    music_gain: f32,
    stats: PoolStats,
    mix: [f32; 4],
    restored: bool,
    bank_fingerprint: u64,
    resume_loops: Vec<(Loop, u32)>,
    last_loops: Vec<Loop>,
    last_pose: Option<([f32; 3], [f32; 3], [f32; 3])>,
    device: Executor<B>,
    sounds: Vec<Sound>,
    voices: Vec<Option<Voice>>,
    pending: VecDeque<Event>,
    last: u64,
    epoch: u64,
    sequence: u64,
    frames: u64,
    last_played: Option<(u64, u64)>,
    age: u64,
    fault: bool,
}
impl<B: Backend> Scene<B> {
    pub fn open(backend: B, bytes: &[u8], epoch: u64) -> Result<Self, String> {
        let pcm = bank(bytes)?;
        let music = split_bank(bytes)?
            .1
            .map(crate::mission_music::bank)
            .transpose()?
            .unwrap_or_default();
        if epoch == 0 || epoch > i64::MAX as u64 {
            return Err("Invalid scene audio epoch".into());
        }
        let mut device = Executor::new(backend, epoch)?;
        for source in 1..=16 {
            device.create(Kind::Source, source)?;
        }
        let mut sounds = vec![];
        for (i, p) in pcm.into_iter().enumerate() {
            let normal = 1000 + i as u32;
            let spatial = 2000 + i as u32;
            device.create(Kind::Buffer, normal)?;
            device.create(Kind::Buffer, spatial)?;
            let mono = if p.info.channels == 1 {
                p.bytes.clone()
            } else if p.info.bits == 16 {
                p.bytes
                    .chunks_exact(4)
                    .map(|b| {
                        ((i32::from(i16::from_le_bytes([b[0], b[1]]))
                            + i32::from(i16::from_le_bytes([b[2], b[3]])))
                            / 2) as i16
                    })
                    .flat_map(i16::to_le_bytes)
                    .collect()
            } else {
                p.bytes
                    .chunks_exact(2)
                    .map(|b| ((u16::from(b[0]) + u16::from(b[1])) / 2) as u8)
                    .collect()
            };
            let ops = [
                Op::Upload {
                    buffer: normal,
                    format: p.format(),
                    frequency: p.info.frequency,
                    bytes: p.bytes,
                },
                Op::Upload {
                    buffer: spatial,
                    format: if p.info.bits == 16 { 0x1101 } else { 0x1100 },
                    frequency: p.info.frequency,
                    bytes: mono,
                },
            ];
            device.apply(&device.prepare(&ops)?).map_err(|e| e.reason)?;
            sounds.push(Sound {
                frames: p.info.data_len / u32::from(p.info.block_align),
                normal_mono: p.info.channels == 1,
                normal,
                spatial,
            });
        }
        Ok(Self {
            effects_duck: 1.,
            music: crate::mission_music::Player::new(music),
            music_gain: 1.,
            stats: PoolStats::default(),
            mix: [1.; 4],
            restored: false,
            bank_fingerprint: fingerprint(bytes),
            resume_loops: vec![],
            last_loops: vec![],
            last_pose: None,
            device,
            sounds,
            voices: vec![None; 16],
            pending: VecDeque::new(),
            last: 0,
            epoch,
            sequence: 0,
            frames: 0,
            last_played: None,
            age: 0,
            fault: false,
        })
    }
    fn apply(&mut self, ops: &[Op]) -> Result<(), String> {
        self.device
            .apply(&self.device.prepare(ops)?)
            .map(|_| ())
            .map_err(|e| e.reason)
    }
    fn gain(&self, sound: usize, volume: f32, spatial: bool, looped: bool, category: u32) -> f32 {
        let group = if looped {
            3
        } else if category == 1 {
            2
        } else {
            1
        };
        0.35 * volume
            * if !spatial && self.sounds[sound].normal_mono {
                std::f32::consts::SQRT_2
            } else {
                1.
            }
            * self.mix[0]
            * self.mix[group]
            * if group == 2 { 1. } else { self.effects_duck }
    }
    /// Platform volume controls, independent of original source parameters and RAM snapshots.
    pub fn set_mix(&mut self, bytes: &[u8]) -> Result<(), String> {
        if self.fault
            || !(bytes.len() == 24 && &bytes[..8] == b"RCMIX001"
                || bytes.len() == 28 && &bytes[..8] == b"RCMIX002")
        {
            return Err("Invalid mission mix packet".into());
        }
        let mut next = [0.; 4];
        for (i, gain) in next.iter_mut().enumerate() {
            *gain = f32::from_le_bytes(bytes[8 + i * 4..12 + i * 4].try_into().unwrap());
            if !gain.is_finite() || !(0. ..=1.).contains(gain) {
                return Err("Invalid mission mix gain".into());
            }
        }
        let music_gain = if bytes.len() == 28 {
            f32::from_le_bytes(bytes[24..28].try_into().unwrap())
        } else {
            1.
        };
        if !music_gain.is_finite() || !(0. ..=1.).contains(&music_gain) {
            return Err("Invalid music gain".into());
        }
        if next == self.mix && music_gain == self.music_gain {
            return Ok(());
        }
        self.mix = next;
        self.music_gain = music_gain;
        let ops: Vec<_> = self
            .voices
            .iter()
            .enumerate()
            .filter_map(|(i, voice)| {
                voice.as_ref().map(|v| Op::Float {
                    source: i as u32 + 1,
                    parameter: 0x100a,
                    value: self
                        .gain(
                            v.sound,
                            v.volume,
                            v.spatial,
                            v.activation.is_some(),
                            v.category,
                        )
                        .to_bits(),
                })
            })
            .collect();
        if let Err(error) = self.apply(&ops) {
            self.fault = true;
            return Err(error);
        }
        Ok(())
    }
    fn stop(&mut self, slot: usize) -> Result<(), String> {
        self.apply(&[
            Op::Stop(slot as u32 + 1),
            Op::Integer {
                source: slot as u32 + 1,
                parameter: 0x1009,
                value: 0,
            },
        ])?;
        self.voices[slot] = None;
        Ok(())
    }
    fn reap(&mut self) -> Result<(), String> {
        for i in 0..16 {
            if self.voices[i]
                .as_ref()
                .is_some_and(|v| v.activation.is_none())
                && self.device.observe(i as u32 + 1, 0x1010)? == 0x1014
            {
                self.stop(i)?;
                PoolStats::inc(&mut self.stats.completed);
            }
        }
        Ok(())
    }
    fn slot(&mut self) -> Result<Option<usize>, String> {
        if let Some(i) = self.voices.iter().position(Option::is_none) {
            return Ok(Some(i));
        }
        let i = self
            .voices
            .iter()
            .enumerate()
            .filter_map(|(i, v)| {
                v.as_ref()
                    .filter(|v| v.activation.is_none())
                    .map(|v| (i, v.age))
            })
            .min_by_key(|(_, age)| *age)
            .map(|(i, _)| i);
        let Some(i) = i else {
            return Ok(None);
        };
        self.stop(i)?;
        PoolStats::inc(&mut self.stats.preempted);
        Ok(Some(i))
    }
    /// Mission intelligibility policy: ordinary effects cannot evict speech;
    /// speech chooses an effect before an unprotected older dialogue.
    fn event_slot(&mut self, category: u32) -> Result<Option<usize>, String> {
        if let Some(i) = self.voices.iter().position(Option::is_none) {
            return Ok(Some(i));
        }
        let candidate = self
            .voices
            .iter()
            .enumerate()
            .filter_map(|(i, v)| {
                let v = v.as_ref()?;
                if v.activation.is_some() || v.category == 1 && (category != 1 || v.no_override) {
                    return None;
                }
                Some((i, u8::from(v.category == 1), v.age))
            })
            .min_by_key(|(_, speech, age)| (*speech, *age))
            .map(|(i, _, _)| i);
        if let Some(i) = candidate {
            self.stop(i)?;
            PoolStats::inc(&mut self.stats.preempted);
        }
        Ok(candidate)
    }
    fn duck_effects(&mut self, frames: usize, speech: bool) -> Result<(), String> {
        let target = if speech { 0.25 } else { 1. };
        let next = self.effects_duck
            + (target - self.effects_duck).clamp(
                -(0.75 * frames as f32 / 24000.),
                0.75 * frames as f32 / 24000.,
            );
        if next == self.effects_duck {
            return Ok(());
        }
        self.effects_duck = next;
        let ops: Vec<_> = self
            .voices
            .iter()
            .enumerate()
            .filter_map(|(i, v)| {
                let v = v.as_ref()?;
                if v.category == 1 && v.activation.is_none() {
                    return None;
                }
                Some(Op::Float {
                    source: i as u32 + 1,
                    parameter: 0x100a,
                    value: self
                        .gain(
                            v.sound,
                            v.volume,
                            v.spatial,
                            v.activation.is_some(),
                            v.category,
                        )
                        .to_bits(),
                })
            })
            .collect();
        self.apply(&ops)
    }
    #[allow(clippy::too_many_arguments)]
    fn params(
        &mut self,
        slot: usize,
        sound: usize,
        volume: f32,
        pitch: f32,
        spatial: bool,
        location: [f32; 3],
        radius: f32,
        no_distance: bool,
        start: bool,
        looped: bool,
        category: u32,
    ) -> Result<(), String> {
        let source = slot as u32 + 1;
        let mut ops = vec![];
        if start {
            ops.push(Op::Integer {
                source,
                parameter: 0x1009,
                value: if spatial {
                    self.sounds[sound].spatial
                } else {
                    self.sounds[sound].normal
                },
            });
        }
        ops.extend([
            Op::Float {
                source,
                parameter: 0x100a,
                value: self
                    .gain(sound, volume, spatial, looped, category)
                    .to_bits(),
            },
            Op::Float {
                source,
                parameter: 0x1003,
                value: pitch.to_bits(),
            },
            Op::Integer {
                source,
                parameter: 0x1007,
                value: u32::from(looped),
            },
            Op::Integer {
                source,
                parameter: 0x202,
                value: u32::from(!spatial),
            },
            Op::Vector {
                source,
                parameter: 0x1004,
                value: if spatial { position(location) } else { [0; 3] },
            },
            Op::Float {
                source,
                parameter: 0x1020,
                value: (radius * 0.01).max(0.00001).to_bits(),
            },
            Op::Float {
                source,
                parameter: 0x1021,
                value: if no_distance || radius == 0. || !spatial {
                    0f32
                } else {
                    1f32
                }
                .to_bits(),
            },
        ]);
        if start {
            ops.push(Op::Play(source));
        }
        self.apply(&ops)
    }
    /// Both packets are fully validated before changing any listener/source state.
    pub fn update(&mut self, event_bytes: &[u8], loop_bytes: &[u8]) -> Result<(), String> {
        if self.fault {
            return Err("Scene audio faulted".into());
        }
        let events = events(event_bytes, self.sounds.len(), self.last)?;
        let (loop_bytes, music_bytes) = split_loops(loop_bytes)?;
        let loops = loops(loop_bytes, self.sounds.len())?;
        let music = music_bytes
            .map(|b| crate::mission_music::commands(b, self.music.count()))
            .transpose()?;
        if self.pending.len() + events.len() > 512 {
            return Err("Mission event queue full".into());
        }
        let result = (|| {
            if let Some(commands) = music {
                self.music.update(commands)?;
            }
            self.reap()?;
            let (sy, cy) = loops.yaw.sin_cos();
            let (sp, cp) = loops.pitch.sin_cos();
            let forward = [cp * cy, sp, -cp * sy];
            let up = [-sp * cy, cp, sp * sy];
            let pose = (position(loops.listener).map(f32::from_bits), forward, up);
            if self.last_pose != Some(pose) {
                self.device.listener(pose.0, pose.1, pose.2)?;
                self.last_pose = Some(pose);
            }
            let desired = loops.voices.clone();
            for i in 0..16 {
                if self.voices[i].as_ref().is_some_and(|v| {
                    v.activation.is_some()
                        && !loops.voices.iter().any(|l| {
                            l.owner == v.owner
                                && l.sound == v.sound
                                && Some(l.activation) == v.activation
                                && v.spatial == (l.flags & 1 != 0 && l.flags & 2 == 0)
                        })
                }) {
                    self.stop(i)?;
                }
            }
            for l in loops.voices {
                let spatial = l.flags & 1 != 0 && l.flags & 2 == 0;
                let existing = self.voices.iter().position(|v| {
                    v.as_ref().is_some_and(|v| {
                        v.activation == Some(l.activation)
                            && v.owner == l.owner
                            && v.sound == l.sound
                    })
                });
                let start = existing.is_none();
                let i = if let Some(i) = existing {
                    i
                } else {
                    self.slot()?.ok_or("No slot for desired mission loop")?
                };
                if start || self.last_loops.iter().find(|v| v.owner == l.owner) != Some(&l) {
                    self.params(
                        i,
                        l.sound,
                        l.volume,
                        l.pitch,
                        spatial,
                        l.location,
                        l.radius,
                        l.flags & 4 != 0,
                        start,
                        true,
                        0,
                    )?;
                }
                if let Some(v) = self.voices[i].as_mut() {
                    v.volume = l.volume;
                    v.pitch = l.pitch;
                    v.spatial = spatial;
                }
                if start {
                    if let Some((_, offset)) = self.resume_loops.iter().find(|(old, _)| {
                        old.owner == l.owner
                            && old.sound == l.sound
                            && old.activation == l.activation
                            && old.flags == l.flags
                    }) {
                        self.apply(&[Op::Integer {
                            source: i as u32 + 1,
                            parameter: 0x1025,
                            value: *offset,
                        }])?;
                    }
                    self.age += 1;
                    self.voices[i] = Some(Voice {
                        no_override: false,
                        volume: l.volume,
                        pitch: l.pitch,
                        sound: l.sound,
                        owner: l.owner,
                        category: 0,
                        activation: Some(l.activation),
                        spatial,
                        age: self.age,
                    });
                }
            }
            self.resume_loops.clear();
            self.last_loops = desired;
            Ok::<_, String>(())
        })();
        if result.is_err() {
            self.fault = true;
            return result;
        }
        if let Some(e) = events.last() {
            self.last = e.sequence;
        }
        self.pending.extend(events);
        Ok(())
    }
    pub fn render(&mut self, frames: usize, epoch: u64, sequence: u64) -> Result<Vec<u8>, String> {
        if self.fault
            || !(1..=480).contains(&frames)
            || epoch != self.epoch
            || sequence != self.sequence
            || sequence >= i64::MAX as u64
        {
            return Err("Invalid scene render request".into());
        }
        let result = (|| {
            self.reap()?;
            while let Some(e) = self.pending.front() {
                let due = if let Some((tick, frame)) = self.last_played {
                    frame.saturating_add(e.tick.saturating_sub(tick).saturating_mul(800))
                } else {
                    self.frames
                };
                if due > self.frames {
                    break;
                }
                let e = self.pending.pop_front().unwrap();
                if e.category == 1 {
                    // The owner has already been matched. Project that occupied
                    // identity onto the original FindSource matching-ID gate;
                    // this is not a reconstruction of the whole 16-slot policy.
                    let blocked = self
                        .voices
                        .iter()
                        .flatten()
                        .filter(|v| v.category == 1 && v.owner == e.owner && v.activation.is_none())
                        .try_fold(false, |blocked, v| {
                            let choice = crate::audio_pool::select(
                                crate::audio_pool::Incoming {
                                    identity: 0,
                                    priority: 0.,
                                },
                                &[crate::audio_pool::Slot {
                                    occupied: true,
                                    identity: u32::from(v.no_override),
                                    priority: 0.,
                                }],
                            )?;
                            Ok::<_, String>(blocked || choice.is_none())
                        })?;
                    if blocked {
                        PoolStats::inc(&mut self.stats.protected_rejected);
                        // Consumed once even when rejected. Preserve relative
                        // tick spacing for subsequent pending events.
                        self.last_played = Some((e.tick, self.frames));
                        continue;
                    }
                    for i in 0..16 {
                        if self.voices[i].as_ref().is_some_and(|v| {
                            v.category == 1 && v.owner == e.owner && v.activation.is_none()
                        }) {
                            self.stop(i)?;
                        }
                    }
                }
                // Ambient sources reserve their slots; a saturated loop bank may reject a one-shot.
                if let Some(i) = self.event_slot(e.category)? {
                    self.params(
                        i, e.sound, e.volume, e.pitch, false, [0.; 3], 0., true, true, false,
                        e.category,
                    )?;
                    self.age += 1;
                    self.voices[i] = Some(Voice {
                        no_override: e.no_override,
                        volume: e.volume,
                        pitch: e.pitch,
                        sound: e.sound,
                        owner: e.owner,
                        category: e.category,
                        activation: None,
                        spatial: false,
                        age: self.age,
                    });
                    PoolStats::inc(&mut self.stats.admitted);
                } else {
                    PoolStats::inc(&mut self.stats.capacity_rejected);
                }
                self.last_played = Some((e.tick, self.frames));
            }
            let speech = self
                .voices
                .iter()
                .flatten()
                .any(|v| v.category == 1 && v.activation.is_none());
            self.duck_effects(frames, speech)?;
            let mut pcm = self.device.render(frames)?;
            if pcm.frequency != 48000 {
                return Err("Scene audio rate mismatch".into());
            }
            self.music.mix(
                &mut pcm.samples,
                0.35 * 0.7 * self.mix[0] * self.music_gain,
                speech,
            )?;
            Packet::from_rendered(pcm, epoch, sequence)?
                .encode()
                .map_err(str::to_owned)
        })();
        if result.is_err() {
            self.fault = true;
        } else {
            self.frames += frames as u64;
            self.sequence += 1;
        }
        result
    }
    pub fn last_sequence(&self) -> u64 {
        self.last
    }
    /// Trusted producer receipt: all speech from the sealed mailbox was submitted;
    /// other received events were deliberately discarded, never replayed.
    pub fn consume_through(&mut self, cursor: u64) -> Result<(), String> {
        if self.fault || cursor < self.last || cursor > i64::MAX as u64 {
            return Err("Invalid mission consumption receipt".into());
        }
        self.last = cursor;
        Ok(())
    }
    pub fn active(&self) -> usize {
        self.voices.iter().flatten().count()
    }
    /// Fixed, read-only tracked-pool status. Does not query/advance the device.
    pub fn status(&self) -> Vec<u8> {
        let loops = self
            .voices
            .iter()
            .flatten()
            .filter(|v| v.activation.is_some())
            .count();
        let mut bytes = b"RCPOOL01".to_vec();
        for value in [
            16,
            self.active(),
            self.active() - loops,
            loops,
            self.pending.len(),
            usize::from(self.fault),
        ] {
            bytes.extend((value as u32).to_le_bytes());
        }
        for value in [self.frames, self.last]
            .into_iter()
            .chain(self.stats.values())
        {
            bytes.extend(value.to_le_bytes());
        }
        bytes
    }
    /// Snapshot at the controlled render boundary, not the hardware playback head.
    pub fn snapshot(&mut self) -> Result<Vec<u8>, String> {
        let result = self.snapshot_inner();
        if result.is_err() {
            self.fault = true;
        }
        result
    }
    fn snapshot_inner(&mut self) -> Result<Vec<u8>, String> {
        if self.fault {
            return Err("Cannot snapshot faulted mission audio".into());
        }
        let mut dialogs = vec![];
        let mut loops = self.resume_loops.clone();
        for i in 0..self.voices.len() {
            let Some(v) = self.voices[i].clone() else {
                continue;
            };
            if v.activation.is_none() && v.category != 1 {
                continue;
            }
            if self.device.observe(i as u32 + 1, 0x1010)? != 0x1012 {
                continue;
            }
            let offset = self.device.observe(i as u32 + 1, 0x1025)?;
            if offset < 0 || offset as u32 >= self.sounds[v.sound].frames {
                return Err("Invalid retained sample offset".into());
            }
            if v.activation.is_some() {
                if let Some(l) = self.last_loops.iter().find(|l| {
                    l.owner == v.owner && l.sound == v.sound && Some(l.activation) == v.activation
                }) {
                    loops.push((l.clone(), offset as u32));
                }
            } else {
                dialogs.push((v, offset as u32));
            }
        }
        let saved = Resume {
            effects_duck: self.effects_duck,
            music: if self.music.count() > 0 {
                Some(self.music.saved())
            } else {
                None
            },
            stats: self.stats.clone(),
            schema: 1,
            bank: self.bank_fingerprint,
            frames: self.frames,
            last: self.last,
            last_played: self.last_played,
            age: self.age,
            pose: self.last_pose,
            pending: self
                .pending
                .iter()
                .filter(|e| e.category == 1)
                .cloned()
                .collect(),
            dialogs,
            loops,
        };
        let data = serde_json::to_vec(&saved).map_err(|e| e.to_string())?;
        if data.len() > MAX_RESUME - 12 {
            return Err("Mission resume snapshot too large".into());
        }
        Ok([
            b"RCRES001".as_slice(),
            &(data.len() as u32).to_le_bytes(),
            &data,
        ]
        .concat())
    }
    /// Restore only into a fresh scene. Ambient loops wait for authoritative gameplay input.
    pub fn restore(&mut self, bytes: &[u8]) -> Result<(), String> {
        if self.fault
            || self.restored
            || self.frames != 0
            || self.last != 0
            || self.active() != 0
            || self.last_pose.is_some()
            || !self.pending.is_empty()
            || bytes.len() < 12
            || bytes.len() > MAX_RESUME
            || &bytes[..8] != b"RCRES001"
        {
            return Err("Invalid mission resume target/header".into());
        }
        let len = u32::from_le_bytes(bytes[8..12].try_into().unwrap()) as usize;
        if len != bytes.len() - 12 {
            return Err("Mission resume length".into());
        }
        let s: Resume = serde_json::from_slice(&bytes[12..]).map_err(|e| e.to_string())?;
        if s.schema != 1
            || !s.effects_duck.is_finite()
            || !(0.25..=1.).contains(&s.effects_duck)
            || s.bank != self.bank_fingerprint
            || s.pending.len() > 512
            || s.dialogs.len() + s.loops.len() > 16
            || s.frames > i64::MAX as u64
            || s.last > i64::MAX as u64
            || s.age > i64::MAX as u64
            || s.stats
                .values()
                .iter()
                .any(|value| *value > i64::MAX as u64)
        {
            return Err("Mission resume bank/counter/budget mismatch".into());
        }
        if s.last_played
            .is_some_and(|(tick, frame)| tick > i64::MAX as u64 || frame > s.frames)
        {
            return Err("Invalid retained audio timeline".into());
        }
        let valid = |sound: usize, volume: f32, pitch: f32| {
            sound < self.sounds.len()
                && volume.is_finite()
                && (0.0001..=1.).contains(&volume)
                && pitch.is_finite()
                && (0.5..=2.).contains(&pitch)
        };
        let mut sequence = 0;
        for e in &s.pending {
            if !valid(e.sound, e.volume, e.pitch)
                || e.category != 1
                || e.sequence <= sequence
                || e.sequence > s.last
                || e.tick > i64::MAX as u64
            {
                return Err("Invalid retained dialog queue".into());
            }
            sequence = e.sequence;
        }
        let mut owners = BTreeSet::new();
        for (v, offset) in &s.dialogs {
            if !valid(v.sound, v.volume, v.pitch)
                || v.category != 1
                || v.activation.is_some()
                || v.spatial
                || *offset >= self.sounds[v.sound].frames
                || v.age > s.age
                || !owners.insert(v.owner)
            {
                return Err("Invalid retained dialog".into());
            }
        }
        owners.clear();
        for (l, offset) in &s.loops {
            if !valid(l.sound, l.volume, l.pitch)
                || l.owner == 0
                || l.owner > i32::MAX as u32
                || !owners.insert(l.owner)
                || l.activation > i64::MAX as u64
                || *offset >= self.sounds[l.sound].frames
                || l.flags & !7 != 0
                || (l.flags & 1 == 0
                    && (l.flags != 0 || l.volume != 1. || l.pitch != 1. || l.radius != 0.))
                || l.location.iter().any(|v| !v.is_finite() || v.abs() > 1e7)
                || !l.radius.is_finite()
                || !(0. ..=1e6).contains(&l.radius)
            {
                return Err("Invalid retained ambient".into());
            }
        }
        // Validate listener geometry before all physical effects, including source creation.
        if let Some((p, f, u)) = s.pose {
            if p.iter().any(|v| !v.is_finite() || v.abs() > 1e6)
                || f.iter().chain(&u).any(|v| !v.is_finite())
                || (f.iter().map(|v| v * v).sum::<f32>() - 1.).abs() > 0.001
                || (u.iter().map(|v| v * v).sum::<f32>() - 1.).abs() > 0.001
                || f.iter().zip(u).map(|(a, b)| a * b).sum::<f32>().abs() > 0.001
            {
                return Err("Invalid retained listener".into());
            }
        }
        // Decoder/curve validation and replay occur before any physical restore IO.
        if let Some(music) = s.music.clone() {
            self.music.restore(music)?;
        } else if self.music.count() > 0 {
            return Err("Missing soundtrack resume state".into());
        }
        let result = (|| {
            self.effects_duck = s.effects_duck;
            if let Some((p, f, u)) = s.pose {
                self.device.listener(p, f, u)?;
            }
            for (v, offset) in &s.dialogs {
                let i = self.slot()?.ok_or("No slot for restored mission dialog")?;
                self.params(
                    i, v.sound, v.volume, v.pitch, false, [0.; 3], 0., true, true, false,
                    v.category,
                )?;
                self.apply(&[Op::Integer {
                    source: i as u32 + 1,
                    parameter: 0x1025,
                    value: *offset,
                }])?;
                self.voices[i] = Some(v.clone());
            }
            Ok::<_, String>(())
        })();
        if result.is_err() {
            self.fault = true;
            return result;
        }
        self.restored = true;
        self.last_pose = s.pose;
        self.frames = s.frames;
        self.last = s.last;
        self.last_played = s.last_played;
        self.age = s.age;
        self.stats = s.stats;
        self.pending = s.pending.into();
        self.resume_loops = s.loops;
        Ok(())
    }
}
