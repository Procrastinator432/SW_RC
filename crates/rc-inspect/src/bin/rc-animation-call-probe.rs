//! Prepare original AnimProp requests; examine base dispatch on supplied mesh snapshots.
use rc_package::{
    animation_call::{
        play_anim_base, AnimationArguments, BaseAnimationHost, NativeAnimationWrapper,
        PlayAnimParameters,
    },
    event_lookup::EventNameSnapshot,
    prop_state::{AnimationKind, AnimationRequest},
};
use std::{env, fs, path::Path};
#[derive(Default)]
struct Host {
    events: Vec<&'static str>,
}
impl BaseAnimationHost for Host {
    fn log_missing_mesh(&mut self) -> Result<(), String> {
        self.events.push("LogMissingMesh");
        Ok(())
    }
    fn mesh_get_instance(&mut self) -> Result<(), String> {
        self.events.push("MeshGetInstance");
        Ok(())
    }
    fn actor_mesh_instance_play(&mut self, _: &PlayAnimParameters) -> Result<bool, String> {
        self.events.push("ActorMeshInstancePlay");
        Err("unresolved mesh instance playback".into())
    }
}
fn main() -> Result<(), Box<dyn std::error::Error>> {
    let args: Vec<_> = env::args().collect();
    if args.len() != 4 {
        return Err(
            "Usage: rc-animation-call-probe <GameData> <anim-prop-transitions.json> <report.json>"
                .into(),
        );
    }
    let bytes = fs::read(Path::new(&args[1]).join("System/engine.u"))?;
    let package = rc_package::read_package(&bytes)?;
    let mut functions = Vec::new();
    for path in ["Actor.PlayAnim", "Actor.LoopAnim"] {
        let mut found = None;
        for (i, export) in package.exports.iter().enumerate() {
            if package.object_path(i as i32 + 1)? == path {
                found = Some(rc_package::script::read_function(&package, &bytes, export)?);
                break;
            }
        }
        functions.push(found.ok_or("missing original animation function")?);
    }
    let wrappers = [
        NativeAnimationWrapper::verify("Actor.PlayAnim", &functions[0])?,
        NativeAnimationWrapper::verify("Actor.LoopAnim", &functions[1])?,
    ];
    let original: serde_json::Value = serde_json::from_slice(&fs::read(&args[2])?)?;
    let mut cases = Vec::new();
    for class in original["anim_prop_transition_probes"]["probes"]
        .as_array()
        .ok_or("missing AnimProp probes")?
    {
        for (index, case) in class["cases"]
            .as_array()
            .ok_or("missing cases")?
            .iter()
            .enumerate()
        {
            for request in case["animation_requests"]
                .as_array()
                .ok_or("missing requests")?
            {
                let (kind, wrapper) = match request["kind"].as_str() {
                    Some("PlayAnim") => (AnimationKind::PlayAnim, wrappers[0]),
                    Some("LoopAnim") => (AnimationKind::LoopAnim, wrappers[1]),
                    _ => return Err("unknown animation kind".into()),
                };
                let request = AnimationRequest {
                    kind,
                    native_index: request["native_index"]
                        .as_u64()
                        .ok_or("missing native index")?
                        .try_into()?,
                    name: EventNameSnapshot {
                        handle: request["name"]["handle"]
                            .as_u64()
                            .ok_or("missing handle")?
                            .try_into()?,
                        resolved_index: request["name"]["resolved_index"]
                            .as_i64()
                            .ok_or("missing name index")?
                            .try_into()?,
                    },
                };
                let parameters = wrapper.prepare(request, AnimationArguments::default())?;
                let mut base_cases = Vec::new();
                for (mesh_present, object_flags) in [(false, 0), (false, 0x4000), (true, 0)] {
                    let mut host = Host::default();
                    let outcome =
                        play_anim_base(mesh_present, object_flags, &parameters, &mut host);
                    base_cases.push(serde_json::json!({"mesh_present":mesh_present,"object_flags":object_flags,"outcome":outcome,"events":host.events}));
                }
                cases.push(serde_json::json!({"class":class["class"],"source_case":index,"source_outcome":case["outcome"],"request":request,"parameters":parameters,"base_cases":base_cases}));
            }
        }
    }
    let report = serde_json::json!({"scope":"Original function signatures and AnimProp requests; omitted arguments prepared from reviewed native locals. Mesh flags and AActor base dispatch target explicitly supplied diagnostic scenarios, not resolved game objects. Mesh playback remains unresolved; previous state outcomes unchanged.","functions":functions,"cases":cases});
    fs::write(&args[3], serde_json::to_string_pretty(&report)? + "\n")?;
    println!("{} animation requests prepared", cases.len());
    Ok(())
}
