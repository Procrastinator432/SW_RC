"""Independent mesh entry/channel checks and original PE/vtable evidence."""
from pathlib import Path
from collections import Counter
import hashlib
import json
import struct

root=Path(__file__).resolve().parents[1]
binary=Path(r"D:\SteamLibrary\steamapps\common\Star Wars Republic Commando\GameData\System\engine.dll")
data=binary.read_bytes()
pe=struct.unpack_from("<I",data,60)[0]; optional=pe+24
assert data[:2]==b"MZ" and data[pe:pe+4]==b"PE\0\0"
assert struct.unpack_from("<H",data,optional)[0]==0x10b
base=struct.unpack_from("<I",data,optional+28)[0]
size=struct.unpack_from("<H",data,pe+20)[0]
sections=[struct.unpack_from("<IIII",data,optional+size+i*40+8) for i in range(struct.unpack_from("<H",data,pe+6)[0])]
def read(address,n):
    offsets=[raw+address-base-start for _,start,size,raw in sections if start<=address-base and address-base+n<=start+size]
    assert len(offsets)==1
    return data[offsets[0]:offsets[0]+n]

vtables={
    "ULodMeshInstance":(0x1066a2b0,{0x9c:0x103b0730,0xb0:0x10451100,0xf8:0x103b05f0,0x104:0x103b0780}),
    "USkeletalMeshInstance":(0x1066a5c8,{0x9c:0x103b0730,0xb0:0x10451100,0xf8:0x10509540,0x104:0x104ffde0}),
    "UVertMeshInstance":(0x1067bd88,{0x9c:0x103b0730,0xb0:0x10451100,0xf8:0x1055f890,0x104:0x103b0780}),
}
exports=json.loads((root/"analysis/reports/binaries/System__engine.dll.json").read_text())["exports"]
export_names={int(e["address"],16):e["name"] for e in exports if e.get("address")}
for name,(address,slots) in vtables.items():
    assert export_names[address]==f"??_7{name}@@6B@"
    for slot,target in slots.items(): assert read(address+slot,4)==struct.pack("<I",target)
assert read(0x103b0450,5)==bytes.fromhex("32c0c20400") # UMeshInstance.PlayAnim false
assert read(0x103b05f0,5)==bytes.fromhex("33c0c20400") # UMeshInstance.GetChannel null
assert read(0x103b0730,4)==bytes.fromhex("8b4158c3") # GetActor +0x58
entry=(root/"analysis/decompiled/mesh-animation-entry.asm").read_text()
for marker in (
    "104511cf SETZ AL", "104511da CALL dword ptr [EDX + 0xac]",
    "104511e5 JNZ 0x10451316", "104511eb CMP dword ptr [EBX],0x0", "104511ee JZ 0x10451316",
    "1045131a CALL dword ptr [EAX + 0x9c]", "1045136e CALL dword ptr [EAX + 0xf8]",
    "1045141a CMP dword ptr [ESI],EDX", "1045160d CALL dword ptr [EAX + 0x104]",
    "10451633 CALL 0x103ba470", "10451638 MOV byte ptr [EDI + 0x61],0x0",
):assert marker in entry,marker
channel=(root/"analysis/decompiled/skeletal-animation-channel.asm").read_text()
for marker in (
    "10509559 JZ 0x10509565", "10509563 JG 0x1050956e", "10509568 MOV EAX,dword ptr [EDX + 0x1b4]",
    "10509590 CMP dword ptr [EDX + -0x4],EDI", "10509597 CMP EBP,dword ptr [EBX + 0xc]",
    "105095ae CALL 0x10500410", "105095f5 JG 0x10509606", "105095fc JG 0x10509606",
    "10509640 CALL dword ptr [0x10650c24]", "10509650 MOV ECX,0x12", "10509655 STOSD.REP ES:EDI",
    "1050965f MOV dword ptr [EAX + 0x8],ECX", "10509662 MOV dword ptr [EAX + 0x3c],EBP",
    "10509676 MOV EDX,dword ptr [ECX + EDX*0x1 + 0x38]", "1050967a LEA ECX,[EDX + EBP*0x1 + 0x1]",
    "10509684 MOV dword ptr [EAX + 0x40],ECX", "1050968b MOV dword ptr [EAX + 0xc],ECX",
):assert marker in channel,marker
bone=(root/"analysis/decompiled/skeletal-bone-match.asm").read_text()
for marker in (
    "1050041d JZ 0x10500499", "10500440 CMP EBP,dword ptr [ESI]",
    "10500451 MOV EBP,dword ptr [EDI + ECX*0x8 + 0x4]", "1050045e JZ 0x1050046a",
    "10500482 CMP dword ptr [ESI],EBP", "10500487 ADD ESI,0x40",
):assert marker in bone,marker

