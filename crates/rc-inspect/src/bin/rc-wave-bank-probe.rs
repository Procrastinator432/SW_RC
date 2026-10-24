//! Local original-asset bank regression. Extracted WAV stays in build output.
use rc_package::{
    audio_buffer::Reply,
    audio_buffer_bank::{Bank, Event, Registration},
    audio_wave::Pcm,
    read_package,
};
use std::{fs, path::Path};
fn run() -> Result<(), Box<dyn std::error::Error>> {
    let a: Vec<_> = std::env::args_os().collect();
    if a.len() != 3 {
        return Err("Usage: rc-wave-bank-probe PACKAGE.uax OUTPUT_DIRECTORY".into());
    }
    let raw = fs::read(&a[1])?;
    let pkg = read_package(&raw)?;
    let out = Path::new(&a[2]);
    fs::create_dir_all(out)?;
    let mut records = vec![];
    for (i, e) in pkg.exports.iter().enumerate() {
        if pkg.object_path(e.class)? != "Engine.Sound" {
            continue;
        }
        let payload = pkg.payload(&raw, e)?;
        let Some(at) = payload.windows(4).position(|b| b == b"RIFF") else {
            continue;
        };
        let length = u32::from_le_bytes(
            payload
                .get(at + 4..at + 8)
                .ok_or("Short original RIFF")?
                .try_into()?,
        ) as usize
            + 8;
        let bytes = payload
            .get(at..at + length)
            .ok_or("Truncated original RIFF")?;
        let pcm = Pcm::from_wave(bytes)?;
        let mut bank = Bank::new(1);
        let registration = Registration {
            sound: i as u32 + 1,
            handle: 0x100000 + i as u32,
            allocation: 0x200000 + i as u32,
            generated: 0x300000 + i as u32,
            allow_stereo: true,
            generation_error: 0,
            upload_error: 0,
        };
        let result = bank.register(bytes, &registration)?;
        let uploaded = result
            .events
            .iter()
            .find_map(|e| {
                if let Event::Upload(u) = e {
                    Some(u)
                } else {
                    None
                }
            })
            .ok_or("No original PCM upload")?;
        if uploaded.bytes != pcm.bytes {
            return Err("Original PCM changed during registration".into());
        }
        let saved = bank.checkpoint()?;
        let mut cold = Bank::new(1);
        cold.load(&saved)?;
        if bank.register(&[], &registration)? != cold.register(&[], &registration)? {
            return Err("Cold bank cache mismatch".into());
        }
        let reply = Reply {
            stops: vec![],
            reallocated: 0,
        };
        if bank.unregister(registration.sound, &reply)?
            != cold.unregister(registration.sound, &reply)?
            || bank.checkpoint()? != cold.checkpoint()?
        {
            return Err("Cold bank disposal mismatch".into());
        }
        let name = format!("{:02}.wav", records.len());
        fs::write(out.join(&name), bytes)?;
        records.push(serde_json::json!({"source":pkg.object_path(i as i32+1)?,"wave":name,"info":pcm.info,"format":pcm.format(),"pcm_bytes":pcm.bytes.len(),"upload_exact":true,"cold_cache_and_unregister_exact":true}));
        if records.len() >= 24 {
            break;
        }
    }
    if records.is_empty() {
        return Err("No PCM sound assets found".into());
    }
    let report = serde_json::json!({"passed":true,"files":records});
    fs::write(
        out.join("validation.json"),
        serde_json::to_vec_pretty(&report)?,
    )?;
    println!("{} original WAV assets checked", records.len());
    Ok(())
}
fn main() {
    if let Err(e) = run() {
        eprintln!("{e}");
        std::process::exit(1);
    }
}
