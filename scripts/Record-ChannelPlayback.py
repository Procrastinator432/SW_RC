"""Check LOD continuation outputs against closed-form diagnostic expectations."""
from pathlib import Path
from collections import Counter
import hashlib
import json
import struct

root=Path(__file__).resolve().parents[1]
binary=Path(r"D:\SteamLibrary\steamapps\common\Star Wars Republic Commando\GameData\System\engine.dll")
data=binary.read_bytes();pe=struct.unpack_from("<I",data,60)[0];optional=pe+24
assert data[:2]==b"MZ" and data[pe:pe+4]==b"PE\0\0"
base=struct.unpack_from("<I",data,optional+28)[0];size=struct.unpack_from("<H",data,pe+20)[0]
sections=[struct.unpack_from("<IIII",data,optional+size+i*40+8) for i in range(struct.unpack_from("<H",data,pe+6)[0])]
def read(a,n):
    offsets=[raw+a-base-start for _,start,size,raw in sections if start<=a-base and a-base+n<=start+size]
    assert len(offsets)==1
    return data[offsets[0]:offsets[0]+n]
constants={0x106739cc:0x3e800000,0x1065efc4:0x3f000000,0x10664f44:0x38000100}
for address,bits in constants.items():assert read(address,4)==struct.pack("<I",bits)
asm=(root/"analysis/decompiled/mesh-animation-entry.asm").read_text()
for marker in (
    "104513ce MOVSS XMM1,dword ptr [0x106739cc]", "104513e7 CVTSI2SS XMM0,dword ptr [ECX + 0x10]",
    "104513ec DIVSS XMM0,dword ptr [ECX + 0x14]", "104513f1 MULSS XMM0,dword ptr [0x1065efc4]",
    "1045140b COMISS XMM2,XMM0", "10451423 JNZ 0x1045145e", "10451431 JP 0x10451441",
    "1045143f JP 0x1045145e", "10451449 JC 0x104515b9", "10451460 MOV dword ptr [ESI],EDX",
    "10451462 MOV dword ptr [ESI + 0x44],ECX", "10451480 DIVSS XMM4,XMM3", "10451484 SUBSS XMM1,XMM4",
    "104514aa CALL dword ptr [0x10650c2c]", "104514b9 MULSS XMM0,dword ptr [0x10664f44]",
    "10451520 JBE 0x10451530", "1045152e JNZ 0x104514d1", "1045155d JZ 0x104514ce",
    "10451566 MOV dword ptr [EAX + 0x28],EBX", "104515f2 JP 0x10451601",
    "1045160d CALL dword ptr [EAX + 0x104]", "10451633 CALL 0x103ba470",
    "10451638 MOV byte ptr [EDI + 0x61],0x0", "1045168f MOV AL,0x1",
):assert marker in asm,marker
# Virtual post-init and direct replication call bytes, preserving native ordering.
assert read(0x1045160d,6)==bytes.fromhex("ff9004010000")
assert read(0x10451633,1)==b"\xe8"
assert 0x10451638+struct.unpack("<i",read(0x10451634,4))[0]==0x103ba470

