"""Compare event-driven static movement to earlier own-edge trajectories."""
from pathlib import Path
import json

root = Path(__file__).resolve().parents[1]
folder = root / "analysis/collision"
probes = []
legacy_ticks = 0
for name in ("geo_01a", "dm_hangar"):
    current = json.loads((folder / f"{name}-script-motion.json").read_text())
    previous = json.loads((folder / f"{name}-jump-script.json").read_text())
    assert current["pawn_diagnostic_input_policy"] == "script_event"
    excluded = {"pawn_diagnostic_probes", "pawn_diagnostic_properties", "pawn_diagnostic_scope", "pawn_diagnostic_input_policy"}
    assert {k:v for k,v in current.items() if k not in excluded} == {k:v for k,v in previous.items() if k not in excluded}
    assert current["pawn_diagnostic_properties"][:2] == previous["pawn_diagnostic_properties"]
    crouch = current["pawn_diagnostic_properties"][2]
    assert crouch["name"] == "bCanCrouch" and crouch["resolved"]["value"]["type"] == "Bool"
    legacy_ticks += sum(len(p["frames"]) for p in current["controller_probes"])
    assert len(current["pawn_diagnostic_probes"]) == len(previous["pawn_diagnostic_probes"])
    for new, old in zip(current["pawn_diagnostic_probes"], previous["pawn_diagnostic_probes"]):
        assert new["start"] == old["start"] and new["runtime"] == old["runtime"]
        assert new["completed"] and new["error"] is None and new["jump_count"] == 2
        assert len(new["frames"]) == len(old["frames"])
        events = calls = 0
        for a, b in zip(new["frames"], old["frames"]):
            assert a["phase"] == b["phase"]
            axes = dict(b["input"]); axes["jump"] = False
            assert a["input"]["view"] == axes
            assert a["input"]["jump_event"] == (a["phase"] in {"first_jump", "second_jump"})
            assert not a["input"]["cannot_jump_now"] and a["input"]["duck"] == 0
            assert a["input"]["can_crouch"] == crouch["resolved"]["value"]["value"]
            events += int(a["input"]["jump_event"])
            na, ob = a["result"], b["result"]
            assert na["state"]["body"] == ob["state"]["body"]
            for key in ("mode", "next_mode", "jump_started", "volume", "motion"):
                assert na[key] == ob[key], f"{name}/{new['start']}/{key} differs"
            phase = na["input_phase"]
            calls += int(phase["do_jump_called"])
            assert phase["jump_started"] == na["jump_started"]
            assert not phase["saved_jump"] and not phase["pressed_jump_after"]
            assert not na["state"]["pressed_jump"] and not na["state"]["wants_to_crouch"]
        assert events == calls == 2
        state = new["final_state"]
        assert state["body"] == old["final_state"]["body"]
        assert state["body"]["grounded"] and state["body"]["velocity"] == [0.0]*3
        assert not state["pressed_jump"] and not state["wants_to_crouch"]
        probes.append({"map":name,"start":new["start"],"ticks":len(new["frames"]),
                       "events":events,"do_jump_calls":calls,"jumps":new["jump_count"],
                       "body_and_motion_frames_unchanged":True,"final_state":state})
result = {"date":"2026-10-06","rust_tests":116,"probes":probes,
          "ticks":sum(p["ticks"] for p in probes),"jumps":sum(p["jumps"] for p in probes),
          "legacy_ticks_unchanged":legacy_ticks,
          "scope":"event-driven jump phase with static PC collision; normal cases equal earlier own-edge body/motion frames. Deferred events/crouch/error cases checked synthetically; no native runtime parity",
          "android":"deferred until end per user"}
assert result["ticks"] == 1906 and result["jumps"] == 6 and legacy_ticks == 2340
(folder / "script-motion-validation.json").write_text(json.dumps(result,indent=2)+"\n",encoding="utf-8")
print(json.dumps({"ticks":result["ticks"],"jumps":result["jumps"],"legacy_ticks_unchanged":legacy_ticks}))
