"""Verify reviewed control flow against actual decoded package bytes, without a VM."""
from pathlib import Path
import hashlib
import json
import re

root = Path(__file__).resolve().parents[1]
reports = root / "analysis/reports"

def load(name):
    report = json.loads((reports / name).read_text(encoding="utf-8"))
    assert report["errors"] == 0
    data = Path(report["package"]).read_bytes()
    return report, data

def fmt(e):
    operand = e["operand"]
    value = operand.get("value")
    if isinstance(value, dict):
        value = value.get("path", f"ctx:{value.get('skip')}/{value.get('result_size')}")
    return (f"{e['opcode']:02x}" + (f":{value}" if value is not None else "")
            + ("(" + ",".join(fmt(c) for c in e["children"]) + ")" if e["children"] else ""))

def inst(path):
    return f"01:{path}"

def boolean(path):
    return f"2d({inst(path)})"

def not_(value):
    return f"81:129({value},16)"

def and_(a, b, skip):
    return f"82:130({a},18:{skip}({b}),16)"

def or_(a, b, skip):
    return f"84:132({a},18:{skip}({b}),16)"

def physics(value):
    return f"9a:154(39:58({inst('Actor.Physics')}),39:58(24:{value}),16)"

def z(value):
    return f"36:Core.Object.Vector.Z({value})"

velocity = inst("Actor.Velocity")
velocity_z = z(velocity)
entry = and_(and_(not_(boolean("Pawn.bIsCrouched")), not_(boolean("Pawn.bWantsToCrouch")), 9),
             or_(or_(physics(1), physics(11), 14), physics(9), 14), 50)
base = inst("Actor.Base")
base_guard = and_(f"77:119({base},2a,16)",
                  not_(f"19:ctx:6/4({base},{boolean('Actor.bWorldGeometry')})"), 18)
expected_jump = [
    (0, f"07:295({entry})"),
    (78, "07:94(9a:154(39:58(01:Actor.Role),39:58(24:4),16))"),
    (94, "1c:Pawn.PlayOwnedCue(24:24,16)"),
    (102, f"07:139({physics(9)})"),
    (118, f"0f({velocity},d5:213(01:Pawn.JumpZ,01:Pawn.Floor,16))"),
    (136, "06:218"),
    (139, f"07:174({physics(11)})"),
    (155, f"0f({velocity_z},1e:0.0)"),
    (171, "06:218"),
    (174, f"07:202({boolean('Pawn.bIsWalking')})"),
    (183, f"0f({velocity_z},02:Pawn.JumpZ)"),
    (199, "06:218"),
    (202, f"0f({velocity_z},01:Pawn.JumpZ)"),
    (218, f"07:282({base_guard})"),
    (251, f"b8:184({velocity_z},{z(f'19:ctx:5/12({base},{velocity})')},16)"),
    (282, "1c:Pawn.PlayJumpSoundOnCurrentMaterial(16)"),
    (288, "6f:3970(24:2,16)"),
    (293, "04(27)"), (295, "04(28)"), (297, "04(0b)"),
]
expected = {
    "Pawn.DoJump": expected_jump,
    "Pawn.CannotJumpNow": [(0, "04(28)"), (2, "04(0b)")],
    "PlayerController.Jump": [
        (0, "07:34(72:114(19:ctx:5/4(01:Actor.Level,01:LevelInfo.Pauser),01:Controller.PlayerReplicationInfo,16))"),
        (24, "1b:SetPause(28,16)"), (31, "06:42"),
        (34, "14(2d(01:PlayerController.bPressedJump),27)"), (42, "04(0b)"),
    ],
}
report, data = load("jump-bytecode.json")
assert set(expected) == {f["path"] for f in report["functions"]}
functions = []
for f in report["functions"]:
    decoded = f["decoded"]
    statements = [(e["logical_offset"], fmt(e)) for e in decoded["expressions"]]
    assert statements == expected[f["path"]], f"bytecode mismatch: {f['path']}"
    payload = data[f["serial_offset"]:f["serial_offset"] + f["serial_size"]]
    functions.append({"path": f["path"], "logical_bytes": decoded["logical_script_bytes"],
                      "serialized_bytes": decoded["serialized_script_bytes"],
                      "payload_sha256": hashlib.sha256(payload).hexdigest(),
                      "reviewed_instructions": statements})

operators, core_data = load("jump-native-operators.json")
expected_indices = {"Not_PreBool": 129, "AndAnd_BoolBool": 130, "OrOr_BoolBool": 132,
                    "AddEqual_FloatFloat": 184, "Multiply_FloatVector": 213,
                    "NotEqual_ObjectObject": 119, "EqualEqual_IntInt": 154,
                    "EqualEqual_ObjectObject": 114}
assert len(operators["functions"]) == len(expected_indices)
for f in operators["functions"]:
    assert f["decoded"]["native_index"] == expected_indices[f["path"].split(".")[-1]]
setphysics, engine_data = load("jump-setphysics.json")
assert engine_data == data
assert setphysics["functions"][0]["path"] == "Actor.SetPhysics"
assert setphysics["functions"][0]["decoded"]["native_index"] == 3970
enum_values = {}
for file, enum, names in [
    ("Actor.uc", "EPhysics", {"PHYS_Walking": 1, "PHYS_Falling": 2, "PHYS_Spider": 9, "PHYS_Ladder": 11}),
    ("Actor.uc", "ENetRole", {"ROLE_Authority": 4}),
    ("PawnAudioTable.uc", "EPawnAudioEvent", {"PAE_JumpGrunt": 24}),
]:
    text = (reports / "sources/System__engine.u" / file).read_text(encoding="utf-8-sig")
    body = re.search(r"enum\s+" + enum + r"\s*\{(.*?)\}", text, re.S).group(1)
    body = re.sub(r"//[^\n]*|/\*.*?\*/", "", body, flags=re.S)
    values = [s.strip() for s in body.split(",") if s.strip()]
    for name, index in names.items():
        assert values.index(name) == index
        enum_values[name] = index
result = {"date": "2026-10-06", "rust_tests": 104, "function_count": len(functions),
          "functions": functions, "operator_indices": expected_indices,
          "setphysics_index": 3970, "engine_package_sha256": hashlib.sha256(data).hexdigest(),
          "core_package_sha256": hashlib.sha256(core_data).hexdigest(),
          "source_enum_values": enum_values,
          "scope": "reviewed bytecode AST/control flow matches stored jump source; no VM/runtime/native-call parity",
          "android": "deferred until end per user"}
(reports / "jump-bytecode-validation.json").write_text(json.dumps(result, indent=2) + "\n", encoding="utf-8")
print(json.dumps({"functions": len(functions), "operators": len(expected_indices), "rust_tests": 104}))
