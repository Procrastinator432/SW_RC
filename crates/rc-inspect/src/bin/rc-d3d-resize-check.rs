use rc_package::{
    d3d_bindings::Deferred,
    d3d_buffers::State,
    d3d_dynamic::{self, IndexSource, StreamBinding, VertexSource},
    d3d_resize::{self, Context, Responses, Runtime, UploadResponses},
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
    Vertex {
        source: VertexSource,
        answers: UploadResponses,
        context: Context,
        binding: StreamBinding,
    },
    Index {
        source: IndexSource,
        answers: UploadResponses,
        context: Context,
        base: u32,
        reported_size: u32,
    },
}
fn apply(world: &mut World, s: Step) -> Result<serde_json::Value, String> {
    let mut next = world.clone();
    let r = &mut next.runtime;
    let result = match s {
        Step::Vertex {
            source,
            answers,
            context,
            binding,
        } => {
            let transfer = r.vertex(&source, context, &answers, &mut next.target)?;
            d3d_dynamic::bind_vertex(
                &mut next.state,
                &mut r.deferred,
                &mut r.vertex,
                &mut r.device,
                binding,
            )?;
            serde_json::json!({"transfer":transfer,"pool":null})
        }
        Step::Index {
            source,
            answers,
            context,
            base,
            reported_size,
        } => {
            let transfer = r.index(&source, context, &answers, &mut next.target)?;
            d3d_dynamic::bind_index(
                &mut next.state,
                &mut r.deferred,
                &r.index,
                &mut r.device,
                base,
                reported_size,
            )?;
            serde_json::json!({"transfer":transfer,"pool":d3d_dynamic::index_pool(source.width)})
        }
    };
    *world = next;
    Ok(result)
}
#[derive(Deserialize)]
struct Case {
    id: usize,
    world: World,
    steps: Vec<Step>,
}
#[derive(Deserialize)]
struct Resize {
    id: usize,
    kind: String,
    runtime: Runtime,
    size: u32,
    context: Context,
    answers: Responses,
}
#[derive(Deserialize)]
struct Invalidation {
    id: usize,
    deferred: Deferred,
    handle: u32,
    capacity: i32,
    device: u32,
}
#[derive(Deserialize)]
struct Input {
    source_sha256: String,
    cases: Vec<Case>,
    errors: Vec<Case>,
    resizes: Vec<Resize>,
    invalidations: Vec<Invalidation>,
}
fn main() -> Result<(), Box<dyn std::error::Error>> {
    let args: Vec<_> = env::args().collect();
    if args.len() != 3 {
        return Err("Expected INPUT OUTPUT".into());
    }
    let input: Input = serde_json::from_slice(&fs::read(&args[1])?)?;
    let (mut probes, mut errors, mut resizes, mut invalidations) = (vec![], vec![], vec![], vec![]);
    for c in input.invalidations {
        let mut d = c.deferred;
        let commands = d3d_resize::invalidate(&mut d, c.handle, c.capacity, c.device);
        invalidations.push(serde_json::json!({"id":c.id,"commands":commands,"deferred":d}));
    }
    for c in input.resizes {
        let mut r = c.runtime;
        let result = match c.kind.as_str() {
            "vertex" => d3d_resize::vertex(
                &mut r.vertex,
                &mut r.deferred,
                c.size,
                c.context,
                &c.answers,
            ),
            "index" => d3d_resize::index(&mut r.index, c.size, c.context.device, &c.answers),
            _ => return Err("Invalid resize kind".into()),
        };
        resizes.push(serde_json::json!({"id":c.id,"result":result,"runtime":r}));
    }
    for (safe, cases) in [(false, input.cases), (true, input.errors)] {
        for c in cases {
            let mut w = c.world;
            let mut steps = vec![];
            for s in c.steps {
                let result = apply(&mut w, s);
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
            &serde_json::json!({"source_sha256":input.source_sha256,"probes":probes,"errors":errors,"resizes":resizes,"invalidations":invalidations,"scope":"Original stream handle invalidation, dynamic vertex pair and index resize Create/Release/Retry/Evict planning, automatic resize-to-ring/direct CPU upload and resolved renderer binding. Explicit COM/getter/fill/shader answers; no actual GPU/allocator, native error logging, scratch path, pool construction, dynamic draw offsets or Android execution."}),
        )?,
    )?;
    Ok(())
}
