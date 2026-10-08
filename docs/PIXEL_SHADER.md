# Hologram pixel program: CPU diagnostic execution

Follow-up: [Vertex program and linked point probes](VERTEX_SHADER.md) now
execute the original vertex program with explicit diagnostic host constants.
The pixel-only fixtures below remain useful regression evidence; triangle
interpolation and complete render-state integration are still open.

The next three tasks add a bounded ps.1.1 parser, a diagnostic f32 evaluator,
and an independently checked fragment preview using original textures.
This executes the original **pixel** program. It does not execute the vertex
program or provide a complete hologram renderer or playable Android build.

## Parser and evaluator

`rc-package::pixel_shader` supports the original program's `tex`, `mov`, `mul`
and `dp3`, `_sat`, RGBA/RGB/A write masks and alpha source replication.
Registers are r0..1, t0..3, c0..7 and v0..1. Text is limited to 64 KiB,
16 instructions and eight arithmetic blocks. Texture loads must precede
arithmetic. Unsupported syntax, repeated loads and malformed co-issue pairs
are rejected. This is a diagnostic subset, not a complete D3D assembler or
hardware instruction validator; Xbox xps.1.1 is unsupported.

Every block records both temporary registers as raw f32 words and separate
component-defined flags. Reading an undefined component, incomplete r0 output,
nonfinite inputs and arithmetic overflow are errors. A co-issued RGB/alpha
pair evaluates both instructions against the same prior registers before
writing either result. Varying input colors are clamped to [0,1].

The semantics follow Microsoft's [write masks and co-issue documentation](https://learn.microsoft.com/en-us/windows/win32/direct3dhlsl/dx9-graphics-reference-asm-ps-registers-modifiers-write-mask),
[dp3 definition](https://learn.microsoft.com/en-us/windows/win32/direct3dhlsl/dp3---ps),
[saturation modifier](https://learn.microsoft.com/en-us/windows/win32/direct3dhlsl/dx9-graphics-reference-asm-ps-instructions-modifiers-ps-1-x)
and [input registers](https://learn.microsoft.com/en-us/windows/win32/direct3dhlsl/dx9-graphics-reference-asm-ps-registers-ps-1-x).
Operations use separate f32 multiplication/addition with explicit dot-product
association. [Legacy GPU precision](https://learn.microsoft.com/en-us/windows/win32/direct3dhlsl/dx9-graphics-reference-asm-ps-1-x)
is not guaranteed to match these diagnostic f32 values.

## Original input and fragment fixture

`Assets::pixel_constants` reads complete nested SConstantsInfo/Plane tagged
payloads rather than the report's truncated Raw prefix. Only serialized
material-defined constants are consumed. Dynamic constant kinds are rejected;
absent slots/components are diagnostic zeros, without class-default inference.
For HardwareShaders.Hologram.DynamicHologram, original c0 is
(0.5, 0.6, 0.5, 0); c1 is also read, then overridden for each wrapper using
its previously verified native R/G/B conversion and alpha 1.

The nine-instruction original pixel program forms eight blocks:

```text
L = saturate(dot(base.rgb, c0.rgb))
rgb = (((L * glow.rgb) * noise.rgb) * clamp(v0.rgb)) * c1.rgb
alpha = glow.a * c1.a
```

`rc-render::fragment` samples nearest/repeat textures and evaluates that
program. The separate diagnostic composite uses source alpha plus destination,
clamps RGB to [0,1], rounds to 8-bit and emits opaque ARGB. It does not alter
the existing geometry renderer. Original alpha test, fog, depth, filtering,
vertex deformation and full render-state handling remain outside this pass.

Twelve 128x128 previews cover six nonnull wrapper diffuse paths at fixture
phases 0 and 0.5. Holo_Blank is omitted because its effective runtime texture
requires host state. Glow/noise use the original GreyHoloFade/NoiseDigital
bindings. For pixel centers u/v, the explicit **test** UVs are (u,0.5), (u,v),
(2u+phase*0.17,2v+phase*0.07); v0 is (0.8,0.8,0.8,0.8) and the background is
FF101820. These values are not claimed as original vertex shader outputs.

## Validation and reproduction

533 workspace tests passed, together with Clippy (`-D warnings`) and formatting.
The independent Python oracle reads the shader, complete constants and original
textures directly from game packages. It compares 1,024 generated inputs,
8,192 raw register snapshots, final fragment floats, and all 196,608 preview
pixels. Original program identity is asserted before its separately written
formula is applied. This verifies the diagnostic arithmetic, not original GPU
execution or an Android device.

```powershell
cargo run -p rc-inspect --bin rc-pixel-check -- analysis/reports/hologram-materials.json 'D:\SteamLibrary\steamapps\common\Star Wars Republic Commando\GameData' analysis/reports/pixel-shader.json
cargo test --workspace 2>&1 | Tee-Object analysis/reports/pixel-tests.log
python scripts/Record-PixelShader.py
cargo clippy --workspace --all-targets -- -D warnings
cargo fmt --all -- --check
```

Evidence is in `analysis/reports/pixel-validation.json` and the
`pixel_shader_validation` entry of `analysis/evidence.json`. PNGs are
`analysis/reports/pixel-preview-00.png` through `pixel-preview-11.png`.
The general skeletal diffuse resolver remains at 271/289 slots; the diagnostic
fragment pass does not change that coverage. Android verification stays last.
