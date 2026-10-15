# Computed scalar renderer inverse

The scalar inverse at D3DDrv10031709 is now reconstructed under an explicit
nearest-rounding x87 precision-control=53 policy. This is a supported portable
scalar fallback, not automatic emulation of the running game's CPU selection.
Optimized dispatch implementations and x87 precision24/64 remain open.

Five parts are covered: original stack/dataflow arithmetic and f32 spills,
optional determinant storage, rounded-reciprocal finite write gate, computed
inverse integration into persistent renderer state, and independent instruction
replay/transaction rollback verification.

Generate-RendererInverseCode.py translates the reviewed original instructions to
312 Rust arithmetic/load/store assignments. Each arithmetic instruction rounds
to binary64, matching the explicit 53-bit significand policy. Original f32
spills/reloads are retained. With finite f32 inputs and finite reloaded spills,
the bounded products/sums lie within binary64's exponent range. No reassociation
or fused multiply-add is used. Nonfinite inputs/reloaded spills are explicit
safe host errors; final output stores may still contain overflow infinities.
This policy is separate from Core FMatrix::Inverse, whose singular behavior and
SSE order differ; the existing Core implementation is not substituted here.

The determinant remains in the arithmetic stack for reciprocal calculation.
Its optional f32 store does not replace the extended determinant. The reciprocal
is then stored as f32 and that stored value is tested by the original finite
predicate. Zero/sufficiently tiny determinants can produce a nonfinite stored
reciprocal: no destination words are written. A finite stored zero reciprocal
still takes the write path. Existing inverse destination bytes survive the
no-write path, rather than being replaced by identity. Determinant and
reciprocal words are returned in a trace; the renderer uses a null determinant
pointer as in the original call.

RendererRuntime::submit_scalar53 derives the world matrix, computes this inverse
and overwrites the obsolete supplied answer before entering the persistent
renderer/render adapter. The original inverse body is executed at the +48
setter's CALL site in replay, using the actual cache+5e4 output pointer. Complete
scene/cache, request and runtime state commit together; a later failure leaves
all pending computed work uncommitted. Existing explicit-response APIs remain
available for inspecting other CPU-policy boundaries.

## Verification

568 matrix probes include dense random matrices, duplicate/near-duplicate rows,
singular and signed-zero diagonal cases, scale extremes, optional determinant
pointers, input/output aliasing, positive/negative zero reciprocals, final-output
overflow and nonfinite host-error probes. Successful
cases execute the original x87/integer instructions in an independent exact
rational stack interpreter, rounding to 53 bits after each arithmetic operation
and to f32 at each store. The original reciprocal finite call is a predicate
boundary; its CRT body is not interpreted. The rational oracle does not use the
Rust routine's binary64 arithmetic as its numerical reference.

50 sequences /146 integrated requests run the original scalar inverse at the
real renderer call, joined with MeshToWorld, preprocessing, camera, animation,
geometry, uploads and draw plans. Twelve full rollback cases include nonfinite
inverse spills and late upload/material failures. Debug/Release reports are byte
identical; all 920 existing workspace tests pass. Clippy all targets -D warnings,
format and diff checks pass. No new unit-test count is claimed.

```powershell
python scripts/Generate-RendererInverseCode.py
python scripts/Generate-RendererInverse.py
cargo run -p rc-inspect --bin rc-renderer-inverse-check -- analysis/reports/renderer-inverse.input.json analysis/reports/renderer-inverse.json
cargo run --release -p rc-inspect --bin rc-renderer-inverse-check -- analysis/reports/renderer-inverse.input.json analysis/reports/renderer-inverse-release.json
cargo test --workspace *> analysis/reports/renderer-inverse-tests.log
python scripts/Record-RendererInverse.py
python analysis/finalize-renderer-inverse.py
```

Reports: analysis/reports/renderer-inverse*; evidence renderer_inverse_validation.
Open: CPU-optimized inverse selection/implementations and other precision modes,
camera/FPTAN, original asset loading, physical allocators, material compilation
and live GPU. Not yet playable; Android verification remains deferred until end.
