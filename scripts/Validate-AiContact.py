"""Summarize original static AI contact diagnostics and unchanged prior reports."""
from pathlib import Path
from collections import Counter
import hashlib
import json
import argparse

parser=argparse.ArgumentParser(description=__doc__)
parser.add_argument("--duck-targets",action="store_true")
args=parser.parse_args()
suffix="ai-duck-target" if args.duck_targets else "ai-contact"
direction_count=32 if args.duck_targets else 8

root=Path(__file__).resolve().parents[1]
folder=root/"analysis/collision"
probes=[]
script_ticks=legacy_ticks=0
for name in ("geo_01a","dm_hangar"):
    report=json.loads((folder/f"{name}-{suffix}.json").read_text())
    previous=json.loads((folder/f"{name}-script-motion.json").read_text())
    assert {k:v for k,v in report.items() if k not in {"ai_contact_profile","ai_contact_probes","ai_contact_scope"}}==previous
    script_ticks+=sum(len(p["frames"]) for p in report["pawn_diagnostic_probes"])
    legacy_ticks+=sum(len(p["frames"]) for p in report["controller_probes"])
    for p in report["ai_contact_probes"]:
        assert p["error"] is None
        assert len(p["searches"])==direction_count
        assert all(s["query"]["complete"] for s in p["searches"])
        decisions=Counter()
        objects=Counter()
        crouched=changes=falling=0
        for f in p["frames"]:
            result=f["result"]
            physics=result["physics"]
            assert physics["movement"]["volume"]["complete"]
            assert not physics["movement"]["jump_started"]
            assert result["state"]["script"]["body"]["velocity"]==physics["state"]["script"]["body"]["velocity"]
            assert not result["state"]["script"]["pressed_jump"]
            crouched+=int(result["state"]["crouched"])
            falling+=int(physics["movement"]["mode"]=="Falling")
            for transition in (physics["before_movement"],physics["after_movement"]):
                changes+=int(transition is not None and transition["change"]=="Changed")
            dispatch=result["dispatch"]
            if dispatch:
                decisions[dispatch["decision"]]+=1
                objects[result["wall_object"]]+=1
                assert result["incoming_velocity"] is not None
                if dispatch["decision"] in ("ArmedFirst","ArmedSecond"):
                    state=result["state"]
                    assert state["script"]["wants_to_crouch"] and state["try_to_uncrouch"]
                    assert state["uncrouch_time"]==0.5
        if p["outcome"]=="NoTarget":
            assert not p["frames"]
        else:
            assert sum(f["phase"]=="idle" for f in p["frames"])==90
        predictions=Counter(prediction["result"]["decision"] for search in p["searches"] for prediction in search.get("predictions",[]))
        probes.append({"map":name,"start":p["start"],"ticks":len(p["frames"]),"outcome":p["outcome"],"dispatch_decisions":dict(decisions),"contact_objects":dict(objects),"crouched_ticks":crouched,"shape_changes":changes,"falling_ticks":falling,"search_predictions":dict(predictions),"target":p.get("target"),"final_state":p["final_state"]})
assert script_ticks==1906 and legacy_ticks==2340
sources=["crates/rc-inspect/src/pawn_probes.rs","crates/rc-inspect/src/bin/rc-mesh-probe.rs","crates/rc-package/src/ai_contact.rs"]
result={"date":"2026-10-06","rust_tests":147,"probes":probes,"ticks":sum(p["ticks"] for p in probes),"dispatches":sum(sum(p["dispatch_decisions"].values()) for p in probes),"script_ticks_unchanged":script_ticks,"legacy_ticks_unchanged":legacy_ticks,"source_sha256":{s:hashlib.sha256((root/s).read_bytes()).hexdigest() for s in sources},"scope":"Original static geometry with diagnostic player dimensions and supplied nonhuman/controller/event snapshots; own post-step timing and eight-direction target search. No original AI controller/event or native runtime parity.","android":"deferred until end per user"}
result["target_search"]="32directions/1500units with separate estimated-impact CanCrouchWalk predictions; prefer predicted Armed but actual contact required" if args.duck_targets else "8directions/600units; nearest approaching nonfloor contact"
result["scope"]="Original static geometry with diagnostic player dimensions and supplied nonhuman/controller/event snapshots; own post-step timing. Target predictions are not actual contact/arming coverage. No native AI runtime parity."
(folder/f"{suffix}-validation.json").write_text(json.dumps(result,indent=2)+"\n",encoding="utf-8")
print(json.dumps({"ticks":result["ticks"],"dispatches":result["dispatches"],"probes":[{k:p[k] for k in ("map","start","ticks","outcome","dispatch_decisions","shape_changes")} for p in probes]}))
