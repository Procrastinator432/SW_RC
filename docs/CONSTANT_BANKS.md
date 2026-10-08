# Raw matrix/camera constants and persistent banks

Follow-up: [Native composed shader matrices](SHADER_MATRIX_COMPOSITION.md)
implements cases 2,3,32 and supplies current validation with 560 tests. The
sections below focus on the raw/camera bank cases. Light setup and live scene
integration remain open.
The subsequent [light constants milestone](SHADER_LIGHTS.md) adds colors,
ambient and spotlight cases with current validation at 566 tests. Light
position/radius and live scene integration remain open.

Three tasks reconstruct raw ObjectToWorld/WorldToCamera uploads, camera/eye
selection with a per-dispatch cache, and bounded persistent shader constant
banks. This extends the earlier scalar routines; it does not yet run the entire
original material's constant setup or connect it to a live scene.

## Task 1: raw matrix upload

In D3DDrv SetShaderConstants (1001c130 / 1001c260), case 4 copies the 16 words
of ObjectToWorld from render state +2c and transposes them into four consecutive
constant registers. Case 6 does the same for WorldToCamera at +6c. The switch
then advances by four registers, skipping any bindings in continuation slots.

`shader_constants::transpose` and `Bank::update` preserve raw u32 values,
including signed zero and NaN payloads. No inversion, normalization, arithmetic
or special treatment of the fourth component is performed. The host supplies
the original matrix words. Evidence remains in the original-driver pseudocode
and instruction export at `analysis/decompiled/shader-constants.c` / `.asm`.

## Task 2: camera and eye selection

Cases 5 (CameraToWorld) and 12 (EyePosition) share a lazy camera-to-world matrix
cache inside one native dispatch. The cache resets on the next dispatch.

Outside editor mode, a provided runtime camera-to-world matrix is used. In
editor mode, the host inverse of WorldToCamera is requested instead. Case 12
also uses that inverse when the runtime camera is absent; case 5's original
non-editor branch has no null-camera guard and would dereference it. The Rust
API returns an explicit error for that unsafe case. If an earlier EyePosition
case already populated the cache through its null fallback, case 5 reuses it.

Case 5 uploads the transpose of the selected matrix to four registers. Case 12
copies the untransposed last row, including its fourth word, to one register.
The native code does not force EyePosition.w=1. One inverse callback can serve
both kinds and repeated eye bindings during an update.

`Host` models the required stable context; the inverse callback takes raw
WorldToCamera words and returns raw CameraToWorld words. Matrix inversion itself
and scene/camera pointer acquisition are not reconstructed here. Test matrices
include arbitrary payload words to check copying; callback fixture outputs are
not claimed as mathematical inverses.

## Task 3: persistent bank/count dispatch

`Bank::new` accepts 96 vertex or 8 pixel registers with explicitly supplied
initial words. It does not infer initial GPU state or reset unused registers.
The corresponding native count markers are -2 and -3. On first initialization,
the last nonzero binding kind determines the cached register count. Native
span-table bytes at 1001ecc0/1001eccc independently establish:

| Binding kinds | Count span |
| --- | --- |
| 2..7 and 32 | 4 |
| 34 | 2 |
| All others | 1 |

An all-unused bank still computes count 1 because the native last-index value
starts at zero. A cached nonnegative count stays unchanged when bindings change;
the host must explicitly invalidate it when appropriate. A matrix writes its
four registers even if its continuation passes the cached iteration count,
provided they fit the allocated bank.

The raw/scalar dispatch cases are unused 0, material-defined 1, ObjectToWorld 4,
CameraToWorld 5, WorldToCamera 6, Time 8, CosTime 9, EyePosition 12 and Flicker
27. Case 1 copies raw words. Case 0 leaves the previous register contents alone.
Scalar cases use the previously verified shared Flicker instance and host RNG.
Composed cases 2,3,32 are now supported and validated separately in the follow-up.

Unsupported cases return an explicit error instead of silently providing
invented values; the native default branch skips unknown kinds. Native counts
or matrix writes beyond the allocated bank also return errors. These are safe
host restrictions, not claims that the original performed bounds checks.
Counts, previous writes and consumed flicker state remain visible after errors.
Updates are not transactional.

This resolves how an absent binding such as DynamicHologram c17 can retain
buffer contents; it does not establish that register's live initial value or
history. The original material still requires unreconstructed light cases
14/21 and live matrix acquisition. The existing mesh preview
therefore continues to use its recorded explicit fixtures.

## Validation and reproduction

560 workspace tests, Clippy (`-D warnings`) and formatting pass. Tests cover
raw transpose payload preservation, matrix continuation skipping, retained
unused words, stale cached counts, all-unused counts, safe buffer limits,
runtime/editor/null camera selection, shared/reset inverse caching, fourth eye
word preservation and partial state after unsupported/host-error branches.

An independent Python oracle reads the original PE span tables and verifies
1,024 interleaved updates across persistent 8/96-register banks. It checks
425,984 raw before/after register words, 780 matrix uploads, 595 eye uploads,
291 inverse requests, 288 random draws and 162 expected error paths. The
unsupported-error fixture now uses kind 35 instead of newly supported kind 3;
its different native span also changes subsequent cached counts. Inputs,
callbacks and seeds are regenerated independently from the probe index. Every
reported register/cache/state/result matches the separately written oracle.

```powershell
cargo run -p rc-inspect --bin rc-constant-bank-check -- analysis/reports/constant-banks.json
cargo test --workspace 2>&1 | Tee-Object analysis/reports/constant-banks-tests.log
python scripts/Record-ConstantBanks.py
cargo clippy --workspace --all-targets -- -D warnings
cargo fmt --all -- --check
```

Evidence: `analysis/reports/constant-banks-validation.json` and
constant_banks_validation in `analysis/evidence.json`. Native matrix
inversion, light setup, initial buffer ownership/history and live
scene integration remain open. Diffuse coverage remains 271/289; Android
verification stays last.
