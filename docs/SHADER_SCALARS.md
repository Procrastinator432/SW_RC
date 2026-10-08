# Native shader time, CosTime and shared flicker

Follow-up: [Raw matrix/camera uploads and persistent constant banks](CONSTANT_BANKS.md)
now combine these scalar routines with verified upload/retention rules. Full
matrix composition, inversion and live scene integration remain open.

Three tasks reconstruct D3DDrv constant cases 8 (Time), 9 (CosTime) and
27 (Flicker). `rc-package::material_constants` exposes portable routines with
explicit finite-input contracts. Matrices, lights, camera state, persistent
constant-buffer contents and the live engine clock/random source remain host
responsibilities. Existing mesh preview fixtures are unchanged by these APIs.

## Native evidence

The engine's FRenderInterface::SetHardwareShaderMaterial at 10356360 is a base
stub. The original `GameData/System/D3DDrv.dll` supplies the implementation.
Its SetShaderConstants dispatcher starts at 1001c130, with the shared switch
body at 1001c260. Ghidra separated the prologue from that body; the body alone
has misleading undefined frame variables in pseudocode. Instruction evidence
is therefore retained alongside it:

- `analysis/decompiled/shader-constants.c`
- `analysis/decompiled/shader-constants.asm`
- `analysis/decompiled/shader-constant-references.txt`

The driver was imported/analyzed in the existing RepublicCommando Ghidra
project. The original DLL hash and relevant data/import bytes are recorded in
the independent validation. The earlier render-interface stub and constructor
inspection are in `analysis/decompiled/hardware-constants.c`.

## Task 1: signed shader time

The prologue loads f32 GEngineTime and double 120.0, calls the imported
MSVCR71 `_CIfmod` thunk at 1006c238, then stores a f32 time. Case 8 copies
that one value into all four components. `shader_time` uses signed remainder:
240.5 becomes 0.5, -121.25 becomes -1.25, and -120 preserves negative zero.
This is not a positive wrap into [0,120). Nonfinite engine times are rejected.

## Task 2: CosTime

Case 9 at 1001dfa8 uses Value.X as frequency, substituting 1 when X==0.
It multiplies the wrapped f32 time and f32 frequency on the x87 stack, applies
FCOS and writes the result to all four components of **one** register. It
does not fill the following register. In particular, original DynamicHologram
binds CosTime at c16 but has no serialized c17 binding; native persistent buffer
contents for c17 are still unresolved.

`cos_time` uses the exact f64 product of two f32 values, f64 cosine and a f32
result. It rejects nonfinite inputs and absolute arguments above 8192. That
last limit is an explicit diagnostic bound, not a native engine restriction.
Portable transcendental implementations and original FCOS range reduction are
not universally equivalent. On the 1,024 recorded samples, all f32 result
words match a separately compiled x87 instruction probe and Python's independent
math evaluation. This sample evidence does not establish every possible input
or Android library's last-bit behavior.

## Task 3: shared flicker state

Case 27 at 1001eb4b uses Value.X as threshold and Value.Y as amplitude; Z/W
are ignored. Three DLL globals A/B/C at 100801a0/1008019c/10080198 are
initialized to 0.5. A global flag and cached f32 time start at zero.

On the first flicker evaluation, the flag is set and current **unwrapped**
GEngineTime is cached. No random values are drawn. Whenever a subsequent call
has a different f32 engine time, the time is updated first, then three CRT
rand values are consumed in A/B/C order. Each sample is f32(rand) multiplied
by the original f32 normalization word 38000100 at 10072370. Identical times,
including +0/-0 equality, reuse all three samples across different materials
and constants. Backward time changes also refresh. Time changes separated by
120 seconds still refresh, even though shader_time has the same remainder.

For threshold t, amplitude m and base=f32(1-m), the output is:

```text
x = 1 if A <= t else f32(f32(m*C) + base)
y = 1 if B <= t else f32(f32(m*A) + base)
z = 1 if C <= t else f32(f32(m*B) + base)
w = 1
```

The native assembly uses separate SSE MULSS/ADDSS operations. Threshold and
amplitude are not clamped; negative outputs are possible for explicit inputs.
Original DynamicHologram uses this kind at c21, c22 and c25. Their stored
threshold/amplitude values are approximately (0.99,0.3), (0.1,0.4) and (0,1).

One `Flicker` instance must be shared across all constants/materials belonging
to the original DLL-global lifetime. Creating one instance per material would
change native behavior. The host owns the actual global CRT sequence and call
order; the recorded diagnostic sequence is not claimed as the live game RNG.
Only finite time/threshold/amplitude and CRT values 0..32767 are accepted.
Callback errors retain the updated time and samples consumed so far; native
rand cannot fail, so that is explicitly a Rust host-error contract.

## Validation and reproduction

550 workspace tests, Clippy (`-D warnings`) and formatting pass. The independent
validator reads original tagged bindings and original PE constants/seeds/imports.
It verifies 1,024 time/CosTime probes (including signed zero, cycle boundaries,
negative rates and maximum finite times), plus 1,024 interleaved shared-flicker
evaluations with 765 random draws and all state/output words.

The offline `Probe-ShaderScalars.rs` runs FPREM with C2 repetition to implement
the signed remainder and then FLD/FMUL/FCOS/FSTP. It sets/restores control word
037f (nearest, extended precision). It does not execute the original DLL or
prove the original process's complete floating-point environment. The f32
frequency/time product is exactly representable in f64 for the diagnostic
portable path; original x87 instructions remain the separate reference probe.

```powershell
cargo run -p rc-inspect --bin rc-constant-check -- 'D:\SteamLibrary\steamapps\common\Star Wars Republic Commando\GameData' analysis/reports/shader-scalars.json
rustc scripts/Probe-ShaderScalars.rs -o analysis/reports/probe-shader-scalars.exe
./analysis/reports/probe-shader-scalars.exe analysis/reports/shader-scalars.input.bin analysis/reports/shader-scalars.x87.bin
cargo test --workspace 2>&1 | Tee-Object analysis/reports/shader-scalars-tests.log
python scripts/Record-ShaderScalars.py
cargo clippy --workspace --all-targets -- -D warnings
cargo fmt --all -- --check
```

Evidence: `analysis/reports/shader-scalars-validation.json` and
shader_scalars_validation in `analysis/evidence.json`. Native matrix/light
constant branches, persistent unwritten buffer slots and connection to live
scene rendering remain open. General diffuse coverage remains 271/289.
Android verification stays at the end.
