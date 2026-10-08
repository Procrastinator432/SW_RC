# Hologram wrapper, ConstantColor and original shader inputs

Follow-up: [CPU pixel program execution and diagnostic previews](PIXEL_SHADER.md)
now evaluate the original pixel shader subset. The wrapper/catalog evidence
below records the earlier stage; full vertex/render-state execution remains open.

Three tasks completed: the native HsHologram wrapper host contract, exact
ConstantColor value copying, and a verified original shader/input catalog.

## Native wrapper contract

`UHsHologram::SetupShaderWrapper` at 10429760 returns zero immediately if the
shader implementation is null. Otherwise it saves the four words at shader
offsets 824..830 and the word at 8ac. A nonnull wrapper DiffuseTexture replaces
the shader word at 8b0. A null DiffuseTexture leaves that word alone; it does
not select the serialized FallbackMaterial as a replacement diffuse.

Unless UseMarkerColorInstead is set, the shader's four color words receive
R/255, G/255, B/255, 1 with the original f32 conversion constant at 10665594.
Stored HologramColor alpha is ignored for this temporary constant. The wrapper
FallbackMaterial field receives the effective word at 8b0 before the render
interface virtual +54 is called with the implementation and two zero arguments.

After a normal callback return, the saved **8ac** word is written into **8b0**.
This is not restoration of 8b0's own prior value. The four saved color words are
restored only outside marker-color mode; marker mode retains callback color
writes. The callback's signed integer result is returned unchanged, including
failure/negative values. Null shaders perform no callback or fallback update.

`material_hologram::setup_hologram` preserves these operations. Its shader
fields named texture0/texture1 model the observed words at 8ac/8b0; they are
raw host slots, not serialized object indices. Synthetic resource handles and
color state explicitly stand in for a live shader object. Reflected texture
array bindings are inventoried separately. Host-side wrapper/pointer mutation
or exception unwinding during the callback is outside this explicit contract.

## ConstantColor

`UConstantColor::GetColor` at 1030f2d0 copies the stored FColor word at +64
without inspecting time. Colors are stored B,G,R,A and exposed as the same
little-endian ARGB word. `stored_color` validates complete four-byte Color
structs, overlays instance values over class values, and retains zero alpha.
Absent values use the zero-initialized word.

The original GunshipLow_Interior constant is **0x00282b24**, including alpha
zero. The new kernel does not force opacity or silently convert it to a texture.
The existing diffuse resolver still omits ConstantColor pending material-pass
integration; this turn establishes its value semantics rather than a new
visible opaque surface.

## Original hardware shader and texture inputs

Ten used hologram slots correspond to seven material paths. All inherit
ShaderImplementation `HardwareShaders.Hologram.DynamicHologram` from the
HsHologram class defaults in Engine.u. The default HologramColor is the original
BGRA bytes ff ce 7a 32, overlaid by each instance's serialized color. Six paths
have explicit DiffuseTexture references; Holo_Blank retains the shader's
existing diffuse binding despite having a serialized fallback.

`rc-hologram-check` reads complete original VertexShaderText, PixelShaderText
and XPixelShaderText strings and all fixed-array Textures entries with their
indices. Stage 0 and stage 1 both reference the original GreyHoloFade texture;
stage 2 references NoiseDigital. Together with the six diffuse textures this
gives eight distinct input paths, with 413696 original pixels decoded at the
chosen preview mip limits.

`Assets::material_properties` and `class_properties` expose the tagged inputs
without claiming shader evaluation. The verified shader texts are also saved
as `analysis/decompiled/hologram-VertexShaderText.asm`,
`hologram-PixelShaderText.asm` and `hologram-XPixelShaderText.asm`.
The shader programs, their constants, vertex flicker, blending and complete
hologram effect are not executed by this turn.

## Independent evidence

`Record-Hologram.py` separately reads original package tables, tagged defaults,
array indices and complete string payloads. It validates every original wrapper
color/reference, all three shader texts, eight decoded texture inputs and the
ConstantColor word. It independently reproduces 1024 wrapper probes covering
marker/nonmarker paths, null/nonnull diffuse, host mutations, saved NaN/signed
zero color words, and negative/zero/positive return values. Nine Rust tests
also cover a null implementation and malformed color overrides.

526 workspace tests, Clippy with denied warnings, formatting and diff checks
pass. Hashes: `analysis/reports/hologram-validation.json`.

```powershell
cargo run -p rc-inspect --bin rc-hologram-check -- analysis/reports/skeletal-materials.json 'D:\SteamLibrary\steamapps\common\Star Wars Republic Commando\GameData' analysis/reports/hologram-materials.json
cargo test --workspace | Tee-Object -FilePath analysis/reports/hologram-tests.log
python scripts/Record-Hologram.py
cargo clippy --workspace --all-targets -- -D warnings
cargo fmt --all -- --check
```

The previous diffuse coverage remains 271/289 with 18 omissions. Reconstructing
the wrapper and inputs does not count as completing the hologram renderer.
Hardware shader execution, complete material passes and live game integration
remain open. Android verification is last.
