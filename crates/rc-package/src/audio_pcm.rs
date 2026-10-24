//! Bounded transport for rendered stereo S16 PCM, independent of platform audio APIs.
use crate::audio_device::Rendered;

pub const HEADER: usize = 40;
pub const MAX_FRAMES: usize = 4096;
pub const MAX_FILE: usize = 16 * 1024 * 1024;

#[derive(Debug, PartialEq, Eq)]
pub struct Packet {
    pub rate: u32,
    pub epoch: u64,
    pub sequence: u64,
    pub samples: Vec<i16>,
}
impl Packet {
    fn validate(&self) -> Result<(), &'static str> {
        if !(8000..=192000).contains(&self.rate)
            || self.epoch == 0
            || self.epoch > i64::MAX as u64
            || self.sequence >= i64::MAX as u64
            || self.samples.is_empty()
            || !self.samples.len().is_multiple_of(2)
            || self.samples.len() > MAX_FRAMES * 2
        {
            return Err("invalid PCM packet");
        }
        Ok(())
    }
    pub fn from_rendered(
        rendered: Rendered,
        epoch: u64,
        sequence: u64,
    ) -> Result<Self, &'static str> {
        let packet = Self {
            rate: rendered.frequency,
            epoch,
            sequence,
            samples: rendered.samples,
        };
        packet.validate()?;
        Ok(packet)
    }
    pub fn encode(&self) -> Result<Vec<u8>, &'static str> {
        self.validate()?;
        let mut bytes = Vec::with_capacity(HEADER + self.samples.len() * 2);
        bytes.extend_from_slice(b"RCPCM001");
        bytes.extend(self.rate.to_le_bytes());
        bytes.extend(2u16.to_le_bytes());
        bytes.extend(16u16.to_le_bytes());
        bytes.extend(self.epoch.to_le_bytes());
        bytes.extend(self.sequence.to_le_bytes());
        bytes.extend((self.samples.len() as u32 / 2).to_le_bytes());
        bytes.extend((self.samples.len() as u32 * 2).to_le_bytes());
        for sample in &self.samples {
            bytes.extend(sample.to_le_bytes());
        }
        Ok(bytes)
    }
    pub fn decode(bytes: &[u8]) -> Result<Self, &'static str> {
        if bytes.len() < HEADER
            || bytes.len() > HEADER + MAX_FRAMES * 4
            || &bytes[..8] != b"RCPCM001"
        {
            return Err("invalid PCM header");
        }
        let u32_at = |n| u32::from_le_bytes(bytes[n..n + 4].try_into().unwrap());
        let u64_at = |n| u64::from_le_bytes(bytes[n..n + 8].try_into().unwrap());
        let frames = u32_at(32) as usize;
        if bytes[12..16] != [2, 0, 16, 0]
            || frames == 0
            || frames > MAX_FRAMES
            || u32_at(36) as usize != frames * 4
            || bytes.len() != HEADER + frames * 4
        {
            return Err("invalid PCM format or length");
        }
        let packet = Self {
            rate: u32_at(8),
            epoch: u64_at(16),
            sequence: u64_at(24),
            samples: bytes[HEADER..]
                .chunks_exact(2)
                .map(|b| i16::from_le_bytes([b[0], b[1]]))
                .collect(),
        };
        packet.validate()?;
        Ok(packet)
    }
}

/// Diagnostic container, not a saved mixer state. Sequence is continuous within epoch 1.
pub fn package(
    rate: u32,
    samples: &[i16],
    frames_per_packet: usize,
) -> Result<Vec<u8>, &'static str> {
    if samples.is_empty()
        || !samples.len().is_multiple_of(2)
        || !(1..=MAX_FRAMES).contains(&frames_per_packet)
    {
        return Err("invalid PCM package input");
    }
    let count = samples.len().div_ceil(frames_per_packet * 2);
    let length = 16usize
        .checked_add(count.checked_mul(HEADER + 4).ok_or("PCM size")?)
        .and_then(|n| n.checked_add(samples.len().checked_mul(2)?))
        .ok_or("PCM size")?;
    if length > MAX_FILE {
        return Err("PCM package too large");
    }
    let mut out = Vec::with_capacity(length);
    out.extend(b"RCPCMSET");
    out.extend(rate.to_le_bytes());
    out.extend((count as u32).to_le_bytes());
    for (sequence, chunk) in samples.chunks(frames_per_packet * 2).enumerate() {
        let bytes = Packet {
            rate,
            epoch: 1,
            sequence: sequence as u64,
            samples: chunk.to_vec(),
        }
        .encode()?;
        out.extend((bytes.len() as u32).to_le_bytes());
        out.extend(bytes);
    }
    Ok(out)
}

#[cfg(test)]
mod tests {
    use super::*;
    fn packet() -> Packet {
        Packet {
            rate: 48000,
            epoch: 1,
            sequence: 0,
            samples: vec![0, -1, 32767, -32768],
        }
    }
    #[test]
    fn roundtrip_and_java_fixture() {
        let bytes = packet().encode().unwrap();
        assert_eq!(Packet::decode(&bytes).unwrap(), packet());
        assert_eq!(
            bytes,
            include_bytes!("../../../fixtures/audio-pcm-packet.bin")
        );
    }
    #[test]
    fn rejects_malformed_packets() {
        let bytes = packet().encode().unwrap();
        for n in [0, 12, 14, 32, 36] {
            let mut b = bytes.clone();
            b[n] = 255;
            assert!(Packet::decode(&b).is_err(), "offset {n}");
        }
        for (start, end) in [(8, 12), (16, 24)] {
            let mut b = bytes.clone();
            b[start..end].fill(0);
            assert!(Packet::decode(&b).is_err());
        }
        for len in 0..bytes.len() {
            assert!(Packet::decode(&bytes[..len]).is_err());
        }
        let mut b = bytes;
        b.push(0);
        assert!(Packet::decode(&b).is_err());
    }
    #[test]
    fn limits_and_signed_java_counters() {
        let mut p = packet();
        p.samples = vec![0; MAX_FRAMES * 2];
        assert!(p.encode().is_ok());
        p.samples.push(0);
        assert!(p.encode().is_err());
        for epoch in [0, i64::MAX as u64 + 1] {
            let mut p = packet();
            p.epoch = epoch;
            assert!(p.encode().is_err());
        }
        let mut p = packet();
        p.sequence = i64::MAX as u64;
        assert!(p.encode().is_err());
    }
    #[test]
    fn package_limits_and_tail() {
        let b = package(48000, &[0; 14], 3).unwrap();
        assert_eq!(&b[..8], b"RCPCMSET");
        assert_eq!(b[12], 3);
        assert!(package(48000, &[], 480).is_err());
        assert!(package(48000, &[0; 2], 0).is_err());
        assert!(package(48000, &vec![0; MAX_FILE], 4096).is_err());
    }
}
