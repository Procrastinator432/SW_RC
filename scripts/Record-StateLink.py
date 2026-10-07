"""Record the reviewed first-link hash builder and its explicit input boundaries."""
from pathlib import Path
import hashlib
import json

root = Path(__file__).resolve().parents[1]
native = (root / "analysis/decompiled/state-link.c").read_text()
iterator = (root / "analysis/decompiled/state-struct-iterator.c").read_text()
assert "10127a30 UState::Link" in native
assert "UStruct::Link((UStruct *)this,param_1,param_2)" in native
assert "iVar7 + 0x18" in native and "this + 0x90" in native
assert "0x200" in native and "0x80" in native and "uVar1 & 0x7f" in native
assert "iVar7 + 0x2c" in native and "iVar7 + 0x28" in native
assert "param_1 + 0x3c" in iterator and "&UStruct::PrivateStaticClass" in iterator
assert "+ 0x28" in iterator
sources = ["crates/rc-package/src/state_link.rs", "analysis/decompiled/state-link.c", "analysis/decompiled/state-struct-iterator.c"]
result = {
    "date": "2026-10-07", "rust_tests": 157,
    "native_link": "UState.Link10127a30", "struct_iterator": "10121750",
    "allocation": "Only on first direct UStruct child; 128 zeroed pointer buckets, 0x200 bytes in PE32",
    "inheritance": "Copy closest ancestor with non-null hash table; childless states retain null and FindStruct walks parent",
    "insertion": "In iterator order: field.HashNext = bucket head; bucket head = field; bucket = resolvedIndex & 127",
    "checks": ["cargo test --workspace", "cargo clippy --workspace --all-targets -- -D warnings", "cargo fmt --all -- --check"],
    "tested": ["child-first input builds parents first", "tableless intermediate ancestor", "inherited function override", "same-bucket distinct names", "insertion order", "unchanged parent and sibling hash chains", "nonfunction shadow", "missing parents/children", "duplicate ownership", "inconsistent global FName identity", "ancestry cycles", "empty graph"],
    "source_sha256": {s: hashlib.sha256((root/s).read_bytes()).hexdigest() for s in sources},
    "scope": "Fresh first-link tables only, using supplied ordered and filtered direct struct children and canonical global FName snapshots. No original package child-list extraction, active runtime state or native relinking. No new map coverage or Android execution.",
}
(root / "analysis/reports/state-link-validation.json").write_text(json.dumps(result, indent=2) + "\n", encoding="utf-8")
path = root / "analysis/evidence.json"
evidence = json.loads(path.read_text(encoding="utf-8-sig"))
evidence["rust_tests"] = 157
evidence["state_link_validation"] = result
path.write_text(json.dumps(evidence, indent=2, ensure_ascii=False) + "\n", encoding="utf-8")
note = "2026-10-07: Erstaufbau der Ereignis-Hashtabellen ergänzt. UState.Link10127a30/Struct-Iterator10121750 aus core.dll geprüft: Kinderliste+3c, Next+28, UStruct-Klassenfilter, Outer+18 begrenzt eigene Kinder. Beim ersten eigenen Struct 128 Buckets/0x200 Bytes anlegen, nächste vorhandene Vorfahrentabelle kopieren, eigene Kinder in Iteratorreihenfolge vorn einfügen (HashNext+2c, resolvedIndex&127). Kinderlose States behalten null. Rust state_link.link_states baut Eltern zuerst und liefert EventLookupSnapshot; geerbte Ketten werden zwischen Geschwistern nicht verändert. Unvollständige Eltern/Kinder, doppelte Ownership, widersprüchliche globale FName-Identität und Zyklen liefern Fehler ohne Ergebnisgraph. 157 Workspace-Tests, Clippy und Format bestanden. Geordnete/Struct-gefilterte Kinder und globale Namen noch explizite Eingabe; Extraktion aus Originalpaketen, aktiver State und Relinking offen. Keine zusätzliche Kartenabdeckung; Androidprüfung weiterhin zum Schluss. Details D:\\Rust Projects\\RepublicCommandoAndroid\\docs\\STATE_LINK.md."
path = root / "README.md"
text = path.read_text(encoding="utf-8-sig")
if "Erstaufbau der Ereignis-Hashtabellen ergänzt" not in text:
    text += "\n" + note + " [Nachweis](docs/STATE_LINK.md).\n"
path.write_text(text, encoding="utf-8")
path = root / "docs/EVENT_LOOKUP.md"
text = path.read_text(encoding="utf-8-sig")
if "## Folgearbeit: Tabellenaufbau" not in text:
    text += "\n## Folgearbeit: Tabellenaufbau\n\nDer Erstaufbau der Hashtabellen ist jetzt für explizite geordnete Kinderlisten und globale Namensdaten rekonstruiert. Der neue Builder erzeugt direkt EventLookupSnapshot. Originalpaket-Kinderextraktion und Runtime-Statewahl bleiben offen. 157 Tests bestanden. Siehe [STATE_LINK.md](STATE_LINK.md).\n"
path.write_text(text, encoding="utf-8")
wiki = Path(r"C:\Users\brand\Documents\Main\05 KI\Project_Knowledge\projects\republic_commando_android")
for relative in ("index.md", "architecture/porting.md", "log.md"):
    path = wiki / relative
    if "Erstaufbau der Ereignis-Hashtabellen ergänzt" not in path.read_text(encoding="utf-8-sig"):
        with path.open("a", encoding="utf-8") as output:
            output.write("\n\n" + note + "\n")
print("First-link hash builder recorded; 157 tests, evidence and wiki updated.")
