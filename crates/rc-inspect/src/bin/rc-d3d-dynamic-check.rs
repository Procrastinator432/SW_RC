use rc_package::{
    d3d_bindings::Deferred,
    d3d_buffers::State,
    d3d_dynamic::{
        self, Device, Index, IndexSource, Responses, StreamBinding, Vertex, VertexSource,
    },
};
use serde::{Deserialize, Serialize};
use std::{env, fs};
#[derive(Clone, Serialize, Deserialize)]
struct World {
    vertex: Vertex,
    index: Index,
    device: Device,
    state: State,
    deferred: Deferred,
    target: Vec<u8>,
}
#[derive(Deserialize)]
#[serde(tag = "kind", rename_all = "snake_case")]
enum Step {
    Vertex {
        source: VertexSource,
        responses: Responses,
        binding: StreamBinding,
    },
    Index {
        source: IndexSource,
        responses: Responses,
        base: u32,
        reported_size: u32,
    },
}
fn apply(world: &mut World, step: Step) -> Result<serde_json::Value, String> {
    let mut next = world.clone();
    let result = match step {
        Step::Vertex {
            source,
            responses,
            binding,
        } => {
            let transfer = d3d_dynamic::vertex(
                &mut next.vertex,
                &mut next.device,
                &source,
                &responses,
                &mut next.target,
            )?;
            d3d_dynamic::bind_vertex(
                &mut next.state,
                &mut next.deferred,
                &mut next.vertex,
                &mut next.device,
                binding,
            )?;
            serde_json::json!({"transfer":transfer,"pool":null})
        }
        Step::Index {
            source,
            responses,
            base,
            reported_size,
        } => {
            let transfer = d3d_dynamic::index(
                &mut next.index,
                &next.device,
                &source,
                &responses,
                &mut next.target,
            )?;
            d3d_dynamic::bind_index(
                &mut next.state,
                &mut next.deferred,
                &next.index,
                &mut next.device,
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
    let mut probes = vec![];
    let mut errors = vec![];
    for (safe, cases) in [(false, input.cases), (true, input.errors)] {
        for c in cases {
            let mut world = c.world;
            let mut steps = vec![];
            for s in c.steps {
                let result = apply(&mut world, s);
                if !safe {
                    if let Err(error) = &result {
                        return Err(error.clone().into());
                    }
                }
                steps.push(serde_json::json!({"result":result,"world":world}));
            }
            let value = serde_json::json!({"id":c.id,"steps":steps,"world":world});
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
            &serde_json::json!({"source_sha256":input.source_sha256,"probes":probes,"errors":errors,"scope":"Dynamic vertex/index ring arithmetic, direct lock/source/unlock CPU mirrors and resolved dynamic stream/index renderer handoff. Explicit resize handles, payload/repeated-getter/shader answers; scratch mode, resize helper execution, allocator/GPU, draw-offset integration, live game and Android excluded."}),
        )?,
    )?;
    Ok(())
}
