# Prepared LOD animation tick

`skeletal_lod_tick::update_lod_animation` connects the previously reconstructed
blend, frame, notify and end-transition blocks into the channel control flow of
engine.dll `ULodMeshInstance::UpdateAnimation` (10450240).

The host supplies loaded sequence metadata and opaque sequence/notify identities,
name lookup, random integers, Actor mesh presence, replication and lifecycle calls.
The function is a prepared runtime entry, not yet a game-world/Actor integration.

Order and behavior:

1. Call the native +a8 boundary with true. Exactly zero delta skips the initial
   flags query; otherwise bit 2 from +ac skips channel processing.
2. Visit channels in ascending index order, observing the current vector length.
   Update blend and weight once per channel, even when playback is inactive.
3. Stop inner processing for null name, zero playback rate, or nonpositive/unordered
   remaining delta. NaN playback rate is not equal to zero and does not stop here.
4. Perform at most four sequence attempts per channel. Editor mode refreshes word
   17 by name with load=false; other mode reads it. Null sequences consume an
   attempt and retry without advancing the frame.
5. For a positive ordered random amplitude, request a random integer and advance
   jitter in native f32 order. Then advance the frame, process a selected notify,
   or handle the end/loop branch. Reacquire the selected vector entry on retry
   after a notify callback; changes and replacement are retained.
6. Replicate the current channel once after its inner loop, even if inactive or
   stopped by the attempt limit. The host receives index and vector, representing
   native `ReplicateAnim(index,index,channel,true)`. Appended channels can be visited
   in the same outer loop, as the native length is read again.
7. On common completion, invalidate byte 61 before querying flags again. Bit 2
   requests destruction; otherwise call the +a8 boundary with false. The lifecycle
   method names express supplied host behavior, not a recovered lock implementation.

A notify callback that removes the selected channel returns `NotifyChannelRemoved`
before replication, cache invalidation or final lifecycle calls. Composing this
entry with `update_animation_with_lod` still accrues Director budgets after that
normal early return, matching the skeletal wrapper. Rust host errors preserve
preceding writes and propagate; no invented rollback or exception cleanup runs.

## Jitter constants and arithmetic

Read directly from original engine.dll:

| Address | Bits | Meaning |
| --- | --- | --- |
| 10672164 | 38800100 | Random scaling factor, approximately 2/32767 |
| 10668e18 | 41200000 | Jitter change factor 10 |
| 10653578 | 3f800000 | 1 |
| 1065efcc | 00000000 | 0 |

The update is `(((((float(random)*factor)-1)*amplitude)*remaining)*10)+prior`,
with each operation rounded separately to f32, followed by native lower/upper
comparison branches. NaN jitter passes through these comparisons. The platform
random generator and its seed/state remain supplied host behavior.

## Verification and boundaries

19 new tests, 426 workspace tests total, Clippy with denied warnings and format
checks pass. Tests include cached/editor/null lookup, the attempt cap, a combined
two-notify/loop/residual-frame tick, mutable notify lists, zero/negative/NaN time,
NaN playback rate, jitter gates/clamps, lifecycle short-circuit order, final flag
recheck, failure writes, replication appends and Director accrual after early exit.
The recording script verifies the constants in the original PE image and hashes
the source, exported instructions and original binary.

Remaining: concrete sequence/notify object binding to this tick host, real Actor
and Script callback execution, random-generator state, replication transport,
native lifecycle implementation, and connection to game tick/frame evaluation.
The existing end host uses a fixed channel reference: clear/AnimEnd may change
fields but may not relocate channel storage. Native end-callback storage lifetime
effects are therefore outside this prepared model. Stats/rdtsc instrumentation,
tagged-array ownership bits, CPU exception modes and NaN payload equivalence are
also not emulated. No original-game execution comparison or playable-game claim
is made. Android/emulator verification remains deferred until the end.
