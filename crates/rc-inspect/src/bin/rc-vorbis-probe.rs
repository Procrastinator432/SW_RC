//! Local streaming decoder inspection; writes no original assets to the repo.
use rc_package::audio_vorbis::Provider;
use std::{io::Write, path::Path};
fn run() -> Result<(), String> {
    let args: Vec<String> = std::env::args().collect();
    if args.len() != 4 {
        return Err("Usage: rc-vorbis-probe INPUT.ogg REPORT.json OUTPUT.s16le".into());
    }
    let mut decoder = Provider::from_file(Path::new(&args[1]))?;
    let metadata = decoder.metadata().clone();
    let mut output = std::fs::File::create(&args[3]).map_err(|e| e.to_string())?;
    let mut reads = 0u64;
    loop {
        let bytes = decoder.read_bytes(65536)?;
        if bytes.is_empty() {
            break;
        }
        output.write_all(&bytes).map_err(|e| e.to_string())?;
        reads += 1;
    }
    let end = decoder.cursor();
    // Restore near the end and compare an actual suffix, including odd reads.
    let at = end.saturating_sub(8193);
    decoder.seek_byte(at)?;
    let saved = decoder.checkpoint()?;
    let mut cold = Provider::restore(&saved)?;
    let mut resumed = 0usize;
    loop {
        let a = decoder.read_bytes(173)?;
        let b = cold.read_bytes(173)?;
        if a != b {
            return Err("Cold decoder suffix mismatch".into());
        }
        if a.is_empty() {
            break;
        }
        resumed += a.len();
    }
    let report = serde_json::json!({"passed":true,"metadata":metadata,"decoded_bytes":end,"reads":reads,"cold_suffix_bytes":resumed,"codec":"lewton 0.10.2","original_libvorbis_bit_parity":false});
    std::fs::write(&args[2], serde_json::to_vec_pretty(&report).unwrap())
        .map_err(|e| e.to_string())?;
    println!("{}", report);
    Ok(())
}
fn main() {
    if let Err(e) = run() {
        eprintln!("{e}");
        std::process::exit(1);
    }
}
