"""Independent GetSequence state oracle and original GetFrame branch evidence."""
from pathlib import Path
import json,hashlib,re,collections
root=Path(__file__).resolve().parents[1];sha=lambda p:hashlib.sha256(p.read_bytes()).hexdigest()
path=root/'analysis/reports/sequence-lookups.json';report=json.loads(path.read_text())
bindings=[[n,s+n] for n in range(0,64,2) for s in [100,200]];assert report['bindings']==bindings
seen=set();stats=collections.Counter()
for c in report['cases']:
    n,e,cached=c['name'],c['editor'],c['cached'];key=(n,e,cached);assert key not in seen;seen.add(key)
    words=[0x12340000+i for i in range(18)];words[0]=n;words[17]=cached;assert c['before']=={'words':words}
    expected=next((s for name,s in bindings if name==n),0) if e else cached
    words[17]=expected;assert c['after']=={'words':words} and c['result']=={'Ok':expected}
    assert c['calls']==([[n,False]] if e else [])
    stats['sequence_cases']+=1;stats['editor_lookups']+=e;stats['cached_reads']+=not e;stats['editor_null_results']+=e and expected==0
assert seen=={(n,e,c) for n in range(64) for e in [False,True] for c in [0,1,0xffffffff,0xdeadbeef]}
asm=(root/'analysis/decompiled/animation-replication-support.asm').read_text()
for marker in ['1044fb06 JZ 0x1044fb1b','1044fb0f PUSH 0x0','1044fb12 CALL dword ptr [EDX + 0xac]','1044fb18 MOV dword ptr [ESI + 0x44],EAX','1044fb1b MOV EAX,dword ptr [ESI + 0x44]']:assert marker in asm
frame=(root/'analysis/decompiled/skeletal-root-frame.asm').read_text()
for marker in ['1050b338 CMP EAX,0x3','1050b33b SETZ AL','1050b33e PUSH EAX','1050b340 CALL 0x10509dc0']:assert marker in frame
log=(root/'analysis/reports/frame-dispatch-tests.log').read_text();assert sum(map(int,re.findall(r'test result: ok\. (\d+) passed',log)))==378 and 'test result: FAILED' not in log
for name in ['frame_type_three_selects_root_before_cache_and_skips_full_hosts','all_other_frame_types_select_full_with_one_preparation','frame_dispatch_preserves_preparation_failure_for_both_branches']:assert f'{name} ... ok' in log
sources=['crates/rc-package/src/skeletal_animation_entry.rs','crates/rc-package/src/skeletal_animation_entry_tests.rs','crates/rc-package/src/skeletal_sequence.rs','crates/rc-inspect/src/bin/rc-sequence-check.rs','scripts/Record-FrameDispatch.py','analysis/decompiled/skeletal-root-frame.asm','analysis/decompiled/animation-replication-support.c','analysis/decompiled/animation-replication-support.asm']
result={'date':'2026-10-08','rust_tests':378,'scope':'Original GetFrame selects root-only exactly for frame type 3, otherwise the verified full entry, with preparation once in selected branch. Original GetSequence cached/noneditor and editor virtual lookup/writeback over supplied opaque binding snapshots. Concrete actor/runtime sequence registry and pose-host integration remain open.','counts':dict(stats),'tested_frame_types':[3,-1,0,1,2,4,-2147483648,2147483647],'source_sha256':{s:sha(root/s) for s in sources},'report_sha256':sha(path),'checks':['cargo test --workspace: 378 passed','cargo clippy --workspace --all-targets -- -D warnings','cargo fmt --all -- --check','512 GetSequence complete channel states and lookup events independently checked','eight frame types integration-tested with only selected branch hosts, preparation exactly once and root-before-cache behavior','preparation failure propagation tested for root/full branches','GetFrame and GetSequence original ASM markers checked','four sequence tests cover raw cached handles, editor null result, host error write ordering, invalid indices and duplicate supplied bindings']}
(root/'analysis/reports/frame-dispatch-validation.json').write_text(json.dumps(result,indent=2)+'\n',encoding='utf-8')
evidence=root/'analysis/evidence.json';e=json.loads(evidence.read_text(encoding='utf-8-sig'));e['rust_tests']=378;e['frame_dispatch_validation']=result;evidence.write_text(json.dumps(e,indent=2,ensure_ascii=False)+'\n',encoding='utf-8')
note=('2026-10-08: Gemeinsame GetFrame-Zweigauswahl (Typ 3 Root-only, sonst Vollpose) und GetSequence-Kanalcache mit Editor-Aktualisierung rekonstruiert. '
      'Acht Frame-Typen integriert getestet; 512 Sequenzzustaende unabhaengig geprueft. Vorbereitung genau einmal im gewaehlten Zweig. '
      '378 Tests, Clippy und Format bestanden. Sequenz-Bindings als Snapshots, konkrete Actor-/Pose-Runtime-Hosts und Skinning offen. Android zum Schluss. '
      'Details D:\\Rust Projects\\RepublicCommandoAndroid\\docs\\SKELETAL_FRAME_DISPATCH.md.')
wiki=Path(r'C:\Users\brand\Documents\Main\05 KI\Project_Knowledge\projects\republic_commando_android')
for p in [root/'README.md',wiki/'index.md',wiki/'architecture/porting.md',wiki/'log.md']:
    if 'Gemeinsame GetFrame-Zweigauswahl (Typ 3 Root-only, sonst Vollpose)' not in p.read_text(encoding='utf-8-sig'):
        with p.open('a',encoding='utf-8') as stream:stream.write('\n\n'+note+'\n')
print(json.dumps(dict(stats)))
