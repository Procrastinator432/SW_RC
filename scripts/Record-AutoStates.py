"""Validate Auto target selection independently from original forward child lists."""
from pathlib import Path
import hashlib
import json
from collections import Counter

root=Path(__file__).resolve().parents[1]
path=root/"analysis/reports/auto-state-selection.json"
report=json.loads(path.read_text())
old=json.loads((root/"analysis/reports/named-state-selection.json").read_text())
assert {k:v for k,v in report.items() if k not in ("scope","auto_class_targets","auto_controller_probes")}=={k:v for k,v in old.items() if k!="scope"}
owners={o["path"].lower():o for o in report["owners"]}
indices={o["path"].lower():i for i,o in enumerate(report["owners"])}
compositions={c["owner"].lower():c for c in report["original_mask_compositions"]}
classes={k for k,c in compositions.items() if c["owner"]==c["object_class"]}
def resolve(start):
    cls=start
    while cls:
        owner=owners[cls]
        for child in owner["ordered_children"]["children"]:
            assert child["class"].lower()!="core.class", "unreviewed metaclass iterator predicate"
            if child["class"].lower()=="core.state":
                candidate=(owner["path"].split(".",1)[0]+"."+child["path"]).lower()
                if owners[candidate]["serialized_masks"]["state_flags"]&2:
                    return candidate,"AutoState"
        cls=owner["parent"].lower() if owner["parent"] else None
    return start,"ClassFallback"
assert len(report["auto_class_targets"])==len(classes)==3170
routes=Counter()
inherited=0
actual={}
for probe in report["auto_class_targets"]:
    cls=probe["class"].lower()
    assert cls in classes and cls not in actual
    selected,route=resolve(cls)
    actual[cls]=probe
    assert probe["selected"].lower()==selected
    assert probe["target"]=={"state_node":indices[selected],"route":route}
    raw=owners[selected]["serialized_masks"]
    class_probe=compositions[cls]["runtime_class_probe"]
    state_probe=compositions[selected]["runtime_class_probe"] if selected in classes else raw["probe_mask"]
    assert probe["frame"]=={"object_class":indices[cls],"state_node":indices[selected],"probe_mask":(class_probe|state_probe)&raw["ignore_mask"]}
    routes[route]+=1
    inherited+=route=="AutoState" and selected.rsplit(".",1)[0]!=cls
assert routes=={"AutoState":517,"ClassFallback":2653}
controllers=report["auto_controller_probes"]
assert len(controllers)==51
controller_routes=Counter()
controller_targets=Counter()
for probe in controllers:
    assert {k:v for k,v in probe.items() if k!="notify"}==actual[probe["class"].lower()]
    assert probe["notify"]=={"handled":False,"route":"MaskedOut"}
    controller_routes[probe["target"]["route"]]+=1
    controller_targets[probe["selected"]]+=1
assert controller_routes=={"AutoState":47,"ClassFallback":4}
native=(root/"analysis/decompiled/auto-state-iterator.c").read_text()
asm=(root/"analysis/decompiled/active-state.asm").read_text()
assert "param_1 + 0x3c" in native and "0x40000" in native and "+ 0x28" in native
assert "1013a16e TEST byte ptr [ESI + 0x88],0x2" in asm
sources=["crates/rc-package/src/state_selection.rs","crates/rc-inspect/src/bin/rc-state-link-probe.rs","analysis/decompiled/auto-state-iterator.c","analysis/decompiled/active-state.asm"]
result={"date":"2026-10-07","rust_tests":173,"class_targets":3170,"routes":dict(routes),"inherited_auto_targets":inherited,
        "controller_routes":dict(controller_routes),"controller_targets":dict(controller_targets),"controller_notify_masked_out":51,
        "report_sha256":hashlib.sha256(path.read_bytes()).hexdigest(),
        "source_sha256":{s:hashlib.sha256((root/s).read_bytes()).hexdigest() for s in sources},
        "checks":["cargo test --workspace","cargo clippy --workspace --all-targets -- -D warnings","cargo fmt --all -- --check","original package Auto targets","independent forward own-child/parent resolution and mask composition","previous named-state and original reports unchanged"],"scope":report["scope"]}
(root/"analysis/reports/auto-state-selection-validation.json").write_text(json.dumps(result,indent=2)+"\n",encoding="utf-8")
path=root/"analysis/evidence.json"
evidence=json.loads(path.read_text(encoding="utf-8-sig"))
evidence["rust_tests"]=173
evidence["auto_state_selection_validation"]=result
path.write_text(json.dumps(evidence,indent=2,ensure_ascii=False)+"\n",encoding="utf-8")
note="2026-10-07: Automatische Zustands-Zielwahl angeschlossen. Native GotoState1013a0c0/Iterator10139f10 prüft eigeneState-Kinder inVorwärtsreihenfolge, dannVorfahrlisten; ersterStateFlags&2-Treffer wird direkt alsStateNode übernommen, keine erneuteNamensauflösung/Hashüberschreibung. AutoStateLookup setzt diese Zielphase um; keinTreffer=>Klassenfallback, fehlendeMetadaten/ZyklenFehler. Originalprobe3170Klassen:517Auto-Ziele/2653Klassenfallbacks.51Controller:47Auto/4Fallback, darunter40CTBot.BotAI und6PlayerWaiting; NotifyHitWall inallen51auskomponiertenMaskenMaskedOut. AuswahlundMasken unabhängig ausOriginalKinderlisten/Flags/Eltern geprüft; bisherigebenannteZiele/Klassen/Masken unverändert.173WorkspaceTests/Clippy/Formatbestanden. BeginState/EndState/rekursiveWechsel undNativeOverrides können anschließendenAblaufändern; keinevollständigeGotoState/Startup/VM-Parität. AndroidzumSchluss. Details D:\\Rust Projects\\RepublicCommandoAndroid\\docs\\AUTO_STATES.md."
path=root/"README.md"
text=path.read_text(encoding="utf-8-sig")
if "Automatische Zustands-Zielwahl angeschlossen" not in text:
    text+="\n"+note+" [Nachweis](docs/AUTO_STATES.md).\n"
path.write_text(text,encoding="utf-8")
path=root/"docs/NAMED_STATES.md"
text=path.read_text(encoding="utf-8-sig")
if "## Folgearbeit: Auto-Zielwahl" not in text:
    text+="\n## Folgearbeit: Auto-Zielwahl\n\nDie Auto-Zielphase ist jetzt separat implementiert und an Original-Stateflags angeschlossen:517Auto-Ziele und2653Klassenfallbacks,51Controller-Masken geprüft. BeginState/EndState und Runtime-Overrides bleiben offen.173Tests bestanden. Siehe [AUTO_STATES.md](AUTO_STATES.md).\n"
path.write_text(text,encoding="utf-8")
wiki=Path(r"C:\Users\brand\Documents\Main\05 KI\Project_Knowledge\projects\republic_commando_android")
for relative in ("index.md","architecture/porting.md","log.md"):
    path=wiki/relative
    if "Automatische Zustands-Zielwahl angeschlossen" not in path.read_text(encoding="utf-8-sig"):
        with path.open("a",encoding="utf-8") as output:
            output.write("\n\n"+note+"\n")
print(json.dumps({k:result[k] for k in ("routes","inherited_auto_targets","controller_routes","controller_targets","rust_tests")}))
