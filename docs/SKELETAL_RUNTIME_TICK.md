# Loaded sequence tick and root integration

`RuntimeTickHost` adapts `RuntimeAnimationBindings` and actual `StoredSequence`
metadata to the prepared `update_lod_animation` entry. It copies signed frame
count, rate bits, word 18 jitter amplitude, and raw notify timestamp words.
Tick lookup uses the same supplied name bindings as the pose host.

Non-null archive notify object indices require explicit `NotifyObjectBinding`
entries. These are scoped by loaded sequence identity because object indices are
package-local; they are never cast to native identities. First matching binding
wins. A missing binding or a non-null archive reference mapped to identity zero
returns an error. Archive index zero remains null even if a spurious binding
exists. Notify names do not substitute for native notify object callbacks.
All notify references are resolved before returning the tick sequence snapshot.

Lifecycle, random input, Actor mesh presence, notify/AnimEnd/clear and replication
remain supplied `RuntimeTickEnvironment` boundaries. The adapter introduces no
implicit no-op implementation of those methods.

`rc-loaded-tick-check` runs four continuous ticks with delta .125/.25/.5/1.25 per
original mesh-animation linkup, alternates cached/editor lookup, and immediately
evaluates the root pose through `apply_animation_root`. There is no manual frame
assignment or cache clearing between tick and root evaluation. It reads original
archives, uses real skeletons and actual name-based linkup refresh, and samples
the original first-sequence root tracks. The same loaded identity catalog is
used for tick metadata and pose tracks.

The diagnostic chooses one looping channel and opaque sequence/name/notify
identities, supplies constant random input 16384 and an identity scene transform.
Notify, AnimEnd and replication calls are logged without executing gameplay.
Those choices are diagnostic inputs, not recovered Actor state or object loading.

Validation: 432 workspace tests, Clippy with denied warnings and format checks
pass. Six new adapter tests exercise metadata copying and object binding failures.
900 tick/root cases over 225 linkup runs are independently checked by
`Record-LoadedAnimationTicks.py`: separate f32 tick arithmetic and event selection
from original metadata, original root-track sampling, bit-exact matrix construction
from the sampled quaternion, cache invalidation, refreshed mappings and unchanged
nonroot buffers. Root sampling tolerances are 2e-6 quaternion and 2e-5 relative
position under the existing portable math policy. Counts and measured errors are
in `analysis/reports/loaded-animation-ticks-validation.json`.

Remaining: actual object registration and Actor/Script/notify execution, RNG state,
lifecycle implementation, replication transport, end-callback channel relocation,
full-pose tick integration and the actual game frame loop. This milestone does not
provide playable gameplay or rendering. Android/emulator verification is deferred
until the end.
