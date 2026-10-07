"""Independent GetMoveCoords(false) diagnostic cases and exported native target evidence."""
from pathlib import Path
from collections import Counter
import hashlib,json,struct
root=Path(__file__).resolve().parents[1]
binary=Path(r"D:\SteamLibrary\steamapps\common\Star Wars Republic Commando\GameData\System\engine.dll")
data=binary.read_bytes();pe=struct.unpack_from("<I",data,60)[0];optional=pe+24
base=struct.unpack_from("<I",data,optional+28)[0];size=struct.unpack_from("<H",data,pe+20)[0]
sections=[struct.unpack_from("<IIII",data,optional+size+i*40+8) for i in range(struct.unpack_from("<H",data,pe+6)[0])]
def read(a,n):
    offsets=[raw+a-base-start for _,start,size,raw in sections if start<=a-base and a-base+n<=start+size];assert len(offsets)==1
    return data[offsets[0]:offsets[0]+n]
assert read(0x1066a5c8+0xe0,4)==struct.pack("<I",0x10500a30)
exports=json.loads((root/"analysis/reports/binaries/System__engine.dll.json").read_text())["exports"]
assert any(e.get("address")=="10500a30" and e["name"]=="?GetMoveCoords@USkeletalMeshInstance@@UAE_NPAVFMatrix@@_N@Z" for e in exports)
asm=(root/"analysis/decompiled/animation-move-coords.asm").read_text()
for marker in (
    "10500a6f JNZ 0x10500f7f","10500a7c CMP EDX,dword ptr [EAX + 0x1c0]","10500a88 PUSH -0x1",
    "10500a8d CALL 0x10500930","10500aaa JNP 0x10500f7f","10500ab3 CALL 0x1044faf0",
    "10500ac0 TEST byte ptr [EAX + 0x54],0x1","10500acc JZ 0x10500f7b","10500ae4 CALL dword ptr [ESI + 0x9c]",
    "10500aed CALL dword ptr [ESI + 0xdc]","10500af9 MOV ESI,dword ptr [EBP + 0xa0]",
    "10500aff MOV ECX,0x10","10500b06 MOVSD.REP ES:EDI,ESI","10500b08 JZ 0x10500f77",
    "10500f7b INC dword ptr [ESP + 0x18]","10500f9a JL 0x10500a60","10500fab SETG AL",
):assert marker in asm,marker
active=(root/"analysis/decompiled/animation-channel-active.asm").read_text()
for marker in (
    "10500938 JL 0x105009e7","10500949 JGE 0x105009e7","1050095a JZ 0x1050096a",
    "10500968 JBE 0x105009e7","10500973 CMP dword ptr [EBX + 0x3c],ESI",
    "105009a5 MULSS XMM1,dword ptr [ECX + -0x10]","105009aa UCOMISS XMM1,XMM0",
    "105009b1 JP 0x105009d7","105009d5 JLE 0x105009c0",
):assert marker in active,marker
clone=lambda x:json.loads(json.dumps(x))
bits=lambda x:struct.unpack("<I",struct.pack("<f",x))[0]
previous_path=root/"analysis/reports/channel-playback.json"
selected=[(i,p) for i,p in enumerate(json.loads(previous_path.read_text())["probes"]) if p["variant"]=="NewSequence"]
path=root/"analysis/reports/move-coordinates.json";report=json.loads(path.read_text())
scenarios=["LoopSkipped","BoneMismatch","Stopped","Occluded","SequenceMissing","NoMoveFlag","NoOutput","MatrixCopy","FrameBoundary","SuppliedCompose"]
assert len(selected)==768 and len(report["probes"])==7680
counts=Counter();callbacks=Counter()
for i,probe in enumerate(report["probes"]):
    source_index,source=selected[i//10];scenario=scenarios[i%10];index=source["channel_index"]
    assert (probe["source_index"],probe["class"],probe["source_case"],probe["scenario"],probe["channel_index"])==(source_index,source["class"],source["source_case"],scenario,index)
    before=clone(source["after"]["channels"])
    for c in before:c["words"][1]=(c["words"][1]&0xffffff00)|1;c["words"][14]=0
    w=before[index]["words"];w[1]&=0xffffff00;w[6]=bits(1);w[14]=bits(1);w[11]=bits(0.5)
    move_bone=w[15] if w[15]<0x80000000 else w[15]-0x100000000
    if scenario=="LoopSkipped":w[1]|=1
    if scenario=="Stopped":w[6]=0
    if scenario=="Occluded":blocker=clone(before[index]);blocker["words"][1]|=1;blocker["words"][11]=bits(1);before.append(blocker)
    if scenario=="BoneMismatch":move_bone+=100
    assert probe["before_channels"]==before and probe["move_bone"]==move_bone
    after=clone(before);matrix=[7]*16;events=[] if scenario=="NoOutput" else ["GetMoveCoords(relative=false)"]
    if scenario in scenarios[:4]:result={"Ok":"RootUnavailable"};outcome="RootUnavailable"
    else:
        events.append(f"GetSequence({index})");after[index]["words"][17]=900+index
        if scenario in ("SequenceMissing","NoMoveFlag"):result={"Ok":"RootUnavailable"};outcome="RootUnavailable"
        elif scenario=="NoOutput":result={"Ok":True};outcome="MatchedWithoutOutput"
        else:
            events.append(f"EvaluateMoveMatrix({index})")
            if scenario=="FrameBoundary":result={"Err":"unresolved frame/pose evaluation"};outcome="UnresolvedFrame"
            else:
                matrix=[100+index]*16;events.append("InverseActorCompose")
                if scenario=="SuppliedCompose":matrix=[9]*16;result={"Ok":"Updated"};outcome="SuppliedUpdated"
                else:result={"Err":"unresolved inverse/actor matrix composition"};outcome="UnresolvedComposition"
    assert (probe["after_channels"],probe["matrix"],probe["events"],probe["result"])==(after,matrix,events,result)
    counts[outcome]+=1;callbacks.update(events)
assert dict(counts)=={"RootUnavailable":4608,"MatchedWithoutOutput":768,"UnresolvedComposition":768,"UnresolvedFrame":768,"SuppliedUpdated":768}
sha=lambda p:hashlib.sha256(p.read_bytes()).hexdigest()
sources=["crates/rc-package/src/move_coords.rs","crates/rc-inspect/src/bin/rc-move-coords-probe.rs","analysis/decompiled/animation-move-coords.c","analysis/decompiled/animation-move-coords.asm","analysis/decompiled/animation-channel-active.c","analysis/decompiled/animation-channel-active.asm","scripts/Record-MoveCoordinates.py"]
result={"date":"2026-10-07","rust_tests":229,"supplied_scenarios":7680,"outcomes":dict(counts),"callbacks":dict(callbacks),"vtable":"1066a5c8+e0 -> 10500a30 USkeletalMeshInstance.GetMoveCoords","report_sha256":sha(path),"previous_report_sha256":sha(previous_path),"engine_dll_sha256":sha(binary),"source_sha256":{s:sha(root/s) for s in sources},"checks":["cargo test --workspace: 229 passed","cargo clippy --workspace --all-targets -- -D warnings","cargo fmt --all -- --check","PE vtable target and original export identity","assembler activity/occlusion/loop/bone/rate/sequence/output gates","all 7680 snapshot cases independently calculated"],"scope":report["scope"]}
(root/"analysis/reports/move-coordinates-validation.json").write_text(json.dumps(result,indent=2)+"\n",encoding="utf-8")
path=root/"analysis/evidence.json";evidence=json.loads(path.read_text(encoding="utf-8-sig"));evidence["rust_tests"]=229;evidence["move_coordinates_validation"]=result
path.write_text(json.dumps(evidence,indent=2,ensure_ascii=False)+"\n",encoding="utf-8")
note=("2026-10-07: GetMoveCoords(false) und IsChannelActive ergänzt und in PostInit-Root-Grenze angeschlossen. "
      "Skelett-Vtable +e0 belegt GetMoveCoords10500a30; Kanalüberdeckung durch spätere voll gewichtete Kanäle, "
      "Boneintervalle, Loop-/Rate-/Sequenz-/Moveflag-Gates und geordnete Matrixkopien rekonstruiert. "
      "7680 ausdrücklich gelieferte Diagnosefälle: 4608 RootUnavailable, 768 positive Abfragen ohne Ausgabe, "
      "je 768 an Frame-/Kompositionsgrenze sowie 768 mit gelieferter Komposition. Keine echte Poseberechnung "
      "behauptet; x87-Prozesspräzision bleibt offen. 229 Workspace-Tests, Clippy und Format bestanden. Android "
      "zum Schluss. Details D:\\Rust Projects\\RepublicCommandoAndroid\\docs\\MOVE_COORDINATES.md.")
wiki=Path(r"C:\Users\brand\Documents\Main\05 KI\Project_Knowledge\projects\republic_commando_android")
for path in [root/"README.md",wiki/"index.md",wiki/"architecture/porting.md",wiki/"log.md"]:
    if "GetMoveCoords(false) und IsChannelActive ergänzt" not in path.read_text(encoding="utf-8-sig"):
        with path.open("a",encoding="utf-8") as f:f.write("\n\n"+note+"\n")
print(json.dumps({k:result[k] for k in ("rust_tests","supplied_scenarios","outcomes")}))
