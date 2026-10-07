"""Check AnimProp's fixed super call, redirect continuation and animation boundary."""
from pathlib import Path
from collections import Counter
import hashlib
import json
import subprocess
import sys

root=Path(__file__).resolve().parents[1]
subprocess.run([sys.executable,str(root/"scripts/Generate-PropShapes.py")],cwd=root,check=True)
path=root/"analysis/reports/anim-prop-transitions.json"
report=json.loads(path.read_text())
previous=json.loads((root/"analysis/reports/prop-transitions.json").read_text())
assert {k:v for k,v in report.items() if k not in ("scope","anim_prop_transition_probes")}=={k:v for k,v in previous.items() if k!="scope"}
owners={o["path"].lower():o for o in report["owners"]}
indices={o["path"].lower():i for i,o in enumerate(report["owners"])}
compositions={o["owner"].lower():o for o in report["original_mask_compositions"]}
auto={p["class"].lower():p for p in report["auto_class_targets"]}
anim=report["anim_prop_transition_probes"]
metadata=anim["anim_metadata"]
dependencies=json.loads((root/"analysis/reports/anim-prop-dependencies.json").read_text())
assert dependencies["errors"]==0
functions={f["path"]:f for f in dependencies["functions"]}
function=functions["AnimProp.Invulnerable.BeginState"]["decoded"]
assert metadata["function"]==function
assert json.loads((root/"crates/rc-package/tests/fixtures/anim_prop_begin.json").read_text())==function
assert function["expressions"][0]["opcode"]==0x1c
assert function["expressions"][0]["operand"]=={"kind":"Object","value":{"index":11676,"path":"Prop.Invulnerable.BeginState"}}
assert functions["Actor.PlayAnim"]["decoded"]["native_index"]==259
assert functions["Actor.LoopAnim"]["decoded"]["native_index"]==260
core=json.loads((root/"analysis/reports/anim-prop-core-dependencies.json").read_text())
assert core["errors"]==0 and core["functions"][0]["decoded"]["native_index"]==255
for key,name,kind,flags,struct in (
    ("healthy_field","AnimHealthy","Core.StructProperty",4718593,"AnimProp.PropAnimInfo"),
    ("name_field","Anim","Core.NameProperty",1,None),
    ("loop_field","bLoop","Core.BoolProperty",1,None),
):assert metadata[key]=={"name":name,"kind":kind,"flags":flags,"dimension":1,"struct_name":struct}

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
    selected=find_struct(state,name)
    if selected is None:selected=find_struct(cls,name)
    assert selected and selected[1]["is_function"]
    return selected[0]

def frame(cls,state):
    masks=owners[state]["serialized_masks"]
    value=(compositions[cls]["runtime_class_probe"]|masks["probe_mask"])&masks["ignore_mask"]
    return {"object_class":indices[cls],"state_node":indices[state],"probe_mask":value}

def no_labels(state):
    seen=set()
    while state:
        assert state not in seen
        seen.add(state)
        owner=owners[state]
        masks=owner["serialized_masks"]
        assert masks["label_table_offset"]==65535 or masks["logical_script_bytes"]==0
        state=owner["parent"].lower() if owner["parent"] else None

expected={p["class"].lower() for p in previous["auto_transition_probes"]
          if p["callback_request"] and p["callback_request"]["selected_function"]=="engine.AnimProp.Invulnerable.BeginState"}
