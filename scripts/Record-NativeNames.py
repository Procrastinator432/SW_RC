"""Compare fixed-name original package lookup with the historical diagnostic-name report."""
from pathlib import Path
import hashlib
import json

root=Path(__file__).resolve().parents[1]
old=json.loads((root/"analysis/reports/original-state-link.json").read_text())
new_path=root/"analysis/reports/native-named-state-link.json"
new=json.loads(new_path.read_text())
proof=json.loads((root/"analysis/reports/hardcoded-names-proof.json").read_text())
assert proof["count"]==384 and proof["maximum_fixed_index"]==936
bindings=new["name_bindings"]
assert bindings["notify_index"]==353 and bindings["notify_bucket"]==97 and bindings["notify_mask_bit"]==53
assert bindings["base_with_supplied_enabled_mask"]=={"handled":False,"route":"EmptyBase"}
assert bindings["base_with_supplied_disabled_mask"]=={"handled":False,"route":"MaskedOut"}
assert bindings["fixed_native_indices"]+bindings["diagnostic_dynamic_indices"]==3405
assert len(new["controller_class_lookups"])==51
def normalize(report):
    return {key:value for key,value in report.items() if key not in ("scope","name_bindings")}
assert normalize(old)==normalize(new)
sources=["crates/rc-package/src/name_bindings.rs","crates/rc-package/src/hardcoded_names.rs","crates/rc-inspect/src/bin/rc-state-link-probe.rs","analysis/decompiled/name-registration.c","analysis/decompiled/name-constructor.c","analysis/decompiled/name-static-init.asm"]
result={"date":"2026-10-07","rust_tests":162,"fixed_name_count":384,"name_bindings":bindings,
        "unchanged_controller_class_lookups":51,"unchanged_original_owners":3430,
        "native_proof":"analysis/reports/hardcoded-names-proof.json",
        "report_sha256":hashlib.sha256(new_path.read_bytes()).hexdigest(),
        "source_sha256":{s:hashlib.sha256((root/s).read_bytes()).hexdigest() for s in sources},
        "checks":["Verify-HardcodedNames.py: assembly operands, PE opcode immediates/call targets, PE strings, decompiled registration calls, generated Rust table","cargo test --workspace","cargo clippy --workspace --all-targets -- -D warnings","cargo fmt --all -- --check","rc-state-link-probe --native-hardcoded-names on original packages","historical diagnostic lookup report equal except name binding metadata/scope"],
        "scope":new["scope"]}
(root/"analysis/reports/native-names-validation.json").write_text(json.dumps(result,indent=2)+"\n",encoding="utf-8")
path=root/"analysis/evidence.json"
evidence=json.loads(path.read_text(encoding="utf-8-sig"))
evidence["rust_tests"]=162
evidence["native_names_validation"]=result
path.write_text(json.dumps(evidence,indent=2,ensure_ascii=False)+"\n",encoding="utf-8")
note=f"2026-10-07: Native feste FName-Indizes angeschlossen. FName.StaticInit101504f0/Hardcode10150390/AllocateNameEntry1014f880 und Konstruktor1014ff20 geprüft. Alle 384 festen Namen mittels Assembleroperanden, raw-PE-MOV-Immediates/Callzielen/Strings und dekompilierten Registrierungspaaren bestätigt, generierte Rust-Tabelle reproduzierbar. NotifyHitWall fest Index353, Bucket97, Maskenbit53/0x0020000000000000; bisher offener Zusammenhang zwischen Tabellenslot und aufgelöstem Index jetzt für diesen Namen belegt. Originaltabellenprobe bindet {bindings['fixed_native_indices']} verwendete Namen an feste native Indizes; {bindings['diagnostic_dynamic_indices']} übrige Indizes bleiben Diagnosekennungen oberhalb höchstem festem Index936. Handles sind eigene opaque Tokens statt Runtimeadressen. Alle 3430 Ownerlisten und 51 Controller-Klassensuchen unverändert; gelieferte enabled/disabled Masken liefern EmptyBase/MaskedOut. 162 Workspace-Tests, Clippy und Format bestanden. Dynamische Registrierung hängt von Ladefolge/Freelist ab, nicht rekonstruiert; aktiver State, VM und Runtime-Overrides offen. Android weiterhin zum Schluss. Details D:\\Rust Projects\\RepublicCommandoAndroid\\docs\\NATIVE_NAMES.md."
path=root/"README.md"
text=path.read_text(encoding="utf-8-sig")
if "Native feste FName-Indizes angeschlossen" not in text:
    text+="\n"+note+" [Nachweis](docs/NATIVE_NAMES.md).\n"
path.write_text(text,encoding="utf-8")
for relative in ("docs/NOTIFY_WALL.md","docs/ORIGINAL_STATE_LINK.md"):
    path=root/relative
    text=path.read_text(encoding="utf-8-sig")
    if "## Folgearbeit: feste native Namen" not in text:
        text+="\n## Folgearbeit: feste native Namen\n\nDie feste Registrierung aus core.dll belegt jetzt NotifyHitWall als aufgelösten Index353, Bucket97 und Maskenbit53. Die Originalprobe kann alle verwendeten fest registrierten Namen mit nativen Indizes binden; übrige Namen bleiben Diagnosekennungen. Handles sind opaque eigene Identitäten. 162 Tests bestanden; unveränderte 51 Controller-Klassensuchen. Siehe [NATIVE_NAMES.md](NATIVE_NAMES.md).\n"
    path.write_text(text,encoding="utf-8")
wiki=Path(r"C:\Users\brand\Documents\Main\05 KI\Project_Knowledge\projects\republic_commando_android")
for relative in ("index.md","architecture/porting.md","log.md"):
    path=wiki/relative
    if "Native feste FName-Indizes angeschlossen" not in path.read_text(encoding="utf-8-sig"):
        with path.open("a",encoding="utf-8") as output:
            output.write("\n\n"+note+"\n")
print(json.dumps({"fixed_names_verified":384,"original_name_bindings":bindings,"unchanged_controller_lookups":51,"rust_tests":162}))
