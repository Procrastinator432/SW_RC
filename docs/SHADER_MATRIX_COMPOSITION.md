# Native composed shader matrices

Follow-up: [Native light constants](SHADER_LIGHTS.md) reconstructs light selection,
colors (including kind21), ambient and spotlight cases. Kind14 light position
and live scene acquisition still prevent complete DynamicHologram setup.

Three tasks extend `shader_constants::Bank` with the original driver's matrix
cases 2 (WorldToScreen), 3 (ObjectToScreen), and 32 (ObjectToCamera). All three
now upload four consecutive registers and skip continuation bindings. Live
scene matrices remain host inputs; this is not a playable-game milestone.

## Task 1: WorldToScreen

Native case 2 starts at D3DDrv 1001c2b3. Render state `*(this+9c0c)` contains
WorldToCamera at +6c and Projection at +ac. The driver computes their row-major
product, then transposes its sixteen stored f32 words into the output buffer.
`world_to_screen(view, projection)` returns that uploaded representation.

The unrolled scalar-SSE additions have a different order for different output
elements. For example, element (0,0) adds products in k order 2,1,0,3, whereas
(0,1) uses 3,1,0,2. A generic left-to-right dot product changes some results.

## Task 2: ObjectToScreen

Native case 3 starts at 1001c8f1. ObjectToWorld comes from render state +2c.
The original first computes ObjectToWorld * WorldToCamera and stores all
sixteen f32 results. It copies this intermediate matrix, multiplies it by
Projection with a second distinct instruction sequence, and transposes the
result. `object_to_screen(object, view, projection)` preserves both stages,
including the intermediate f32 rounding. It does not reassociate the chain.

The first stage's (0,0) dot order is 3,2,1,0; the projection stage's is
1,2,0,3. The projected fourth component is retained without a perspective
divide. The existing vertex shader and rasterizer handle later stages.

## Task 3: ObjectToCamera and bank integration

Native case 32 starts at 1001d718. It computes ObjectToWorld * WorldToCamera,
then transposes it. Its per-element instruction order differs from the first
stage of case 3, despite the same mathematical formula. For example, its
(0,0) order is 0,3,2,1. `object_to_camera(object, view)` follows this sequence.

`Host` now supplies a raw Projection matrix alongside ObjectToWorld and
WorldToCamera. Bank dispatch supports cases 2,3,32 using the same native
four-register span rules as earlier matrix cases. An unused binding leaves
old words untouched. Continuation bindings are skipped, including unknown
kinds. Cached iteration counts can end inside an uploaded matrix; the full
four rows are still written when the allocated bank has room.

Finite inputs and every f32 product/sum are required. Nonfinite values or
overflow return explicit host errors, whereas the original scalar SSE would
propagate nonfinite results. Arithmetic completes before the matrix is
uploaded, so these errors leave that matrix's destination words unchanged;
earlier register writes and cached counts remain. Buffer overruns also return
errors. Matrix inversion is still an external host operation.

## Verification

`Generate-ShaderMatrixOrders.py` symbolically follows the archived native
MOVSS/MOVAPS/MULSS/ADDSS instructions across four stages, asserts all 64 dot
products contain the correct row/column and each k exactly once, and emits
the compact Rust order tables. It also creates an offline x86_64 SSE probe
by relocating the original scalar arithmetic's memory operands. That probe
sets MXCSR=1f80 (nearest rounding, gradual underflow, masked exceptions),
restores the caller's MXCSR afterwards, and does not execute the original DLL.

The independent `Record-ShaderMatrices.py` numerically interprets the archived
instruction stream without reading the Rust order tables. It regenerates
all raw inputs, validates native PE switch targets and shared upload jumps,
and compares Rust output against both the interpreter and the offline SSE
probe. Arbitrary finite matrices include cancellation, signed zero and
subnormal inputs. They are synthetic arithmetic fixtures, not live transforms.

Results:

- 1,024 sets / 3,072 input and output matrices.
- All 49,152 output words match the independent interpreter and SSE probe.
- 3,072 bank dispatches, checking 1,277,952 raw before/after register words.
- Twelve expected errors cover nonfinite input, product/sum overflow and
  safe buffer limits for all three new cases, preserving prior writes.
- 560 workspace tests pass, including analytic affine/perspective composition,
  case-specific cancellation, continuation skipping and error state.
- Optimized Release and Debug reports and binary inputs are byte-identical.
- Clippy (`-D warnings`), formatting and whitespace checks pass.

These checks establish the recorded cases under nearest rounding and gradual
underflow. They do not establish the original game's live MXCSR settings,
universal GPU precision equivalence or native behavior for nonfinite matrices.

```powershell
python scripts/Generate-ShaderMatrixOrders.py
cargo fmt --all
cargo run -p rc-inspect --bin rc-shader-matrix-check -- analysis/reports/shader-matrices.json analysis/reports/shader-matrices.input.bin
cargo run --release -p rc-inspect --bin rc-shader-matrix-check -- analysis/reports/shader-matrices-release.json analysis/reports/shader-matrices-release.input.bin
rustc scripts/Probe-ShaderMatrices.rs -o analysis/reports/probe-shader-matrices.exe
./analysis/reports/probe-shader-matrices.exe analysis/reports/shader-matrices.input.bin analysis/reports/shader-matrices.sse.bin
cargo test --workspace 2>&1 | Tee-Object analysis/reports/shader-matrices-tests.log
python scripts/Record-ShaderMatrices.py
cargo clippy --workspace --all-targets -- -D warnings
cargo fmt --all -- --check
```

Evidence: `analysis/reports/shader-matrices-validation.json` and
`shader_matrices_validation` in `analysis/evidence.json`. Native light cases
14/21 still prevent a complete DynamicHologram constant dispatch. Its preview
therefore retains the separately documented explicit constants. Live actor,
camera and projection acquisition, buffer history and scene integration remain
open. General diffuse coverage remains 271/289. Android verification stays last.