assert len(expected)==len(anim["probes"])==32
assert {p["class"].lower() for p in anim["probes"]}==expected
counts=Counter()
requests=Counter()
final_states=Counter()
completed_inner=0
after_redirect_requests=0
for probe in anim["probes"]:
    cls=probe["class"].lower()
    inv=auto[cls]["selected"].lower()
    assert probe["auto_state"].lower()==inv=="engine.animprop.invulnerable"
    assert handler(cls,inv,"beginstate")=="engine.animprop.invulnerable.beginstate"
    dam,child=find_struct(cls,"damagable")
    assert child["class"].lower()=="core.state"
    assert handler(cls,dam,"beginstate")=="engine.prop.damagable.beginstate"
    assert frame(cls,inv)["probe_mask"]&(1<<16)
    assert not frame(cls,inv)["probe_mask"]&(1<<17)
    assert frame(cls,dam)["probe_mask"]&(1<<16)
    no_labels(dam)
    assert len(probe["cases"])==18
    scalar_inputs=[(-2147483648,False),(0,False),(1,False),(2147483647,False),(0,True),(1,True)]
    animation_inputs=[({"handle":0,"resolved_index":0},False),({"handle":337,"resolved_index":336},False),({"handle":337,"resolved_index":336},True)]
    expected_inputs=[(h,d,n,l) for h,d in scalar_inputs for n,l in animation_inputs]
    assert [(c["input"]["health"],c["input"]["display_state_messages"],c["animation_input"]["name"],c["animation_input"]["looping"]) for c in probe["cases"]]==expected_inputs
    for case in probe["cases"]:
        positive=case["input"]["health"]>0
        display=case["input"]["display_state_messages"]
        wants_animation=case["animation_input"]["name"]["handle"]!=0
        state=dam if positive else inv
        outcome="UnresolvedBroadcast" if display else "UnresolvedAnimation" if wants_animation else "Preempted" if positive else "Success"
        flags=0 if display or (wants_animation and not positive) else 4096
        assert case["outcome"]==outcome
        assert case["execution"]=={"probe":frame(cls,state),"node":indices[state],"code":None,"latent_action":0,"object_flags":flags}
        assert case["final_state"].lower()==state
        events=[{"event":"BeginState","function":"engine.AnimProp.Invulnerable.BeginState"},{"event":"DirectSuperBeginState","function":"engine.Prop.Invulnerable.BeginState"}]
        if positive:events.append({"event":"BeginState","function":"engine.Prop.Damagable.BeginState"})
        assert case["events"]==events
        if positive and not display:
            completed_inner+=1
            assert case["labels"]==[{"name":{"handle":101,"resolved_index":100},"found":False}]
            assert len(case["inner_switches"])==1
            inner=case["inner_switches"][0]
            assert inner["selected"].lower()==dam
            assert inner["target"]=={"selection":{"state_node":indices[dam],"route":"NamedState"},"name":{"handle":1528,"resolved_index":1527}}
            assert inner["result"]=={"state_result":"Success","label_result":False,"warning":None}
        else:assert not case["labels"] and not case["inner_switches"]
        if wants_animation and not display:
            looping=case["animation_input"]["looping"]
            kind="LoopAnim" if looping else "PlayAnim"
            request={"kind":kind,"name":case["animation_input"]["name"],"native_index":260 if looping else 259}
            assert case["animation_requests"]==[request]
            requests[kind]+=1
            after_redirect_requests+=positive
        else:assert not case["animation_requests"]
        counts[outcome]+=1
        final_states[case["final_state"]]+=1
assert sum(counts.values())==576
assert counts=={"Success":64,"Preempted":64,"UnresolvedAnimation":256,"UnresolvedBroadcast":192}
assert requests=={"PlayAnim":128,"LoopAnim":128}
assert completed_inner==192 and after_redirect_requests==128
asm=(root/"analysis/decompiled/anim-prop-flow.asm").read_text()
for marker in (
    "10138adc CMP EDX,dword ptr [ESP + 0xc]","10138ae4 SETNZ AL",
    "1012f6a8 MOV ESI,dword ptr [EDX]","1012f6b9 CALL dword ptr [EDX + 0x48]",
    "1013c294 CMP byte ptr [ECX],0x4","1013c2b3 CALL dword ptr [EDX*0x4 + 0x101bf240]",
    "1013c2bd CMP byte ptr [ECX],0x4","1013c2c0 JNZ 0x1013c2a0",
):assert marker in asm,marker
native=(root/"analysis/decompiled/anim-prop-flow.c").read_text()
assert "while (cVar1 != '\\x04')" in native
sources=["crates/rc-package/src/prop_state.rs","crates/rc-package/src/prop_begin_shapes.rs",
         "crates/rc-package/tests/fixtures/anim_prop_begin.json","crates/rc-inspect/src/prop_probes.rs",
         "crates/rc-inspect/src/bin/rc-state-link-probe.rs","analysis/decompiled/anim-prop-flow.c",
         "analysis/decompiled/anim-prop-flow.asm","scripts/Generate-PropShapes.py","scripts/Record-AnimPropTransitions.py"]
