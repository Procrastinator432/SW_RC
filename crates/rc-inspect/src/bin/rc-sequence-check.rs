use rc_package::{
    mesh_animation::AnimationChannel,
    skeletal_sequence::{get_sequence, SequenceLookupHost, SnapshotSequenceLookup},
};
use std::{env, fs};
struct Host<'a> {
    lookup: SnapshotSequenceLookup<'a>,
    calls: Vec<(u32, bool)>,
}
impl SequenceLookupHost for Host<'_> {
    fn find_sequence(&mut self, n: u32, l: bool) -> Result<u32, String> {
        self.calls.push((n, l));
        self.lookup.find_sequence(n, l)
    }
}
fn main() -> Result<(), Box<dyn std::error::Error>> {
    let args: Vec<_> = env::args_os().collect();
    if args.len() != 2 {
        return Err("usage: rc-sequence-check OUTPUT".into());
    }
    let bindings: Vec<_> = (0..64u32)
        .filter(|n| n % 2 == 0)
        .flat_map(|n| [(n, 100 + n), (n, 200 + n)])
        .collect();
    let mut cases = vec![];
    for name in 0..64u32 {
        for editor in [false, true] {
            for cached in [0, 1, 0xffffffff, 0xdeadbeef] {
                let mut channel = AnimationChannel {
                    words: std::array::from_fn(|i| 0x12340000 + i as u32),
                };
                channel.words[0] = name;
                channel.words[17] = cached;
                let before = channel.clone();
                let mut host = Host {
                    lookup: SnapshotSequenceLookup {
                        bindings: &bindings,
                    },
                    calls: vec![],
                };
                let result = get_sequence(std::slice::from_mut(&mut channel), 0, editor, &mut host);
                cases.push(serde_json::json!({"name":name,"editor":editor,"cached":cached,"before":before,"after":channel,"result":result,"calls":host.calls}));
            }
        }
    }
    fs::write(
        &args[1],
        serde_json::to_vec_pretty(
            &serde_json::json!({"scope":"Original GetSequence cached/noneditor and refreshed/editor paths over supplied opaque identities. Diagnostic first-match sequence bindings only; no native registry, loading, Actor or pose-runtime-host claim.","bindings":bindings,"cases":cases}),
        )?,
    )?;
    println!("{} sequence lookup cases", cases.len());
    Ok(())
}
