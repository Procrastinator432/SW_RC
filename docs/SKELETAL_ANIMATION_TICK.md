# UpdateAnimation arithmetic blocks

Subsequent milestone: `SKELETAL_TICK_TRANSITIONS.md` adds prepared notify dispatch
and end/loop transitions. The counts and remaining-scope list below describe the
original arithmetic-only milestone.

Source: `analysis/decompiled/animation-update-entry.c` and `.asm`, engine.dll
`USkeletalMeshInstance::UpdateAnimation` 105001f0 and
`ULodMeshInstance::UpdateAnimation` 10450240.

Implemented in `crates/rc-package/src/skeletal_animation_tick.rs`:

* Channel prefix 10450345..104503bd advances blend word 11 from rate word 10,
  with an upper clamp of 1, and current weight word 14 toward target word 4
  at speed word 5. Negative delta is accepted; blend has no lower clamp.
* Frame arithmetic 104504ad..104504d7 uses the current jitter word 8, signed
  sequence frame count, sequence rate, remaining time and channel rate word 6.
  It writes frame word 7 without wrapping or clamping. Division by zero retains
  the floating-point infinity/NaN behavior rather than introducing a guard.
* Director budget 10500226..10500242 accrues word 26 from nonnegative rate word
  21, regardless of the director's enabled flags. Negative/NaN rates are skipped;
  negative zero qualifies. Multiplication and addition remain separate f32 steps.
* The skeletal wrapper calls a supplied LOD host first and then uses the current
  director list, including host replacements. A normal early return still accrues;
  a Rust host error preserves preceding host writes and skips accrual.

The ASM matters where the decompiler's floating-point comparisons are misleading.
`COMISS 1,blend` / `JNC` clamps an unordered blend result to 1. Weight target/current
unordered comparisons skip the weight write. A computed weight that is unordered
clamps to the target in either moving direction. Equal weights preserve the raw
word, including negative zero and NaN payloads in skipped comparisons.

Validation: eight focused tests, 394 workspace tests total, Clippy with denied
warnings and formatting checks pass. Tests cover crossings and untouched words,
negative delta, unordered branches, signed zero, zero frame count, director gates,
host mutation order and partial writes on host failure. The exported ASM is the
instruction-level source; this milestone does not claim execution equivalence
against a running original game or CPU exception/NaN-payload emulation.

Still open: the full LOD loop and its lock/unlock/lifetime behavior, random jitter
generation, sequence resolution during ticks, the four-iteration limit, notify
selection and callbacks, end/loop handling, AnimEnd, replication, and common-exit
cache invalidation. These arithmetic helpers are not automatically connected to
`evaluate_animation_frame`; connecting them as a complete tick now would omit
those behaviors. Android/emulator verification remains deferred to the end.
