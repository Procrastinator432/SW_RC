# Stateful TexOscillator jitter

Three tasks completed: native dual-axis jitter state transitions, a shared
random-stream host boundary, and persistent original material/renderer binding.

## Native branch behavior

The two remaining TexOscillator slots both use `OT_Jitter` on both axes of
`VehicleTextures.Dropship.DropshipEngine_Osc`, reached through the engine shader.
Their parameters are U rate 80, V rate zero, U phase one, U amplitude one,
V amplitude zero. The original stored current offsets are retained until an
update actually generates new values; zero V amplitude does not erase its
stored offset immediately. Cached M is ignored by the native matrix calculation.

`UTexOscillator::GetMatrix` at 10453960 evaluates U before V. For either axis,
the phase is f32(rate*time). A reset sets LastS to float(floor(phase))+offset
phase. The native temporary MXCSR 0x3f80 selects rounding toward negative
infinity for CVTSS2SI; it does not select nearest rounding.

U resets when LastSu is below one or above phaseU+one. V resets when LastSv is
below one or **LastSu** is above phaseV+one, using U's already updated state.
This cross-axis read is visible at 10453b86 and is intentionally preserved.
New random values are consumed only if phase-LastS is strictly above one.
Every random update uses separate f32 operations: rand*amplitude followed by
multiplication with the original PE constant at 10664f44 (bits 38000100).
It consumes a random value even when amplitude is zero.

The resulting matrix has unit scales and the held CurrentU/VJitter offsets.
Only dual-axis jitter with zero pivots is implemented in this API. Periodic
pan/stretch oscillator modes, mixed modes and nonzero pivot matrix products
remain explicit errors rather than inferred equivalents.

## Runtime and host boundary

`TexJitter::from_properties` overlays original Engine.u defaults, zero fields
and serialized instance parameters/state. `TexJitter::step` handles the state
transitions and accepts a callback for the shared CRT rand stream (0..32767).
The global game seed, interleaving with other consumers and CRT implementation
are not supplied or reconstructed by this turn. Tests use explicit fixture
sequences and verify exact call counts/order, not original random outcomes.

`Assets::diffuse_with_jitter` receives a caller-owned `MaterialState` and random
callback. It initializes state once per qualified oscillator object and keeps
it across sampling calls. Lowercase keys share the state across case variants
and shaders referencing the same oscillator. Stateless APIs retain an omission.
Callers must evaluate material sampling at the intended frequency: state and
random consumption are observable. Completed state writes survive callback
failure; consumed random values cannot be rolled back. Invalid input/state,
unhandled modes/pivots and out-of-range integer phases are bounded errors.
Script-driven parameter changes, instance lifecycle and live game integration
remain outside this offline resolver.

## Evidence and diagnostic rendering

Two previously omitted slots resolve with the stateful API. The combined API
coverage is 271 of 289 original used skeletal material slots, with 18 omissions:
ten HsHologram, three null selections, three missing exports, one Shader without
Diffuse and one ConstantColor. These are base-diffuse counts, not complete
shader/game functionality or a completion percentage.

`rc-jitter-check` samples the original material at 0, 0.05, 0.075 and 0.1 seconds
with explicit random fixtures. It binds 16384 original decoded pixels to
diagnostic quads and records state, UV matrix and random cursor for every call.
At those times the cursor is [0,1,1,2]: the middle two snapshots/images hold the
same offset, and the four frames produce three distinct images. No original
opacity/self-illumination pass or rendered dropship actor is claimed.

`Record-Jitter.py` independently reads original properties/defaults and the PE
rand scale, decodes pixels, checks every snapshot's geometry/UV/pixels and
compares 3072 synthetic state transitions with 1414 total checked random calls
(including the original samples). Panner and Panner2D regression reports match
their earlier verified hashes exactly. PNG conversion is lossless and a frame
was visually inspected. Hashes: `analysis/reports/jitter-validation.json`.

517 workspace tests, Clippy with denied warnings and formatting pass.

```powershell
cargo run -p rc-inspect --bin rc-jitter-check -- analysis/reports/skeletal-materials.json 'D:\SteamLibrary\steamapps\common\Star Wars Republic Commando\GameData' analysis/reports/jitter-materials.json
cargo run -p rc-inspect --bin rc-panner-check -- analysis/reports/skeletal-materials.json 'D:\SteamLibrary\steamapps\common\Star Wars Republic Commando\GameData' analysis/reports/panner-jitter-regression.json
cargo run -p rc-inspect --bin rc-panner2d-check -- analysis/reports/skeletal-materials.json 'D:\SteamLibrary\steamapps\common\Star Wars Republic Commando\GameData' analysis/reports/panner2d-jitter-regression.json
cargo test --workspace | Tee-Object -FilePath analysis/reports/jitter-tests.log
python scripts/Record-Jitter.py
cargo clippy --workspace --all-targets -- -D warnings
cargo fmt --all -- --check
```

Android verification remains at the end. Hologram effects, general UV modifier
composition, remaining material passes and live game systems remain open.

Follow-up: `MATERIAL_HOLOGRAM.md` reconstructs the hologram wrapper/color
contract and inventories original shader programs and input textures. Hardware
shader execution is still open; the prior diffuse coverage is unchanged.
