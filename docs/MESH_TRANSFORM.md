# MeshToWorld and renderer transform handoff

Five related tasks completed:

1. Core `FMatrix(FRotator, FVector)` at 101444a0 and low16 `IsZero` at
   10115130; shared reconstructed 16384-entry native sine table.
2. Engine `ULodMeshInstance::MeshToWorld` at 1044e760: effective scale is
   `mesh_scale * (actor_scale3d * actor_scale)`, with separate f32 rounding.
   DWORD instance+74 suppresses negative mesh-origin translation; byte +78
   bypasses mesh origin/rotation and both actor location adjustments.
3. Optional mesh rotation composition, preserving per-component scalar SSE
   summation orders and zero terms, including signed zero.
4. Actor PrePivot addition or flag 0x800 height correction along actor matrix
   row2. Native FLD/FADD/FSTP stores f32(height + original stored double
   5.850000090897083), then scalar SSE products/subtractions adjust location.
5. Final actor rotation/adjusted-location composition and original caller
   +e8 result pointer, 16-word copies and renderer +48 slot-zero handoff.

The portable module is `crates/rc-package/src/mesh_transform.rs`.
`Input::build` returns scale, base matrix, adjusted location, constructor trace
and final matrix. `Runtime::submit_transformed_vertmesh` joins this to the previous
render adapter. The transform actor snapshot replaces stale legacy camera scale
and position fields. Camera Project still uses the original actor location, not
MeshToWorld's adjusted location. Both original early draw gates occur after the
transform handoff and are covered. All request/runtime updates commit together;
a late downstream error discards pending transform/preprocessing/instance work.
The renderer call is captured as a typed boundary (receiver, +48, slot, matrix);
its body is not executed and the Runtime does not claim a live GPU model matrix.

## Native evidence

The complete MeshToWorld assembly was exported through its RET 4 at 1044f1af;
the previous mesh-transform.asm stopped midway through the mesh product and is
not used for this proof. Core constructor/IsZero bodies are exported separately.
DLL import names, Core export addresses, scalar constants and every sine-table
word are verified against original PE data / independent sine initialization.

512 standalone synthetic transforms cover quantization, wrapping angles, height exponent extremes/cancellation,
low16 zero, all flag routes, negative/zero scale and unused NaN fields behind
bypassed reads. 50 sequences /146 render requests then execute the same original
transform/copy/caller path joined to the prior preprocessing, camera, instance,
frame, index, upload and material draw replay. Complete transform traces,
plans, committed requests and runtime images match word for word.
Eleven Rust-only safe failures verify full rollback, including late index-unlock
and material errors. Debug/Release reports are byte identical. All 920 existing
workspace tests passed; Clippy all targets with -D warnings, format/diff checks
passed. No additional unit-test count is claimed.

The +9c actor accessor is a stable-address response boundary. Three restricted
x87 height instructions use an independent exact rational oracle with nearest
64-bit-significand extended rounding followed by nearest f32 rounding; the replay
VM does not implement general x87/FPTAN. Core matrix and Engine SSE/copy/branch
instructions execute in the checked interpreter; renderer +48 is captured only.
Counts across standalone and integrated transform probes: 658 MeshToWorld calls /
658 +48 handoffs, 1147 Core matrix constructors, 415 IsZero calls, 282 mesh
rotations, 208 PrePivot corrections, 207 height corrections and 243 bypasses.
The 146 joined render requests execute 668527 replay instruction steps and retain
112 enabled frame/upload paths, 14 no-sections /20 no-actor-state returns,
1353 indices and 253 draws /506 passes. Transform-only and handoff counters also
include the 512 standalone cases; component counters overlap and must not be added.

```powershell
python scripts/Generate-MeshTransform.py
cargo run -p rc-inspect --bin rc-mesh-transform-check -- analysis/reports/mesh-transform.input.json analysis/reports/mesh-transform.json
cargo run --release -p rc-inspect --bin rc-mesh-transform-check -- analysis/reports/mesh-transform.input.json analysis/reports/mesh-transform-release.json
cargo test --workspace *> analysis/reports/mesh-transform-tests.log
python scripts/Record-MeshTransform.py
python analysis/finalize-mesh-transform.py
```

Reports: analysis/reports/mesh-transform*; evidence mesh_transform_validation.
Open: renderer +48 implementation, camera matrices/FPTAN, original asset/name
loading, physical allocator ownership/capacity, material compilation and live GPU.
This remains a synthetic CPU reconstruction, not a playable game. Android checks
remain deferred until the end per user instruction.
