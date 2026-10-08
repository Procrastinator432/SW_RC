# Three skinning preparation tasks

## 1. Original legacy mesh point field

`read_skeletal_mesh_prefix` now decodes and retains the array at mesh +1d0,
including its archive offset, instead of skipping it. Raw f32 words are preserved.
The serializer call is visible at 105140b7..105140bd. Three tests check raw signed
zero/NaN/infinity words, array boundaries, truncation, empty and negative counts.

Independent archive reads of all 130 supported original exports show that this
legacy point array is empty in every one. This does not mean the meshes lack
geometry. ComputeSkinVerts uses the selected LOD structure, whose vertex/command
data remains to be decoded. The existing bone/linkup/end offsets are preserved.

## 2. Native skin matrix palette

ComputeSkinVerts 1050bd96..1050c14f multiplies the mesh-owned inverse-reference
matrix by the current instance pose matrix, with output-specific scalar SSE
operation order. 1050c16f..1050c279 transposes the result into the global palette.
`skeletal_skin_product.rs` is generated from these instructions; it does not reuse
the differently ordered hierarchy or reference-parent matrix product.

`build_skin_palette` resizes its supplied output first and fills in bone order.
A missing inverse matrix reports an explicit boundary error after prior completed
writes. Tests check transpose/translation, multiplication order and resize/error
state. In-memory allocator flags/global ownership are not emulated.

## 3. Rigid command and prepared stream

`skin_rigid_vertex` reconstructs 1050c2c5..1050c44d for command top nibble zero.
The palette index is unsigned `(command & 0xfff)/6`, including nonmultiples of six.
Middle command bits are unused in this branch. Three 10-bit unsigned packed normal
components are biased by 511; the top two bits are ignored. The original path
neither normalizes nor divides the resulting normal by 511. Positions include
translation; normals do not. Per-output arithmetic order follows the scalar SSE.

`skin_rigid_stream` handles only these prepared commands and raw 0xffffffff as
terminator, consuming one 16-byte bind vertex and producing a position/normal
pair per command. Unsupported weighted/cached-copy commands and missing input,
palette entries, output capacity or terminator report errors with prior writes
retained. It is explicitly a rigid subset, not the whole original command decoder.

## Verification and scope

446 workspace tests, Clippy with denied warnings and formatting checks pass.
14 new tests cover the three tasks. The independent verifier reads original
archive point fields, interprets matrix SSE instructions and transpose stores,
and checks the rigid arithmetic with separate f32 rounding.

29912 palettes from 900 previously verified continuous full poses match bit-exactly.
Because the legacy point arrays are empty, 6664 kernel inputs use original reference
bone positions, with diagnostic bone assignments and packed normals. These are
clearly labeled `ReferenceBonePosition`, not original mesh vertices. An additional
64 general matrix/vertex cases exercise non-affine products and varied inputs.

Still open: actual LOD vertex, influence, packed normal and command data; weighted
and cached-copy commands; mesh triangles/materials and visible rendering. Real
Actor/Script/frame-loop integration remains open too. This milestone does not
claim actual mesh deformation or a playable native game. Android checks remain
deferred until the end.
