# Skeletal sections, rigid geometry and diagnostic rendering

2026-10-08. Three tasks completed: soft triangle selection, rigid triangle
selection/transforms, and a portable diagnostic renderer bridge with four
original-animation preview frames. Android testing remains deferred.

## Original draw selection

USkeletalMeshInstance::Render at `1050eaa0` binds two distinct paths. Native LOD
offset +18 contains soft sections and +28 their index resource. Rigid sections
are at +20, their index resource at +40 and their stored vertex stream at +58.
The exported instruction and decompiler evidence is `skeletal-render.asm/.c`.

The section serializer stores nine u16 values, omitting native memory gaps:

| Serialized index | Native offset | Verified draw use |
| --- | --- | --- |
| 0 | +00 | material slot passed to the material resolver |
| 1 | +02 | rigid first index; soft path uses cumulative indices instead |
| 2, 3 | +04, +06 | minimum and maximum vertex indices |
| 4 | +08 | retained raw, not assigned a render meaning |
| 5 | +0c | rigid section bone |
| 6, 7 | +0e, +10 | retained raw |
| 8 | +12 | primitive count, three indices per triangle |

The soft path advances its first-index cursor by `count * 3` after each section.
The rigid path uses the stored first index. Both pass the min/max vertex bounds
and primitive count to the render interface. Zero-primitive sections do not
dereference vertices or bones. Indices beyond section ranges remain explicit
in `unused_indices`; the rigid buffers contain additional unused data.

`build_skeletal_draw` follows these draw selections. Soft triangle corners
retain full CPU skin output, normal and UV words. Rigid corners use the stored
eight-word vertex stream. This handles the 12 original LODs with no skin
commands through their rigid sections without inventing a soft stream.

## Rigid transforms

The rigid path builds inverse-reference * pose using its own scalar SSE order
at `10510647..10510a12`, before the actor transform. This is generated into
`skeletal_render_product.rs` by `Generate-RigidRenderProduct.py`. It differs in
operation order from the earlier skin palette product.

The diagnostic CPU bridge then applies this row-vector matrix to stored
positions and normals in mesh-local space. UVs remain raw. The transform is
independently checked as implemented, but is not asserted to reproduce the
original GPU's rounding, normal handling or actor/world transform. Native
render-interface state changes, material passes, projectors and effects remain
outside this bridge.

## Visible preview

`Scene::from_skeletal_draw` connects the reconstructed triangles to the existing
portable depth-tested renderer and snapshot format. It retains material slot
numbers and original UVs, using colored checker textures for diagnosis. These
are not original game materials. Nonfinite UVs and excessive material-slot
counts are rejected. No image-generation model is used.

`rc-skeletal-draw-check` produces all-LOD diagnostic triangle records plus four
512x512 Clone Commando frames from the already verified full-animation tick
report. Camera center, radius and orientation stay fixed across these frames.
The saved rcscene snapshots remain viewable with the existing renderer tools.
The frames demonstrate different sampled poses, not a live game animation loop.

## Evidence and checks

All 130 supported meshes and 426 LODs produce:

- 315,754 soft triangles and 252,010 rigid triangles.
- 1,343 section records, with bounded index and vertex references.
- All selected rigid matrix products independently checked against original SSE.
- 64 additional general matrix-product probes.
- Four distinct fully framed previews and independently checked scene snapshots.

`Record-SkeletalDraw.py` independently derives section fields from the original
verified LOD archives, uses the independent skin report, interprets the rigid
matrix SSE instructions numerically, and compares every generated triangle
record including position, normal and UV words. All four preview snapshots are
rebuilt from their original sampled poses and checked as well. PNGs are lossless
conversions of the renderer's PPM output; each frame is checked for nonempty
foreground entirely inside the image.

The binary diagnostic report begins with `RCSKDRAW` and u32 version 1. Each
132-byte record contains 33 little-endian u32 values: mesh source index, LOD,
bank, native section index, material slot, bone (ffffffff for soft), three
indices, then three position/normal/UV eight-word vertices. The JSON supplies
section metadata, unused counts, rigid products and preview paths. Source,
input-report, binary and preview hashes are recorded in the validation report.

Ten new tests cover section fields/cursors, index bounds, unused index slots,
rigid-only empty-command LODs, absent vertices/poses, zero primitives,
inverse/pose order, vector/UV treatment and renderer snapshot/validation behavior.
All 484 workspace tests, Clippy with denied warnings and formatting pass.

```powershell
cargo run -p rc-inspect --bin rc-skeletal-draw-check -- analysis/reports/original-skeletal-linkups.json analysis/reports/loaded-full-animation-ticks.json analysis/reports/reference-caches.json analysis/reports/skeletal-draw.json
python scripts/Record-SkeletalDraw.py
cargo test --workspace
cargo clippy --workspace --all-targets -- -D warnings
cargo fmt --all -- --check
```

Artifacts are under `analysis/reports`: `skeletal-draw.json/.bin`,
`skeletal-draw-validation.json`, and `skeletal-preview-0..3.png/.ppm/.rcscene`.
Remaining work includes original material/texture resolution, actor/world
transforms, instance lifecycle, live animation/render integration and the
broader game runtime. This does not establish a playable native game.
