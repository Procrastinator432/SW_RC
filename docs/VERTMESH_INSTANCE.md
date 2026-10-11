# Vertexmesh instance cache and channel integration

2026-10-10. Module `vertmesh_instance.rs`, CLI `rc-vertmesh-instance-check`.
Five completed tasks:

1. Reuse `skeletal_sequence::get_sequence` for the vertexmesh channel-zero path.
2. Implement GetFrame's cache gate and preservation of an existing matching cache.
3. Reset both logical cache arrays when the mesh identity changes.
4. Append vertex storage, zero appended ranges and set the initialization metadata.
5. Connect instance cache/channels to scheduling, decode, indices, upload and draws
   with atomic request/runtime commit.

## Original behavior

Engine `1055f0c5..1055f1f2` initializes when position count low29 is zero or
instance `+98` differs from the current mesh `+54`. The normal count is not part
of this gate. A nonempty matching position cache bypasses all initialization and
preserves cache contents and metadata.

A mesh change calls array helper `10332670(0,0)` for positions `+7c/+80` and
normals `+84/+88`, then appends mesh vertex count to both arrays. Without a mesh
change, empty positions still trigger appends to both arrays. Thus a preexisting
normal prefix can survive and the final normal array can be longer than the
vertex count. Only appended ranges are zeroed by original REP STOS instructions.
All three high descriptor flag bits survive the logical count update.

Initialization sets `+9c` to the original `10653578` constant, verified as
float32 `1.0`, updates `+98` to current mesh, and zeroes `+90` and `+8c`.
It does not change `+94` or `+60/+61`; the subsequent GetFrame body sets them.
Zero vertices still perform initialization and metadata writes. The position
count remains zero, so a later call initializes again.

The existing shared GetSequence implementation matches `1044faf0..1044fb1f`.
It reads cached channel word17 (`+44`) in runtime mode without calling a host.
Editor mode calls mesh virtual `+ac(name_handle, false)` and refreshes that word,
including null results. Channel records have stride `0x48`. This batch reuses
that implementation rather than introducing another lookup implementation.

## Instance adapter

`Input` contains the cache descriptors/images, mesh identity, complete channel
words, editor state, supplied lookup bindings and sequence records. It also
contains the previous animation input. That input's sequence/time/cache fields
are superseded by the authoritative instance/channel fields: selected sequence
identity, channel word7 (`+1c`) and prepared instance cache. A nonzero selected
identity must have a supplied sequence record; zero selects frame zero.

The adapter passes a vertex-count-sized view of each prepared array to the
existing frame path, then merges the resulting cache prefix back into the full
instance arrays. Extra normal/position tails remain intact. Output includes the
pre-decode `preparation` snapshot and the final `cache` snapshot separately.

`Runtime.submit_vertmesh_instance` commits channels, instance cache, generated
vertex bytes and canonical nested geometry only after the existing transactional
animation/geometry/upload/material chain succeeds. Legacy animation fields are
synchronized on success so the resulting request is inspectable. Any failure
leaves the original request and runtime unchanged, including an editor lookup
refresh or cache reset that happened before a later unlock/material error.
Geometry LOD remains the pre-GetFrame value despite the callback count clamp.

Safe snapshot bounds: nonzero mesh identity, vertices and each signed low29 cache
count 0..65536, exact count/image lengths, append result at most 65536; at most
256 channels and 65536 bindings/sequence records. The reused frame adapter also
requires both arrays to contain the vertex-count-sized view. Inconsistent reused
arrays are rejected rather than read through invalid memory. Existing animation,
destination and geometry bounds still apply.

## Evidence

The replay executes original cache branches/destruction-counter loops, append
argument calculation, zero stores and metadata writes. Array storage growth is
a supplied allocator boundary, poisoned with `a5` so original zeroing must clear
it. The original helper's `103327ca..103327dc` instructions commit packed counts
and preserve flags. Native allocator calls, ownership/capacity decisions and
physical reallocation are not implemented by this logical storage model.

The complete GetSequence body executes over supplied channels; its editor virtual
lookup is the supplied binding boundary. Selected raw sequence identity is retained
as the scheduler's record address. Original scheduling and packed decode/output
then execute, followed by the existing native geometry/upload/material replay.
Synthetic mesh addresses are disjoint from renderer scratch and cache storage.

**259 cache probes, 267 instance/frame probes, 4 cache errors and 5 instance errors.**
**50 draw sequences / 146 requests**, **696,076 original instructions**, 2,628
indices, 146 vertex and 146 index uploads, **438 draws / 876 material passes**.
**8 transaction errors** cover missing channels/records/banks, layout/LOD conflicts
and late unlock/material/index errors, with unchanged request/runtime.

Combined standalone/integrated totals: 672 cache preparations (450 initializations,
222 reuses), 1,346 logical array boundaries and 48,685 cache instructions; 413
GetSequence calls, 104 editor lookup boundaries and 5,271 sequence instructions;
413 scheduler/decode calls, 1,359 direct and 1,370 interpolated vertices. These
counts overlap the integrated instruction total and must not be added to it.

**12 new Rust tests; 920 workspace tests passed.** Debug/Release byte identical;
Clippy all targets with `-D warnings`, formatting and diff checks passed.
The new checks additionally exercise deliberately stale legacy sequence/time/cache
fields, extra cache tails, duplicate lookup bindings, null editor results and
preservation of all other channel words.

```powershell
python scripts/Generate-VertMeshInstance.py
cargo run -p rc-inspect --bin rc-vertmesh-instance-check -- analysis/reports/vertmesh-instance.input.json analysis/reports/vertmesh-instance.json
cargo run --release -p rc-inspect --bin rc-vertmesh-instance-check -- analysis/reports/vertmesh-instance.input.json analysis/reports/vertmesh-instance-release.json
cargo test --workspace *> analysis/reports/vertmesh-instance-tests.log
python scripts/Record-VertMeshInstance.py
```

Evidence: `vertmesh_instance_validation`; reports `analysis/reports/vertmesh-instance*`.
The data are synthetic banks and supplied sequence/material/COM/allocator answers.
Asset/name loading behind mesh `+ac`, physical allocator/ownership behavior, mesh
transform, camera matrix construction, FPTAN and real GPU execution remain open.
The full camera/actor/preprocessing call tree is not yet connected to this adapter.
No original animation clip playback or playable game is demonstrated. Android
validation remains deferred until the end as requested.
