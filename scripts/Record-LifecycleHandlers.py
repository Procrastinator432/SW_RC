"""Record complete selected-handler AST inventory and exact PendingMatch adapter."""
from pathlib import Path
from collections import Counter
import hashlib
import json

root = Path(__file__).resolve().parents[1]
report_dir = root / "analysis/reports"
original_path = report_dir / "state-transitions.json"
original = json.loads(original_path.read_text())
pending_path = report_dir / "pending-match-begin.json"
pending = json.loads(pending_path.read_text())
requests = Counter(p["callback_request"]["selected_function"]
                   for key in ("auto_transition_probes", "resolved_auto_exit_probes")
                   for p in original[key] if p["callback_request"])
assert len(requests) == 21 and sum(requests.values()) == 451
dump_paths = sorted((report_dir / "lifecycle-functions").glob("*.json"))
assert len(dump_paths) == 6
inventory = {}
strings = []
package_hashes = {}


def flatten(expressions):
    for e in expressions:
        yield e
        yield from flatten(e["children"])


for dump_path in dump_paths:
    dump = json.loads(dump_path.read_text())
    assert dump["errors"] == 0
    package = Path(dump["package"])
    raw = package.read_bytes()
    package_hashes[str(package)] = hashlib.sha256(raw).hexdigest()
    for entry in dump["functions"]:
        qualified = package.stem + "." + entry["path"]
        assert qualified in requests and qualified not in inventory
        function = entry["decoded"]
        assert "error" not in function and function["native_index"] == 0
        assert function["flags"] in (0x20002, 0x20102)
        calls, references = [], set()
        nodes = list(flatten(function["expressions"]))
        for e in nodes:
            assert 0 <= e["logical_offset"] < e["logical_end"] <= function["logical_script_bytes"]
            operand = e["operand"]
            if e["opcode"] == 0x1f:
                assert operand["kind"] == "String" and not e["children"]
                value = operand["value"]
                encoded = bytes(ord(c) for c in value)
                start = entry["serial_offset"] + function["script_offset"] + e["serialized_offset"]
                assert raw[start:start + len(encoded) + 2] == bytes([0x1f]) + encoded + b"\0"
                assert e["logical_end"] - e["logical_offset"] == len(encoded) + 2
                strings.append({"handler": qualified, "logical_offset": e["logical_offset"], "value": value})
            if e["opcode"] in (0x1b, 0x1c) or e["opcode"] >= 0x60:
                calls.append({"logical_offset": e["logical_offset"], "operand": operand})
            if operand["kind"] == "Object":
                references.add(operand["value"]["path"])
        inventory[qualified] = {
            "selected_requests": requests[qualified], "logical_bytes": function["logical_script_bytes"],
            "serialized_bytes": function["serialized_script_bytes"], "ast_nodes": len(nodes),
            "calls": calls, "object_references": sorted(references),
            "adapter": "ReviewedScalar" if qualified == pending["handler"] else "UnresolvedExecution",
        }
assert set(inventory) == set(requests)
assert strings == [
    {"handler": "engine.Prop.Invulnerable.BeginState", "logical_offset": 56, "value": ""},
    {"handler": "engine.Prop.Invulnerable.BeginState", "logical_offset": 62, "value": " Is Invulnerable"},
]
core = json.loads((report_dir / "lifecycle-core-dependencies.json").read_text())
assert core["errors"] == 0
core_indices = {e["path"]: e["decoded"]["native_index"] for e in core["functions"]}
assert core_indices == {"Object.Greater_IntInt": 151, "Object.GotoState": 113}
engine = json.loads((report_dir / "lifecycle-functions/engine.json").read_text())
prop = next(e["decoded"] for e in engine["functions"] if e["path"] == "Prop.Invulnerable.BeginState")
assert prop["expressions"][0]["children"][0]["operand"] == {"kind": "Native", "value": 151}
assert prop["expressions"][1]["operand"] == {"kind": "Native", "value": 113}
assert prop["expressions"][1]["children"][0]["operand"] == {"kind": "Name", "value": "Damagable"}

mp_dump = json.loads((report_dir / "lifecycle-functions/mpgame.json").read_text())
function = next(e["decoded"] for e in mp_dump["functions"] if e["path"] == "DMGame.PendingMatch.BeginState")
assert pending["function"] == function
fixture_path = root / "crates/rc-package/tests/fixtures/pending_match_begin.json"
assert json.loads(fixture_path.read_text()) == function
assert pending["waiting_field"] == {"name": "bWaitingToStartMatch", "kind": "Core.BoolProperty",
                                    "dimension": 1, "flags": 0, "struct_name": None}
assert pending["startup_field"] == {"name": "StartupStage", "kind": "Core.ByteProperty",
                                    "dimension": 1, "flags": 0, "struct_name": None}
original_entries = {p["class"]: p for p in original["auto_transition_probes"]
                    if p["callback_request"] and p["callback_request"]["selected_function"] == pending["handler"]}
assert len(pending["probes"]) == len(original_entries) == 5
assert {p["class"] for p in pending["probes"]} == set(original_entries)
for probe in pending["probes"]:
    base = original_entries[probe["class"]]
    assert len(probe["cases"]) == 4
    selected = original["owners"][base["execution"]["probe"]["state_node"]]
    assert probe["selected_state"] == selected["path"]
    if selected["path"] == "mpgame.TDGame.PendingMatch":
        assert selected["parent"].lower() == "mpgame.dmgame.pendingmatch"
    for case in probe["cases"]:
        assert case["after"] == {"waiting_to_start_match": True, "startup_stage": 0}
        assert case["callback_calls"] == 1 and case["outcome"] == "Success"
        assert case["execution"] == {**base["execution"], "object_flags": 4096}

