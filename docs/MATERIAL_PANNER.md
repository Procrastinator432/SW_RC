# Directional TexPanner reconstruction

Three tasks completed: quantized native direction lookup, periodic TexPanner
UV calculation, and original parameter/diffuse/renderer integration.

## Direction table

Core `FRotator::Vector` (10143710) reads 16384 f32 sine entries initialized by
`FGlobalMath` (10143480). Indexing uses arithmetic shift by two and mask 0x3fff;
cosine adds 16384 with wrapping i32 arithmetic before indexing. Roll is ignored.
Direction is [cos(yaw)*cos(pitch), sin(yaw)*cos(pitch), sin(pitch)]. Products
are rounded to f32. Quarter turns retain the original nonzero sine residuals.

The standalone offline generator reads the native float step at 101885d4
(bits 39c90fdb), reproduces FILD/FMUL/FSIN/FSTP on x86_64, and stores a 65536-byte
little-endian table. It saves/restores the x87 control word and uses extended
precision with nearest rounding. The portable runtime includes these words;
it does not need x87, executable memory or the original DLL. Python independently
checks every table entry with a separate mathematical sine evaluation.
This reconstructs the observed initialization; no running-game memory capture
or universal equivalence across all x87 control settings is claimed.

## TexPanner matrix

Engine `UTexPanner::GetMatrix` at 10452050 gets that direction and computes
each UV phase in the original scalar SSE order:
`phase = ((PanRate * direction_axis) * time) * (1/1024)`.
The x87 add-double/extract-low-signed-word/shift reduction matches TexPanner2D;
the residual is multiplied by 1024, not one. Matrix diagonal remains one.
Nonfinite rates/times and overflowing intermediate phases are rejected.

The class default suffix in Engine.u supplies PanRate=0.1; omitted PanDirection
is zero initialized. Instance floats/rotators override class values, including
zero rates. The cached M property is overwritten by native calculation.

## Original material integration

The five previously omitted TexPanner slots all reference the Bacta canister
shader. Its base diffuse chain now resolves in `Assets::diffuse_at_time`.
The scalar-only API still rejects it rather than silently dropping offsets.
Multiple UV modifiers and nontrivial bump-scale composition remain errors.

Four time samples (0, 2, 4, 6 seconds) bind original pixels and sampled UVs to
two diagnostic triangles. Snapshot geometry, all UV bit patterns and texture
pixels are independently checked. Four distinct PPM frames are losslessly
converted to PNG and visually inspected. This shows base texture motion on a
material fixture, not original Bacta transparency/self-illumination effects or
a live rendered pickup actor.

The combined new APIs cover 269 of the original 289 used skeletal slots:
212 previously supported, 52 TexPanner2D and five TexPanner. The 20 remaining
omissions are ten HsHologram, two TexOscillator, three null selections, three
missing exports, one Shader without Diffuse and one ConstantColor. Earlier
reports retain their historical counts. The prior documentation's arithmetic
for TexPanner2D remaining slots is corrected from 37 to 25.

## Verification

510 workspace tests, Clippy with denied warnings and formatting pass.
`Record-Panner.py` checks the original table step and panner PE constants,
complete class-default suffix, all 16384 table entries, 520 additional
direction/phase cases, 4096 original decoded pixels and four snapshots/images.
The unchanged TexPanner2D corpus is rerun and its report hash matches the
previous verified report. Evidence: `analysis/reports/panner-validation.json`.

```powershell
rustc scripts/Generate-MaterialSineTable.rs --edition 2021 -o target/material-sine-table.exe
./target/material-sine-table.exe 'D:\SteamLibrary\steamapps\common\Star Wars Republic Commando\GameData\System\core.dll' crates/rc-package/src/material_sine_table.bin
cargo run -p rc-inspect --bin rc-panner-check -- analysis/reports/skeletal-materials.json 'D:\SteamLibrary\steamapps\common\Star Wars Republic Commando\GameData' analysis/reports/panner-materials.json
cargo test --workspace | Tee-Object -FilePath analysis/reports/panner-tests.log
python scripts/Record-Panner.py
cargo clippy --workspace --all-targets -- -D warnings
cargo fmt --all -- --check
```

TexOscillator/hologram effects, general modifier composition, script parameter
updates and live game integration remain open. Android verification is last.

Follow-up: `MATERIAL_JITTER.md` covers the original dual-axis OT_Jitter materials
through an explicit persistent-state/shared-random host API. Two more original
slots resolve there, leaving 18 omissions in the combined offline scope.
