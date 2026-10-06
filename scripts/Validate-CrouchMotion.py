"""Summarize original-map crouched locomotion and unchanged earlier diagnostics."""
from pathlib import Path
from collections import Counter
import hashlib
import json

root=Path(__file__).resolve().parents[1]
folder=root/"analysis/collision"
probes=[]
script_ticks=legacy_ticks=0
for name in ("geo_01a","dm_hangar"):
    report=json.loads((folder/f"{name}-crouch-motion.json").read_text())
    previous=json.loads((folder/f"{name}-script-motion.json").read_text())
    assert {k:v for k,v in report.items() if k not in {"crouch_motion_profile","crouch_motion_probes","crouch_motion_scope"}}==previous
    shape=report["crouch_motion_profile"]
    assert shape["standing"]==[40,40,84] and shape["crouching"]==[40,40,56]
    script_ticks+=sum(len(p["frames"]) for p in report["pawn_diagnostic_probes"])
    legacy_ticks+=sum(len(p["frames"]) for p in report["controller_probes"])
    for probe in report["crouch_motion_probes"]:
        assert probe["completed"]
        state=probe["final_state"]
        assert not state["crouched"] and not state["try_to_uncrouch"]
        assert not state["script"]["pressed_jump"] and not state["script"]["wants_to_crouch"]
        assert state["script"]["body"]["grounded"] and state["script"]["body"]["velocity"]==[0.0]*3
        phases=Counter(f["phase"] for f in probe["frames"])
        assert phases["duck_move"]==30 and phases["release"]==1 and phases["stand_idle"]>=30
        crouched=falling=blocked=changes=0
        peak=0.0
        for frame in probe["frames"]:
            f=frame["result"];movement=f["movement"]
            assert movement["volume"]["complete"] and not movement["jump_started"]
            assert not frame["input"]["jump_event"]
            crouched+=int(f["state"]["crouched"])
            falling+=int(movement["mode"]=="Falling")
            velocity=movement["state"]["body"]["velocity"]
            peak=max(peak,(velocity[0]**2+velocity[1]**2)**0.5)
            for transition in (f["before_movement"],f["after_movement"]):
                if transition:
                    blocked+=int(transition["change"]=="Blocked")
                    changes+=int(transition["change"]=="Changed")
        assert crouched>0 and changes>=2
        z=[f["result"]["state"]["script"]["body"]["position"][2] for f in probe["frames"][-30:]]
        assert max(z)-min(z)==0
        probes.append({"map":name,"start":probe["start"],"ticks":len(probe["frames"]),
                       "phase_ticks":dict(phases),"crouched_end_ticks":crouched,"falling_ticks":falling,
                       "shape_changes":changes,"blocked_standing_attempts":blocked,
                       "peak_horizontal_speed":peak,"last30_height_drift":max(z)-min(z),"final_state":state})
engine=Path(r"D:\SteamLibrary\steamapps\common\Star Wars Republic Commando\GameData\System\engine.dll")
result={"date":"2026-10-06","rust_tests":124,"probes":probes,"crouch_motion_ticks":sum(p["ticks"] for p in probes),
        "script_ticks_unchanged":script_ticks,"legacy_ticks_unchanged":legacy_ticks,
        "physics_dispatch_address":"104955f0","engine_dll_sha256":hashlib.sha256(engine.read_bytes()).hexdigest(),
        "scope":"normal crouch-before/uncrouch-after order with own static solver; auto timer rejected; no callbacks/native runtime parity. Blocked standing/retry synthetically tested.",
        "android":"deferred until end per user"}
assert script_ticks==1906 and legacy_ticks==2340
(folder/"crouch-motion-validation.json").write_text(json.dumps(result,indent=2)+"\n",encoding="utf-8")
print(json.dumps({"ticks":result["crouch_motion_ticks"],"probes":[{k:p[k] for k in ("start","ticks","peak_horizontal_speed","falling_ticks","shape_changes")} for p in probes]}))
