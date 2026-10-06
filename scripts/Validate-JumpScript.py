"""Compare new original-map reports to the earlier common motion milestone."""
from pathlib import Path
import copy
import json

root = Path(__file__).resolve().parents[1]
probes = []
legacy_ticks = 0
for name in ("geo_01a", "dm_hangar"):
    folder = root / "analysis/collision"
    current = json.loads((folder / f"{name}-jump-script.json").read_text(encoding="utf-8-sig"))
    previous = json.loads((folder / f"{name}-pawn-diagnostic.json").read_text(encoding="utf-8-sig"))
    normalized = copy.deepcopy(current)
    normalized["pawn_diagnostic_scope"] = previous["pawn_diagnostic_scope"]
    for probe in normalized["pawn_diagnostic_probes"]:
        runtime = probe["runtime"]
        assert runtime.pop("wants_to_crouch") is False
        assert runtime.pop("current_jump_z") == current["controller_options"]["jump_speed"]
    assert normalized == previous, f"unexpected report/trajectory change in {name}"
    legacy_ticks += sum(len(p["frames"]) for p in current["controller_probes"])
    for probe in current["pawn_diagnostic_probes"]:
        assert probe["completed"] and probe["error"] is None
        assert probe["jump_count"] == 2
        state = probe["final_state"]
        assert state["body"]["grounded"] and state["body"]["velocity"] == [0.0] * 3
        assert not state["jump_held"]
        probes.append({"map": name, "start": probe["start"], "ticks": len(probe["frames"]),
                       "jumps": probe["jump_count"], "frames_and_final_state_unchanged": True})
result = {"date": "2026-10-06", "rust_tests": 100,
          "source_audit": "analysis/reports/jump-script-source.json",
          "source_basis": "stored TextBuffer, executable bytecode/runtime parity unverified",
          "probes": probes, "ticks": sum(p["ticks"] for p in probes),
          "jumps": sum(p["jumps"] for p in probes), "legacy_ticks_unchanged": legacy_ticks,
          "report_equality": "entire JSON equal except updated scope and two explicit runtime fields",
          "android": "deferred until end per user"}
assert result["ticks"] == 1906 and result["jumps"] == 6 and legacy_ticks == 2340
path = root / "analysis/collision/jump-script-validation.json"
path.write_text(json.dumps(result, indent=2, ensure_ascii=False) + "\n", encoding="utf-8")
print(json.dumps(result, ensure_ascii=False))
