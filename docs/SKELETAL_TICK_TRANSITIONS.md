# Prepared notify and end transitions

Follow-up: `SKELETAL_LOD_TICK.md` connects these branches to a prepared LOD channel
loop, jitter and common completion. Counts and scope below describe this earlier
transition-only milestone.

Continuation of `SKELETAL_ANIMATION_TICK.md`. Source is the exported engine.dll
`ULodMeshInstance::UpdateAnimation` 10450240 in `animation-update-entry.asm`.
Implementation: `crates/rc-package/src/skeletal_animation_tick.rs`.

## Notify selection and dispatch, 104504dc..10450640

`select_tick_notify` scans prepared notify records in array order, accepting
`old < time <= current`. It selects the smallest f32 distance from old; ties keep
the first record. No sorting, wrapping or reverse playback is introduced.
NaN comparisons reject candidates; infinite distance ties also keep the first.

`dispatch_tick_notify` additionally checks the supplied Actor mesh presence and
channel byte 5 (word 1 bits 8..15). The chosen raw time word is committed before
the opaque non-null notify object is passed to `TickNotifyHost`. Remaining time
is `((current-time)*remaining)/(current-old)` with separate f32 operations.
A null object still consumes time. Callbacks can replace the channel vector;
the function does not overwrite their mutations. If the selected channel index
no longer exists after a callback, `ChannelRemoved` exposes the original early
return that bypasses the common cache invalidation and unlock. Host errors
preserve committed writes and mutations; invalid input indices return an error.

## End transitions, 10450645..10450769

`dispatch_tick_end` skips the branch only when `end > current` is ordered.
Looping is native byte 4 (word 1 low byte). Before frame 1, remaining time becomes
zero without resetting the frame. At or beyond 1, the frame becomes raw +0 and
remaining time is `((current-1)*remaining)/(current-old)`. The native unordered
comparison also takes this wrap path for NaN. AnimEnd is emitted only when
`old < end` and byte 5 is clear.

A nonlooping channel copies the raw end word and computes the analogous residual
time. Only a positive ordered playback rate enters stop handling. If signed
start-bone word 15 or channel-number word 3 is positive, the first eight words
are copied to the clear/play host with word 0 zeroed. This happens before setting
the live channel rate to +0. The current byte 5 is then read, and AnimEnd receives
the current channel number if permitted. A clear callback can change these fields.
The Rust fixed channel reference permits field mutations, not vector replacement
from end callbacks; native storage-lifetime effects remain an explicit boundary.
Failures preserve the writes made before the failing host call.

## Validation and remaining scope

13 additional tests cover unsorted notifies, boundary inclusion, duplicate times,
NaN/infinity, null callbacks, replacement/removal, suppress flags, partial writes,
clear/stop/AnimEnd ordering, before-end and inactive rates, loop residual time,
raw signed zero, and unordered loop handling. One composed tick test performs
frame advancement, two notifies, a loop wrap and the remaining frame step without
duplicate notifications. It exercises the helpers together; it is not a production
LOD tick implementation or an original-game runtime comparison.

407 workspace tests, Clippy with denied warnings, and formatting checks pass.
No Android checks were run.

Still external: notify object registration and actual Script/Actor callbacks,
sequence resolution during ticks, random jitter, the production four-iteration
loop, replication, lifecycle/lock/unlock, common-exit cache invalidation, and the
connection between the real game tick and frame evaluation. The new prepared
branches do not by themselves constitute playable animation or gameplay.
