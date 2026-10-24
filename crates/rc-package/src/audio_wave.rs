//! Bounded WAVE chunk/field extraction and strict portable PCM admission.
use crate::audio_stream_data::MAX_BYTES;
use serde::{Deserialize, Serialize};
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Info {
    pub tag: u16,
    pub channels: u16,
    pub frequency: u32,
    pub byte_rate: u32,
    pub block_align: u16,
    pub bits: u16,
    pub data_start: u32,
    pub data_len: u32,
    pub loop_count: Option<u32>,
}
fn word(b: &[u8], at: usize) -> u16 {
    u16::from_le_bytes(b[at..at + 2].try_into().unwrap())
}
fn dword(b: &[u8], at: usize) -> u32 {
    u32::from_le_bytes(b[at..at + 4].try_into().unwrap())
}
/// First fmt/data/smpl chunks as in ReadWaveInfo, with owned-slice bounds.
/// Native ReadWaveInfo checks WAVE, but does not validate the RIFF signature.
pub fn scan(bytes: &[u8]) -> Result<Info, String> {
    if bytes.len() < 12 || bytes.len() > MAX_BYTES || &bytes[8..12] != b"WAVE" {
        return Err("Invalid WAVE container".into());
    }
    let (mut fmt, mut data, mut smpl) = (None, None, None);
    let mut at = 12usize;
    while at < bytes.len() {
        let h = bytes.get(at..at + 8).ok_or("Truncated WAVE chunk header")?;
        let n = dword(h, 4) as usize;
        let start = at + 8;
        let end = start.checked_add(n).ok_or("WAVE chunk overflow")?;
        let chunk = bytes.get(start..end).ok_or("Truncated WAVE chunk body")?;
        match &h[..4] {
            b"fmt " if fmt.is_none() => {
                if n < 16 {
                    return Err("Short WAVE format".into());
                }
                fmt = Some((
                    word(chunk, 0),
                    word(chunk, 2),
                    dword(chunk, 4),
                    dword(chunk, 8),
                    word(chunk, 12),
                    word(chunk, 14),
                ));
            }
            b"data" if data.is_none() => data = Some((start as u32, n as u32)),
            b"smpl" if smpl.is_none() => {
                if n < 36 {
                    return Err("Short WAVE sampler".into());
                }
                smpl = Some(dword(chunk, 28));
            }
            _ => {}
        }
        at = end.checked_add(n & 1).ok_or("WAVE padding overflow")?;
        if at > bytes.len() {
            return Err("Missing WAVE chunk padding".into());
        }
    }
    let (tag, channels, frequency, byte_rate, block_align, bits) =
        fmt.ok_or("Missing WAVE format")?;
    let (data_start, data_len) = data.ok_or("Missing WAVE samples")?;
    Ok(Info {
        tag,
        channels,
        frequency,
        byte_rate,
        block_align,
        bits,
        data_start,
        data_len,
        loop_count: smpl,
    })
}
/// Literal native registration mapping; non-8-bit values use the 16-bit AL form.
pub fn registration_format(channels: u16, bits: u16) -> u32 {
    0x1100 + u32::from(channels != 1) * 2 + u32::from(bits != 8)
}
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Pcm {
    pub info: Info,
    pub bytes: Vec<u8>,
}
impl Pcm {
    pub fn from_wave(bytes: &[u8]) -> Result<Self, String> {
        let info = scan(bytes)?;
        if &bytes[..4] != b"RIFF" || u64::from(dword(bytes, 4)) + 8 != bytes.len() as u64 {
            return Err("Invalid RIFF length/signature".into());
        }
        let pcm = Self {
            bytes: bytes[info.data_start as usize..(info.data_start + info.data_len) as usize]
                .to_vec(),
            info,
        };
        pcm.validate()?;
        Ok(pcm)
    }
    pub fn validate(&self) -> Result<(), String> {
        let i = &self.info;
        if i.tag != 1
            || !(1..=2).contains(&i.channels)
            || ![8, 16].contains(&i.bits)
            || i.frequency == 0
            || i.frequency > 1_000_000
        {
            return Err("Unsupported PCM format".into());
        }
        let align = i.channels * (i.bits / 8);
        if i.block_align != align
            || i.byte_rate != i.frequency * u32::from(align)
            || self.bytes.is_empty()
            || self.bytes.len() > MAX_BYTES
            || self.bytes.len() != i.data_len as usize
            || !self.bytes.len().is_multiple_of(usize::from(align))
        {
            return Err("Invalid PCM sample layout".into());
        }
        Ok(())
    }
    pub fn format(&self) -> u32 {
        registration_format(self.info.channels, self.info.bits)
    }
}
