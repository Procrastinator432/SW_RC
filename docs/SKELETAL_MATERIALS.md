# Original skeletal material selection and diffuse previews

2026-10-08. Three tasks completed: mesh material references/native slot
selection, original diffuse chain/mip resolution, and texture/UV renderer
binding with four sampled animation previews. Android remains deferred.

## Native references and selection

ULodMesh::Serialize (`10450d40`) writes the material reference array after its
packed vertex array, before scale/origin/rotator. The array serializer
`10450980` stores a compact count and object references. The skeletal prefix
reader now retains `materials_offset` and the validated signed object indices.
Local export paths are qualified with the source package; imported paths
remain qualified by their original import chain.

`select_lod_material` reconstructs ULodMeshInstance::GetMaterial (`1044f750`)
for nonnegative slots:

- With an actor, dispatch its material override. If non-null, dispatch it
  again and return the second result, including a changed or null result.
- For slot zero or out-of-range slots, prefer the first actor skin when its
  array is nonempty, even when that entry is null.
- Otherwise use the in-range mesh entry, including null. Out-of-range slots
  fall back to the first mesh entry, or null for an empty array.

The helper's unsigned slot input intentionally excludes the original function's
unsafe negative-index case. Actual Actor virtual dispatch/skin registration
remains external; the original-asset report supplies no actor. Eight tests
cover selection and callback order without inventing actor instance bindings.

## Original diffuse assets

The existing offline `Assets` resolver is exposed as a reusable inspection
library. It can now discover animation/system packages as well as texture and
static mesh packages. `diffuse_at_size` permits a bounded requested size
(1..1024); the existing world-scene method keeps its 64-pixel behavior. Skeletal
previews request 256 pixels, selecting the closest stored original mip and
using nearest sampling only when that mip needs bounding.

Reports retain source format, source mip dimensions, decoded dimensions,
straight-alpha ARGB pixels, material class chain and UV scale. Supported chains
reuse the established base-diffuse preview rules: Texture, Shader.Diffuse,
FinalBlend.Material, supported HsBump DiffuseTexture fields/DiffUVScale,
selected Combiner base input and zero-channel TexCoordSource. This is not a
full shader evaluator or original material-pass implementation.

Across 130 supported meshes, 212 used slots resolve to 138 unique material
diffuse results. Pixel formats encountered are DXT1 (3), BGRA (5), DXT3 (7)
and DXT5 (8). Original references and lazy mip offsets are validated.

77 used slots remain explicit omissions: 52 TexPanner2D, 10 HsHologram,
5 TexPanner, 2 TexOscillator, 3 null selections, 3 missing referenced exports,
one Shader without Diffuse and one ConstantColor. These counts describe the
offline resolver's supported scope, not corrupt original files. Dynamic UV
transforms and hologram/effect behavior have not been substituted with guesses.

Follow-up: `MATERIAL_PANNER2D.md` documents the new explicit time-sampled API
which resolves those 52 TexPanner2D slots. The historical report and scalar-only
API retain the above split; 37 of those omissions remain in the new scope.

## Renderer and visible result

`Scene::apply_skeletal_diffuse_slots` applies resolved textures and scales source
UVs once, preserving geometry and material slot IDs. Missing slots retain flat
diagnostic shading. It validates dimensions, pixel counts, budgets, finite
scales and resulting UVs before committing scene changes. Four additional tests
cover binding, omissions, invalid dimensions/scales and UV overflow.

`rc-skeletal-textured-preview` uses the prior independently checked Clone
Commando scene snapshots, without recomputing or changing their geometry. All
3,500 triangles in each of four frames receive the original diffuse texture.
The camera stays fixed. These are sampled animation frames in the portable
renderer, not a live game loop; lighting/material effects are still diagnostic.

## Independent verification

`Record-SkeletalMaterials.py` separately reads the original package tables,
compact material arrays, tagged properties, material references, mip data and
absolute lazy ends. It independently decodes all 138 textures pixel-for-pixel,
checks every chain/scale and reproduces all 77 omissions. It then compares every
preview snapshot triangle's unchanged position/material data, scaled UV words
and decoded texture pixels. The four distinct PPM frames are losslessly saved
as PNG and checked for fully framed visible geometry. No image-generation
model or painted replacement texture is involved.

All 496 workspace tests, Clippy with denied warnings and formatting pass.
Original package, source, report and preview hashes are recorded in
`analysis/reports/skeletal-materials-validation.json`.

```powershell
cargo run -p rc-inspect --bin rc-skeletal-material-check -- analysis/reports/original-skeletal-linkups.json 'D:\SteamLibrary\steamapps\common\Star Wars Republic Commando\GameData' analysis/reports/skeletal-materials.json
cargo run -p rc-inspect --bin rc-skeletal-textured-preview -- analysis/reports/skeletal-draw.json analysis/reports/skeletal-materials.json analysis/reports/skeletal-textured-preview.json
python scripts/Record-SkeletalMaterials.py
cargo test --workspace
cargo clippy --workspace --all-targets -- -D warnings
cargo fmt --all -- --check
```

Preview files: `analysis/reports/skeletal-textured-preview-0..3.png/.ppm/.rcscene`.
Remaining work includes unsupported/dynamic material chains, original effects,
Actor overrides/world transforms, live runtime integration, instance lifecycle,
mesh tails and the wider game runtime. A playable native game is not established.
