"""Compare class-derived wall filters against historical AI target probes."""
from pathlib import Path
import hashlib
import json

root=Path(__file__).resolve().parents[1]
folder=root/"analysis/collision"
maps=[]
for name in ("geo_01a","dm_hangar"):
    new=json.loads((folder/f"{name}-ai-wall-metadata.json").read_text())
    old=json.loads((folder/f"{name}-ai-duck-target.json").read_text())
    actor_classes={a["actor"]:a["class"] for a in new["actors"]}
    expected={a["actor"] for a in new["actors"] if "result" in a or "error" in a}
    for p in new["ai_contact_probes"]:
        metadata=p["wall_metadata"]
        assert {m["object"] for m in metadata}==expected
        for m in metadata:
            assert m["class"]==actor_classes[m["object"]]
            assert not m["notify_handled"] and m["notify_source"]=="supplied diagnostic false"
        for frame in p["frames"]:
            result=frame["result"]
            if result["wall_object"] not in (None,"World.BSP"):
                assert result["wall_object"] in expected
    def normalize(report):
        report.pop("ai_contact_scope")
        for p in report["ai_contact_probes"]:
            p.pop("wall_metadata",None)
        return report
    probes=new["ai_contact_probes"]
    maps.append({"map":name,"classified_static_objects":len(expected),"probe_ticks":sum(len(p["frames"]) for p in probes),"dispatches":sum(f["result"]["dispatch"] is not None for p in probes for f in p["frames"]),"outcomes":[p["outcome"] for p in probes],"excluded_pawn_objects":sum(m["excluded_wall_class"] for m in probes[0]["wall_metadata"])})
    assert normalize(new)==normalize(old), f"Changed previous AI target report: {name}"
proof=json.loads((root/"analysis/reports/hit-wall-class-controller-proof.json").read_text())
sources=["crates/rc-package/src/ai_contact.rs","crates/rc-inspect/src/pawn_probes.rs","crates/rc-inspect/src/bin/rc-mesh-probe.rs","analysis/decompiled/pawn-human-controller.c"]
result={"date":"2026-10-06","rust_tests":148,"native_proof":proof,"maps":maps,"total_static_objects":sum(m["classified_static_objects"] for m in maps),"unchanged_ai_ticks":sum(m["probe_ticks"] for m in maps),"checks":["cargo test --workspace","cargo clippy --workspace --all-targets -- -D warnings","cargo fmt --all -- --check"],"source_sha256":{s:hashlib.sha256((root/s).read_bytes()).hexdigest() for s in sources},"scope":"Actor class exclusion now from original serialized ancestry with native APawn identity proof; NotifyHandled remains supplied false, world policy explicit. Controller virtual predicate verified for base classes only, runtime overrides/possession not implemented. Entire prior AI target report unchanged except class metadata and scope string.","android":"deferred until end per user"}
assert result["unchanged_ai_ticks"]==424
(folder/"ai-wall-metadata-validation.json").write_text(json.dumps(result,indent=2)+"\n",encoding="utf-8")
print(json.dumps({"static_objects":result["total_static_objects"],"unchanged_ai_ticks":424,"maps":maps}))
