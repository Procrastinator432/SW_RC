"""Check shared physics refactor against stored original-map crouch reports."""
from pathlib import Path
import hashlib
import json

root=Path(__file__).resolve().parents[1]
folder=root/"analysis/collision"
def normalize(value):
    if isinstance(value,dict):
        result={}
        for key,item in value.items():
            if key=="uncrouch_time":
                assert item==0.0, "Original regression probes must keep timer inactive"
                continue
            if key=="crouch_motion_scope":
                continue
            result[key]=normalize(item)
        return result
    if isinstance(value,list):
        return [normalize(item) for item in value]
    return value

maps=[]
for name in ("geo_01a","dm_hangar"):
    new=json.loads((folder/f"{name}-physics-crouch.json").read_text())
    old=json.loads((folder/f"{name}-crouch-motion.json").read_text())
    assert normalize(new)==normalize(old), f"Regression mismatch: {name}"
    maps.append({"map":name,"crouch_ticks":sum(len(p["frames"]) for p in new["crouch_motion_probes"]),"script_ticks":sum(len(p["frames"]) for p in new["pawn_diagnostic_probes"]),"legacy_ticks":sum(len(p["frames"]) for p in new["controller_probes"]),"comparison":"All report values equal after removing added inactive timer0 fields and updated scope string"})
sources=["crates/rc-package/src/crouch_motion.rs","crates/rc-package/src/pawn_motion.rs","crates/rc-package/src/hit_wall.rs"]
result={"date":"2026-10-06","rust_tests":138,"maps":maps,"crouch_ticks_unchanged":sum(m["crouch_ticks"] for m in maps),"script_ticks_unchanged":sum(m["script_ticks"] for m in maps),"legacy_ticks_unchanged":sum(m["legacy_ticks"] for m in maps),"source_sha256":{s:hashlib.sha256((root/s).read_bytes()).hexdigest() for s in sources},"scope":"Physics-only entry preserves stored request and pending script event; synthetic HitWall arming->crouch->timer expiry->stand/retry tests. Original-map runs compare existing player path with inactive timer; no original AI runtime parity or automatic HitWall dispatch.","checks":["cargo test --workspace","cargo clippy --workspace --all-targets -- -D warnings","cargo fmt --all -- --check"],"android":"deferred until end per user"}
assert result["crouch_ticks_unchanged"]==210 and result["script_ticks_unchanged"]==1906 and result["legacy_ticks_unchanged"]==2340
(folder/"physics-crouch-validation.json").write_text(json.dumps(result,indent=2)+"\n",encoding="utf-8")
print(json.dumps({"crouch_ticks":210,"script_ticks":1906,"legacy_ticks":2340,"unchanged":True}))
