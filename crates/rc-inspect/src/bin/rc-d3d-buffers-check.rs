use rc_package::{
    d3d_bindings::Deferred,
    d3d_buffers::{self, Request, Shader, State, Submission},
    d3d_draw::{Context, Draw},
};
use serde::Deserialize;
use std::{env, fs};
#[derive(Deserialize)]
struct Case {
    id: usize,
    state: State,
    deferred: Deferred,
    request: Request,
}
#[derive(Deserialize)]
struct Restore {
    id: usize,
    state: State,
    deferred: Deferred,
    shader: Option<Shader>,
}
#[derive(Deserialize)]
struct Sequence {
    id: usize,
    submission: Submission,
    request: Request,
    context: Context,
    draw: Draw,
}
#[derive(Deserialize)]
struct Input {
    source_sha256: String,
    streams: Vec<Case>,
    indices: Vec<Case>,
    restores: Vec<Restore>,
    cases: Vec<Sequence>,
}
fn main() -> Result<(), Box<dyn std::error::Error>> {
    let args: Vec<_> = env::args().collect();
    if args.len() != 3 {
        return Err("Expected INPUT OUTPUT".into());
    }
    let input: Input = serde_json::from_slice(&fs::read(&args[1])?)?;
    let mut streams = vec![];
    let mut indices = vec![];
    let mut restores = vec![];
    let mut probes = vec![];
    for c in input.streams {
        let mut state = c.state;
        let mut deferred = c.deferred;
        let r = c.request;
        let result = state.set_streams(&mut deferred, &r.streams, r.shader_kind, r.shader, r.frame);
        streams
            .push(serde_json::json!({"id":c.id,"result":result,"state":state,"deferred":deferred}));
    }
    for c in input.indices {
        let mut state = c.state;
        let mut deferred = c.deferred;
        let r = c.request;
        let result = state.set_index(&mut deferred, r.index, r.base_vertex, r.frame);
        indices
            .push(serde_json::json!({"id":c.id,"result":result,"state":state,"deferred":deferred}));
    }
    for c in input.restores {
        let mut state = c.state;
        let mut deferred = c.deferred;
        let result = state.restore_fixed(&mut deferred, c.shader);
        restores
            .push(serde_json::json!({"id":c.id,"result":result,"state":state,"deferred":deferred}));
    }
    for c in input.cases {
        let mut submission = c.submission;
        let result = d3d_buffers::submit(&mut submission, &c.request, c.context, c.draw)?;
        probes.push(serde_json::json!({"id":c.id,"result":result,"submission":submission}));
    }
    fs::write(
        &args[2],
        serde_json::to_vec(
            &serde_json::json!({"source_sha256":input.source_sha256,"streams":streams,"indices":indices,"restores":restores,"probes":probes,"scope":"Resolved static vertex/index handoff: declarations, conditional tail clearing, revision upload decisions, frame usage, full stride/byte shadow, fixed VS restore and bounded buffer-to-pass-to-draw integration. Cache lookup/allocation/upload, shader acquisition, live GPU and Android excluded."}),
        )?,
    )?;
    Ok(())
}
