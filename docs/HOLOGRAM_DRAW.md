# Hologram shader triangle diagnostic

Follow-up: [Native time/CosTime/shared flicker routines](SHADER_SCALARS.md)
now reconstruct three scalar constant cases. The mesh preview below still uses
its recorded host fixtures; live buffer and scene integration remain open.

Three tasks extend the vertex/pixel point probes into rendered triangles:
homogeneous clipping and perspective interpolation, explicit alpha/depth/blend
controls, and independently verified images of the reconstructed original
CloneCommando LOD0 mesh using the original shader programs and textures.

## Raster stage

`rc-render::shader_raster` takes completed vertex shader outputs. It requires
defined oPos, oD0, oT0.x, oT1.xy, oT2.xy and oFog.x. The ten interpolants are
T0.x, T1.xy, T2.xy, D0.rgba and fog.x; the diagnostic sampler fixes T0.y=0.
Fog is carried but not applied to the framebuffer.

Triangles are clipped before division to -w<=x/y<=w and 0<=z<=w, plus an
explicit diagnostic w>=1e-6 guard. Position and varyings interpolate together
at clip intersections, then the polygon is triangulated as a fan. Screen Y
points down; both windings render. Top-left coverage assigns shared edges to
one triangle. The ten varyings interpolate as sum(barycentric*value/w) /
sum(barycentric/w); depth uses linear screen interpolation of z/w. Depth is
clamped to [0,1] after conversion to f32 to remove clip-boundary roundoff.

The clip volume follows Microsoft's [viewports and clipping reference](https://learn.microsoft.com/en-us/windows/win32/direct3d9/viewports-and-clipping).
This diagnostic implementation uses f64 clip/raster arithmetic and half-integer
pixel centers, then f32 shader inputs/depth. Original [D3D9 rasterization rules](https://learn.microsoft.com/en-us/windows/win32/direct3d9/rasterization-rules)
describe integer pixel centers and hardware variations. No pixel-exact original
GPU equivalence is claimed. The existing opaque Scene renderer is unaffected.

Targets are bounded to 1..1024 in each dimension, clear depth to 1 and use opaque
ARGB pixels. Nonfinite/excessive vertices and incomplete shader outputs are
rejected. Shader errors may leave pixels already drawn; drawing is not atomic.

## Explicit diagnostic render controls

The caller chooses depth testing (less-equal), independent depth writes, optional
alpha testing (clamped float alpha > reference/255), and opaque or source-alpha
additive blending. RGBA is clamped before tests/blending; RGB is rounded to
8-bit and target alpha is opaque. Failed alpha tests do not write color/depth;
failed depth tests skip shader evaluation. Coverage, depth rejection, alpha
rejection and written-fragment counts are exposed.

Original DynamicHologram serializes ZTest=true, AlphaTest=true,
AlphaBlending=true, SrcBlend=5 and DestBlend=2. AlphaRef and ZWrite are absent
from this instance. The original HardwareShader enum maps 5 to SRCALPHA and
2 to ONE. The preview explicitly selects depth test on, writes off, alpha>0,
and source-alpha additive blending. Missing class defaults and native state
setup/comparison/alpha quantization have not been reconstructed; these choices
are diagnostic controls rather than a claim about the complete original pass.

## Original mesh fixture

The already independently verified `skeletal-draw.bin` supplies source index
25 (CloneCommando), LOD0: 3,500 triangles at its recorded animated pose.
Position, normal and UV words are retained in the new report. This is the
reconstructed mesh-local draw stream, without an actor/world instance.

Positions are normalized using the axis-aligned bounding-box center and maximum
half extent. The diagnostic view uses clip rows (0,0.9,0,0), (0,0,0.9,0),
(0.4,0,0,0.5), (0.3,0,0,1), identity world/normal matrices and eye (4,0,0,1).
These are explicit host fixtures, not native camera constant generation.
Material-defined constants, including scan thresholds (123000,123000), remain
original. Dynamic fixtures use c16=(2.25,0.7,0.2,0), c17=phase, c21=1,
c22=(0.9+phase*0.05,0.01,0.02,0), c25=(0.3+phase*0.1,0.2,0.1,0), and zeros
elsewhere. Phase is 0 or 0.5, not native game time. The earlier vertex probes
separately exercise all three scan regions with deliberately reduced thresholds.

Original GreyHoloFade, CloneCommandoSmall and NoiseDigital textures and serialized
pixel constants feed the fragment program. The preview uses no wrapper callback
or actor material overrides. Two 128x128 frames show the full mesh on FF101820;
the original pixel and vertex programs run before sampling/compositing.

## Validation

544 workspace tests, Clippy with `-D warnings` and formatting pass. Analytic
tests cover shared edges with both windings, perspective interpolation distinct
from affine interpolation, screen depth, all six clip planes and the w guard,
interpolated clip attributes, alpha threshold equality, preserved depth after
alpha rejection, disabled depth writes/tests, equal/farther depth and additive
clamping. Inputs outside target/finite bounds are rejected.

The independent Python oracle checks the prior mesh binary hash and selects
the same original words independently. It reconstructs normalized inputs,
applies the separately written original vertex algorithm to all 21,000 vertex
outputs, then clips/interpolates/samples/composites separately. All 32,768
image pixels, counters and unchanged depth words match. Both frame PNGs were
visually inspected. This validates the stated diagnostic pipeline, not native
GPU precision, live game constants or a playable Android runtime.

```powershell
cargo run -p rc-inspect --bin rc-hologram-draw-check -- analysis/reports/skeletal-draw.bin 'D:\SteamLibrary\steamapps\common\Star Wars Republic Commando\GameData' analysis/reports/hologram-draw.json
cargo test --workspace 2>&1 | Tee-Object analysis/reports/hologram-draw-tests.log
python scripts/Record-HologramDraw.py
cargo clippy --workspace --all-targets -- -D warnings
cargo fmt --all -- --check
```

Evidence: `analysis/reports/hologram-draw-validation.json`, the
hologram_draw_validation entry of `analysis/evidence.json`, and previews
`analysis/reports/hologram-mesh-0.png` / `hologram-mesh-1.png`.
Native dynamic constant generation, original render-state handling, actor/scene
material integration and Android verification remain open. General skeletal
diffuse coverage remains 271/289; this explicit shader pass does not expand it.
