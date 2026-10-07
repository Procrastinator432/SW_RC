"""Verify native animation defaults, original request coverage and supplied base gates."""
from pathlib import Path
from collections import Counter
import hashlib
import json
import struct

root=Path(__file__).resolve().parents[1]
game=Path(r"D:\SteamLibrary\steamapps\common\Star Wars Republic Commando\GameData\System")
report_path=root/"analysis/reports/animation-calls.json"
report=json.loads(report_path.read_text())
previous=json.loads((root/"analysis/reports/anim-prop-transitions.json").read_text())
dependencies=json.loads((root/"analysis/reports/anim-prop-dependencies.json").read_text())
functions={f["path"]:f["decoded"] for f in dependencies["functions"]}
for i,(name,fixture) in enumerate((("Actor.PlayAnim","play_anim.json"),("Actor.LoopAnim","loop_anim.json"))):
    assert report["functions"][i]==functions[name]
    assert functions[name]==json.loads((root/"crates/rc-package/tests/fixtures"/fixture).read_text())

def pe_reader(path):
    data=path.read_bytes()
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
    return read

engine=pe_reader(game/"engine.dll"); core=pe_reader(game/"core.dll")
assert engine(0x10653578,4)==struct.pack("<f",1.0)
assert engine(0x10664fc4,4)==struct.pack("<f",-1.0)
# Exact native loads: Rate defaults to 1, StartFrame defaults to -1.
for address,constant in ((0x104f71b0,0x10653578),(0x104f71d9,0x10664fc4),(0x104f8753,0x10653578),(0x104f877c,0x10664fc4)):
    assert engine(address,8)==b"\xf3\x0f\x10\x05"+struct.pack("<I",constant)
assert core(0x1012f350,3)==b"\xc2\x08\x00"
assert core(0x1012f3be,3)==b"\xff\x48\x0c"
assert engine(0x104f72ef,6)==b"\xff\x90\xb0\x01\x00\x00"
assert engine(0x104f88b3,6)==b"\xff\x92\xb0\x01\x00\x00"
asm=(root/"analysis/decompiled/anim-native-wrappers.asm").read_text()
for marker in (
    "104f7191 MOV dword ptr [ESP + 0x18],EBX", "104f7202 MOV dword ptr [ESP + 0x74],EBX",
    "104f8734 MOV dword ptr [ESP + 0x1c],EBX", "104f87a5 MOV dword ptr [ESP + 0x10],EBX",
    "104f7290 MOV byte ptr [ESP + 0x34],BL", "104f8839 MOV byte ptr [ESP + 0x28],0x1",
    "104e969f MOV EAX,dword ptr [ESI + 0xb0]", "104e96b7 TEST AH,0x40",
    "104e96f5 CALL dword ptr [EDX + 0xa0]", "104e96fb MOV ECX,dword ptr [ESI + 0xd4]",
    "104e9707 CALL dword ptr [EAX + 0xb0]",
): assert marker in asm,marker
expected=[]
def animations(expression):
    if expression["operand"].get("kind")=="Native" and expression["operand"]["value"] in (259,260):
        assert len(expression["children"])==2 and expression["children"][-1]["opcode"]==0x16
    for child in expression["children"]: animations(child)
for expression in functions["AnimProp.Invulnerable.BeginState"]["expressions"]: animations(expression)
for cls in previous["anim_prop_transition_probes"]["probes"]:
    for i,case in enumerate(cls["cases"]):
        for request in case["animation_requests"]:
            expected.append((cls["class"],i,case["outcome"],request))
assert len(expected)==len(report["cases"])==256
counts=Counter()
for case,(cls,i,outcome,request) in zip(report["cases"],expected):
    assert (case["class"],case["source_case"],case["source_outcome"],case["request"])==(cls,i,outcome,request)
    assert case["parameters"]=={"sequence":request["name"],"looping":request["kind"]=="LoopAnim","bone":{"handle":0,"resolved_index":0},"channel":0,"float_10":1.0,"rate":1.0,"start_frame":-1.0}
    counts[request["kind"]]+=1
    assert case["base_cases"]==[
        {"mesh_present":False,"object_flags":0,"outcome":{"Ok":"NoMesh"},"events":["LogMissingMesh"]},
        {"mesh_present":False,"object_flags":0x4000,"outcome":{"Ok":"NoMesh"},"events":[]},
        {"mesh_present":True,"object_flags":0,"outcome":{"Err":"unresolved mesh instance playback"},"events":["MeshGetInstance","ActorMeshInstancePlay"]},
    ]
