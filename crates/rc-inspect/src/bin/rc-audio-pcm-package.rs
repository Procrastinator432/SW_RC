//! Convert a bounded stereo S16 WAV (e.g. loopback output) to Android diagnostic packets.
fn main() -> Result<(), Box<dyn std::error::Error>> {
    let args: Vec<_> = std::env::args().collect();
    if args.len() != 3 {
        return Err("usage: rc-audio-pcm-package INPUT.wav OUTPUT.rcpcm".into());
    }
    if std::fs::metadata(&args[1])?.len() > rc_package::audio_pcm::MAX_FILE as u64 {
        return Err("WAV too large".into());
    }
    let pcm = rc_package::audio_wave::Pcm::from_wave(&std::fs::read(&args[1])?)?;
    if pcm.info.channels != 2 || pcm.info.bits != 16 {
        return Err("requires stereo S16 PCM".into());
    }
    let samples: Vec<_> = pcm
        .bytes
        .chunks_exact(2)
        .map(|b| i16::from_le_bytes([b[0], b[1]]))
        .collect();
    let frames_per_packet = (pcm.info.frequency / 100) as usize;
    let bytes = rc_package::audio_pcm::package(pcm.info.frequency, &samples, frames_per_packet)?;
    std::fs::write(&args[2], &bytes)?;
    println!(
        "{}",
        serde_json::json!({"rate":pcm.info.frequency,"frames":samples.len()/2,"bytes":bytes.len(),"frames_per_packet":frames_per_packet})
    );
    Ok(())
}
