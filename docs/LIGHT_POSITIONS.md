# Light positions, shared inverse and original material constants

Three tasks implement light positions, share the native object-inverse cache
with case7, and exercise every serialized DynamicHologram VS/PS binding through
the reconstructed bank dispatcher. This is a diagnostic material milestone,
with explicit host scene inputs and initial register seeds.

## Task 1: four light positions

Cases 14/17/20/23 use logical light indices 0..3 through the previously verified
sparse source table. A request beyond the counted total writes
[10000000,10000000,10000000,0], using native constant 1007266c. A hole inside
the counted total returns the existing safe slot -1 error. Neither path requests
an inverse matrix.

For a selected source, actor byte +2a chooses the position path. Values other
than 0x13 use the three raw floats at source +18/+1c/+20. Value 0x13 instead
constructs a distant point from ObjectToWorld's translation and source direction:

```text
point[k] = ObjectToWorld[3][k] - direction[k] * 65365.0f32
```

The unusual distance is the exact original float at 10072670, not a rounded
65535 or an inferred directional-light convention. Direction is not normalized.
Each multiply and subtraction rounds separately to f32. Native branch evidence
is at 1001e831..e89e; the ordinary position branch begins at 1001e996.

The point is transformed with the inverse ObjectToWorld matrix. For output X,
ordinary positions sum products in k order 2,1,0, while directional positions
use 0,2,1. Y and Z use 0,2,1 for both paths. The corresponding inverse translation
is added last. There is no perspective divide and output W is forced to 1.
The inverse's fourth column does not participate. Native arithmetic is archived
at 1001e8da..e977 and 1001e9c6..ea76.

Nonfinite points, products or sums return explicit safe host errors. The original
would propagate such arithmetic under masked exceptions. This bounded API does
not claim equivalence for those excluded values. The destination register is
written only after successful evaluation; earlier bank writes/count remain.

## Task 2: shared object inverse and case7

Light position and WorldToObject case7 share the native cache represented by
the stack flag at EBP-1c and matrix at EBP-6c. It is computed lazily once per
bank update and reset on the next update. Case7 (1001d5f7) uploads the raw
transpose of the cached inverse to four registers, including arbitrary fourth
column words. Matrix continuation bindings are skipped.

For the directional branch, point preparation precedes the lazy inverse call;
for an ordinary position, the inverse call precedes loading/evaluating the point.
The error-path tests preserve this distinction. Inverse callback failures retain
previous writes and do not populate the cache. Camera inversion uses a separate
cache, even within the same update. EyePositionObjectSpace case33 remains open.

The driver's import at 1006f250 is FMatrix::Inverse, exported by Core at
10143b40. The material diagnostic reuses the existing independently validated
`skeletal_matrix_inverse::inverse_matrix` reconstruction. It does not introduce
a second matrix inversion algorithm. The generic bank retains an external
inverse callback so its ownership/failure behavior remains explicit.

## Task 3: all serialized DynamicHologram bindings

`rc-hologram-constant-check` loads the original HardwareShaders package, all
twenty serialized VS binding records, both PS records and the original 53-step
vertex program. It constructs 96/8-register bindings and updates persistent banks
using the reconstructed dispatcher. The resulting cached counts are 32 and 2.
All matrix, light, eye, CosTime, material-defined and Flicker values are produced
through those cases; no register is overwritten after a dispatch.

The diagnostic supplies explicit object/view/projection/camera matrices, three
valid lights, engine time and a deterministic host CRT15 sequence. It alternates
ordinary and directional light kinds. Object inversion uses the existing Core
reconstruction. Flicker state is shared by the banks and persists across updates.
The original shader is evaluated at one diagnostic vertex per update.

Initial register words are explicitly zero except c17=[0.25;4]. c17 has no
serialized binding and stays unchanged. This seed is a fixture, not a recovered
live value. The original c18 scan thresholds are retained. Serialized PS values
are used directly; HsHologram wrapper changes, render-state ownership and the
game's actual register history are not inferred here. The earlier rendered
mesh preview is not silently replaced by these point probes.

## Verification and reproduction

- 571 workspace tests, Clippy (`-D warnings`), formatting and whitespace checks pass.
- 1,024 synthetic position/cache contexts, 786,432 raw bank words, 1,108 inverse
  callback requests and 716 expected sparse/callback errors checked independently.
- 1,024 scalar position fixtures: all 7,168 point/output words match both the
  native arithmetic instruction interpreter and a separate offline SSE probe.
- 256 complete original VS/PS cycles (512 bank dispatches), checking 212,992
  before/after bank words and 189 shared RNG draws.
- 256 original vertex shader outputs and defined-component masks independently
  verified from the generated constants.
- Debug/Release reports and position binary inputs are byte-identical.

The generic callback fixtures deliberately contain arbitrary inverse matrices,
including NaNs in the unused fourth column; they are not mathematical inverse
claims. The material diagnostic's inverse results are independently replayed
from Core's original instructions. Matrix composition and shader evaluation use
the separately written existing Python instruction/algorithm oracles, read-only.
Original package/DLL and current source/report hashes are recorded.

The SSE probe uses MXCSR=1f80, nearest rounding and gradual underflow, restoring
the caller's state afterwards. It does not execute original DLL code or establish
the game's live rounding settings or universal Android/GPU precision equivalence.

```powershell
cargo run -p rc-inspect --bin rc-light-position-check -- analysis/reports/light-positions.json analysis/reports/light-positions.input.bin
cargo run --release -p rc-inspect --bin rc-light-position-check -- analysis/reports/light-positions-release.json analysis/reports/light-positions-release.input.bin
rustc scripts/Probe-LightPositions.rs -o analysis/reports/probe-light-positions.exe
./analysis/reports/probe-light-positions.exe analysis/reports/light-positions.input.bin analysis/reports/light-positions.sse.bin
cargo run -p rc-inspect --bin rc-hologram-constant-check -- 'D:\SteamLibrary\steamapps\common\Star Wars Republic Commando\GameData' analysis/reports/hologram-constants.json
cargo run --release -p rc-inspect --bin rc-hologram-constant-check -- 'D:\SteamLibrary\steamapps\common\Star Wars Republic Commando\GameData' analysis/reports/hologram-constants-release.json
cargo test --workspace 2>&1 | Tee-Object analysis/reports/light-positions-tests.log
python scripts/Record-LightPositions.py
cargo clippy --workspace --all-targets -- -D warnings
cargo fmt --all -- --check
```

Evidence: analysis/reports/light-positions-validation.json and
light_positions_validation in analysis/evidence.json. Light radius/attenuation,
case33 and other remaining constant cases, live scene acquisition, wrapper/render
state integration and actual unused-register history remain open. General diffuse
coverage remains 271/289. Android verification stays last.
