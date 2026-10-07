"""Independently rebuild class probe masks from audited function flags and ancestry."""
from pathlib import Path
import hashlib
import json

root=Path(__file__).resolve().parents[1]
path=root/"analysis/reports/original-state-masks.json"
report=json.loads(path.read_text())
old=json.loads((root/"analysis/reports/initialized-probe-frames.json").read_text())
def normalize(value):
    copy=json.loads(json.dumps(value))
    for key in ("scope","original_mask_compositions","controller_declared_state_probes"):
        copy.pop(key,None)
    for owner in copy["owners"]:
        for key in ("serialized_masks","own_defined_probe_bits","probe_function_flags"):
            owner.pop(key,None)
    return copy
assert normalize(report)==normalize(old)
owners={o["path"].lower():o for o in report["owners"]}
compositions={c["owner"].lower():c for c in report["original_mask_compositions"]}
assert len(owners)==len(compositions)==3430
classes={key for key,c in compositions.items() if c["owner"]==c["object_class"]}
assert len(classes)==3170
function_count=0
for owner in owners.values():
    expected=0
    for function in owner["probe_function_flags"]:
        function_count+=1
        index=function["native_name_index"]
        assert 300<=index<364
        if function["flags"]&2:
            expected |= 1<<(index-300)
    assert expected==owner["own_defined_probe_bits"]
cache={}
def class_mask(key,seen):
    if key in cache: return cache[key]
    assert key in classes and key not in seen
    owner=owners[key]
    parent=owner["parent"]
    inherited=class_mask(parent.lower(),seen|{key}) if parent else 0
    cache[key]=inherited|owner["own_defined_probe_bits"]
    return cache[key]
changed=0
for key,c in compositions.items():
    class_probe=class_mask(c["object_class"].lower(),set())
    raw=owners[key]["serialized_masks"]
    state_probe=class_probe if key in classes else raw["probe_mask"]
    assert c["runtime_class_probe"]==class_probe
    assert c["runtime_state_probe"]==state_probe
    assert c["ignore_mask"]==raw["ignore_mask"]
    assert c["composed_mask"]==(class_probe|state_probe)&raw["ignore_mask"]
    changed+=key in classes and raw["probe_mask"]!=class_probe
assert changed==28
probes=report["controller_declared_state_probes"]
assert len(probes)==31
for probe in probes:
    assert probe["frame"]["probe_mask"]==compositions[probe["state"].lower()]["composed_mask"]
    assert probe["notify"]=={"handled":False,"route":"MaskedOut"}
native=(root/"analysis/decompiled/class-serializers.c").read_text()
assert "*(byte *)(piVar5 + 0x1e) & 2" in native
assert "*(undefined4 *)(this + 0x78) = uVar11" in native
sources=["crates/rc-package/src/state_masks.rs","crates/rc-inspect/src/bin/rc-state-link-probe.rs","analysis/decompiled/class-serializers.c","analysis/decompiled/active-state.c"]
result={"date":"2026-10-07","rust_tests":167,"original_classes":3170,"original_states":260,
        "audited_probe_function_layouts":function_count,"mask_compositions":3430,
        "class_masks_changed_from_serialized":changed,"declared_controller_states_masking_notify":31,
        "report_sha256":hashlib.sha256(path.read_bytes()).hexdigest(),
        "source_sha256":{s:hashlib.sha256((root/s).read_bytes()).hexdigest() for s in sources},
        "checks":["cargo test --workspace","cargo clippy --workspace --all-targets -- -D warnings","cargo fmt --all -- --check","rc-state-link-probe --native-hardcoded-names --original-masks","independent class mask recomposition from function flags/parent graph","prior original lists/class lookups/initial-frame probes unchanged"],
        "scope":report["scope"]}
(root/"analysis/reports/original-state-masks-validation.json").write_text(json.dumps(result,indent=2)+"\n",encoding="utf-8")
path=root/"analysis/evidence.json"
evidence=json.loads(path.read_text(encoding="utf-8-sig"))
evidence["rust_tests"]=167
evidence["original_state_masks_validation"]=result
path.write_text(json.dumps(evidence,indent=2,ensure_ascii=False)+"\n",encoding="utf-8")
note=f"2026-10-07: Original-Zustandsmasken angeschlossen. Neuer begrenzter state_masks-Layoutleser traversiert serialisierten Scriptcode einschließlich kompakter Referenzen/Labeltabellen, liest ProbeMask/IgnoreMask/LabelOffset/StateFlags und relevante Functionflags samt optionalem Net-Replicationoffset. Keine AST-/VM-Auswertung; unbekannte Layouttokens explizit Fehler. Alle3170Klassen/260States erfolgreich; {function_count} Probe-Funktionslayouts geprüft. UClass.Serialize10126200 baut Class.ProbeMask aus geerbter Maske und eigenen Funktionen mit Flags&2 neu auf;28Klassenmasken unterscheiden sich vom gespeicherten Wert.3430Maskenkompositionen separat aus Eltern/Flags geprüft.31explizit ausgewählte deklarierteController-States unterdrückenNotifyHitWall perOriginalmaske, statt gelieferterMaskenvorgabe. Auswahl erfolgt nach deklarierterOwnerklasse/State, keine Aussage über tatsächliche Runtime-/Auto-Statewahl oder vollständigeGotoStateCallbacks. BisherigeOriginal-Kinderlisten/Klassensuchen/Initialframe-Proben unverändert.167Workspace-Tests/Clippy/Formatbestanden. Native Overrides/VM/dynamischeNamenoffen; AndroidzumSchluss. Details D:\\Rust Projects\\RepublicCommandoAndroid\\docs\\ORIGINAL_STATE_MASKS.md."
path=root/"README.md"
text=path.read_text(encoding="utf-8-sig")
if "Original-Zustandsmasken angeschlossen" not in text:
    text+="\n"+note+" [Nachweis](docs/ORIGINAL_STATE_MASKS.md).\n"
path.write_text(text,encoding="utf-8")
path=root/"docs/PROBE_FRAME.md"
text=path.read_text(encoding="utf-8-sig")
if "## Folgearbeit: Originalmasken" not in text:
    text+="\n## Folgearbeit: Originalmasken\n\nOriginale Zustands-/Ignoremasken und beim Laden neu berechnete Klassenmasken sind jetzt angeschlossen. 3430 Kompositionen und31 ausdrücklich ausgewählte deklarierteController-States geprüft; diese31 Masken unterdrücken NotifyHitWall. Tatsächliche Zustandswahl/Wechselcallbacks bleiben offen.167Tests bestanden. Siehe [ORIGINAL_STATE_MASKS.md](ORIGINAL_STATE_MASKS.md).\n"
path.write_text(text,encoding="utf-8")
wiki=Path(r"C:\Users\brand\Documents\Main\05 KI\Project_Knowledge\projects\republic_commando_android")
for relative in ("index.md","architecture/porting.md","log.md"):
    path=wiki/relative
    if "Original-Zustandsmasken angeschlossen" not in path.read_text(encoding="utf-8-sig"):
        with path.open("a",encoding="utf-8") as output:
            output.write("\n\n"+note+"\n")
print(json.dumps({k:result[k] for k in ("original_classes","original_states","audited_probe_function_layouts","mask_compositions","class_masks_changed_from_serialized","declared_controller_states_masking_notify","rust_tests")}))