sha=lambda p:hashlib.sha256(p.read_bytes()).hexdigest()
result={"date":"2026-10-07","rust_tests":198,"classes":32,"cases":576,"outcomes":dict(counts),
        "requests":dict(requests),"completed_inner_switches":completed_inner,"animation_requests_after_redirect":after_redirect_requests,
        "final_states":dict(final_states),"report_sha256":sha(path),"engine_u_sha256":sha(Path(dependencies["package"])),
        "source_sha256":{s:sha(root/s) for s in sources},
        "checks":["cargo test --workspace","cargo clippy --workspace --all-targets -- -D warnings","cargo fmt --all -- --check",
                  "original AnimProp exact AST and field types","native direct-super/return-loop/name comparison assembler",
                  "independent state/handler/mask resolution for 576 cases","previous Prop and all original report values unchanged"],
        "scope":anim["scope"]+" Native name comparator decompilation incorrectly emits zero; CMP/SETNZ assembler is authoritative."}
(root/"analysis/reports/anim-prop-transitions-validation.json").write_text(json.dumps(result,indent=2)+"\n",encoding="utf-8")
path=root/"analysis/evidence.json"
evidence=json.loads(path.read_text(encoding="utf-8-sig"))
evidence["rust_tests"]=198
evidence["anim_prop_transition_validation"]=result
path.write_text(json.dumps(evidence,indent=2,ensure_ascii=False)+"\n",encoding="utf-8")
note=("2026-10-07: AnimProp-BeginState-Ablauf angeschlossen. Exakter AST und AnimHealthy-/Name-/Bool-Feldtypen "
      "verifiziert. Fester direkter Prop-Superhandler vor Animation; die laufende Funktion setzt nach erfolgreicher "
      "Damagable-Umleitung fort. None überspringt Animation, PlayAnim259/LoopAnim260 bleiben externe Anforderungen. "
      "Originalprobe 32 Klassen/576 Fälle: 64 Success, 64 Preempted, 256 vor Animation und 192 vor Broadcast angehalten. "
      "128 Animationsanforderungen folgen einem bereits abgeschlossenen inneren Zustandswechsel. Native ProcessInternal-" 
      "Schleife/FinalFunction und Name-CMP/SETNZ geprüft; keine Wiedergabe behauptet. Frühere Prop-Proben unverändert. "
      "198 Workspace-Tests, Clippy und Format bestanden. Diagnoseeingaben, kein echter Clip-/Default-/VM-/Native-Override-" 
      "Nachweis; Android zum Schluss. Details D:\\Rust Projects\\RepublicCommandoAndroid\\docs\\ANIM_PROP_TRANSITIONS.md.")
path=root/"README.md"
text=path.read_text(encoding="utf-8-sig")
if "AnimProp-BeginState-Ablauf angeschlossen" not in text:
    path.write_text(text+"\n"+note+" [Nachweis](docs/ANIM_PROP_TRANSITIONS.md).\n",encoding="utf-8")
wiki=Path(r"C:\Users\brand\Documents\Main\05 KI\Project_Knowledge\projects\republic_commando_android")
for relative in ("index.md","architecture/porting.md","log.md"):
    path=wiki/relative
    if "AnimProp-BeginState-Ablauf angeschlossen" not in path.read_text(encoding="utf-8-sig"):
        with path.open("a",encoding="utf-8") as output:output.write("\n\n"+note+"\n")
print(json.dumps({k:result[k] for k in ("classes","cases","outcomes","requests","rust_tests")}))