f32=lambda v:struct.unpack("<f",struct.pack("<f",v))[0]
bits=lambda v:struct.unpack("<I",struct.pack("<f",v))[0]
value=lambda word:struct.unpack("<f",struct.pack("<I",word))[0]
def clone(v):return json.loads(json.dumps(v))
previous_path=root/"analysis/reports/mesh-animation-channels.json"
previous=json.loads(previous_path.read_text())["probes"]
path=root/"analysis/reports/channel-playback.json"
report=json.loads(path.read_text());probes=report["probes"]
selected=[(i,p) for i,p in enumerate(previous) if p["selection"] is not None]
variants=["NewSequence","Reuse","Stopped","ExplicitFrame","LoopContinuity","RandomStart","None","PostInitBoundary","ReplicationBoundary"]
assert len(selected)==768 and len(probes)==6912
outcomes=Counter(); callbacks=Counter(); reset_count=0
for i,probe in enumerate(probes):
    source_index,source=selected[i//9];variant=variants[i%9];idx=source["selection"]["index"]
    assert (probe["source_index"],probe["class"],probe["source_case"],probe["source_scenario"],probe["variant"],probe["channel_index"])==(source_index,source["class"],source["source_case"],source["scenario"],variant,idx)
    parameters=clone(source["parameters"])
    if variant=="ExplicitFrame":parameters["start_frame"]=0.25
    if variant=="LoopContinuity":parameters["looping"]=True
    if variant=="None":parameters["sequence"]={"handle":0,"resolved_index":0}
    assert probe["parameters"]==parameters
    sequence=None if variant=="None" else {"token":123,"frames":10,"rate":40.0,"minimum_blend":0.1,"randomize_start":variant=="RandomStart"}
    # serde's f32 JSON uses a short roundtrip decimal; compare numeric f32 bits.
    if sequence is None:assert probe["sequence"] is None
    else:
        assert set(probe["sequence"])==set(sequence)
        for key,v in sequence.items():
            if isinstance(v,float):assert bits(probe["sequence"][key])==bits(v)
            else:assert probe["sequence"][key]==v
    before={"channels":clone(source["after_channels"]),"byte_60":1,"byte_61":7}
    w=before["channels"][idx]["words"]
    w[0]=source["parameters"]["sequence"]["handle"] if variant in ("Reuse","Stopped") else 338
    w[1]=0xaabbcc01;w[5]=0;w[6]=bits(0 if variant=="Stopped" else 1)
    w[7]=bits(0.3);w[12]=bits(0.4);w[10]=bits(2.0)
    assert probe["before"]==before
    after=clone(before);w=after["channels"][idx]["words"]
    reset=variant!="Reuse";reset_count+=reset
    blend=0.25 if variant=="None" or parameters["looping"] else 0.125
    events=[]
    if reset:
        w[0]=parameters["sequence"]["handle"];w[17]=0 if sequence is None else 123
        if sequence is None:
            w[9]=bits(0)
            for sibling in after["channels"][:idx]:
                sibling["words"][10]=bits(2.0);sibling["words"][13]=bits(0);sibling["words"][11]=bits(0)
        else:
            w[14]=bits(0);w[9]=bits(f32(1.0-f32(1.0/10.0)))
            if variant=="ExplicitFrame":w[7]=w[12]=bits(0.25)
            elif variant=="RandomStart":
                events.append("SuppliedRand16384");w[7]=w[12]=bits(f32(16384.0*value(0x38000100)))
            elif variant=="Stopped" or not parameters["looping"]:w[7]=w[12]=bits(0)
        w[8]=bits(0)
        w[10]=bits(0 if variant=="ExplicitFrame" else f32(1.0/blend))
        w[11]=bits(1 if variant=="ExplicitFrame" else 0);w[13]=w[11]
    w[1]=0xaabbcc00|int(parameters["looping"]);w[6]=bits(1.0);w[4]=bits(0 if sequence is None else 1.0);w[14]=w[4]
    if reset:
        events.append("PostInitAnim")
        if variant!="PostInitBoundary":events.append("ReplicateAnim")
    if variant=="PostInitBoundary":result={"Err":"unresolved PostInitAnim"};outcomes["UnresolvedPostInit"]+=1
    elif variant=="ReplicationBoundary":result={"Err":"unresolved ReplicateAnim"};outcomes["UnresolvedReplication"]+=1
    else:result={"Ok":{"reinitialized":reset,"blend_time":blend}};after["byte_61"]=0;outcomes["SuppliedCompleted"]+=1
    assert probe["result"]==result and probe["events"]==events
    assert probe["after"]==after
    callbacks.update(events)
assert reset_count==6144
assert dict(outcomes)=={"SuppliedCompleted":5376,"UnresolvedPostInit":768,"UnresolvedReplication":768}
sha=lambda p:hashlib.sha256(p.read_bytes()).hexdigest()
sources=["crates/rc-package/src/channel_playback.rs","crates/rc-inspect/src/bin/rc-channel-playback-probe.rs","analysis/decompiled/mesh-animation-entry.c","analysis/decompiled/mesh-animation-entry.asm","scripts/Record-ChannelPlayback.py"]
result={"date":"2026-10-07","rust_tests":217,"previous_selected_scenarios":768,"supplied_playback_scenarios":6912,"reinitialized":reset_count,"reused":768,"outcomes":dict(outcomes),"callbacks":dict(callbacks),"constants":{f"{a:08x}":f"{b:08x}" for a,b in constants.items()},"report_sha256":sha(path),"previous_report_sha256":sha(previous_path),"engine_dll_sha256":sha(binary),"source_sha256":{s:sha(root/s) for s in sources},"checks":["cargo test --workspace: 217 passed","cargo clippy --workspace --all-targets -- -D warnings","cargo fmt --all -- --check","PE constants and native PostInit/Replicate instructions","all 6912 channel snapshots independently calculated word-for-word"],"scope":report["scope"]}
(root/"analysis/reports/channel-playback-validation.json").write_text(json.dumps(result,indent=2)+"\n",encoding="utf-8")
path=root/"analysis/evidence.json";evidence=json.loads(path.read_text(encoding="utf-8-sig"))
evidence["rust_tests"]=217;evidence["channel_playback_validation"]=result
path.write_text(json.dumps(evidence,indent=2,ensure_ascii=False)+"\n",encoding="utf-8")
note=("2026-10-07: LOD-Kanalwiedergabe-Fortsetzung ergänzt. Wiederverwendung und Neuinitialisierung, "
      "Frame-/Rate-/Loop-Schreibreihenfolge, Blendberechnung, Zufallsstart und None-Weitergabe an vorherige Kanäle "
      "anhand Originalassembler rekonstruiert. PostInitAnim vor ReplicateAnim, Fehler behalten vorherige Schreibzugriffe "
      "und verhindern das abschließende Löschen von Instanzbyte61. 6912 ausdrücklich gelieferte Diagnosefälle aus den "
      "768 vorherigen Kanalszenarien: 5376 mit gelieferten Callbacks abgeschlossen, je 768 vor PostInit/Replication "
      "angehalten. Keine echten Clips/Posen/nativen Callbacks behauptet. 217 Workspace-Tests, Clippy und Format "
      "bestanden. Android zum Schluss. Details D:\\Rust Projects\\RepublicCommandoAndroid\\docs\\CHANNEL_PLAYBACK.md.")
wiki=Path(r"C:\Users\brand\Documents\Main\05 KI\Project_Knowledge\projects\republic_commando_android")
for path in [root/"README.md",wiki/"index.md",wiki/"architecture/porting.md",wiki/"log.md"]:
    if "LOD-Kanalwiedergabe-Fortsetzung ergänzt" not in path.read_text(encoding="utf-8-sig"):
        with path.open("a",encoding="utf-8") as f:f.write("\n\n"+note+"\n")
print(json.dumps({k:result[k] for k in ("rust_tests","supplied_playback_scenarios","outcomes","callbacks")}))
