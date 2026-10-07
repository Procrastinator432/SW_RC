"""Verify scalar replication packet writes, gates and explicit callback boundaries."""
from pathlib import Path
from collections import Counter
import hashlib,json,struct

root=Path(__file__).resolve().parents[1]
binary=Path(r"D:\SteamLibrary\steamapps\common\Star Wars Republic Commando\GameData\System\engine.dll")
data=binary.read_bytes();pe=struct.unpack_from("<I",data,60)[0];optional=pe+24
base=struct.unpack_from("<I",data,optional+28)[0];size=struct.unpack_from("<H",data,pe+20)[0]
sections=[struct.unpack_from("<IIII",data,optional+size+i*40+8) for i in range(struct.unpack_from("<H",data,pe+6)[0])]
def read(a,n):
    offsets=[raw+a-base-start for _,start,size,raw in sections if start<=a-base and a-base+n<=start+size]
    assert len(offsets)==1
    return data[offsets[0]:offsets[0]+n]
constants={0x1065efcc:0x00000000,0x1066a17c:0x42f80000,0x1066a178:0xc0800000,0x10668d48:0x40800000,0x1066a174:0x41f80000,0x1066a170:0x42fe0000}
for a,b in constants.items():assert read(a,4)==struct.pack("<I",b)
assert read(0x103b0780,3)==bytes.fromhex("c20400")
asm=(root/"analysis/decompiled/animation-post-replicate.asm").read_text()
for marker in (
    "104ffdee LEA EDI,[ESI + 0x128]","104ffdf5 CALL dword ptr [EAX + 0xe0]","104ffdfd JZ 0x105001d8",
    "104ffe07 CALL dword ptr [EDX + 0x9c]","104ffe16 CALL dword ptr [0x10650244]","104ffe28 CALL dword ptr [EDX + 0xe4]",
    "103ba492 CMP EAX,0x6","103ba49e JGE 0x103ba65e","103ba4ab TEST byte ptr [ESI + 0x6a],0x1",
    "103ba4bb MOV CL,byte ptr [EAX + 0x440]","103ba4cc CMP dword ptr [EDI + 0xc],0x3",
    "103ba4f4 CMP EAX,0x107ac760","103ba508 CALL 0x1044faf0","103ba51e MOV CL,byte ptr [EDI + 0x3c]",
    "103ba534 OR BL,DL","103ba546 MOVSS XMM0,dword ptr [0x1066a17c]",
    "103ba5b4 MULSS XMM0,dword ptr [0x1066a174]","103ba5fe FMUL float ptr [0x1066a170]",
    "103ba63a CMP byte ptr [ESI + EAX*0x8 + 0x242],DL","103ba641 JZ 0x103ba65e",
    "103ba643 OR dword ptr [ESI + 0x68],0x100","103ba650 MOV dword ptr [ESI + EAX*0x8 + 0x23c],ECX",
    "103ba657 MOV dword ptr [ESI + EAX*0x8 + 0x240],EDX",
):assert marker in asm,marker
support=(root/"analysis/decompiled/animation-replication-support.asm").read_text()
for marker in ("1044fb06 JZ 0x1044fb1b","1044fb12 CALL dword ptr [EDX + 0xac]","1044fb18 MOV dword ptr [ESI + 0x44],EAX","10621f03 FISTP qword ptr [ESP + 0x10]","10621f17 FSUBP","10621f47 SBB EAX,0x0"):
    assert marker in support,marker
f=lambda w:struct.unpack("<f",struct.pack("<I",w))[0]
f32=lambda v:struct.unpack("<f",struct.pack("<f",v))[0]
bits=lambda v:struct.unpack("<I",struct.pack("<f",v))[0]
clone=lambda v:json.loads(json.dumps(v))
previous_path=root/"analysis/reports/channel-playback.json"
previous=json.loads(previous_path.read_text())["probes"]
selected=[(i,p) for i,p in enumerate(previous) if p["variant"]=="NewSequence"]
path=root/"analysis/reports/animation-replication.json";report=json.loads(path.read_text())
scenarios=["ActorDisabled","LevelDisabled","ChannelDisabled","SlotHigh","NoNotify","InitialWrite","FrameOnly","NoSequence","FrameBoundary"]
assert len(selected)==768 and len(report["probes"])==6912
empty={"sequence_handle":0,"bone":0,"channel_loop":0,"rate":0,"frame":0}
counts=Counter();events_count=Counter()
def packet(w,sequence):
    rate=124 if f(w[9])==0 else int(f32(f32(max(-4.0,min(4.0,f(w[6])))+4.0)*31.0))
    return {"sequence_handle":sequence or 0,"bone":w[15]&255,"channel_loop":(w[3]&255)|(128 if w[1]&255 else 0),"rate":rate,"frame":int((max(-1.0,min(1.0,f(w[7])))+1.0)*127.0)}
