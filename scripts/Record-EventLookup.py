"""Record reviewed native lookup and inventory NotifyHitWall declarations in package reports."""
from pathlib import Path
import hashlib
import json

root = Path(__file__).resolve().parents[1]
lookup = (root / "analysis/decompiled/notify-find-struct.c").read_text()
event = (root / "analysis/decompiled/notify-event-resolution.c").read_text()
assert "10123c40 UState::FindStruct" in lookup
assert "this + 0x90" in lookup and "this + 0x30" in lookup
assert "*puVar2 & 0x7f" in lookup and "pUVar1 + 0x2c" in lookup
assert "pUVar1 + 0x20" in lookup
assert "101548c0 UObject::FindFunction" in event
assert "param_3 == 0" in event and "+ 0x18" in event
assert "UState::FindStruct(*(UState **)(this + 0x24),param_2)" in event
assert "0x80000" in event and "FindFunctionChecked(this,param_2,0)" in event
assert "1012f350 UObject::execNothing" in lookup

reports = sorted((root / "analysis/reports/packages").glob("*.json"))
assert len(reports) == 273
declarations = []
counts = {"exports": 0, "functions": 0, "states": 0, "classes": 0}
for path in reports:
    report = json.loads(path.read_text(encoding="utf-8-sig"))
    exports = report["exports"]
    assert len(exports) == report["summary"]["export_count"]
    for index, export in enumerate(exports, 1):
        counts["exports"] += 1
        kind = export["class_path"].lower()
        counts["functions"] += kind == "core.function"
        counts["states"] += kind == "core.state"
        counts["classes"] += export["class"] == 0
        if kind == "core.function" and export["name"].lower() == "notifyhitwall":
            package_name = path.stem.split("__")[-1].removesuffix(".u")
            declarations.append({"report": path.name, "export_index": index,
                                 "path": export["object_path"],
                                 "qualified_path": package_name + "." + export["object_path"], "outer_index": export["outer"],
                                 "serial_offset": export["serial_offset"], "serial_size": export["serial_size"]})
assert len(declarations) == 1
assert declarations[0]["qualified_path"].lower() == "engine.controller.notifyhitwall"
sources = ["crates/rc-package/src/event_lookup.rs", "crates/rc-package/src/notify_wall.rs",
           "analysis/decompiled/notify-find-struct.c", "analysis/decompiled/notify-event-resolution.c"]
result = {
    "date": "2026-10-06", "rust_tests": 154,
    "native_functions": {"FindStruct": "10123c40", "FindFunction": "101548c0",
                         "FindFunctionChecked": "101571d0", "ProcessEventName": "10104e90",
                         "ProcessEventFunction": "1013e020", "execNothing": "1012f350"},
    "serialized_inventory": {"package_reports": len(reports), **counts, "notify_hit_wall_declarations": declarations},
    "checks": ["cargo test --workspace", "cargo clippy --workspace --all-targets -- -D warnings", "cargo fmt --all -- --check"],
    "source_sha256": {s: hashlib.sha256((root/s).read_bytes()).hexdigest() for s in sources},
    "scope": "Native FindFunction search order implemented on supplied runtime hash-table snapshots. Package inventory found one serialized NotifyHitWall declaration, but does not prove absence of native/runtime overrides. Hash construction, active state selection, GIsScriptable/ProcessEvent execution and native virtual overrides remain open. Existing map diagnostics unchanged; no Android execution.",
}
(root / "analysis/reports/event-lookup-validation.json").write_text(json.dumps(result, indent=2) + "\n", encoding="utf-8")
path = root / "analysis/evidence.json"
evidence = json.loads(path.read_text(encoding="utf-8-sig"))
evidence["rust_tests"] = 154
evidence["event_lookup_validation"] = result
path.write_text(json.dumps(evidence, indent=2, ensure_ascii=False) + "\n", encoding="utf-8")
note = f"2026-10-06: Native Ereignis-Handlersuche ergänzt. FindFunction101548c0 sucht zuerst aktiven StateFrame.StateNode+18, danach Object.Class+24; global_only überspringt State. FindStruct10123c40 geht Parent+30 nur bis zur ersten vorhandenen Hashtabelle+90, verwendet Bucket resolvedIndex&127, HashNext+2c und FName-Handleidentität; fehlender Bucketname löst keinen weiteren Parentwalk aus. Nichtfunktionaler State-Treffer unterdrückt Klassenfallback; Klassentypflag80000 geprüft. Rust event_lookup arbeitet mit expliziten Runtime-Tabellensnapshots, MaskedOut überspringt Suche; unbekannte aktivierte Handler, fehlende Snapshots und Zyklen liefern Fehler. Inventar aller 273 Paketberichte: {counts['functions']} Function- und {counts['states']} State-Exporte, genau ein serialisiertes NotifyHitWall (Engine.Controller); keine Aussage über native/Runtime-Overrides. Native execNothing1012f350 ist leer. 154 Workspace-Tests, Clippy und Format bestanden. Tabellenaufbau, aktive Zustandswahl und VM-/ProcessEvent-Ausführung offen; Kartendiagnosen unverändert, Android zum Schluss. Details D:\\Rust Projects\\RepublicCommandoAndroid\\docs\\EVENT_LOOKUP.md."
path = root / "README.md"
text = path.read_text(encoding="utf-8-sig")
if "Native Ereignis-Handlersuche ergänzt" not in text:
    text += "\n" + note + " [Nachweis](docs/EVENT_LOOKUP.md).\n"
path.write_text(text, encoding="utf-8")
wiki = Path(r"C:\Users\brand\Documents\Main\05 KI\Project_Knowledge\projects\republic_commando_android")
for relative in ("index.md", "architecture/porting.md", "log.md"):
    path = wiki / relative
    if "Native Ereignis-Handlersuche ergänzt" not in path.read_text(encoding="utf-8-sig"):
        with path.open("a", encoding="utf-8") as output:
            output.write("\n\n" + note + "\n")
print(json.dumps(result["serialized_inventory"], ensure_ascii=False))
