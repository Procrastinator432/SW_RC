# Renderer matrix state and deferred output

Five tasks completed: renderer +48 world, view and projection matrix branches,
deferred transform-mask/dirty-group output, and persistent integration with the
MeshToWorld/render CPU path. D3DDrv vtable 1006f8d8+48 resolves to 1001fc70.
This entry was absent from the previous Ghidra function inventory; it is now
explicitly disassembled and exported in a read-only analysis session.

Slot 0 copies 16 raw words to cache+5a4 (matrix slot10) and render-state+2c,
ORs mask400 /dirty20, and calls thunk 10031703 with (cache+5e4, null, matrix).
Slot1 copies to render-state+6c and cache+524 (slot8), ORs mask100 /dirty40.
Slot2 copies to render-state+ac and cache+564 (slot9), ORs mask200 /dirty40.
Other slot values return without dereferencing the supplied matrix. Every valid
set marks even an identical matrix; no applied-matrix equality shortcut exists.
Words are copied unchanged, including NaN, infinity and signed zero.

The original inverse dispatch is a local CPU-dependent D3DX-style table, not an
ordinary DLL import. Initial table100802a8 points to resolver thunk100316f6;
default entry10080390 points to scalar inverse10031709. Resolver10035508 installs
a table and selects optimized variants based on CPU/settings. These bodies were
exported for the next work. Their math/selection is not implemented in this step.
InverseAnswer::Written supplies an explicit raw matrix write, Unchanged preserves
the old inverse image, and Unresolved rejects slot0 atomically. Slots1/2 and
unknown slots do not require an inverse answer. No supplied inverse is presented
as an independently reconstructed result of the input matrix.

Scene::set updates the existing Deferred cache and separate scene matrix image.
The existing transform flush emits view/projection/world as D3D8 transform IDs
2/3/256 in native order, between render/stage and texture calls, then clears the
mask. RendererRuntime::submit applies world state before the joined draw path.
Disabled draw gates retain pending state; enabled material passes consume it in
the existing complete flush. Scene, inverse image, deferred masks/matrices,
request and downstream runtime commit together. Any later error restores all.

## Verification

256 synthetic setter sequences contain 1600 requests: all three valid slots,
unknown positive/high-bit slots, repeated identical matrices, arbitrary raw
payloads, inverse write/no-write answers and missing-input rollback probes.
They verify full scene/cache images and subsequent flush/repeated flush plans.
1280 standalone requests execute the original setter; 320 missing-input errors
are Rust-only rollback cases, not native invalid-pointer executions. Including
the integrated requests, 1426 setters and 658 inverse-response boundaries execute;
standalone flushes emit 1805 transform calls. The 146 joined requests execute
674947 replay steps and retain 112 enabled frame/upload paths, 14 no-sections /
20 no-actor-state returns, 1353 indices and 253 draws /506 material passes.
Component instruction counters overlap and must not be added.
50 sequences /146 integrated render requests reuse original MeshToWorld,
preprocessing, camera, instance/frame, geometry, upload and draw bodies and now
execute the original +48 setter too. Complete plans, persistent scene, requests
and runtime images match word for word. Twelve full render rollback cases cover
missing inverse resolution and late index-unlock/material failures.

All 920 existing workspace tests pass; Debug/Release reports are byte identical.
Clippy all targets -D warnings, format/diff checks pass. No new unit-test count
is claimed. Inverse math is a response boundary; allocator/material/COM answers
remain supplied. Interpreter call plans do not execute a GPU or Android app.

```powershell
python scripts/Generate-RendererTransform.py
cargo run -p rc-inspect --bin rc-renderer-transform-check -- analysis/reports/renderer-transform.input.json analysis/reports/renderer-transform.json
cargo run --release -p rc-inspect --bin rc-renderer-transform-check -- analysis/reports/renderer-transform.input.json analysis/reports/renderer-transform-release.json
cargo test --workspace *> analysis/reports/renderer-transform-tests.log
python scripts/Record-RendererTransform.py
python analysis/finalize-renderer-transform.py
```

Reports: analysis/reports/renderer-transform*; evidence renderer_transform_validation.
Open: dispatched inverse math/CPU policy, camera matrices/FPTAN, original assets,
physical allocator behavior, material compilation and live GPU. Not yet playable.
Android validation remains deferred until the end per user instruction.
