use rc_package::renderer_transform::{InverseAnswer, RendererRuntime as Runtime, Request, Scene};
use serde::Deserialize;
use std::{env, fs};
#[derive(Deserialize)]
struct Case {
    id: usize,
    runtime: Runtime,
    requests: Vec<Request>,
}
#[derive(Deserialize)]
struct Input {
    source_sha256: String,
    setters: Vec<SetterCase>,
    passes_sha256: String,
    cases: Vec<Case>,
    errors: Vec<Case>,
}
#[derive(Deserialize)]
struct Setter {
    slot: u32,
    matrix: Option<[u32; 16]>,
    inverse: InverseAnswer,
}
#[derive(Deserialize)]
struct SetterCase {
    id: usize,
    scene: Scene,
    deferred: rc_package::d3d_bindings::Deferred,
    requests: Vec<Setter>,
    capacity: usize,
    stencil_gate: bool,
}
fn main() -> Result<(), Box<dyn std::error::Error>> {
    let args: Vec<_> = env::args().collect();
    if args.len() != 3 {
        return Err("Expected INPUT OUTPUT".into());
    }
    let input: Input = serde_json::from_slice(&fs::read(&args[1])?)?;
    let mut setters = vec![];
    for mut c in input.setters {
        let mut steps = vec![];
        for q in c.requests {
            let result = c.scene.set(&mut c.deferred, q.slot, q.matrix, &q.inverse);
            steps.push(serde_json::json!({"result":result,"scene":c.scene,"deferred":c.deferred}));
        }
        let plan = c
            .deferred
            .transforms
            .flush(&mut c.deferred.states, c.capacity, c.stencil_gate);
        let repeated =
            c.deferred
                .transforms
                .flush(&mut c.deferred.states, c.capacity, c.stencil_gate);
        setters.push(serde_json::json!({"id":c.id,"steps":steps,"plan":plan,"repeated":repeated,"scene":c.scene,"deferred":c.deferred}));
    }
    let (mut probes, mut errors) = (vec![], vec![]);
    for (safe, cases) in [(false, input.cases), (true, input.errors)] {
        for c in cases {
            let mut rt = c.runtime;
            let mut steps = vec![];
            for mut q in c.requests {
                let result = rt.submit(&mut q);
                if !safe {
                    if let Err(e) = &result {
                        return Err(e.clone().into());
                    }
                }
                steps.push(serde_json::json!({"result":result,"runtime":rt,"request":q}));
            }
            let value = serde_json::json!({"id":c.id,"steps":steps,"runtime":rt});
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
            &serde_json::json!({"source_sha256":input.source_sha256,"passes_sha256":input.passes_sha256,"setters":setters,"probes":probes,"errors":errors,"scope":"Original renderer +48 object/view/projection copies and deferred masks, transform flush and joined MeshToWorld/render CPU path. Inverse dispatch supplied as explicit write/no-write response; full scene/runtime/request rollback. Camera/tangent/assets/allocator/material/COM answers supplied. No live GPU or Android."}),
        )?,
    )?;
    Ok(())
}