path=root/"analysis/reports/mesh-animation-channels.json"
report=json.loads(path.read_text())
original_path=root/"analysis/reports/animation-calls.json"
original=json.loads(original_path.read_text())["cases"]
scenarios=["MissingSequence","NoActor","NoChannel","CreateRoot","ReuseRoot","InsertAlias","UnknownBone"]
assert len(original)==256 and len(report["probes"])==1792
counts=Counter(); inserted=reused=0
for i,probe in enumerate(report["probes"]):
    source_index=i//7; source=original[source_index]; scenario=scenarios[i%7]
    assert (probe["source_index"],probe["class"],probe["source_case"],probe["scenario"])==(source_index,source["class"],source["source_case"],scenario)
    parameters=json.loads(json.dumps(source["parameters"]))
    if scenario in ("CreateRoot","UnknownBone"):parameters["bone"]={"handle":999,"resolved_index":998}
    if scenario=="InsertAlias":parameters["bone"]={"handle":40,"resolved_index":39};parameters["channel"]=-1
    assert probe["parameters"]==parameters
    bones=[{"name_handle":0 if scenario=="NoChannel" else 10,"word_38":2},{"name_handle":20,"word_38":1},{"name_handle":30,"word_38":0}]
    aliases=[{"name_handle":40,"target_handle":20}]
    assert probe["skeleton"]=={"bones":bones,"aliases":aliases}
    before=[]
    if scenario in ("ReuseRoot","InsertAlias","UnknownBone"):
        words=[0x12345678]*18; words[2]=10; words[3]=source["parameters"]["channel"]&0xffffffff; words[15]=0
        before=[{"words":words}]
    assert probe["before_channels"]==before
    after=json.loads(json.dumps(before)); selection=None
    editor=source_index%2==1; found=scenario!="MissingSequence"; actor=scenario!="NoActor"
    assert (probe["is_editor"],probe["sequence_found"],probe["actor_present"])==(editor,found,actor)
    events=[f"FindSequence(load={str(not editor).lower()})"]
    if not found and parameters["sequence"]["handle"]!=0:
        events.append("SuppliedMissingSequenceWarning"); result={"Ok":"MissingSequence"}
    elif not actor:
        events.append("GetActor");result={"Ok":"NoActor"}
    else:
        events.extend(["GetActor","SkeletalGetChannel"])
        bone_handle=parameters["bone"]["handle"] if parameters["bone"]["handle"] and before else bones[0]["name_handle"]
        existing=next((j for j,c in enumerate(before) if c["words"][2]==bone_handle and c["words"][3]==parameters["channel"]&0xffffffff),None)
        if existing is not None:
            selection={"index":existing,"inserted":False};reused+=1
        elif bone_handle:
            target=next((a["target_handle"] for a in aliases if a["name_handle"]==bone_handle),bone_handle)
            matched=next((j for j,b in enumerate(bones) if b["name_handle"]==target),None)
            if matched is not None:
                def signed(n):return n if n<0x80000000 else n-0x100000000
                position=next((j for j,c in enumerate(after) if (signed(c["words"][3]),signed(c["words"][15]))>(parameters["channel"],matched)),len(after))
                words=[0]*18;words[2]=bone_handle;words[3]=parameters["channel"]&0xffffffff;words[15]=matched;words[16]=(bones[matched]["word_38"]+1+matched)&0xffffffff
                after.insert(position,{"words":words});selection={"index":position,"inserted":True};inserted+=1
        if selection is None:result={"Ok":"NoChannel"}
        else:events.append("ChannelPlaybackBoundary");result={"Err":"unresolved LOD channel playback"}
    assert probe["result"]==result and probe["events"]==events
    assert probe["selection"]==selection and probe["after_channels"]==after
    counts[next(iter(result.values()))]+=1
