# Native light selection, colors and spotlight constants

Follow-up: [Light positions and original material constants](LIGHT_POSITIONS.md)
adds light positions, the shared object inverse/case7 and complete serialized
DynamicHologram VS/PS dispatch with explicit host fixtures. Live scene and
render-state integration remain open.

Three tasks extend the constant bank with the original driver's source-index
table, light colors/ambient color, and spotlight direction/cone parameters.
`shader_lights` contains the raw host fields and bounded operations;
`shader_constants::Host::lighting` supplies them to bank dispatch. This does
not yet implement light position/radius or acquire lights from a live scene.

## Task 1: native four-source table

The native shared light branch starts at D3DDrv 1001e467. Four source pointers
come from render state +328..334. A source is counted only when both its pointer
and its first actor pointer are non-null. The first pass counts actor cone byte
+39 != 0; the second counts cone==0. Each eligible source writes its original
index at `[EBP + source_index*8 - 600]`; the two writes are at 1001e49b and
1001e4d3. The index fields initially contain -1 (prologue 1001c210..21f).

This is not a packed list or a spotlight-priority reorder. The count is packed
but the indices remain at original source positions. With sources 1 and 2
valid, count=2 and indices=[-1,1,2,-1]. A request for Light1 would read index
-1, Light2 reads source1, and Light3 receives the missing-light fallback despite
source2 being present. Rust preserves these distinctions. It returns an explicit
error rather than reading source slot -1 (native state+324, outside the modeled
four slots). It does not silently select the next available source.

`Lighting::slots` distinguishes a null source (`None`) from a source with a null
actor (`Some` with actor_present=false). `LightCache` reproduces count and index
fields. Bank dispatch creates this cache lazily on the first supported light
case and resets it on the next update. Its context is stable within an update;
native pointer mutation during dispatch is outside this host model.

## Task 2: light colors and ambient color

Cases 15/18/21/24 request logical light indices 0/1/2/3. A request at or beyond
the counted total yields [0,0,0,0]. An index hole inside that total is the safe
error above. For a selected source, color is a raw FPlane at source+8, and
brightness is a float at source+48. Two calls through import 1006f24c compute
`(color * 2.0f32) * brightness`, rounding and storing after each multiplication.
The imported Core FPlane::operator* at 10114960 uses scalar MULSS for each
component, as separately exported in shader-light-plane.c/.asm.

RGB is not clamped. W is overwritten after the arithmetic: it is 1 only when
the per-dispatch alpha gate is false and both raw source words +38/+3c are
zero; otherwise it is 0. The original color W cannot determine the result.
`Lighting::alpha_gate` is the host-provided native flag: it is set when the
GCubemapManager actor (+2c) exists and its flags at +64 do not contain 0x2000. Acquiring
those pointers/flags from the scene remains host work.

Case 26 (1001e3f2) reads bytes B,G,R at render state +148,+149,+14a and writes
RGB multiplied by the stored f32 factor 0x3c008081 (2/255). Ambient W is 2.0;
the fourth input byte is ignored. Ambient does not request the light cache.

Nonfinite RGB/brightness or multiplication overflow are explicit safe host
errors, rather than claims that the original rejected them. Color W is discarded
and can contain arbitrary raw words. The current register is written only after
successful arithmetic; prior register writes and cached bank count remain.

## Task 3: spotlight direction and cone

Cases 28/29 use logical index zero through the same sparse table. With no
counted light, or with source0 cone==0, direction is [1,0,0,1] and cone is
[0,0,0,1]. A nonzero cone selects the first source's raw direction words at
+24,+28,+2c, with W forced to 1. No normalization or coordinate transform is
performed; signed zero and NaN payloads in direction are copied unchanged.

Case 29 (1001e5cd..61f) does not execute a cosine instruction. With cone byte b,
it performs these separate f32 operations using exact original PE constants:

```text
s = float(b) * float_from_bits(0x3a800142)
t = 1.0 - s
x = (t * t) * float_from_bits(0x3f89999a)
y = x * float_from_bits(0x3dcccccd)
output = [x, y, 1.0 / y, 1.0]
```

The source cone byte and direction are explicit host fields. Their native
actor/light meanings and acquisition are not inferred beyond the observed reads.

## Validation and reproduction

- 566 workspace tests pass, as do Clippy (`-D warnings`), formatting and
  whitespace checks.
- 1,024 host contexts cover all 256 combinations of null source, null actor,
  valid zero-cone source and valid nonzero-cone source across four slots.
- 4,096 independently regenerated selections; 7,168 isolated bank updates and
  2,048 combined/repeated updates, checking 524,288 raw bank words.
- 1,664 expected isolated sparse-table errors and four arithmetic errors retain
  the documented partial bank state. Combined updates also validate the first
  error and rebuild selection when the next update has empty sources.
- 1,024 scalar probes cover all 256 cone bytes. Their 11,264 output words match
  a separate offline SSE probe at MXCSR=1f80: nearest rounding, gradual underflow.
  This probe restores the caller's MXCSR and does not execute original DLLs.
- Debug/Release reports are byte-identical. Existing raw-bank and matrix reports
  and matrix binary inputs remain byte-identical after the added host field.

`Record-ShaderLights.py` reads original PE constants, dispatcher targets, sparse
index-store bytes and the FPlane import, regenerates the fixtures and applies
separately written selection/arithmetic/bank rules. It does not import the Rust
implementation. Original Core/D3DDrv hashes and source/report hashes are recorded.
This establishes the checked arithmetic under the stated rounding environment;
it does not establish the game's live MXCSR or general GPU precision equivalence.

```powershell
cargo run -p rc-inspect --bin rc-shader-light-check -- analysis/reports/shader-lights.json analysis/reports/shader-lights.input.bin
cargo run --release -p rc-inspect --bin rc-shader-light-check -- analysis/reports/shader-lights-release.json analysis/reports/shader-lights-release.input.bin
rustc scripts/Probe-ShaderLights.rs -o analysis/reports/probe-shader-lights.exe
./analysis/reports/probe-shader-lights.exe analysis/reports/shader-lights.input.bin analysis/reports/shader-lights.sse.bin
cargo run -p rc-inspect --bin rc-constant-bank-check -- analysis/reports/constant-banks-light-regression.json
cargo run -p rc-inspect --bin rc-shader-matrix-check -- analysis/reports/shader-matrices-light-regression.json analysis/reports/shader-matrices-light-regression.input.bin
cargo test --workspace 2>&1 | Tee-Object analysis/reports/shader-lights-tests.log
python scripts/Record-ShaderLights.py
cargo clippy --workspace --all-targets -- -D warnings
cargo fmt --all -- --check
```

Evidence: analysis/reports/shader-lights-validation.json and
shader_lights_validation in analysis/evidence.json. DynamicHologram's kind21
LightColor3 is now supported; kind14 LightPos1 still blocks its complete native
constant dispatch. Position/radius, scene light/camera/matrix acquisition,
complete material binding and register history remain open. The earlier mesh
preview keeps its explicit fixtures. Diffuse coverage remains 271/289. Android
verification stays last.
