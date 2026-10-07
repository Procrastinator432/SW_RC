"""Verify prepared root-only selection and native SSE quaternion matrix outputs."""
from pathlib import Path
from collections import Counter
import hashlib,json,struct
root=Path(__file__).resolve().parents[1]
game=Path(r"D:\SteamLibrary\steamapps\common\Star Wars Republic Commando\GameData\System")
def pe_read(path):
    data=path.read_bytes();pe=struct.unpack_from("<I",data,60)[0];o=pe+24
    base=struct.unpack_from("<I",data,o+28)[0];size=struct.unpack_from("<H",data,pe+20)[0]
    sec=[struct.unpack_from("<IIII",data,o+size+i*40+8) for i in range(struct.unpack_from("<H",data,pe+6)[0])]
    def read(a,n):
        offsets=[raw+a-base-start for _,start,size,raw in sec if start<=a-base and a-base+n<=start+size];assert len(offsets)==1
        return data[offsets[0]:offsets[0]+n]
    return read
engine=pe_read(game/"engine.dll");core=pe_read(game/"core.dll")
assert engine(0x1066a5c8+0xdc,4)==struct.pack("<I",0x1050b330)
assert core(0x10186c18,4)==struct.pack("<f",1.0) and core(0x10186f6c,4)==struct.pack("<f",2.0)
asm=(root/"analysis/decompiled/skeletal-root-frame.asm").read_text()
for marker in (
    "1050a4bb JZ 0x1050a615","1050a526 CALL 0x10500930","1050a547 JNZ 0x1050a5d8",
    "1050a552 JNZ 0x1050a5d8","1050a55d JLE 0x1050a5d8","1050a564 CALL 0x1044faf0",
    "1050a56f TEST dword ptr [ESI + 0x60],0x1fffffff","1050a57b CALL 0x105003d0",
    "1050a587 JL 0x1050a5d8","1050a597 JNC 0x1050a59e","1050a5b5 FILD dword ptr [ESI + 0x10]",
    "1050a5be FMUL float ptr [EBP + 0x8]","1050a5d3 CALL 0x10500fc0",
    "1050a601 CALL dword ptr [0x10650a80]","1050a610 JMP 0x1050b28b",
):assert marker in asm,marker
matrix_asm=(root/"analysis/decompiled/quat-translation-matrix.asm").read_text()
for marker in (
    "101446a4 MULSS XMM5,XMM1","101446a8 MULSS XMM7,XMM5","101446c7 MULSS XMM1,XMM5",
    "1014470e ADDSS XMM0,XMM2","10144715 SUBSS XMM7,XMM0","10144726 ADDSS XMM0,XMM1",
    "1014472a SUBSS XMM1,XMM7","10144799 MOVSS dword ptr [EAX + 0x28],XMM6",
):assert marker in matrix_asm,marker
assert "ApplyAnimation(this,param_1,param_6 == 3)" in (root/"analysis/decompiled/skeletal-get-frame.c").read_text()
f32=lambda x:struct.unpack("<f",struct.pack("<f",x))[0]
bits=lambda x:struct.unpack("<I",struct.pack("<f",x))[0]
value=lambda x:struct.unpack("<f",struct.pack("<I",x))[0]
clone=lambda x:json.loads(json.dumps(x))
def matrix(transform):
    x,y,z,w=map(value,transform["rotation"])
    double=[f32(v*2.0) for v in (x,y,z)]
    xx=f32(x*double[0]);yy=f32(y*double[1]);zz=f32(z*double[2])
    wx=f32(w*double[0]);wy=f32(w*double[1]);wz=f32(w*double[2])
    xy=f32(x*double[1]);xz=f32(x*double[2]);yz=f32(y*double[2])
    m=[0]*16
    for i,v in {0:f32(1.0-f32(zz+yy)),1:f32(wz+xy),2:f32(xz-wy),4:f32(xy-wz),5:f32(1.0-f32(zz+xx)),6:f32(wx+yz),8:f32(wy+xz),9:f32(yz-wx),10:f32(1.0-f32(yy+xx))}.items():m[i]=bits(v)
    m[12:15]=transform["position"];m[15]=bits(1.0);return m
