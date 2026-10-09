use rc_package::d3d_resource::{self, Cache, Kind};
use serde::Deserialize;
use serde_json::{json, Value};
use std::{env, fs};
#[derive(Deserialize)]
#[serde(tag = "kind", rename_all = "snake_case")]
enum Action {
    Lookup {
        key: [u32; 2],
    },
    Insert {
        address: u32,
        key: [u32; 2],
        resource_kind: Kind,
    },
    Unlink {
        address: u32,
    },
    ResetVertex {
        address: u32,
    },
    DestroyIndex {
        address: u32,
    },
}
fn apply(cache: &mut Cache, a: Action) -> Result<Value, String> {
    match a {
        Action::Lookup { key } => Ok(json!(cache.lookup(key)?)),
        Action::Insert {
            address,
            key,
            resource_kind,
        } => {
            cache.insert(address, key, resource_kind)?;
            Ok(Value::Null)
        }
        Action::Unlink { address } => {
            cache.unlink(address)?;
            Ok(Value::Null)
        }
        Action::ResetVertex { address } => Ok(json!(cache.reset_vertex(address)?)),
        Action::DestroyIndex { address } => Ok(json!(cache.destroy_index(address)?)),
    }
}
#[derive(Deserialize)]
struct Case {
    id: usize,
    cache: Cache,
    actions: Vec<Action>,
}
#[derive(Deserialize)]
struct Error {
    id: usize,
    cache: Cache,
    action: Action,
}
#[derive(Deserialize)]
struct Input {
    source_sha256: String,
    hashes: Vec<u32>,
    cases: Vec<Case>,
    errors: Vec<Error>,
}
fn main() -> Result<(), Box<dyn std::error::Error>> {
    let args: Vec<_> = env::args().collect();
    if args.len() != 3 {
        return Err("Expected INPUT OUTPUT".into());
    }
    let input: Input = serde_json::from_slice(&fs::read(&args[1])?)?;
    let hashes: Vec<_> = input
        .hashes
        .iter()
        .map(|v| d3d_resource::hash(*v))
        .collect();
    let mut probes = vec![];
    let mut errors = vec![];
    for c in input.cases {
        let mut cache = c.cache;
        let mut steps = vec![];
        for a in c.actions {
            let result = apply(&mut cache, a)?;
            steps.push(json!({"result":result,"cache":cache}));
        }
        probes.push(json!({"id":c.id,"steps":steps,"cache":cache}));
    }
    for c in input.errors {
        let mut cache = c.cache;
        let result = apply(&mut cache, c.action);
        errors.push(json!({"id":c.id,"result":result,"cache":cache}));
    }
    fs::write(
        &args[2],
        serde_json::to_vec(
            &json!({"source_sha256":input.source_sha256,"hashes":hashes,"probes":probes,"errors":errors,"scope":"Native 4096-bucket resource hash, full 64-bit key lookup, base/vertex/index wrapper initialization and two-chain insertion/unlinking, vertex dual-handle reset and index destructor release/unlink semantics. Captured allocation images and planned COM release only; allocator free, GPU execution, upload/draw integration and Android excluded."}),
        )?,
    )?;
    Ok(())
}
