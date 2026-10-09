use rc_package::{
    d3d_buffers::State,
    d3d_pools::{IndexRequest, Runtime, VertexRequest},
};
use serde::{Deserialize, Serialize};
use std::{env, fs};
#[derive(Clone, Serialize, Deserialize)]
struct World {
    runtime: Runtime,
    state: State,
    target: Vec<u8>,
}
#[derive(Deserialize)]
#[serde(tag = "kind", rename_all = "snake_case")]
enum Step {
    Vertex { request: VertexRequest },
    Index { request: IndexRequest },
}
#[derive(Deserialize)]
struct Case {
    id: usize,
    world: World,
    steps: Vec<Step>,
}
#[derive(Deserialize)]
struct Input {
    source_sha256: String,
    cases: Vec<Case>,
    errors: Vec<Case>,
}
fn main() -> Result<(), Box<dyn std::error::Error>> {
    let args: Vec<_> = env::args().collect();
    if args.len() != 3 {
        return Err("Expected INPUT OUTPUT".into());
    }
    let input: Input = serde_json::from_slice(&fs::read(&args[1])?)?;
    let (mut probes, mut errors) = (vec![], vec![]);
    for (safe, cases) in [(false, input.cases), (true, input.errors)] {
        for c in cases {
            let mut w = c.world;
            let mut steps = vec![];
            for s in c.steps {
                let result = match s {
                    Step::Vertex { request } => {
                        w.runtime.vertex(&mut w.state, &request, &mut w.target)
                    }
                    Step::Index { request } => {
                        w.runtime.index(&mut w.state, &request, &mut w.target)
                    }
                };
                if !safe {
                    if let Err(e) = &result {
                        return Err(e.clone().into());
                    }
                }
                steps.push(serde_json::json!({"result":result,"world":w}));
            }
            let value = serde_json::json!({"id":c.id,"steps":steps,"world":w});
            if safe {
                errors.push(value);
            } else {
                probes.push(value);
            }
        }
    }
    fs::write(
        &args[2],
        serde_json::to_vec(
            &serde_json::json!({"source_sha256":input.source_sha256,"probes":probes,"errors":errors,"scope":"Dynamic vertex/index constructors, lazy per-device vertex and independent index-width pools, resource hash/list insertion, scratch/resize/ring upload and renderer binding. Explicit allocation images and COM/getter/callback/shader answers; no live allocator/GPU, dynamic draw-offset consumer or Android execution."}),
        )?,
    )?;
    Ok(())
}
