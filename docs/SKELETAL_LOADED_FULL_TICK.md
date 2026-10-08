# Loaded tick to complete skeletal pose

The `--full` mode of `rc-loaded-tick-check` connects the original-metadata LOD
tick directly to `apply_animation_full`. The tick owns the invalidation of the
shared byte 61; no diagnostic frame assignment or manual cache invalidation is
inserted between ticking and pose evaluation.

Each original mesh-animation linkup runs four continuous delta steps
.125/.25/.5/1.25. One loaded first sequence drives a supplied looping channel,
with cached/editor tick lookup alternating. Pose lookup uses the resulting cached
identity. The full entry prepares cold instance and persistent scratch arrays,
refreshes the real name-based linkup, samples tracks across the skeleton, commits
channel frame/blend history, builds parent matrices and publishes local bounds
before setting completion flags. The inverse-reference cache is shared by runs
on the same original mesh.

Each completed pose is queried a second time without a tick. Cache repeats must
preserve local arrays, matrices, bounds, scratch and channel words; preparation
still fetches the supplied scene transform and refreshes linkups. No additional
sequence resolution or bounds publication may occur.

Validation uses the existing 432 workspace tests plus an independent Python
integration oracle. The original sequence tick is recomputed with explicit f32
rounding; every bone is independently sampled from original tracks or checked
against reference fallback. Matrices are checked with the original ASM-expression
oracle, and bounds/sphere/publication-before-flags are independently recomputed.
The verifier also checks every host event, channel history, buffer/cache gate and
unchanged cached repeat. Counts and numerical errors are recorded in
`analysis/reports/loaded-full-animation-ticks-validation.json`.

Coverage: 900 complete poses, 29912 bone matrices, 900 cached repeats, 225 linkup
runs and 125 inverse-cache builds. Quaternion tolerance is 2e-6; position relative
tolerance is 2e-5 under the existing portable math policy. Clippy with denied
warnings and format checks pass.

Scope remains diagnostic: sequence/name/notify identities, looping channel input,
random integer 16384, identity scene transform, actor scale and bounds padding are
supplied. Callback hosts log events without executing Actor/Script logic. No
Directors are supplied in this particular original-data integration run. Native
object registration, real RNG/lifecycle/replication, end-callback channel-storage
relocation, game frame-loop integration and skinning/rendering remain open. This
is not yet a playable native game. Android verification stays deferred to the end.
