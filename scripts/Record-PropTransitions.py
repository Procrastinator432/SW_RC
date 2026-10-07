"""Independently resolve original Prop state/handler ancestry and phase outcomes."""
from pathlib import Path
from collections import Counter
import hashlib
import json
import subprocess
import sys

root=Path(__file__).resolve().parents[1]
subprocess.run([sys.executable,str(root/"scripts/Generate-PropShapes.py")],cwd=root,check=True)
path=root/"analysis/reports/prop-transitions.json"
report=json.loads(path.read_text())
previous=json.loads((root/"analysis/reports/state-transitions.json").read_text())
assert {k:v for k,v in report.items() if k not in ("scope","prop_transition_probes")}=={k:v for k,v in previous.items() if k!="scope"}
owners={o["path"].lower():o for o in report["owners"]}
indices={o["path"].lower():i for i,o in enumerate(report["owners"])}
compositions={o["owner"].lower():o for o in report["original_mask_compositions"]}
auto={p["class"].lower():p for p in report["auto_class_targets"]}
props=report["prop_transition_probes"]
assert props["health_field"]=={"name":"Health","kind":"Core.IntProperty","dimension":1,"flags":1,"struct_name":None}
assert props["display_field"]=={"name":"bDisplayStateMessages","kind":"Core.BoolProperty","dimension":1,"flags":1,"struct_name":None}
dependencies=json.loads((root/"analysis/reports/prop-state-dependencies.json").read_text())
assert dependencies["errors"]==0
for key,state in (("invulnerable_function","Invulnerable"),("damagable_function","Damagable")):
    original=next(f["decoded"] for f in dependencies["functions"] if f["path"]==f"Prop.{state}.BeginState")
    assert props[key]==original
    assert json.loads((root/f"crates/rc-package/tests/fixtures/prop_{state.lower()}_begin.json").read_text())==original

def find_struct(cls,name):
    seen=set()
    while cls:
        assert cls not in seen
        seen.add(cls)
        owner=owners[cls]
        for child in reversed(owner["ordered_children"]["children"]):
            if child["is_struct"] and child["path"].rsplit(".",1)[-1].lower()==name:
                return (owner["path"].split(".",1)[0]+"."+child["path"]).lower(),child
        cls=owner["parent"].lower() if owner["parent"] else None
    return None

def handler(cls,state,name):
    found=find_struct(state,name)
    if found is None:found=find_struct(cls,name)
    assert found and found[1]["is_function"]
    return found[0]

def frame(cls,state):
    raw=owners[state]["serialized_masks"]
    value=(compositions[cls]["runtime_class_probe"]|raw["probe_mask"])&raw["ignore_mask"]
    return {"object_class":indices[cls],"state_node":indices[state],"probe_mask":value}

def prove_no_labels(state):
    seen=set()
    while state:
        assert state not in seen
        seen.add(state)
        owner=owners[state]
        masks=owner["serialized_masks"]
        assert masks["label_table_offset"]==65535 or masks["logical_script_bytes"]==0
        state=owner["parent"].lower() if owner["parent"] else None

expected={p["class"].lower() for p in previous["auto_transition_probes"]
          if p["callback_request"] and p["callback_request"]["selected_function"]=="engine.Prop.Invulnerable.BeginState"}
assert len(props["probes"])==len(expected)==168
assert {p["class"].lower() for p in props["probes"]}==expected
counts=Counter()
states=Counter()
case_count=0
for probe in props["probes"]:
    cls=probe["class"].lower()
    inv=auto[cls]["selected"].lower()
    assert probe["auto_state"].lower()==inv
    assert handler(cls,inv,"beginstate")=="engine.prop.invulnerable.beginstate"
    dam,child=find_struct(cls,"damagable")
    assert child["class"].lower()=="core.state"
    assert handler(cls,dam,"beginstate")=="engine.prop.damagable.beginstate"
    assert frame(cls,inv)["probe_mask"]&(1<<16)
    assert not frame(cls,inv)["probe_mask"]&(1<<17)
    assert frame(cls,dam)["probe_mask"]&(1<<16)
    prove_no_labels(dam)
    inputs=[(-2147483648,False),(0,False),(1,False),(2147483647,False),(0,True),(1,True)]
    assert [(c["input"]["health"],c["input"]["display_state_messages"]) for c in probe["cases"]]==inputs
    for case in probe["cases"]:
        case_count+=1
        positive=case["input"]["health"]>0
        display=case["input"]["display_state_messages"]
        state=dam if positive else inv
        outcome="UnresolvedBroadcast" if display else "Preempted" if positive else "Success"
        assert case["outcome"]==outcome
        assert case["final_state"].lower()==state
        assert case["execution"]=={"probe":frame(cls,state),"node":indices[state],"code":None,"latent_action":0,"object_flags":0 if display else 4096}
        events=[{"event":"BeginState","function":"engine.Prop.Invulnerable.BeginState"}]
        if positive:events.append({"event":"BeginState","function":"engine.Prop.Damagable.BeginState"})
        assert case["events"]==events
        if positive and not display:
            assert case["labels"]==[{"name":{"handle":101,"resolved_index":100},"found":False}]
            assert len(case["inner_switches"])==1
            inner=case["inner_switches"][0]
            assert inner["selected"].lower()==dam
            assert inner["target"]["selection"]=={"state_node":indices[dam],"route":"NamedState"}
            assert inner["target"]["name"]=={"handle":1528,"resolved_index":1527}
            assert inner["result"]=={"state_result":"Success","label_result":False,"warning":None}
        else:
            assert not case["labels"] and not case["inner_switches"]
        counts[outcome]+=1
        states[case["final_state"]]+=1
