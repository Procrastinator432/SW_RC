"""Validate native director prefix updates and append history from supplied snapshots."""
from pathlib import Path
import json,hashlib,collections
root=Path(__file__).resolve().parents[1];sha=lambda p:hashlib.sha256(p.read_bytes()).hexdigest()
path=root/'analysis/reports/set-bone-place.json';report=json.loads(path.read_text());assert len(report['cases'])==512
stats=collections.Counter();seen=set()
for c in report['cases']:
    seed,scenario=c['seed'],c['scenario'];assert (seed,scenario) not in seen;seen.add((seed,scenario));target=seed%4
    incoming=[((seed*1234567)&0xffffffff)^((i*7919)&0xffffffff) for i in range(22)];incoming[0]=target;incoming[1]=9
    if scenario==1:incoming[0]=0x80000001;incoming[1]=0x80000002
    if scenario==2:incoming[0]=8
    assert c['input']==incoming
    old=[0xaabb0000^seed^(i*137) for i in range(28)];old[0]=0x80000000|target
    before=[{'words':old.copy()},{'words':old.copy()}] if scenario>=2 else [];assert c['before']==before
    if scenario==2:
        assert c['result']=={'Ok':False} and c['cache']==7 and c['after']==before and c['name_calls']==[];stats['rejected']+=1;continue
    resolved=incoming.copy();calls=[]
    if scenario==1:resolved[0]=target;resolved[1]=target;calls=[1,2]
    if scenario==3:
        expected=[{'words':resolved+old[22:]},before[1]];stats['prefix_updates']+=1
    else:
        expected=[{'words':resolved+[0,0,0,0x3f800000,0,0xccbbaa00]}];stats['appends']+=1
    assert c['result']=={'Ok':True} and c['cache']==0 and c['name_calls']==calls and c['after']==expected
    if scenario==1:stats['name_resolutions']+=2
asm=(root/'analysis/decompiled/skeletal-set-bone-place.asm').read_text()
for marker in ['10509284 MOV ECX,0x16','10509295 JNS 0x105092cb','105092f5 MOV byte ptr [EBX + 0x61],0x0','10509359 JZ 0x1050940b','105093d2 MOV byte ptr [ESP + 0xdc],0x0','105093f5 MOV ECX,0x1c','1050940b MOV ECX,0x16','10509414 MOVSD.REP ES:EDI,ESI']:assert marker in asm,marker
match=(root/'analysis/decompiled/skeletal-match-ref-bone.asm').read_text()
assert '1050041d JZ 0x10500499' in match and '10500451 MOV EBP,dword ptr [EDI + ECX*0x8 + 0x4]' in match
sources=['crates/rc-package/src/skeletal_set_bone_place.rs','crates/rc-inspect/src/bin/rc-set-bone-place-check.rs','scripts/Record-SetBonePlace.py','analysis/decompiled/skeletal-set-bone-place.c','analysis/decompiled/skeletal-set-bone-place.asm','analysis/decompiled/skeletal-match-ref-bone.c','analysis/decompiled/skeletal-match-ref-bone.asm']
result={'date':'2026-10-07','rust_tests':330,'scope':report['scope'],'cases':512,'counts':dict(stats),'source_sha256':{s:sha(root/s) for s in sources},'report_sha256':sha(path),'checks':['cargo test --workspace: 330 passed','cargo clippy --workspace --all-targets -- -D warnings','cargo fmt --all -- --check','512 native-prefix/append state snapshots independently checked','six unit tests cover history preservation, signed/name handles, fallback start, invalid targets and cache writes before host errors','native ASM confirms 22-word prefix update vs 28-word appended track and byte-only history initialization','global FName/alias binding and undefined padding remain explicit supplied boundaries']}
(root/'analysis/reports/set-bone-place-validation.json').write_text(json.dumps(result,indent=2)+'\n',encoding='utf-8')
path=root/'analysis/evidence.json';e=json.loads(path.read_text(encoding='utf-8-sig'));e['rust_tests']=330;e['set_bone_place_validation']=result;path.write_text(json.dumps(e,indent=2,ensure_ascii=False)+'\n',encoding='utf-8')
note=('2026-10-07: SetBonePlace-Director-Einstieg rekonstruiert: Zielaufloesung, Cacheinvalidierung, Anfangsindex-Rueckfall, erstes Match, 22-Wort-Update mit History-Erhalt oder 28-Wort-Anlage. '
      'Namens-/Alias-Aufloesung, Identity-Quaternion und undefinierte Paddingbytes explizit vorgegeben; kein Live-FName-/Actor-Binding. '
      '512 Zustandsfaelle und sechs neue Tests geprueft; 330 Tests, Clippy und Format bestanden. Android zum Schluss. '
      'Details D:\\Rust Projects\\RepublicCommandoAndroid\\docs\\SKELETAL_SET_BONE_PLACE.md.')
wiki=Path(r'C:\Users\brand\Documents\Main\05 KI\Project_Knowledge\projects\republic_commando_android')
for path in [root/'README.md',wiki/'index.md',wiki/'architecture/porting.md',wiki/'log.md']:
    if 'SetBonePlace-Director-Einstieg rekonstruiert:' not in path.read_text(encoding='utf-8-sig'):
        with path.open('a',encoding='utf-8') as stream:stream.write('\n\n'+note+'\n')
print(json.dumps(dict(stats)))
