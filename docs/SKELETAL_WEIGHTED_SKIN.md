# Weighted, cached and copy skin commands

2026-10-08. Three tasks complete: native weighted arithmetic, cache-writing
variants, and cache-copy/UV stream execution. Android testing remains deferred
until the end of the port.

## Runtime implementation

`skin_influenced_vertex` executes one non-copy command. `skin_lod_stream` walks
the original combined influence/UV stream into prepared `SkinStreamVertex`
storage, resetting its raw-word cache per call. It returns consumed bind/input
counts, output count, cache words and the command-kind histogram. The existing
prepared rigid-only API remains available.

The implementation reconstructs ComputeSkinVerts (`1050bc00`) with scalar f32
operations in the exact source SSE order. `Generate-WeightedSkin.py` lifts five
arithmetic blocks into `skeletal_weighted_kernels.rs`:

| Block | Original instructions | Use |
| --- | --- | --- |
| weighted_two | 1050c518..1050c7de | two influences, no cache |
| cached_rigid | 1050c813..1050c99b | one bone and cache write |
| cached_two | 1050c9f6..1050cce7 | two influences and cache write |
| weighted_first | 1050cd59..1050cee9 | first of three through eight influences |
| weighted_next | 1050cf00..1050d074 | subsequent influence and accumulation |

The top nibble gives the influence count, `(kind & 7) + 1`. Type 0 uses the
earlier rigid kernel; type 8 has its own arithmetic order. Types 1 and 9 also
have different arithmetic orders. Types 2..7 and a..e accumulate the first
weighted contribution, then each subsequent contribution in source order.
Cancellation-sensitive tests distinguish the special rigid and two-bone paths.

Bone selection is the unsigned quotient `(word & 0xfff) / 6`. The weight is
**not** extracted by shifting away bone bits:

```text
weight = f32(word & 0x0fffffff) * f32::from_bits(0x31800080)
```

Both integer-to-float rounding and the multiplication are retained. The scale
is read from the original Engine.dll PE at address `10679850`; its raw bits
are independently verified. Lower bone-address bits therefore affect weight.
Weights are not renormalized, and output normals retain the native magnitude.
Packed normals still use unsigned ten-bit components minus 511.

Types 8..e append six raw words (position then normal) to the current cache.
Type f, except ffffffff, copies six words from:

```text
offset_in_float_words = ((command & 0x0fffffff) / 3) * 3
```

This follows the original division/addressing at `1050c45d..1050c4ae`. It is not
replaced with an index into six-word entries. A nonaligned quotient can read a
normal followed by the next entry's position; the bounds-checked implementation
and tests preserve this behavior. Copy commands consume no bind vertex and do
not append to the cache. All output records take their own two raw UV words.
ffffffff terminates only at a command boundary.

Malformed inputs return errors instead of reading outside slices. Completed
prior records and cache writes survive; output tails remain unchanged. The
current record is checked for complete influences/UV data and available output
before execution. This is a defined safe boundary for malformed data, not a
claim about native undefined out-of-bounds behavior.

## Independent validation

`rc-skin-stream-check` executes every nonempty LOD stream from all 130 supported
original meshes. It uses the first verified full animation pose for 125 meshes
and original hierarchy-composed reference poses for five meshes without a
matching animation. Twelve LODs have no skin command stream and are explicitly
reported as null; their alternative render path remains outside this work.

Results:

- 414 full original streams, 289,085 output vertices and all corresponding UV words.
- 19,165 original weighted vertices, including two-, three- and four-influence paths.
- 78,383 cache writes / 470,298 raw cache words.
- 102,237 original cache-copy commands.
- 64 additional synthetic streams / 1,408 outputs covering every command kind
  and every influence count from one through eight with general matrices.

`Check-SkinStreams.py` independently interprets the original scalar SSE
instructions numerically, retaining each register/stack operation as a separate
f32 step. It does not import the Rust code generator or generated expressions.
It uses independently verified LOD archive bytes, inverse transforms and pose
reports, and independently constructs the five reference-only poses. Every
position/normal word, UV word, cache word and consumption counter is compared
bit-for-bit. The prior type 0 oracle remains independent of the new kernels.
This is an instruction oracle, not an execution capture of the original game.

Sixteen new unit tests cover weight scale/no normalization, lower weight bits,
all influence counts, different cancellation orders, invalid inputs, cache reset,
copy addressing/UV behavior, and partial writes on failure. Workspace total:
474 passing tests, Clippy with warnings denied and format check passed.

```powershell
cargo run -p rc-inspect --bin rc-skin-stream-check -- analysis/reports/original-skeletal-linkups.json analysis/reports/loaded-full-animation-ticks.json analysis/reports/reference-caches.json analysis/reports/skeletal-skin-streams.json
python scripts/Check-SkinStreams.py
python scripts/Record-SkinStreams.py
cargo test --workspace
cargo clippy --workspace --all-targets -- -D warnings
cargo fmt --all -- --check
```

Reports: `analysis/reports/skeletal-skin-streams.json` and its hashed independent
validation in `skeletal-skin-streams-validation.json`.

Native allocations/global scratch ownership, profiling counters and optional
post-terminator instance-buffer copies remain external to this prepared-slice
kernel. Triangle/material submission, empty-command LOD rendering and a visible
animated mesh in the runtime are still open. This work establishes full CPU
skin command output for the tested LOD streams, not a playable native game.