for i,probe in enumerate(report["probes"]):
    source_index,source=selected[i//9];scenario=scenarios[i%9];index=source["channel_index"]
    assert (probe["source_index"],probe["class"],probe["source_case"],probe["scenario"],probe["channel_index"])==(source_index,source["class"],source["source_case"],scenario,index)
    original=clone(source["after"]["channels"][index]);channel=clone(original)
    if scenario in ("ChannelDisabled","NoNotify"):channel["words"][3]=3
    slot=6 if scenario=="SlotHigh" else index;notify=scenario!="NoNotify";level=scenario!="LevelDisabled"
    sequence=None if scenario=="NoSequence" else source["parameters"]["sequence"]["handle"]
    before={"flags_68":0 if scenario in ("ActorDisabled","NoNotify") else 0x10000,"slots":[clone(empty) for _ in range(6)]}
    if scenario=="FrameOnly":before["slots"][slot]=packet(original["words"],sequence);channel["words"][7]=bits(-0.5)
    assert (probe["slot"],probe["notify"],probe["level_enabled"],probe["supplied_sequence"],probe["channel"],probe["before"])==(slot,notify,level,sequence,channel,before)
    after=clone(before);events=[]
    if scenario=="SlotHigh":outcome="SlotOutOfRange"
    elif scenario=="ActorDisabled":outcome="ActorDisabled"
    elif scenario=="LevelDisabled":events=["LevelAnimationEnabled"];outcome="LevelDisabled"
    elif scenario=="ChannelDisabled":events=["LevelAnimationEnabled"];outcome="ChannelDisabled"
    else:
        if notify:events.append("LevelAnimationEnabled")
        events.extend([f"LodGetSequence({index})","FrameQuantizer"])
        outcome="UnresolvedFrameQuantizer" if scenario=="FrameBoundary" else "Unchanged" if scenario=="FrameOnly" else "Written"
        if outcome=="Written":
            after["slots"][slot]=packet(channel["words"],sequence)
            if notify:after["flags_68"]|=0x100
    result={"Err":"unresolved x87 frame quantization"} if outcome=="UnresolvedFrameQuantizer" else {"Ok":outcome}
    assert probe["result"]==result and probe["after"]==after and probe["events"]==events
    counts[outcome]+=1;events_count.update(events)
assert counts["Written"]==2304 and counts["Unchanged"]==768 and counts["UnresolvedFrameQuantizer"]==768
assert report["post_init"]==[
    {"root":False,"supplied_composition":False,"matrix":[7]+[0]*15,"result":{"Ok":"RootUnavailable"},"events":["GetRootLocation"]},
    {"root":True,"supplied_composition":False,"matrix":[7]+[0]*15,"result":{"Err":"unresolved inverse/actor matrix composition"},"events":["GetRootLocation","InverseActorCompose"]},
    {"root":True,"supplied_composition":True,"matrix":[9]*16,"result":{"Ok":"Updated"},"events":["GetRootLocation","InverseActorCompose"]},
]
sha=lambda p:hashlib.sha256(p.read_bytes()).hexdigest()
sources=["crates/rc-package/src/animation_replication.rs","crates/rc-inspect/src/bin/rc-animation-replication-probe.rs","analysis/decompiled/animation-post-replicate.c","analysis/decompiled/animation-post-replicate.asm","analysis/decompiled/animation-replication-support.c","analysis/decompiled/animation-replication-support.asm","scripts/Record-AnimationReplication.py"]
result={"date":"2026-10-07","rust_tests":223,"replication_scenarios":6912,"post_init_scenarios":3,"outcomes":dict(counts),"callbacks":dict(events_count),"constants":{f"{a:08x}":f"{b:08x}" for a,b in constants.items()},"report_sha256":sha(path),"previous_report_sha256":sha(previous_path),"engine_dll_sha256":sha(binary),"source_sha256":{s:sha(root/s) for s in sources},"checks":["cargo test --workspace: 223 passed","cargo clippy --workspace --all-targets -- -D warnings","cargo fmt --all -- --check","original assembler replication gates, packet equality fields and dirty flag","PE quantization constants and PostInit base return","all 6912 actor slot/flag snapshots independently calculated","three post-init root/matrix boundary scenarios"],"scope":report["scope"]}
(root/"analysis/reports/animation-replication-validation.json").write_text(json.dumps(result,indent=2)+"\n",encoding="utf-8")
path=root/"analysis/evidence.json";evidence=json.loads(path.read_text(encoding="utf-8-sig"));evidence["rust_tests"]=223;evidence["animation_replication_validation"]=result
path.write_text(json.dumps(evidence,indent=2,ensure_ascii=False)+"\n",encoding="utf-8")
note=("2026-10-07: Animationsreplikations-Skalarablauf und Skeletal-PostInit-Gate ergänzt. Originalassembler belegt "
      "Slot-/Actor-/Level-/Kanal-Gates, Rate-/Frame-Clamps, Paketbytes und Dirtyflag. Frame-only-Änderungen werden "
      "beim notify-Aufruf bewusst nicht geschrieben; notify=false umgeht die Gates und schreibt ohne Dirtymarkierung. "
      "6912 gelieferte Replikationsfälle: 2304 Written, 768 Unchanged, je 768 Slot-/Actor-/Level-/Channel-Abbruch "
      "sowie 768 an offener x87-Framekonvertierung. PostInit prüft Root-Aufruf und erhält Teilschreibzugriffe; "
      "Matrixinverse/-komposition bleiben Hostgrenze. Diagnose nutzt ausdrücklich gelieferte binary64-Quantisierung, "
      "keine native x87-Präzision behauptet. 223 Workspace-Tests, Clippy und Format bestanden. "
      "Android zum Schluss. Details D:\\Rust Projects\\RepublicCommandoAndroid\\docs\\ANIMATION_REPLICATION.md.")
wiki=Path(r"C:\Users\brand\Documents\Main\05 KI\Project_Knowledge\projects\republic_commando_android")
for path in [root/"README.md",wiki/"index.md",wiki/"architecture/porting.md",wiki/"log.md"]:
    if "Animationsreplikations-Skalarablauf und Skeletal-PostInit-Gate ergänzt" not in path.read_text(encoding="utf-8-sig"):
        with path.open("a",encoding="utf-8") as f:f.write("\n\n"+note+"\n")
print(json.dumps({k:result[k] for k in ("rust_tests","replication_scenarios","outcomes")}))
