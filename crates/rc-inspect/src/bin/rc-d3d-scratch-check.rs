use rc_package::{
    d3d_buffers::State,
    d3d_dynamic::{self, IndexSource, StreamBinding, VertexSource},
    d3d_resize::{Context, UploadResponses},
    d3d_scratch::{Answers, Runtime},
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
        array: Answers,
        binding: StreamBinding,
    },
    Index {
        source: IndexSource,
        answers: UploadResponses,
        context: Context,
        array: Answers,
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
            array,
            binding,
        } => {
            let transfer = r.vertex(&source, context, &answers, array, &mut next.target)?;
            d3d_dynamic::bind_vertex(
                &mut next.state,
                &mut r.buffers.deferred,
                &mut r.buffers.vertex,
                &mut r.buffers.device,
                binding,
            )?;
            serde_json::json!({"transfer":transfer,"pool":null})
        }
        Step::Index {
            source,
            answers,
            context,
            array,
            base,
            reported_size,
        } => {
            let transfer = r.index(&source, context, &answers, array, &mut next.target)?;
            d3d_dynamic::bind_index(
                &mut next.state,
                &mut r.buffers.deferred,
                &r.buffers.index,
                &mut r.buffers.device,
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
            &serde_json::json!({"source_sha256":input.source_sha256,"probes":probes,"errors":errors,"scope":"Original stream handle invalidation, dynamic vertex pair and index resize Create/Release/Retry/Evict planning, automatic resize-to-ring/direct CPU upload and resolved renderer binding. Explicit COM/getter/fill/shader answers; shared scratch growth/guard/fill/copy included; explicit array-helper pointers, no actual GPU/allocator, native error logging, pool construction, dynamic draw offsets or Android execution."}),
        )?,
    )?;
    Ok(())
}
