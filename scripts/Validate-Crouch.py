"""Validate isolated original-dimension shape cycles and unchanged movement reports."""
from pathlib import Path
import hashlib
import json

root = Path(__file__).resolve().parents[1]
folder = root / "analysis/collision"
defaults = json.loads((root / "analysis/reports/crouch-defaults.json").read_text())
assert defaults["standing"] == [40.0, 40.0, 84.0]
assert defaults["crouching"] == [40.0, 40.0, 56.0]
probes = []
motion_ticks = legacy_ticks = 0
for name in ("geo_01a", "dm_hangar"):
    report = json.loads((folder / f"{name}-crouch.json").read_text())
    previous = json.loads((folder / f"{name}-script-motion.json").read_text())
    assert {k:v for k,v in report.items() if k not in {"crouch_profile", "crouch_probes", "crouch_scope"}} == previous
    assert report["crouch_profile"] == defaults
    motion_ticks += sum(len(p["frames"]) for p in report["pawn_diagnostic_probes"])
    legacy_ticks += sum(len(p["frames"]) for p in report["controller_probes"])
    assert len(report["crouch_probes"]) == len(report["pawn_diagnostic_probes"])
    for probe in report["crouch_probes"]:
        initial = probe["initial_state"]
        feet = initial["body"]["position"][2] - 84.0
        assert probe["final_state"] == initial
        assert len(probe["frames"]) == 20
        for i, frame in enumerate(probe["frames"]):
            crouched = i % 2 == 0
            assert frame["change"] == "Changed" and frame["state"]["crouched"] == crouched
            assert frame["extent"] == defaults["crouching" if crouched else "standing"]
            assert frame["height_adjustment"] == (28.0 if crouched else -28.0)
            body = frame["state"]["body"]
            assert body["position"][2] - frame["extent"][2] == feet
            assert body["position"][:2] == initial["body"]["position"][:2]
            assert body["velocity"] == initial["body"]["velocity"]
            assert body["grounded"] == initial["body"]["grounded"]
            assert frame["target_placement"]["state"] != "Penetrating"
        probes.append({"map":name,"start":probe["start"],"shape_changes":20,
                       "feet_z":feet,"final_state_unchanged":True})
engine = Path(r"D:\SteamLibrary\steamapps\common\Star Wars Republic Commando\GameData\System\engine.dll")
result = {"date":"2026-10-06","rust_tests":120,"probes":probes,
          "shape_changes":sum(p["shape_changes"] for p in probes),
          "script_motion_ticks_unchanged":motion_ticks,"legacy_ticks_unchanged":legacy_ticks,
          "native_exports":{"Crouch":"104819c0","UnCrouch":"10481c80","ForceCrouch":"10485040"},
          "engine_dll_sha256":hashlib.sha256(engine.read_bytes()).hexdigest(),
          "scope":"isolated shape changes at static original endpoints; native height compensation, own AABB placement. Blocked ceiling synthetically tested; no crouched locomotion/native tick timing/FarMove/callback parity",
          "android":"deferred until end per user"}
assert result["shape_changes"] == 60 and motion_ticks == 1906 and legacy_ticks == 2340
(folder / "crouch-validation.json").write_text(json.dumps(result,indent=2)+"\n",encoding="utf-8")
print(json.dumps({"shape_changes":result["shape_changes"],"unchanged_script_ticks":motion_ticks,"unchanged_legacy_ticks":legacy_ticks}))
