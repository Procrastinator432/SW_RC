"""Validate and record original ordered field lists and diagnostic linked-table searches."""
from pathlib import Path
import hashlib
import json

root = Path(__file__).resolve().parents[1]
path = root / "analysis/reports/original-state-link.json"
report = json.loads(path.read_text())
assert report["classes"] == 3170 and report["states"] == 260
assert report["ordered_fields"] == 15915
assert len(report["owners"]) == report["classes"] + report["states"]
assert sum(len(o["ordered_children"]["children"]) for o in report["owners"]) == report["ordered_fields"]
assert sum(c["is_struct"] for o in report["owners"] for c in o["ordered_children"]["children"]) == report["direct_structs"]
assert len(report["controller_class_lookups"]) == 51
for lookup in report["controller_class_lookups"]:
    assert lookup["selected"].lower() == "engine.controller.notifyhitwall"
    assert lookup["result"] == {"handled": False, "route": "EmptyBase"}
for owner in report["owners"]:
    children = owner["ordered_children"]["children"]
    assert len({c["export_index"] for c in children}) == len(children)
sources = ["crates/rc-package/src/state_children.rs", "crates/rc-inspect/src/bin/rc-state-link-probe.rs", "crates/rc-package/src/state_link.rs"]
package_sources = sorted({o["package"] for o in report["owners"]})
result = {
    "date": "2026-10-07", "rust_tests": 160,
    "counts": {k: report[k] for k in ["packages", "classes", "states", "ordered_fields", "direct_structs", "allocated_tables", "diagnostic_global_names"]},
    "controller_class_lookups": len(report["controller_class_lookups"]),
    "report_sha256": hashlib.sha256(path.read_bytes()).hexdigest(),
    "source_sha256": {s: hashlib.sha256((root/s).read_bytes()).hexdigest() for s in sources},
    "original_package_sha256": {s: hashlib.sha256(Path(s).read_bytes()).hexdigest() for s in package_sources},
    "checks": ["cargo test --workspace", "cargo clippy --workspace --all-targets -- -D warnings", "cargo fmt --all -- --check", "rc-state-link-probe on original System/Properties packages"],
    "scope": report["scope"],
}
(root / "analysis/reports/original-state-link-validation.json").write_text(json.dumps(result, indent=2) + "\n", encoding="utf-8")
path = root / "analysis/evidence.json"
evidence = json.loads(path.read_text(encoding="utf-8-sig"))
evidence["rust_tests"] = 160
evidence["original_state_link_validation"] = result
path.write_text(json.dumps(evidence, indent=2, ensure_ascii=False) + "\n", encoding="utf-8")
note = "2026-10-07: Original-Kinderlisten an Ereignistabellen angeschlossen. Neuer begrenzter state_children-Leser liest SuperField, ScriptText, geordnete nullterminierte Children und FriendlyName aus Klassen-/State-Prefix; State-UObject-Properties müssen leer sein. Lokale Referenzen, Outer, Duplikate, FriendlyName und bekannte Core-Feldtypen geprüft, restlicher Script-/Statepayload hier nicht interpretiert. Originalprobe System/Properties: 3170 Klassen, 260 States, 15915 geordnete Felder, keine fehlenden eigenen UStruct-Exporte. Daraus Erstaufbau mit deterministischen globalen Diagnose-Namenskennungen statt nativer FName-Werte; 51 Controller-Klassen finden ohne aktiven State/Maske das aus Originalbytecode verifizierte Engine.Controller.NotifyHitWall-Basisereignis und false. Native Namensregistrierung, Runtime-Overrides/aktiver State und VM-Ausführung offen; keine neue Karten- oder Androidabdeckung. 160 Workspace-Tests, Clippy und Format bestanden. Nachweise analysis/reports/original-state-link.json/original-state-link-validation.json, Details D:\\Rust Projects\\RepublicCommandoAndroid\\docs\\ORIGINAL_STATE_LINK.md."
path = root / "README.md"
text = path.read_text(encoding="utf-8-sig")
if "Original-Kinderlisten an Ereignistabellen angeschlossen" not in text:
    text += "\n" + note + " [Nachweis](docs/ORIGINAL_STATE_LINK.md).\n"
path.write_text(text, encoding="utf-8")
path = root / "docs/STATE_LINK.md"
text = path.read_text(encoding="utf-8-sig")
if "## Folgearbeit: Original-Kinderlisten" not in text:
    text += "\n## Folgearbeit: Original-Kinderlisten\n\nDie geordneten Kinderlisten sind jetzt direkt aus Originalklassen und -zuständen angeschlossen. Die Diagnose baut Tabellen mit eigenen global konsistenten Namenskennungen; native FName-Werte und aktive Zustandswahl bleiben offen. 3170 Klassen, 260 Zustände, 15915 Felder und 51 Controller-Klassensuchen geprüft. 160 Tests bestanden. Siehe [ORIGINAL_STATE_LINK.md](ORIGINAL_STATE_LINK.md).\n"
path.write_text(text, encoding="utf-8")
wiki = Path(r"C:\Users\brand\Documents\Main\05 KI\Project_Knowledge\projects\republic_commando_android")
for relative in ("index.md", "architecture/porting.md", "log.md"):
    path = wiki / relative
    if "Original-Kinderlisten an Ereignistabellen angeschlossen" not in path.read_text(encoding="utf-8-sig"):
        with path.open("a", encoding="utf-8") as output:
            output.write("\n\n" + note + "\n")
print(json.dumps({**result["counts"], "controller_class_lookups": 51, "rust_tests": 160}))