assert case_count==1008
assert counts=={"Success":336,"Preempted":336,"UnresolvedBroadcast":336}
asm=(root/"analysis/decompiled/script-goto-state.asm").read_text()
for marker in (
    "10130c72 CMP EAX,ECX","10130c78 SETG DL",
    "1013bb12 CMP EAX,EDI","1013bb15 JZ 0x1013bb24",
    "1013bb1c CALL dword ptr [EDX + 0x2c]","1013bb1f CMP EAX,0x1",
    "1013bb31 MOV EAX,dword ptr [EAX + 0x190]",
    "1013bb3c CALL dword ptr [EDX + 0x30]",
    "1013bb43 MOV ECX,dword ptr [ESP + 0x10]","1013bb49 JZ 0x1013bbb0",
    "1013792f MOV word ptr [EAX + 0x24],0x0",
    "10137948 CMP AX,0xffff","10137956 JLE 0x1013796f",
    "1013796f MOV ESI,dword ptr [ESI + 0x30]",
    "10137979 MOV dword ptr [ECX + 0xc],0x0",
):assert marker in asm,marker
fixed=json.loads((root/"analysis/reports/hardcoded-names-proof.json").read_text())
assert next(e["index"] for e in fixed["entries"] if e["name"]=="Begin")==100
sources=["crates/rc-package/src/script_state.rs","crates/rc-package/src/prop_state.rs",
         "crates/rc-package/src/prop_begin_shapes.rs","crates/rc-package/src/state_transition.rs",
         "crates/rc-inspect/src/prop_probes.rs","crates/rc-inspect/src/bin/rc-state-link-probe.rs",
         "analysis/decompiled/script-goto-state.c","analysis/decompiled/script-goto-state.asm",
         "scripts/Generate-PropShapes.py","scripts/Record-PropTransitions.py"]
sha=lambda p:hashlib.sha256(p.read_bytes()).hexdigest()
result={"date":"2026-10-07","rust_tests":194,"classes":168,"cases":case_count,
        "outcomes":dict(counts),"final_states":dict(states),"completed_nested_script_switches":336,
        "native_addresses":{"execGotoState":"1013ba80","GotoLabel":"10137920","execGreater_IntInt":"10130c00"},
        "report_sha256":sha(path),"source_sha256":{s:sha(root/s) for s in sources},
        "engine_u_sha256":sha(Path(dependencies["package"])),
        "checks":["cargo test --workspace","cargo clippy --workspace --all-targets -- -D warnings","cargo fmt --all -- --check",
                  "generated exact AST shapes match original dumps","independent named-state/handler ancestry and mask composition",
                  "1008 original-metadata phase cases","no label tables proven along actual selected-state ancestry","previous report values unchanged"],
        "scope":props["scope"]+" Script wrapper assumes arguments with no state-changing evaluation side effects; base GotoState and supplied label handler only. Assembler signed SETG is authoritative where the decompiler incorrectly emits constant zero."}
(root/"analysis/reports/prop-transitions-validation.json").write_text(json.dumps(result,indent=2)+"\n",encoding="utf-8")
path=root/"analysis/evidence.json"
evidence=json.loads(path.read_text(encoding="utf-8-sig"))
evidence["rust_tests"]=194
evidence["prop_transition_validation"]=result
path.write_text(json.dumps(evidence,indent=2,ensure_ascii=False)+"\n",encoding="utf-8")
note=("2026-10-07: Prop-Zustandsumleitung angeschlossen. Exakte Original-ASTs von Prop.Invulnerable.BeginState "
      "und Prop.Damagable.BeginState verifiziert; typisierte Health-/Display-Eingaben wählen Return, GotoDamagable "
      "oder ausdrücklich offene Broadcast-Anforderung. Skript-GotoState um gleiche Namen, Basiswechsel-Ergebnis, "
      "Begin-Label und Warnbedingungen ergänzt; nativer Assembler bestätigt signiertes Health>0 trotz fehlerhafter "
      "Decompiler-Konstante. Originalprobe 168 Klassen, 1008 Fälle: 336 normale Abschlüsse, 336 korrekt vorgezogene "
      "rekursive Damagable-Wechsel, 336 an Broadcast angehalten. KarmaProp-eigene Zustände mit geerbten Prop-Handlern "
      "korrekt unterschieden. 336 Begin-Labelsuchen ohne Treffer durch Originalmetadaten belegt. 194 Workspace-Tests, "
      "Clippy und Format bestanden. AnimProp-Override, Broadcast, allgemeine VM/Labelausführung und native Overrides "
      "bleiben offen; Eingaben keine echten Defaults/Spielinstanzen. Android zum Schluss. Details "
      "D:\\Rust Projects\\RepublicCommandoAndroid\\docs\\PROP_TRANSITIONS.md.")
path=root/"README.md"
text=path.read_text(encoding="utf-8-sig")
if "Prop-Zustandsumleitung angeschlossen" not in text:
    path.write_text(text+"\n"+note+" [Nachweis](docs/PROP_TRANSITIONS.md).\n",encoding="utf-8")
wiki=Path(r"C:\Users\brand\Documents\Main\05 KI\Project_Knowledge\projects\republic_commando_android")
for relative in ("index.md","architecture/porting.md","log.md"):
    path=wiki/relative
    if "Prop-Zustandsumleitung angeschlossen" not in path.read_text(encoding="utf-8-sig"):
        with path.open("a",encoding="utf-8") as output:output.write("\n\n"+note+"\n")
print(json.dumps({k:result[k] for k in ("classes","cases","outcomes","rust_tests")}))
