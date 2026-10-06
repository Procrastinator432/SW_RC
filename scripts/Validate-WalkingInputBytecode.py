"""Reviewed ProcessMove/ShouldCrouch and PlayerMove jump-tail structure, no VM."""
from pathlib import Path
import hashlib
import json

root = Path(__file__).resolve().parents[1]
reports = root / "analysis/reports"
report = json.loads((reports / "player-walking-bytecode.json").read_text())
assert report["errors"] == 0
data = Path(report["package"]).read_bytes()

def fmt(e):
    value = e["operand"].get("value")
    if isinstance(value, dict):
        value = value.get("path", f"ctx:{value.get('skip')}/{value.get('result_size')}")
    return (f"{e['opcode']:02x}" + (f":{value}" if value is not None else "")
            + ("(" + ",".join(fmt(c) for c in e["children"]) + ")" if e["children"] else ""))

expected_process = [
    (0, "07:13(72:114(01:Controller.Pawn,2a,16))"), (11, "04(0b)"),
    (13, "0f(00:PlayerController.PlayerWalking.ProcessMove.OldAccel,19:ctx:5/12(01:Controller.Pawn,01:Actor.Acceleration))"),
    (33, "07:77(da:218(19:ctx:5/12(01:Controller.Pawn,01:Actor.Acceleration),00:PlayerController.PlayerWalking.ProcessMove.newAccel,16))"),
    (57, "0f(19:ctx:5/12(01:Controller.Pawn,01:Actor.Acceleration),00:PlayerController.PlayerWalking.ProcessMove.newAccel)"),
    (77, "07:107(2d(01:PlayerController.bPressedJump))"),
    (86, "19:ctx:12/4(01:Controller.Pawn,1b:DoJump(2d(01:PlayerController.bUpdating),16))"),
    (107, "07:220(9b:155(39:58(19:ctx:5/1(01:Controller.Pawn,01:Actor.Physics)),39:58(24:2),16))"),
    (132, "14(2d(00:PlayerController.PlayerWalking.ProcessMove.OldCrouch),19:ctx:6/4(01:Controller.Pawn,2d(01:Pawn.bWantsToCrouch)))"),
    (154, "07:186(9a:154(39:58(01:Controller.bDuck),25,16))"),
    (167, "19:ctx:7/0(01:Controller.Pawn,1b:ShouldCrouch(28,16))"), (183, "06:220"),
    (186, "07:220(19:ctx:6/4(01:Controller.Pawn,2d(01:Pawn.bCanCrouch)))"),
    (204, "19:ctx:7/0(01:Controller.Pawn,1b:ShouldCrouch(27,16))"), (220, "04(0b)"),
]
expected_crouch = [
    (0, "14(2d(01:Pawn.bWantsToCrouch),2d(00:Pawn.ShouldCrouch.Crouch))"), (13, "04(0b)"),
]
prefix = "PlayerController.PlayerWalking.PlayerMove."
expected_tail = [
    (774, "14(2d(01:PlayerController.bDoubleJump),28)"),
    (782, "07:843(82:130(82:130(2d(01:PlayerController.bPressedJump),18:9(77:119(01:Controller.Pawn,2a,16)),16),18:16(19:ctx:6/4(01:Controller.Pawn,1b:CannotJumpNow(16))),16))"),
    (824, f"14(2d(00:{prefix}bSaveJump),27)"),
    (832, "14(2d(01:PlayerController.bPressedJump),28)"), (840, "06:851"),
    (843, f"14(2d(00:{prefix}bSaveJump),28)"),
    (851, "07:904(96:150(39:58(01:Actor.Role),39:58(24:4),16))"),
    (867, f"1b:ReplicateMove(00:{prefix}DeltaTime,00:{prefix}newAccel,00:{prefix}DoubleClickMove,61:317(00:{prefix}OldRotation,01:Actor.Rotation,16),16)"),
    (901, "06:938"),
    (904, f"1b:ProcessMove(00:{prefix}DeltaTime,00:{prefix}newAccel,00:{prefix}DoubleClickMove,61:317(00:{prefix}OldRotation,01:Actor.Rotation,16),16)"),
    (938, f"14(2d(01:PlayerController.bPressedJump),2d(00:{prefix}bSaveJump))"), (951, "04(0b)"),
]
expected = {"PlayerController.PlayerWalking.ProcessMove": (0, expected_process),
            "Pawn.ShouldCrouch": (0, expected_crouch),
            "PlayerController.PlayerWalking.PlayerMove": (774, expected_tail)}
assert set(expected) == {f["path"] for f in report["functions"]}
functions = []
for f in report["functions"]:
    minimum, instructions = expected[f["path"]]
    decoded = f["decoded"]
    actual = [(e["logical_offset"], fmt(e)) for e in decoded["expressions"] if e["logical_offset"] >= minimum]
    assert actual == instructions, f"bytecode mismatch: {f['path']}"
    payload = data[f["serial_offset"]:f["serial_offset"] + f["serial_size"]]
    functions.append({"path": f["path"], "logical_bytes": decoded["logical_script_bytes"],
                      "serialized_bytes": decoded["serialized_script_bytes"],
                      "decoded_instructions": len(decoded["expressions"]),
                      "reviewed_from_logical_offset": minimum,
                      "reviewed_instructions": instructions,
                      "payload_sha256": hashlib.sha256(payload).hexdigest()})
operators = json.loads((reports / "player-walking-native-operators.json").read_text())
assert operators["errors"] == 0
indices = {"Object.NotEqual_VectorVector": 218, "Object.NotEqual_IntInt": 155,
           "Object.Less_IntInt": 150, "Object.Subtract_RotatorRotator": 317}
assert {f["path"]: f["decoded"]["native_index"] for f in operators["functions"]} == indices
result = {"date": "2026-10-06", "rust_tests": 112, "functions": functions,
          "engine_package_sha256": hashlib.sha256(data).hexdigest(), "native_indices": indices,
          "core_package_sha256": hashlib.sha256(Path(operators["package"]).read_bytes()).hexdigest(),
          "scope": "full ProcessMove and ShouldCrouch AST; only PlayerMove jump tail reviewed. Isolated authority jump/crouch helper, no VM/collision/body-size/input binding/network parity",
          "android": "deferred until end per user"}
(reports / "walking-input-bytecode-validation.json").write_text(json.dumps(result, indent=2) + "\n", encoding="utf-8")
print(json.dumps({"reviewed_instructions": sum(len(f['reviewed_instructions']) for f in functions), "rust_tests": 112}))
