"""Verify exported wrapper instructions and record the bounded NotifyHitWall milestone."""
from pathlib import Path
import hashlib
import json

root = Path(__file__).resolve().parents[1]
asm = (root / "analysis/decompiled/controller-notify-wall.asm").read_text()
required = [
    "103604fd MOV EDI,dword ptr [EAX + 0x584]",
    "1036050d CMP dword ptr [ECX],0x12c",
    "1036051d CMP dword ptr [ECX],0x16c",
    "10360525 MOV ESI,dword ptr [EBX + 0xc]",
    "10360538 SUB ECX,0x12c",
    "1036053e MOV EAX,0x1",
    "10360543 XOR EDX,EDX",
    "10360545 CALL 0x10621f70",
    "1036054a AND EAX,dword ptr [ESI + 0x1c]",
    "1036054d AND EDX,dword ptr [ESI + 0x20]",
    "10360556 XOR EAX,EAX",
    "10360589 MOV dword ptr [ESP + 0x28],0x0",
    "10360591 CALL dword ptr [0x10650284]",
]
assert all(line in asm for line in required)
bytecode = json.loads((root / "analysis/reports/controller-wall-bytecode.json").read_text())
assert bytecode["errors"] == 0
function = bytecode["functions"][0]
assert function["path"] == "Controller.NotifyHitWall"
assert function["decoded"]["flags"] == 0x20800
assert function["decoded"]["expressions"][0]["opcode"] == 4
assert function["decoded"]["expressions"][0]["children"][0]["opcode"] == 11
validation = json.loads((root / "analysis/reports/notify-wall-base-validation.json").read_text())
assert validation["result"] == {"handled": False, "route": "EmptyBase"}
sources = ["analysis/decompiled/controller-notify-wall.asm", "analysis/reports/controller-wall-bytecode.json", "crates/rc-package/src/notify_wall.rs", "crates/rc-inspect/src/bin/rc-notify-wall-probe.rs"]
result = {
    "date": "2026-10-06", "rust_tests": 151,
    "native_wrapper": "AController.NotifyHitWall103604f0",
    "native_name_table_slot": "0x161 (offset 0x584); distinct from resolved FName index",
    "guard": "With state frame and resolved index in [300,364), test 1u64 << (index-300) against words +0x1c/+0x20; absent bit returns zero without ProcessEvent",
    "base_event": validation,
    "checks": ["cargo test --workspace", "cargo clippy --workspace --all-targets -- -D warnings", "cargo fmt --all -- --check", "rc-notify-wall-probe on original engine.u"],
    "scope": "No general VM or runtime handler resolution. Empty base accepted only after exact AST recognition. Enabled unresolved handlers error. Existing map diagnostics still use supplied false; no new map or Android execution.",
    "source_sha256": {s: hashlib.sha256((root/s).read_bytes()).hexdigest() for s in sources},
}
(root / "analysis/reports/notify-wall-validation.json").write_text(json.dumps(result, indent=2) + "\n", encoding="utf-8")
path = root / "analysis/evidence.json"
evidence = json.loads(path.read_text(encoding="utf-8-sig"))
evidence["rust_tests"] = 151
evidence["notify_wall_validation"] = result
path.write_text(json.dumps(evidence, indent=2, ensure_ascii=False) + "\n", encoding="utf-8")
note = "2026-10-06: NotifyHitWall-Ereignisadapter ergänzt. Native Maske aus Assembler 103604f0 bestätigt: StateFrame vorhanden und aufgelöster Namensindex 300..363, Bit index-300 in 64-Bit-Maske +1c/+20; deaktiviert liefert false ohne ProcessEvent. Names-Tabellenslot 0x161 ist kein belegter Runtime-Namensindex. Original Engine.Controller.NotifyHitWall als exakt leerer Return/Nothing-AST geprüft; eigener enger Adapter liefert initialisiertes false, kein allgemeiner VM-Interpreter. Aktivierte unbekannte Handler liefern Fehler; überschriebene Klassen-/State-Handler bleiben offen. Kartendiagnosen behalten explizite false-Snapshots bis Runtimeauflösung, kein neuer Kartentest. 151 Tests, Clippy und Format bestanden; Original-engine.u-Probe bestanden. Androidprüfung weiterhin zum Schluss. Details: D:\\Rust Projects\\RepublicCommandoAndroid\\docs\\NOTIFY_WALL.md."
path = root / "README.md"
text = path.read_text(encoding="utf-8-sig")
if "NotifyHitWall-Ereignisadapter ergänzt" not in text:
    text += "\n" + note + " [Nachweis](docs/NOTIFY_WALL.md).\n"
path.write_text(text, encoding="utf-8")
wiki = Path(r"C:\Users\brand\Documents\Main\05 KI\Project_Knowledge\projects\republic_commando_android")
for relative in ("index.md", "architecture/porting.md", "log.md"):
    path = wiki / relative
    if "NotifyHitWall-Ereignisadapter ergänzt" not in path.read_text(encoding="utf-8-sig"):
        with path.open("a", encoding="utf-8") as output:
            output.write("\n\n" + note + "\n")
print("NotifyHitWall native guard and original empty base verified; evidence/wiki updated, 151 tests.")
