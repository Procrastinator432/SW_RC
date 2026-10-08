# TexPanner2D material sampling

Three tasks completed: native UV matrix calculation, serialized class/instance
parameter loading, and explicit time-sampled diffuse/renderer integration.

## Native calculation

`engine.dll!UTexPanner2D::GetMatrix` at `10452210` reads SpeedU/V at
0x70/74, OffsetU/V at 0x78/7c, ScaleU/V at 0x80/84 and ClampedSizeU/V at
0x88/8c. Matrix scale is stored at elements 0/5, offsets at 8/9, and
elements 10/15 are one. The cached M is overwritten each call.

Each phase uses scalar f32 division and multiplication. Periodic reduction
adds binary64 constant 103079215104 (PE address 10673b00), stores a double,
extracts its low signed dword and shifts right by 16. `TexPanner2D::at`
retains this procedure instead of floor/fract. Subtract/multiply/add use
separate f32 operations. For a clamped size above one, offsets strictly above
size-minus-one have size subtracted once. Finite zero/negative scales are
retained; nonfinite input/intermediate output and zero clamp sizes are rejected.

## Original parameters

The class parser reads TexPanner2D defaults from Engine.u: SpeedU/V=0.5,
ScaleU/V=1, ClampedSizeU/V=1; absent offsets are zero initialized. Instance
floats override these defaults, including explicit zero. Transient M is ignored.

All 15 distinct material paths previously blocking 52 skeletal slots resolve
through the new time-sampled path. Their stored speeds are zero, so these
assets do not move until runtime code changes parameters. Script-driven
ammunition/digit updates are not connected in this turn.

## Integration and boundaries

`Assets::diffuse_at_time(path, limit, time)` returns original diffuse pixels,
parameters and sampled diagonal UV transform. Legacy scalar-only resolver
entry points retain their panner omission, avoiding accidental loss of offsets.
Multiple panners or a nontrivial bump scale combined with a panner remain
explicit errors; general modifier order is open.

`Scene::apply_skeletal_uv_transforms` transforms UVs by material slot after
texture binding. All matrices and transformed UVs validate before commit.
Use fresh source UVs each frame; repeated calls accumulate transforms.
Fifteen diagnostic triangle snapshots exercise texture binding and sampled
UV serialization. These are material fixtures, not animated game meshes.

The earlier report retains its historical 212/77 split. The new report
recovers 52 of those omissions (37 remain). This is base-diffuse coverage in
the new offline API, not full material/game completion.

## Verification

`Record-Panner2D.py` independently parses package tables/property tags,
validates the complete class-default suffix and PE rounding constant,
decodes 417792 original pixels, and checks all 15 snapshots.
F32 bit patterns match for 90 original time samples and 265 numerical probes,
including positive/negative phases, different clamp sizes, strict branches
and values near periodic rounding boundaries.

504 workspace tests, Clippy with denied warnings and formatting pass.
Evidence/hashes: `analysis/reports/panner2d-validation.json`.

```powershell
cargo run -p rc-inspect --bin rc-panner2d-check -- analysis/reports/skeletal-materials.json 'D:\SteamLibrary\steamapps\common\Star Wars Republic Commando\GameData' analysis/reports/panner2d-materials.json
cargo test --workspace | Tee-Object -FilePath analysis/reports/panner2d-tests.log
python scripts/Record-Panner2D.py
cargo clippy --workspace --all-targets -- -D warnings
cargo fmt --all -- --check
```

Remaining: TexPanner/TexOscillator, hologram and other effects, modifier
composition, script parameter changes, live material/animation integration
and wider game systems. Android verification remains at the end.
