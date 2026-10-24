//! Bounded, incremental Rust Vorbis provider. Not libvorbis bit parity.
use crate::audio_stream_data::{Decode, Decoder, MAX_BYTES};
use lewton::inside_ogg::OggStreamReader;
use serde::{Deserialize, Serialize};
use std::{
    io::{Cursor, Read},
    path::Path,
    sync::Arc,
};

pub const CHUNK_BYTES: usize = 0x10000;
const MAX_PCM_BYTES: u64 = 512 * 1024 * 1024;
/// FALAudioStreamOgg::Init's exact supported channel-to-AL format mapping.
pub fn al_format(channels: i32) -> Option<u32> {
    match channels {
        1 => Some(0x1101),
        2 => Some(0x1103),
        _ => None,
    }
}
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Metadata {
    pub channels: u8,
    pub frequency: u32,
    pub format: u32,
    pub frames: u64,
}
impl Metadata {
    pub fn duration(&self) -> f32 {
        (self.frames as f64 / f64::from(self.frequency)) as f32
    }
}
type Reader = OggStreamReader<Cursor<Arc<[u8]>>>;
pub struct Provider {
    encoded: Arc<[u8]>,
    reader: Reader,
    metadata: Metadata,
    pending: Vec<u8>,
    pending_at: usize,
    cursor: u64,
    produced: u64,
    eof: bool,
}
#[derive(Clone, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub(crate) struct Saved {
    schema: u32,
    encoded: Vec<u8>,
    cursor: u64,
}
fn page_frames(bytes: &[u8]) -> Result<u64, String> {
    let mut at = 0usize;
    let mut serial = None;
    let mut sequence = 0u32;
    let mut frames = None;
    let mut ended = false;
    while at < bytes.len() {
        let h = bytes
            .get(at..at.saturating_add(27))
            .ok_or("Truncated Ogg header")?;
        if &h[..4] != b"OggS" || h[4] != 0 || h[5] & !7 != 0 || ended {
            return Err("Unsupported Ogg envelope".into());
        }
        let id = u32::from_le_bytes(h[14..18].try_into().unwrap());
        let seq = u32::from_le_bytes(h[18..22].try_into().unwrap());
        if serial.is_none() {
            if h[5] & 2 == 0 || h[5] & 1 != 0 {
                return Err("Missing Ogg beginning".into());
            }
            serial = Some(id);
        } else if h[5] & 2 != 0 {
            return Err("Chained Ogg unsupported".into());
        }
        if serial != Some(id) || seq != sequence {
            return Err("Multiplexed or discontinuous Ogg unsupported".into());
        }
        sequence = sequence.checked_add(1).ok_or("Ogg sequence overflow")?;
        let table_end = at + 27 + usize::from(h[26]);
        let table = bytes
            .get(at + 27..table_end)
            .ok_or("Truncated Ogg lacing")?;
        let end = table_end + table.iter().map(|&n| usize::from(n)).sum::<usize>();
        let page = bytes.get(at..end).ok_or("Truncated Ogg body")?;
        let expected = u32::from_le_bytes(h[22..26].try_into().unwrap());
        let mut crc = 0u32;
        for (i, &b) in page.iter().enumerate() {
            crc ^= u32::from(if (22..26).contains(&i) { 0 } else { b }) << 24;
            for _ in 0..8 {
                crc = (crc << 1) ^ if crc & 0x80000000 != 0 { 0x04c11db7 } else { 0 };
            }
        }
        if crc != expected {
            return Err("Ogg checksum mismatch".into());
        }
        ended = h[5] & 4 != 0;
        if ended {
            frames = Some(u64::from_le_bytes(h[6..14].try_into().unwrap()));
        }
        at = end;
    }
    frames
        .filter(|&n| n != u64::MAX)
        .ok_or_else(|| "Ogg lacks completed final page".into())
}
impl Provider {
    pub fn from_file(path: &Path) -> Result<Self, String> {
        let file = std::fs::File::open(path).map_err(|e| e.to_string())?;
        let mut bytes = vec![];
        file.take(MAX_BYTES as u64 + 1)
            .read_to_end(&mut bytes)
            .map_err(|e| e.to_string())?;
        Self::new(bytes)
    }
    pub fn new(bytes: Vec<u8>) -> Result<Self, String> {
        if bytes.is_empty() || bytes.len() > MAX_BYTES {
            return Err("Vorbis input byte budget exceeded".into());
        }
        let frames = page_frames(&bytes)?;
        let encoded: Arc<[u8]> = bytes.into();
        let reader = Reader::new(Cursor::new(encoded.clone())).map_err(|e| e.to_string())?;
        let channels = reader.ident_hdr.audio_channels;
        let frequency = reader.ident_hdr.audio_sample_rate;
        if !(1..=2).contains(&channels) || frequency == 0 || frequency > 1_000_000 {
            return Err("Unsupported Vorbis channel count/rate".into());
        }
        if frames
            .checked_mul(u64::from(channels) * 2)
            .is_none_or(|n| n > MAX_PCM_BYTES)
        {
            return Err("Decoded Vorbis duration budget exceeded".into());
        }
        let metadata = Metadata {
            channels,
            frequency,
            format: al_format(i32::from(channels)).ok_or("Unsupported Vorbis channels")?,
            frames,
        };
        Ok(Self {
            encoded,
            reader,
            metadata,
            pending: vec![],
            pending_at: 0,
            cursor: 0,
            produced: 0,
            eof: frames == 0,
        })
    }
    pub fn metadata(&self) -> &Metadata {
        &self.metadata
    }
    pub fn cursor(&self) -> u64 {
        self.cursor
    }
    pub fn encoded_len(&self) -> usize {
        self.encoded.len()
    }
    pub fn read_bytes(&mut self, max: usize) -> Result<Vec<u8>, String> {
        if max == 0 || max > MAX_BYTES {
            return Err("Invalid Vorbis read size".into());
        }
        while self.pending_at == self.pending.len() && !self.eof {
            match self
                .reader
                .read_dec_packet_itl()
                .map_err(|e| e.to_string())?
            {
                Some(samples) => {
                    if self.reader.ident_hdr.audio_channels != self.metadata.channels
                        || self.reader.ident_hdr.audio_sample_rate != self.metadata.frequency
                    {
                        return Err("Vorbis format changed".into());
                    }
                    self.pending = samples.into_iter().flat_map(i16::to_le_bytes).collect();
                    // Lewton does not trim a single audio page whose initial
                    // granule position is unknown. Bound its final padding by
                    // the completed stream's declared PCM-frame count.
                    let total = self.metadata.frames * u64::from(self.metadata.channels) * 2;
                    self.pending
                        .truncate((total - self.produced).min(self.pending.len() as u64) as usize);
                    self.pending_at = 0;
                    self.produced = self
                        .produced
                        .checked_add(self.pending.len() as u64)
                        .ok_or("Decoded size overflow")?;
                    if self.produced > MAX_PCM_BYTES {
                        return Err("Decoded Vorbis byte budget exceeded".into());
                    }
                    if self.produced == total {
                        self.eof = true;
                    }
                }
                None => {
                    if self.produced != self.metadata.frames * u64::from(self.metadata.channels) * 2
                    {
                        return Err("Vorbis ended before declared PCM frames".into());
                    }
                    self.eof = true;
                }
            }
        }
        let n = max.min(self.pending.len() - self.pending_at);
        let out = self.pending[self.pending_at..self.pending_at + n].to_vec();
        self.pending_at += n;
        self.cursor += n as u64;
        Ok(out)
    }
    /// Exact decoded-byte replay, independent of page-granular codec seeks.
    pub fn seek_byte(&mut self, at: u64) -> Result<(), String> {
        if at > MAX_PCM_BYTES {
            return Err("Vorbis seek beyond budget".into());
        }
        let mut next = Self::new(self.encoded.to_vec())?;
        while next.cursor < at {
            let out = next.read_bytes((at - next.cursor).min(CHUNK_BYTES as u64) as usize)?;
            if out.is_empty() {
                return Err("Vorbis seek beyond decoded end".into());
            }
        }
        *self = next;
        Ok(())
    }
    pub fn checkpoint(&self) -> Result<Vec<u8>, String> {
        let out = serde_json::to_vec(&self.saved()).map_err(|e| e.to_string())?;
        if out.len() > MAX_BYTES {
            return Err("Vorbis snapshot budget exceeded".into());
        }
        Ok(out)
    }
    pub fn restore(bytes: &[u8]) -> Result<Self, String> {
        if bytes.len() > MAX_BYTES {
            return Err("Vorbis snapshot budget exceeded".into());
        }
        let saved: Saved = serde_json::from_slice(bytes).map_err(|e| e.to_string())?;
        Self::restore_saved(saved)
    }
    pub(crate) fn saved(&self) -> Saved {
        Saved {
            schema: 1,
            encoded: self.encoded.to_vec(),
            cursor: self.cursor,
        }
    }
    pub(crate) fn restore_saved(saved: Saved) -> Result<Self, String> {
        if saved.schema != 1 {
            return Err("Unsupported Vorbis snapshot schema".into());
        }
        let mut next = Self::new(saved.encoded)?;
        next.seek_byte(saved.cursor)?;
        Ok(next)
    }
}
impl Decoder for Provider {
    fn read(&mut self, max: usize) -> Result<Decode, String> {
        let bytes = self.read_bytes(max)?;
        Ok(if bytes.is_empty() {
            Decode::End
        } else {
            Decode::Data {
                bytes,
                bitstream: 0,
            }
        })
    }
    fn rewind(&mut self) -> Result<(), String> {
        self.seek_byte(0)
    }
}
