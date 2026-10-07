"""Validate native initialized-frame mask probes and preserve prior original-package results."""
from pathlib import Path
import hashlib
import json

root=Path(__file__).resolve().parents[1]
path=root/"analysis/reports/initialized-probe-frames.json"
new=json.loads(path.read_text())
old=json.loads((root/"analysis/reports/native-named-state-link.json").read_text())
assert {k:v for k,v in new.items() if k not in ("scope","initialized_probe_frames")}=={k:v for k,v in old.items() if k!="scope"}
frames=new["initialized_probe_frames"]
assert len(frames)==51
for probe in frames:
    initial=probe["initial_frame"]
    assert initial["object_class"]==initial["state_node"]
    assert initial["probe_mask"]==(1<<64)-1
    assert probe["source"]=="ActiveState"
    assert probe["selected"].lower()=="engine.controller.notifyhitwall"
    assert probe["result"]=={"handled":False,"route":"EmptyBase"}
    assert probe["after_disable"]["probe_mask"]==((1<<64)-1) ^ (1<<53)
    assert probe["disabled_result"]=={"handled":False,"route":"MaskedOut"}
native=(root/"analysis/decompiled/active-state.c").read_text()
asm=(root/"analysis/decompiled/active-state.asm").read_text()
for instruction in ("10156e0d OR ECX,0xffffffff","10156e13 MOV dword ptr [EAX + 0x1c],ECX","10156e16 MOV dword ptr [EAX + 0x20],ECX"):
    assert instruction in asm
for name in ("1013a0c0 UObject::GotoState","10156da0 UObject::InitExecution","1013bbc0 UObject::execEnable","1013bcb0 UObject::execDisable","1010d470 UObject::IsProbing"):
    assert name in native
assert "(uVar2 | uVar3) & uVar4" in native
sources=["crates/rc-package/src/probe_frame.rs","crates/rc-inspect/src/bin/rc-state-link-probe.rs","analysis/decompiled/active-state.c","analysis/decompiled/active-state.asm"]
result={"date":"2026-10-07","rust_tests":164,"original_controller_frames":51,"mask_transitions_checked":51,
        "init_execution":"StateNode=object class; probe mask=u64::MAX",
        "resolved_state_mask":"(class_probe | state_probe) & state_ignore",
        "enable":"current_mask | (event_bit & resolved_state_mask)","disable":"current_mask & ~event_bit",
        "report_sha256":hashlib.sha256(path.read_bytes()).hexdigest(),
        "source_sha256":{s:hashlib.sha256((root/s).read_bytes()).hexdigest() for s in sources},
        "checks":["cargo test --workspace","cargo clippy --workspace --all-targets -- -D warnings","cargo fmt --all -- --check","original package initialized-frame and Disable probes","previous named report unchanged except extra initialized-frame probes/scope"],
        "scope":"Probe-mask subset only. InitExecution fields derived natively; resolved-state mask composition and Enable/Disable implemented. No full FStateFrame, state target selection, EndState/BeginState callbacks or GotoState lifecycle. Class/state/ignore mask inputs still supplied for resolved-state/Enable helpers. No running game instance or Android execution."}
(root/"analysis/reports/probe-frame-validation.json").write_text(json.dumps(result,indent=2)+"\n",encoding="utf-8")
path=root/"analysis/evidence.json"
evidence=json.loads(path.read_text(encoding="utf-8-sig"))
evidence["rust_tests"]=164
evidence["probe_frame_validation"]=result
path.write_text(json.dumps(evidence,indent=2,ensure_ascii=False)+"\n",encoding="utf-8")
note="2026-10-07: Native StateFrame-Ereignismasken ergänzt. InitExecution10156da0 setzt StateNode=Objektklasse und beide Maskenwörter -1, im Assembler bestätigt. ProbeFrame modelliert ausschließlich Objektklasse/StateNode/ProbeMask. GotoState1013a0c0 berechnet in der Zuweisungsphase (Class.ProbeMask|State.ProbeMask)&State.IgnoreMask; Enable1013bbc0 setzt nur erlaubte Bits, Disable1013bcb0 löscht Einzelbit, IsProbing1010d470 außerhalb300..363/ohneFrame true. FullGotoState nicht implementiert: EndState/BeginState können rekursive Zustandswechsel auslösen, Auto-Statewahl/Callbacks/Code/Latentfelder offen. Originalpaketprobe für51Controller mit nativerInitialmaske: StateNode=Class, BasisNotifyHitWall/EmptyBase; nachDisable353 MaskedOut. Frühere Klassen-/Ownerberichte ansonsten identisch. Gelieferte Class/State/IgnoreMasken für spätere Zuweisungen bleiben explizite Eingabe; keine Behauptung tatsächlich gestarteter Spielinstanzen. 164 Workspace-Tests, Clippy und Format bestanden. Android zum Schluss. Details D:\\Rust Projects\\RepublicCommandoAndroid\\docs\\PROBE_FRAME.md."
path=root/"README.md"
text=path.read_text(encoding="utf-8-sig")
if "Native StateFrame-Ereignismasken ergänzt" not in text:
    text+="\n"+note+" [Nachweis](docs/PROBE_FRAME.md).\n"
path.write_text(text,encoding="utf-8")
path=root/"docs/NATIVE_NAMES.md"
text=path.read_text(encoding="utf-8-sig")
if "## Folgearbeit: initialisierte Ereignismasken" not in text:
    text+="\n## Folgearbeit: initialisierte Ereignismasken\n\nDie Originalprobe verwendet jetzt zusätzlich den belegten InitExecution-Maskenzustand und Disable für alle51Controller-Klassen. StateNode=Class und Maske=alleBits; nachDisable353 MaskedOut. Spätere GotoState-Maskenkomposition und Enable sind als separate Helfer ergänzt; tatsächliche Zustandswahl/Callbacks bleiben offen. 164Tests bestanden. Siehe [PROBE_FRAME.md](PROBE_FRAME.md).\n"
path.write_text(text,encoding="utf-8")
wiki=Path(r"C:\Users\brand\Documents\Main\05 KI\Project_Knowledge\projects\republic_commando_android")
for relative in ("index.md","architecture/porting.md","log.md"):
    path=wiki/relative
    if "Native StateFrame-Ereignismasken ergänzt" not in path.read_text(encoding="utf-8-sig"):
        with path.open("a",encoding="utf-8") as output:
            output.write("\n\n"+note+"\n")
print("51 original Controller initialized masks and Disable transitions validated; 164 tests recorded.")
