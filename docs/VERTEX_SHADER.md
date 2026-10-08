# Hologram vertex program: CPU point probes

Follow-up: [Triangle shader diagnostic](HOLOGRAM_DRAW.md) now renders the
reconstructed CloneCommando mesh with clipping and perspective interpolation.
The point-probe evidence below remains the earlier focused validation stage.

Three follow-up tasks are complete: original vertex constant binding inventory,
bounded vertex program parsing/evaluation, and linkage to the pixel program
for independently checked point probes. This is not yet a triangle renderer
for the complete hologram effect or an Android runtime test.

## Original constants

`Assets::shader_constant_bindings` reads complete nested SConstantsInfo/Plane
payloads for VSConstants (96 slots) or PSConstants (8 slots). It preserves
the serialized kind and value, including unused and dynamic bindings. The
existing `pixel_constants` API still rejects dynamic kinds. Missing entries
are not inferred from class defaults. Its previous pixel report is unchanged
byte for byte after this refactoring.

The original DynamicHologram material contains 20 VS bindings. Names below
come from the original Engine.HardwareShader SConstant enum, extracted in
`analysis/reports/sources/System__engine.u/HardwareShader.uc`:

| Slots | Kind | Original purpose |
| --- | --- | --- |
| 0 | 3 | ObjectToScreenMatrix (four registers) |
| 4 | 14 | LightPos1 |
| 5, 11 | 4 | ObjectToWorldMatrix (four registers each) |
| 10 | 12 | EyePosition |
| 16 | 9 | CosTime |
| 21, 22, 25 | 27 | Flicker |
| 23 | 0 | Unused, retaining its stored Plane |
| 24 | 21 | LightColor3 |
| 15, 18, 20, 26..31 | 1 | MaterialDefined |

This inventories bindings; it does not reconstruct the native engine's matrix,
time, light or flicker constant generation. Matrix continuation registers and
other runtime values are supplied explicitly by the diagnostic host.

## Vertex interpreter

`rc-package::vertex_shader` reads the original vs.1.0 program, including line
and block comments, and supports its 53 active instructions: mov, add, mul,
mad, max, frc, sge, slt, dp3, dp4 and rsq. The bounded subset also accepts
vs.1.1/vs_1_1 headers. Limits are 64 KiB text and 128 instructions.

Registers are r0..11, v0..15, c0..95, oPos, oD0..1, oT0..7 and oFog.x.
Constant brackets such as c[20], ordered write masks, source negation and
component swizzles are handled. Short swizzles repeat their final component;
an absent swizzle preserves xyzw for vector operations. See Microsoft's
[source swizzling reference](https://learn.microsoft.com/en-us/windows/win32/direct3dhlsl/dx9-graphics-reference-asm-vs-registers-modifiers-source-swizzling).
For the original shader's explicit scalar rsq sources, the interpreter uses
f32 reciprocal square root of the absolute source. Zero rsq is rejected by
the finite diagnostic contract rather than emulating legacy special values;
see [rsq](https://learn.microsoft.com/en-us/windows/win32/direct3dhlsl/rsq---vs).
[frc](https://learn.microsoft.com/en-us/windows/win32/direct3dhlsl/frc---vs)
uses x-floor(x), with the 1.x .y/.xy write-mask restriction.

Sources are read before any components of a destination are written. Each
instruction captures raw words and defined flags for all 24 temporary/output
registers. Undefined temporary reads, invalid register ranges/masks, output
reads, relative constants, unsupported instructions, nonfinite inputs/results
and incomplete position outputs are rejected. Partially written varyings retain
explicit defined masks. This is a diagnostic subset, not a complete hardware
assembler/validator. Arithmetic is separate f32 operations without FMA; legacy
GPU precision and GPU rsq approximation remain unverified.

## Vertex-to-pixel point linkage

`rc-render::fragment::shade_vertex_output` checks that the original shader's
required color and texture components are defined, then supplies oD0 and
oT0.x/oT1.xy/oT2.xy to the existing original pixel program. Since this shader
writes only oT0.x, the point sampler explicitly fixes T0.y=0. This is a
diagnostic policy, not an inference about unspecified GPU output components.

Ninety-six probes use original material-defined constants except c18, whose
original scan thresholds (123000,123000) are deliberately replaced with
(-0.25,-0.5) to exercise all three scan/distortion regions. Explicit fixtures
also supply identity position/world/normal transforms, eye (0,0,4,1),
c16=(2.25,0.7,0.2,0), c17=(phase,phase,phase,phase), c21=(1,1,1,1),
c22=(0.9+phase*0.05,0.01,0.02,0), and
c25=(0.3+phase*0.1,0.2,0.1,0). All remaining unspecified host registers are
diagnostic zeros. Phase is 0, 0.5 or 1; it is not native game time.

Positions cover z=-0.75,-0.4,0,0.75 with small x/y variations, normals are
(0,0,1,0), and v2 covers an 8x4 UV grid. The report contains every input.
Original GreyHoloFade, CloneCommandoSmall and NoiseDigital textures and original
pixel constants feed the point pass on background FF101820. Neither projected
triangle coverage nor interpolation is claimed. Alpha test, fog/depth render
state, live constant generation and full material integration remain open.

## Validation

539 workspace tests pass; Clippy with `-D warnings` and formatting pass.
`Record-VertexShader.py` independently reads the original shader and constants,
asserts its active instruction sequence, then applies a separately written
fixed algorithm. All 5,088 raw register snapshots and defined masks match for
the 96 probes, covering three scan regions. All 96 linked fragment pixels
match a separate sampling/pixel formula using independently decoded textures.
The previous 196,608-pixel fixture report remains unchanged.

```powershell
cargo run -p rc-inspect --bin rc-vertex-check -- 'D:\SteamLibrary\steamapps\common\Star Wars Republic Commando\GameData' analysis/reports/vertex-shader.json
cargo run -p rc-inspect --bin rc-pixel-check -- analysis/reports/hologram-materials.json 'D:\SteamLibrary\steamapps\common\Star Wars Republic Commando\GameData' analysis/reports/pixel-regression.json
cargo test --workspace 2>&1 | Tee-Object analysis/reports/vertex-tests.log
python scripts/Record-VertexShader.py
cargo clippy --workspace --all-targets -- -D warnings
cargo fmt --all -- --check
```

Evidence: `analysis/reports/vertex-validation.json` and
`analysis/evidence.json` under vertex_shader_validation. The general skeletal
diffuse resolver stays at 271/289 slots; point probes do not change coverage.
Android verification remains at the end.