assert inserted==512 and reused==256
assert dict(counts)=={"MissingSequence":256,"NoActor":256,"NoChannel":512,"unresolved LOD channel playback":768}
sha=lambda p:hashlib.sha256(p.read_bytes()).hexdigest()
sources=["crates/rc-package/src/mesh_animation.rs","crates/rc-inspect/src/bin/rc-mesh-animation-probe.rs","analysis/decompiled/mesh-animation-entry.c","analysis/decompiled/mesh-animation-entry.asm","analysis/decompiled/mesh-animation-channel.c","analysis/decompiled/skeletal-animation-channel.c","analysis/decompiled/skeletal-animation-channel.asm","analysis/decompiled/skeletal-bone-match.c","analysis/decompiled/skeletal-bone-match.asm","scripts/Record-MeshAnimationChannels.py"]
result={"date":"2026-10-07","rust_tests":210,"original_requests":256,"supplied_scenarios":1792,"outcomes":dict(counts),"inserted_channels":inserted,"reused_channels":reused,"vtables":{name:{"address":f"{address:08x}","slots":{f"{slot:x}":f"{target:08x}" for slot,target in slots.items()}} for name,(address,slots) in vtables.items()},"report_sha256":sha(path),"previous_report_sha256":sha(original_path),"engine_dll_sha256":sha(binary),"source_sha256":{s:sha(root/s) for s in sources},"checks":["cargo test --workspace: 210 passed","cargo clippy --workspace --all-targets -- -D warnings","cargo fmt --all -- --check","PE bytes for base stubs/GetActor and three exported vtables","assembler LOD entry, skeletal insertion and one-pass bone aliases","independent full 1792 scenario calculation including all 18 channel words"],"scope":report["scope"]}
(root/"analysis/reports/mesh-animation-channels-validation.json").write_text(json.dumps(result,indent=2)+"\n",encoding="utf-8")
path=root/"analysis/evidence.json"; evidence=json.loads(path.read_text(encoding="utf-8-sig"))
evidence["rust_tests"]=210;evidence["mesh_animation_channel_validation"]=result
path.write_text(json.dumps(evidence,indent=2,ensure_ascii=False)+"\n",encoding="utf-8")
note=("2026-10-07: Mesh-Animationskanäle und LOD-Eintritt ergänzt. Original-DLL-Vtables belegen gemeinsame LOD-PlayAnim-" 
      "Implementierung bei Skeletal/Vert und unterschiedliche GetChannel-Ziele. Sequenz-/Actor-/Kanal-Grenzen in nativer "
      "Reihenfolge, None-Sequenz-Sonderfall und !GIsEditor berücksichtigt. SkeletalGetChannel mit Root-Fallback beim ersten "
      "Kanal, Wiederverwendung, einmaliger Bone-Aliasauflösung und sortierter Einfügung aus 18 Nullworten rekonstruiert. "
      "256 frühere AnimProp-Anforderungen in 1792 ausdrücklich synthetischen Szenarien geprüft: 256 MissingSequence, "
      "256 NoActor, 512 NoChannel, 768 vor LOD-Kanalwiedergabe angehalten. 512 Kanäle eingefügt, 256 wiederverwendet; "
      "keine Originalskelette/Clips/Posen behauptet. 210 Workspace-Tests, Clippy und Format bestanden. "
      "Android weiterhin zum Schluss. Details D:\\Rust Projects\\RepublicCommandoAndroid\\docs\\MESH_ANIMATION_CHANNELS.md.")
wiki=Path(r"C:\Users\brand\Documents\Main\05 KI\Project_Knowledge\projects\republic_commando_android")
for path in [root/"README.md",wiki/"index.md",wiki/"architecture/porting.md",wiki/"log.md"]:
    if "Mesh-Animationskanäle und LOD-Eintritt ergänzt" not in path.read_text(encoding="utf-8-sig"):
        with path.open("a",encoding="utf-8") as f:f.write("\n\n"+note+"\n")
print(json.dumps({k:result[k] for k in ("rust_tests","supplied_scenarios","outcomes","inserted_channels","reused_channels")}))