native = (root / "analysis/decompiled/lifecycle-scalars.c").read_text()
serializer = (root / "analysis/decompiled/class-serializers.c").read_text()
assert "case 0x1f:" in serializer and "while (*(char *)(iVar2 + -1) != '\\0')" in serializer
for marker in ("1012fa40 UObject::execByteConst", "1012f5a0 UObject::execLetBool",
               "10137e60 UObject::execLet", "1013ce00 UObject::execStringConst",
               "1012fa80 UObject::execTrue", "*(undefined4 *)param_2 = 1",
               "appFromAnsi", "1 << ((byte)pUVar2[0x40] & 0x1f)"):
    assert marker in native, marker
sources = ["crates/rc-package/src/script.rs", "crates/rc-package/src/pending_match.rs",
           "crates/rc-package/Cargo.toml", "crates/rc-inspect/src/bin/rc-pending-match-probe.rs",
           "analysis/decompiled/lifecycle-scalars.c", "analysis/decompiled/class-serializers.c",
           "scripts/Dump-LifecycleFunctions.py", "scripts/Record-LifecycleHandlers.py",
           "crates/rc-package/tests/fixtures/pending_match_begin.json"]
hash_file = lambda p: hashlib.sha256(p.read_bytes()).hexdigest()
result = {
    "date": "2026-10-07", "rust_tests": 187,
    "selected_distinct_handlers": len(inventory), "selected_requests": sum(requests.values()),
    "decoded_ast_nodes": sum(v["ast_nodes"] for v in inventory.values()),
    "string_constants": strings, "handlers": inventory,
    "pending_match_classes": len(pending["probes"]), "completed_scalar_cases": 20,
    "remaining_distinct_handler_implementations": 20,
    "input_transition_report_sha256": hash_file(original_path),
    "pending_report_sha256": hash_file(pending_path),
    "dump_sha256": {str(p.relative_to(root)): hash_file(p) for p in dump_paths},
    "package_sha256": package_hashes,
    "source_sha256": {s: hash_file(root / s) for s in sources},
    "checks": ["cargo test --workspace", "cargo clippy --workspace --all-targets -- -D warnings",
               "cargo fmt --all -- --check", "21 original selected function ASTs decoded",
               "string bytes checked against original payloads", "property kinds/non-replication and original AST verified",
               "20 callback/transition probes checked independently", "existing transition report retained"],
    "scope": pending["scope"] + " Complete bounded AST inspection is not execution of the other 20 handlers. Prop.Invulnerable positive-health redirection identified; native GotoState and dependency execution remain external."
}
(report_dir / "lifecycle-handlers-validation.json").write_text(json.dumps(result, indent=2) + "\n", encoding="utf-8")
path = root / "analysis/evidence.json"
evidence = json.loads(path.read_text(encoding="utf-8-sig"))
evidence["rust_tests"] = 187
evidence["lifecycle_handlers_validation"] = result
path.write_text(json.dumps(evidence, indent=2, ensure_ascii=False) + "\n", encoding="utf-8")
note = (
    "2026-10-07: Originale Zustandsereignisse inventarisiert und PendingMatch ergänzt. Alle 21 in "
    "451 BeginState-/EndState-Anforderungen ausgewählten Funktionen vollständig als begrenzte AST gelesen. "
    "ANSI-StringConst 0x1f unterstützt; Rohbytes und logische Grenzen geprüft, Unicode bleibt offen. "
    "Prop.Invulnerable.BeginState leitet bei Health>0 nach Damagable um (Core native151/113 geprüft); "
    "diese und andere abhängige Ereignisse werden weiterhin nicht ausgeführt. Exakt verifizierter "
    "DMGame.PendingMatch.BeginState-Adapter setzt bWaitingToStartMatch=true und StartupStage=0 auf "
    "Hostfeldern. Originale Bool-/Byte-Propertytypen und fehlendes Replikationsflag geprüft. Fünf "
    "Klassen mit DMGame- oder TDGame-State, geerbter DMGame-Handler; 20 Übergangsfälle abgeschlossen. "
    "187 Workspace-Tests, Clippy und Format bestanden. Keine allgemeine VM/ProcessEvent-, Startlabel-, "
    "Native-Override- oder Netzwerkparität; übrige 20 Handlerimplementierungen offen. Android zum Schluss. "
    "Details D:\\Rust Projects\\RepublicCommandoAndroid\\docs\\LIFECYCLE_HANDLERS.md."
)
path = root / "README.md"
text = path.read_text(encoding="utf-8-sig")
if "Originale Zustandsereignisse inventarisiert" not in text:
    path.write_text(text + "\n" + note + " [Nachweis](docs/LIFECYCLE_HANDLERS.md).\n", encoding="utf-8")
wiki = Path(r"C:\Users\brand\Documents\Main\05 KI\Project_Knowledge\projects\republic_commando_android")
for relative in ("index.md", "architecture/porting.md", "log.md"):
    path = wiki / relative
    if "Originale Zustandsereignisse inventarisiert" not in path.read_text(encoding="utf-8-sig"):
        with path.open("a", encoding="utf-8") as output:
            output.write("\n\n" + note + "\n")
print(json.dumps({k: result[k] for k in ("selected_distinct_handlers", "decoded_ast_nodes", "pending_match_classes", "completed_scalar_cases", "rust_tests")}))