sha=lambda p:hashlib.sha256(p.read_bytes()).hexdigest()
sources=["crates/rc-package/src/animation_call.rs","crates/rc-package/tests/fixtures/play_anim.json","crates/rc-package/tests/fixtures/loop_anim.json","crates/rc-inspect/src/bin/rc-animation-call-probe.rs","analysis/decompiled/anim-native-wrappers.c","analysis/decompiled/anim-native-wrappers.asm","analysis/decompiled/anim-empty-argument.c","analysis/decompiled/anim-empty-argument.asm","scripts/Record-AnimationCalls.py"]
result={"date":"2026-10-07","rust_tests":203,"requests":dict(counts),"supplied_base_scenarios":768,"no_mesh":512,"unresolved_mesh_playback":256,"defaults":{"bone":"None","rate":1.0,"start_frame":-1.0,"channel":0},"report_sha256":sha(report_path),"previous_report_sha256":sha(root/"analysis/reports/anim-prop-transitions.json"),"engine_dll_sha256":sha(game/"engine.dll"),"core_dll_sha256":sha(game/"core.dll"),"engine_u_sha256":sha(game/"engine.u"),"source_sha256":{s:sha(root/s) for s in sources},"checks":["cargo test --workspace: 203 passed","cargo clippy --workspace --all-targets -- -D warnings","cargo fmt --all -- --check","original function signatures and 256 request correspondence","PE constant bytes and native virtual-call instruction bytes","EndFunctionParms decrements Code and leaves parameter locals unchanged"],"scope":report["scope"]+" Native FPlayAnim +0x14 and loop-byte padding are not initialized by these wrappers; no native-layout serialization is claimed."}
(root/"analysis/reports/animation-calls-validation.json").write_text(json.dumps(result,indent=2)+"\n",encoding="utf-8")
path=root/"analysis/evidence.json"; evidence=json.loads(path.read_text(encoding="utf-8-sig"))
evidence["rust_tests"]=203; evidence["animation_call_validation"]=result
path.write_text(json.dumps(evidence,indent=2,ensure_ascii=False)+"\n",encoding="utf-8")
note=("2026-10-07: Native PlayAnim-/LoopAnim-Wrapper ergänzt. Original-UFunction-Signaturen und DLL-Assembler geprüft, "
      "PE-Konstanten belegen Rate 1 und StartFrame -1; Bone None und Channel 0. EndFunctionParms setzt Code zurück, "
      "weshalb ausgelassene optionale Argumente die nativen Initialwerte behalten. 256 vorhandene AnimProp-Anforderungen "
      "aufbereitet; 768 ausdrücklich gelieferte Basisfunktionsszenarien prüfen Mesh-/Log-Grenze und Instanzaufrufreihenfolge. "
      "512 NoMesh, 256 vor offener Mesh-Wiedergabe angehalten; echte Meshes/Clips/Overrides bleiben offen. "
      "FPlayAnim-Feld +0x10 enthält 1, Bedeutung offen; +0x14 und Padding hier uninitialisiert und nicht modelliert. "
      "203 Workspace-Tests, Clippy und Format bestanden. Android zum Schluss. "
      "Details D:\\Rust Projects\\RepublicCommandoAndroid\\docs\\ANIMATION_CALLS.md.")
wiki=Path(r"C:\Users\brand\Documents\Main\05 KI\Project_Knowledge\projects\republic_commando_android")
for path in [root/"README.md",wiki/"index.md",wiki/"architecture/porting.md",wiki/"log.md"]:
    if "Native PlayAnim-/LoopAnim-Wrapper ergänzt" not in path.read_text(encoding="utf-8-sig"):
        with path.open("a",encoding="utf-8") as f:f.write("\n\n"+note+"\n")
print(json.dumps({k:result[k] for k in ("rust_tests","requests","supplied_base_scenarios","defaults")}))