previous_path=root/"analysis/reports/channel-playback.json"
selected=[(i,p) for i,p in enumerate(json.loads(previous_path.read_text())["probes"]) if p["variant"]=="NewSequence"]
path=root/"analysis/reports/skeletal-root-pose.json";report=json.loads(path.read_text())
scenarios=["Blocked","BoneMismatch","EmptyRange","NoSequence","NoTracks","NoLinkup","Sampled","SampleBoundary","LoopZeroRate"]
assert len(selected)==768 and len(report["probes"])==6912 and len(report["matrices"])==4
reference={"rotation":[0,0,0,bits(1)],"position":[bits(1),bits(2),bits(3)]}
counts=Counter()
for i,probe in enumerate(report["probes"]):
    source_index,source=selected[i//9];scenario=scenarios[i%9];index=source["channel_index"]
    assert (probe["source_index"],probe["class"],probe["source_case"],probe["scenario"],probe["channel_index"])==(source_index,source["class"],source["source_case"],scenario,index)
    before=clone(source["after"]["channels"])
    for c in before:c["words"][15]=1;c["words"][14]=0
    w=before[index]["words"];w[15]=int(scenario=="BoneMismatch");w[16]=0 if scenario=="EmptyRange" else 3;w[14]=bits(1);w[11]=bits(0.5);w[7]=bits(0.25)
    if scenario=="LoopZeroRate":w[1]|=1;w[6]=0
    initial={"root":{"rotation":[9]*4,"position":[9]*3},"matrix":[7]*16,"byte_60":1,"byte_61":9}
    assert probe["before"]==initial and probe["reference"]==reference and probe["before_channels"]==before and probe["word_11c"]==int(scenario=="Blocked")
    after_channels=clone(before);after={"root":clone(reference),"matrix":matrix(reference),"byte_60":1,"byte_61":9};events=[];requests=[]
    if scenario not in scenarios[:3]:
        events.append(f"GetSequence({index})");after_channels[index]["words"][17]=123
        if scenario not in ("NoSequence","NoTracks"):
            events.append("GetLinkupRootTrack")
            if scenario!="NoLinkup":
                events.append("GetRotPosBoundary");requests=[{"channel":index,"sequence_token":123,"track":4,"frames":10,"normalized_frame":0.25}]
                after["root"]["position"]=[bits(0.25),bits(4),bits(5)];after["matrix"]=matrix(after["root"])
    if scenario=="SampleBoundary":after["matrix"]=[7]*16;result={"Err":"unresolved track decoding/interpolation"};outcome="UnresolvedTrack"
    elif scenario in ("Sampled","LoopZeroRate"):result={"Ok":1};outcome="SuppliedSampled"
    else:result={"Ok":0};outcome="ReferenceRoot"
    assert (probe["after"],probe["after_channels"],probe["events"],probe["requests"],probe["result"])==(after,after_channels,events,requests,result)
    counts[outcome]+=1
for case in report["matrices"]:assert case["matrix"]==matrix(case["root"])
assert dict(counts)=={"ReferenceRoot":4608,"SuppliedSampled":1536,"UnresolvedTrack":768}
sha=lambda p:hashlib.sha256(p.read_bytes()).hexdigest()
sources=["crates/rc-package/src/skeletal_root_pose.rs","crates/rc-inspect/src/bin/rc-skeletal-root-probe.rs","analysis/decompiled/skeletal-get-frame.c","analysis/decompiled/skeletal-apply-animation.c","analysis/decompiled/skeletal-root-frame.asm","analysis/decompiled/quat-translation-matrix.c","analysis/decompiled/quat-translation-matrix.asm","scripts/Record-SkeletalRootPose.py"]
result={"date":"2026-10-07","rust_tests":234,"prepared_root_scenarios":6912,"matrix_cases":4,"outcomes":dict(counts),"report_sha256":sha(path),"previous_report_sha256":sha(previous_path),"engine_dll_sha256":sha(game/"engine.dll"),"core_dll_sha256":sha(game/"core.dll"),"source_sha256":{s:sha(root/s) for s in sources},"checks":["cargo test --workspace: 234 passed","cargo clippy --workspace --all-targets -- -D warnings","cargo fmt --all -- --check","PE Skelett GetFrame vtable and core matrix constants","root-only native gates and matrix SSE operation order","6912 independent full snapshot calculations and four exact matrix calculations"],"scope":report["scope"]}
(root/"analysis/reports/skeletal-root-pose-validation.json").write_text(json.dumps(result,indent=2)+"\n",encoding="utf-8")
path=root/"analysis/evidence.json";evidence=json.loads(path.read_text(encoding="utf-8-sig"));evidence["rust_tests"]=234;evidence["skeletal_root_pose_validation"]=result
path.write_text(json.dumps(evidence,indent=2,ensure_ascii=False)+"\n",encoding="utf-8")
note=("2026-10-07: Vorbereiteter Root-only-Posezweig und native Quaternion-/Translationsmatrix ergänzt. "
      "GetFrame-Vtable1050b330 leitet Flag3 an ApplyAnimation10509dc0 root-only weiter. Nach ausdrücklich gelieferter "
      "Buffer-/Cache-/Director-Vorbereitung: Referenzroot, aktive Root-Kanäle, Sequenztracks, Linkup, Frameclamp "
      "und GetRotPos-Grenze rekonstruiert; Root-only setzt die Vollposeflags nicht. 6912 Diagnosen: 4608 Referenzroot, "
      "1536 mit gelieferten Samples und 768 vor Track-Decodierung. Vier Quaternionmatrizen bitweise unabhängig "
      "geprüft, keine Normalisierung/Originalposes behauptet. 234 Workspace-Tests, Clippy und Format bestanden. "
      "Android zum Schluss. Details D:\\Rust Projects\\RepublicCommandoAndroid\\docs\\SKELETAL_ROOT_POSE.md.")
wiki=Path(r"C:\Users\brand\Documents\Main\05 KI\Project_Knowledge\projects\republic_commando_android")
for path in [root/"README.md",wiki/"index.md",wiki/"architecture/porting.md",wiki/"log.md"]:
    if "Vorbereiteter Root-only-Posezweig und native Quaternion-/Translationsmatrix ergänzt" not in path.read_text(encoding="utf-8-sig"):
        with path.open("a",encoding="utf-8") as f:f.write("\n\n"+note+"\n")
print(json.dumps({k:result[k] for k in ("rust_tests","prepared_root_scenarios","matrix_cases","outcomes")}))
