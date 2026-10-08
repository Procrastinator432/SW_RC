# Original skeletal LOD archives and rigid vertices

2026-10-08. Three tasks: read the persistent LOD model archive, locate all
influence/UV command boundaries, and validate real rigid vertices with original
animated poses. Android verification remains scheduled for the end of the port.

## Supported archive boundary

`read_skeletal_lods` continues the verified skeletal prefix reader. It requires
package 151..159/licensee 1, mesh archive version 8, and LOD model version 1.
It stops after the LOD model array; `remaining_bytes` explicitly preserves the
unread remainder of USkeletalMesh. Unknown LOD model versions fail rather than
being interpreted with version 1's layout.

USkeletalMesh::Serialize at `10514080` stores the word at +1bc, nested material
link arrays (+1d8), pairs of u16 (+1e0), aliases (+290), the word at +1c4,
then the LOD array (+1c8, array serializer `10513430`). Alias entries have two
compact FName references and a raw 16-word matrix (`105074f0`).

FStaticLODModel::Serialize at `1050b910` is the primary layout evidence:

| Native offset | Persistent archive representation |
| --- | --- |
| +00 | u32 model version |
| +04 | compact count, u32 skin command words |
| +0c | compact count, bind vertices: three raw position words and one packed normal |
| +14 | u32 |
| +18, +20 | two section arrays, each entry **nine u16 / 18 serialized bytes** |
| +28, +40 | index buffer: compact count, u16 entries, then u32 metadata |
| +58 | skin stream: three u32 words, compact array of eight-word vertices |
| +84 | lazy influences: raw weight u32, vertex u16, bone u16 |
| +98 | lazy wedges: 10 serialized bytes each |
| +ac | lazy faces: four u16 / eight serialized bytes each |
| +c0 | lazy points: three raw coordinate words each |
| +d4 through +e8 | six raw u32 tail words |

Native memory strides differ from serialized strides. Sections occupy 28 bytes
in memory (`10503b50`), wedges 12 bytes (`103c5670`). The LOD face element
serializer is `103c5920` (eight bytes), distinct from the 12-byte mesh face
serializer `103c6020`. Influence fields are explicit in `103c53a0`.
Wedges/faces and unidentified metadata remain raw rather than assigning
unverified material semantics.

Persistent FRenderResource has no byte payload. The skin stream omits its
UObject pointer for persistent archives (`103c4630`). Its vertex serializer is
`103c16a0`: position, normal, two UV words, 32 bytes total.

Each lazy array begins with a u32 **absolute file offset of its end**, followed
by the compact-count array. The reader eagerly decodes it and requires
`export.serial_offset + decoded_payload_position == stored_end`. All 1,704 lazy
end offsets in the supported originals match. This is not the engine's lazy
loading/memory registration implementation.

## Command boundaries

ComputeSkinVerts (`1050bc00`, instruction export `skeletal-skin-vertices.asm`)
uses the top nibble of the first word to select a record:

| Top nibble | Influence words | Bind vertices consumed |
| --- | --- | --- |
| 0 or 8 | 1 | 1 |
| 1 or 9 | 2 | 1 |
| 2..7 or a..e | `(kind & 7) + 1` | 1 |
| f, except ffffffff | 1 cache-copy command | 0 |
| ffffffff | terminator | 0 |

Every output record includes **two following raw UV words**, including cache
copies (`1050d113..1050d11f`). UV data and additional influence words can equal
ffffffff without terminating the program. The terminator is recognized only at
a record boundary.

`inspect_skin_program` records the command, bind-vertex and output indices for
top-nibble-zero entries and counts all other record kinds. It rejects truncated
influence/UV records, missing terminators and bind overruns. The archive checker
additionally requires exact exhaustion of both the commands and bind vertices.
Empty command arrays are reported with `programs: null`; they are separate
from terminated streams and do not establish a functioning alternative renderer.

The inspector does not execute weighted commands or copy/cache behavior. Type 8
also uses a distinct SSE operation order and cache writes, so it is excluded
from the existing type 0 rigid kernel. No normal renormalization is introduced.

## Original-data validation

`rc-skeletal-lod-check` reads 130 supported original meshes, with 426 LODs,
186,848 bind vertices and 289,085 command outputs. This advances beyond the
previous diagnostic inputs from reference bone positions: positions, packed
normals and bone commands now come directly from actual LOD archives.

For each prior verified full-pose case, the checker takes up to eight evenly
spaced type 0 entries per LOD. Original inverse reference transforms and full
animation matrices build the existing native skin palette. It checks 25,044
real rigid vertex results across 900 pose cases. This samples the rigid subset;
it does not execute an entire mesh's output stream.

`Record-SkeletalLods.py` independently rereads the original package bytes,
compares every decoded field and absolute lazy offset, walks each nonempty
command stream, and checks all influence bone addresses against the skeleton.
It recomputes the sampled outputs bit-for-bit with the independent f32 oracle
and native SSE palette expressions from the earlier skinning validation.
Source, package, pose, inverse-cache and report hashes are recorded.

Twelve new tests cover exact serialized strides, raw non-finite position bits,
all archive truncation boundaries, absolute lazy ends, unknown model versions,
negative counts, every command nibble, UV/terminator distinction, copy bind
consumption, truncated commands and bind overruns. Workspace total: 458 passing
tests; Clippy with denied warnings and formatting checks pass.

```powershell
cargo run -p rc-inspect --bin rc-skeletal-lod-check -- analysis/reports/original-skeletal-linkups.json analysis/reports/loaded-full-animation-ticks.json analysis/reports/reference-caches.json analysis/reports/skeletal-lods.json
python scripts/Record-SkeletalLods.py
cargo test --workspace
cargo clippy --workspace --all-targets -- -D warnings
cargo fmt --all -- --check
```

The report is `analysis/reports/skeletal-lods.json`; its independent verification
is `analysis/reports/skeletal-lods-validation.json`. Further work includes exact
weighted/cache-copy execution, complete output and triangle/material rendering,
remaining mesh tail fields and the broader game runtime. A playable native
Republic Commando or Android mesh rendering has not been established here.
