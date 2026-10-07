"""Independently resolve named states from ordered original children and parent chains."""
from pathlib import Path
import hashlib
import json

root=Path(__file__).resolve().parents[1]
path=root/"analysis/reports/named-state-selection.json"
report=json.loads(path.read_text())
old=json.loads((root/"analysis/reports/original-state-masks.json").read_text())
assert {k:v for k,v in report.items() if k not in ("scope","named_controller_state_lookups")}=={k:v for k,v in old.items() if k!="scope"}
owners={o["path"].lower():o for o in report["owners"]}
owner_indices={o["path"].lower():i for i,o in enumerate(report["owners"])}
compositions={c["owner"].lower():c for c in report["original_mask_compositions"]}
named=report["named_controller_state_lookups"]
assert len(named["requested_state_names"])==143
def resolve(cls,name):
    while cls:
        owner=owners[cls]
        # Native prepends each own struct in iterator order, so the last own
        # matching struct wins before any inherited entry is considered.
        for child in reversed(owner["ordered_children"]["children"]):
            if child["is_struct"] and child["path"].rsplit(".",1)[-1].lower()==name:
                kind=child["class"].lower()
                assert kind!="core.class", "unreviewed metaclass state predicate"
                return (owner["path"].split(".",1)[0]+"."+child["path"]).lower() if kind=="core.state" else None
        cls=owner["parent"].lower() if owner["parent"] else None
    return None
expected={}
misses=0
for controller in report["controller_class_lookups"]:
    cls=controller["class"].lower()
    for name in named["requested_state_names"]:
        state=resolve(cls,name)
        if state: expected[cls,name]=state
        else: misses+=1
assert len(expected)==289 and misses==7004
actual={}
inherited=0
for probe in named["matched"]:
    cls=probe["class"].lower()
    state=probe["selected"].lower()
    key=(cls,probe["requested"])
    assert key not in actual
    actual[key]=state
    assert probe["target"]["route"]=="NamedState"
    assert probe["target"]["state_node"]==owner_indices[state]
    raw=owners[state]["serialized_masks"]
    class_probe=compositions[cls]["runtime_class_probe"]
    mask=(class_probe|raw["probe_mask"])&raw["ignore_mask"]
    assert probe["frame"]["object_class"]==owner_indices[cls]
    assert probe["frame"]["state_node"]==owner_indices[state]
    assert probe["frame"]["probe_mask"]==mask
    assert probe["notify"]=={"handled":False,"route":"MaskedOut"}
    inherited+=state.rsplit(".",1)[0]!=cls
assert actual==expected and named["missing_name_class_fallbacks"]==misses
assert len(named["none_and_nonstate_fallbacks"])==102
for probe in named["none_and_nonstate_fallbacks"]:
    cls=probe["class"].lower()
    assert resolve(cls,probe["requested"].lower()) is None
    assert probe["target"]=={"state_node":owner_indices[cls],"route":"ClassFallback"}
native=(root/"analysis/decompiled/active-state.c").read_text()
assert "10154910 UObject::FindState" in native and "0x40000" in native
sources=["crates/rc-package/src/state_selection.rs","crates/rc-package/src/event_lookup.rs","crates/rc-inspect/src/bin/rc-state-link-probe.rs","analysis/decompiled/active-state.c"]
result={"date":"2026-10-07","rust_tests":170,"controller_classes":51,"requested_state_names":143,
        "named_state_matches":289,"inherited_state_matches":inherited,"missing_name_class_fallbacks":7004,"none_and_nonstate_fallbacks":102,
        "report_sha256":hashlib.sha256(path.read_bytes()).hexdigest(),
        "source_sha256":{s:hashlib.sha256((root/s).read_bytes()).hexdigest() for s in sources},
        "checks":["cargo test --workspace","cargo clippy --workspace --all-targets -- -D warnings","cargo fmt --all -- --check","rc-state-link-probe --native-hardcoded-names --original-masks --named-states","independent named state selection from original reversed own children/parent chains","prior original mask/list/class/frame reports unchanged"],
        "scope":report["scope"]}
(root/"analysis/reports/named-state-selection-validation.json").write_text(json.dumps(result,indent=2)+"\n",encoding="utf-8")
path=root/"analysis/evidence.json"
evidence=json.loads(path.read_text(encoding="utf-8-sig"))
evidence["rust_tests"]=170
evidence["named_state_selection_validation"]=result
path.write_text(json.dumps(evidence,indent=2,ensure_ascii=False)+"\n",encoding="utf-8")
note=f"2026-10-07: Benannte Zustandswahl angeschlossen. FindState10154910 sucht im Objektklassen-Hash und prüft State-Typflag40000; Nicht-State-Schatten setzt Suche nicht bei Eltern fort. state_selection.NamedStateLookup bildet FindState und benannte GotoState-Zielphase ab, Missing/NotState=>ClassFallback; Auto690 explizit nicht implementiert, unbekannte Typen/Nodes Fehler. Originalprobe fragt143State-Namen über51Controller-Klassen ab:289Treffer, davon{inherited}geerbt;7004fehlendeNamen und102None/NichtState-Fälle führen aufKlassezurück. Masken fürTreffer aus tatsächlicher anfragenderObjektklasse+ausgewähltemState+IgnoreMask, alle289NotifyHitWallMaskedOut. Unabhängig ausOriginal-Kinderlisten rückwärts/Elternkette nachgeprüft; bisherigeMasken/Kinder/Klassensuchen/Initialframes unverändert. NurCore.State-Exporte alsState akzeptiert; weitereOwner/Metaklassenkandidaten bleibenUnresolved stattfehlendeNative-Metaklassenhierarchiezuerraten. BenannteAnfragenexplizit, keine tatsächlicheRuntimezustandswahl/Auto/BeginState/EndState/VM.170WorkspaceTests/Clippy/Formatbestanden; AndroidzumSchluss. Details D:\\Rust Projects\\RepublicCommandoAndroid\\docs\\NAMED_STATES.md."
path=root/"README.md"
text=path.read_text(encoding="utf-8-sig")
if "Benannte Zustandswahl angeschlossen" not in text:
    text+="\n"+note+" [Nachweis](docs/NAMED_STATES.md).\n"
path.write_text(text,encoding="utf-8")
path=root/"docs/ORIGINAL_STATE_MASKS.md"
text=path.read_text(encoding="utf-8-sig")
if "## Folgearbeit: benannte Zustandswahl" not in text:
    text+="\n## Folgearbeit: benannte Zustandswahl\n\nDie Probe löst jetzt benannte Zustände über die tatsächliche anfragende Controller-Klasse auf, einschließlich Vererbung und Klassenfallback. 289gültige Kombinationen aus51Klassen/143Namen geprüft; Masken unterdrücken NotifyHitWall in diesen289Fällen.170Tests bestanden. Siehe [NAMED_STATES.md](NAMED_STATES.md).\n"
path.write_text(text,encoding="utf-8")
wiki=Path(r"C:\Users\brand\Documents\Main\05 KI\Project_Knowledge\projects\republic_commando_android")
for relative in ("index.md","architecture/porting.md","log.md"):
    path=wiki/relative
    if "Benannte Zustandswahl angeschlossen" not in path.read_text(encoding="utf-8-sig"):
        with path.open("a",encoding="utf-8") as output:
            output.write("\n\n"+note+"\n")
print(json.dumps({k:result[k] for k in ("named_state_matches","inherited_state_matches","missing_name_class_fallbacks","none_and_nonstate_fallbacks","rust_tests")}))
