"""Verify the native downward constant and persist the tested static recovery adapter."""
from pathlib import Path
import hashlib
import json
import struct

root=Path(__file__).resolve().parents[1]
binary=Path(r"D:\SteamLibrary\steamapps\common\Star Wars Republic Commando\GameData\System\engine.dll")
data=binary.read_bytes()
pe=struct.unpack_from("<I",data,60)[0]
assert data[:2]==b"MZ" and data[pe:pe+4]==b"PE\0\0"
optional=pe+24
assert struct.unpack_from("<H",data,optional)[0]==0x10b
base=struct.unpack_from("<I",data,optional+28)[0]
count=struct.unpack_from("<H",data,pe+6)[0]
size=struct.unpack_from("<H",data,pe+20)[0]
sections=[struct.unpack_from("<IIII",data,optional+size+40*i+8) for i in range(count)]
rva=0x1067667c-base
offsets=[raw+rva-start for _,start,length,raw in sections if start<=rva and rva+4<=start+length]
assert len(offsets)==1
offset=offsets[0]
value=struct.unpack_from("<f",data,offset)[0]
assert value==-35.0
asm=(root/"analysis/decompiled/pawn-hit-wall.asm").read_text()
for instruction in ("104904fc CALL 0x103ada40","10490529 MOVSS XMM0,dword ptr [0x1067667c]","1049053d CALL dword ptr [EDX + 0xb0]","10490543 TEST EBX,EBX","10490598 CALL 0x1048db00"):
    assert instruction in asm
sources=["analysis/decompiled/pawn-hit-wall.c","analysis/decompiled/pawn-hit-wall.asm","analysis/decompiled/hit-wall-recovery-helper.c","crates/rc-package/src/hit_wall.rs"]
result={"date":"2026-10-06","rust_tests":143,"native_function":"APawn.processHitWall104900b0","constant":{"va":"1067667c","file_offset":offset,"bytes":data[offset:offset+4].hex(),"f32":value},"recovery_delta":[0,0,-35],"helper":"103ada40 initializes hit report, no movement","engine_dll_sha256":hashlib.sha256(data).hexdigest(),"source_sha256":{s:hashlib.sha256((root/s).read_bytes()).hexdigest() for s in sources},"checks":["cargo test --workspace","cargo clippy --workspace --all-targets -- -D warnings","cargo fmt --all -- --check"],"scope":"Eligible Walking/nonhuman first-attempt branch, own single static downward AABB sweep with skin, second probe from actual position using saved direction. No native MoveActor/events/dynamic actors/automatic contact dispatch. Other native branches may still perform downward recovery and are not implemented.","original_map_runs_this_milestone":0,"android":"deferred until end per user"}
(root/"analysis/reports/hit-wall-recovery-validation.json").write_text(json.dumps(result,indent=2)+"\n",encoding="utf-8")
path=root/"analysis/evidence.json"
evidence=json.loads(path.read_text(encoding="utf-8-sig"))
evidence["rust_tests"]=143
evidence["hit_wall_recovery_validation"]=result
path.write_text(json.dumps(evidence,indent=2,ensure_ascii=False)+"\n",encoding="utf-8")
path=root/"README.md"
text=path.read_text(encoding="utf-8-sig").replace("138 Rust-Unit-Tests","143 Rust-Unit-Tests")
if "Abwärtsschritt und zweiter Duckversuch" not in text:
    text+="\nAbwärtsschritt und zweiter Duckversuch im berechtigten KI-Wandkontaktzweig ergänzt. Native Konstante direkt aus DLL als-35 verifiziert; eigener statischer Sweep mit Skin, kein vollständiges MoveActor.143Tests bestehen. [Details](docs/HIT_WALL_RECOVERY.md).\n"
path.write_text(text,encoding="utf-8")
for relative in ("docs/HIT_WALL.md","docs/CROUCH_PHYSICS.md","docs/AUTO_CROUCH.md"):
    path=root/relative
    text=path.read_text(encoding="utf-8-sig")
    if "## Folgearbeit: Abwärtsschritt" not in text:
        text+="\n## Folgearbeit: Abwärtsschritt\n\nDer berechtigte Walking/nonhuman Auto-Crouch-Zweig besitzt jetzt einen zusätzlichen Recovery-Adapter mit Abwärtsschritt und zweitem Duckversuch. Korrektur der früheren Richtungsannahme: Native Konstante1067667c ist-35, keine Aufwärtsbewegung. Eigene statische Query mit Skin und geprüfter Zielplatzierung; MoveActor-Callbacks und automatischer Kontaktdispatch weiterhin offen.143Tests bestehen. Siehe [HIT_WALL_RECOVERY.md](HIT_WALL_RECOVERY.md).\n"
    path.write_text(text,encoding="utf-8")
wiki=Path(r"C:\Users\brand\Documents\Main\05 KI\Project_Knowledge\projects\republic_commando_android")
note="2026-10-06: Abwärtsschritt und zweiter Duckversuch in StaticBodyWorld.hit_wall_auto_crouch_recovery ergänzt, nurberechtigterWalking/nonhumanAutoCrouchzweig. Native processHitWall-Konstante1067667c direktPEgeprüft=-35 (Offset3630716/00000cc2), korrigiertfrühereupwardAnnahme. Helper103ada40 initialisiertHitreport(Time1/Item-1), keineBewegung. Assembler1049053d vtableb0MoveActor, Rückgabewertignoriert; zweiterCanCrouchWalk10490598 anaktuellerLocation mitgespeicherterRichtung. EigenerstatischerEinzelsweepabwärtsmitSkin/Zielplacement,Velocity/Grounded/Formunverändert; ArmedFirst/ArmedSecond/Unresolved/Skipped. FünfTests: freier/teilweiser/bodenblockierterSchritt, zweiterErfolg/Blockade, Gates/ersterErfolgskippen, invalidSkin/initialPenetration/Unknownatomisch.143Tests/Clippy/Formatbestanden. Nachweis analysis/reports/hit-wall-recovery-validation.json samtPE/DLL/Quellhashes. KeineOriginalkartenläufe/neueSpielerpfadänderung; nativeMoveActorCallbacks/dynamischeActorfilter/andereRecoveryzweige/automatischerKontaktdispatchoffen. AndroidzumSchluss. Details: D:\\Rust Projects\\RepublicCommandoAndroid\\docs\\HIT_WALL_RECOVERY.md."
for relative in ("index.md","architecture/porting.md","log.md"):
    path=wiki/relative
    if "Abwärtsschritt und zweiter Duckversuch in StaticBodyWorld.hit_wall_auto_crouch_recovery" not in path.read_text(encoding="utf-8-sig"):
        with path.open("a",encoding="utf-8") as output:
            output.write("\n\n"+note+"\n")
print("Verified native -35 recovery constant and recorded 143-test milestone.")
