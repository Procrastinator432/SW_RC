"""Independent entry ordering oracle and cross-check against verified original-track full poses."""
from pathlib import Path
import json,hashlib,re,collections
root=Path(__file__).resolve().parents[1];sha=lambda p:hashlib.sha256(p.read_bytes()).hexdigest()
path=root/'analysis/reports/animation-entry.json';report=json.loads(path.read_text())
prior_path=root/'analysis/reports/full-poses.json';prior=json.loads(prior_path.read_text())
assert sha(prior_path)==json.loads((root/'analysis/reports/full-poses-validation.json').read_text())['report_sha256']
inverse_path=root/'analysis/reports/reference-caches.json';inverse=json.loads(inverse_path.read_text())
assert sha(inverse_path)==json.loads((root/'analysis/reports/reference-caches-validation.json').read_text())['report_sha256']
inverse_map={o['source_index']:o['inverse'] for o in inverse['objects']}
assert (report['poses'],report['bone_matrices'],len(report['objects']))==(900,29912,130)
identity=[0x3f800000 if i//4==i%4 else 0 for i in range(16)]
stats=collections.Counter()
for o,old in zip(report['objects'],prior['objects']):
    for key in ('source_index','object','file'):assert o[key]==old[key]
    assert len(o['runs'])==len(old['runs'])
    built=False;links=len(o['runs'])
    if links:stats['animated_original_meshes']+=1
    for run,previous in zip(o['runs'],old['runs']):
        for key in ('entry','animation','sequences'):assert run[key]==previous[key]
        assert len(run['ticks'])==len(previous['ticks'])==4
        assert run['inverse']==inverse_map[o['source_index']]
        for step,(c,p) in enumerate(zip(run['ticks'],previous['ticks'])):
            for key in ('step','before','after','previous','scratch_before','scratch_after','local','calls','bounds_before','bounds','published','matrices','byte_61'):assert c[key]==p[key],key
            preparation={'buffers':{'local_resized':False,'matrices_resized':step==0},'inverse_built':not built,'linkups':links}
            assert c['result']=={'Ok':{'preparation':preparation,'scratch_resized':False,'pose':p['result']['Ok']}}
            if not built:stats['inverse_builds']+=1;stats['unique_inverse_matrices']+=len(run['inverse'])
            built=True
            assert c['cached']=={'Ok':{'preparation':{'buffers':{'local_resized':False,'matrices_resized':False},'inverse_built':False,'linkups':links},'scratch_resized':False,'pose':p['cached']['Ok']}}
            assert c['preparation_events']==(['transform']+[f'linkup {i}' for i in range(links)])*2
            assert c['mesh_to_world']==identity
            stats['poses']+=1;stats['matrices']+=len(c['matrices']);stats['cached_repeats']+=1
            stats['transform_host_calls']+=2;stats['linkup_host_calls']+=2*links
        stats['instance_runs']+=1
assert stats['poses']==900 and stats['matrices']==29912 and stats['inverse_builds']==stats['animated_original_meshes']==125 and stats['instance_runs']==225
asm=(root/'analysis/decompiled/skeletal-root-frame.asm').read_text()
for marker in ['10509f24 CALL dword ptr [EDX + 0xe8]','10509f3c TEST dword ptr [EBX + 0x284],0x1fffffff','1050a4ae CALL 0x10509430','1050a4bb JZ 0x1050a615','1050a628 JZ 0x1050b28b','1050a648 JGE 0x1050a671','1050a6a6 CALL 0x10501420']:assert marker in asm
log=(root/'analysis/reports/animation-entry-tests.log').read_text();assert sum(map(int,re.findall(r'test result: ok\. (\d+) passed',log)))==365 and 'test result: FAILED' not in log
sources=['crates/rc-package/src/skeletal_animation_entry.rs','crates/rc-package/src/skeletal_animation_entry_tests.rs','crates/rc-package/src/skeletal_full_pose.rs','crates/rc-package/src/skeletal_preparation.rs','crates/rc-inspect/src/bin/rc-animation-entry-check.rs','scripts/Record-AnimationEntry.py','analysis/decompiled/skeletal-root-frame.asm']
result={'date':'2026-10-08','rust_tests':365,'scope':report['scope'],'counts':dict(stats),'source_sha256':{s:sha(root/s) for s in sources},'report_sha256':sha(path),'prior_full_pose_report_sha256':sha(prior_path),'inverse_report_sha256':sha(inverse_path),'checks':['cargo test --workspace: 365 passed','cargo clippy --workspace --all-targets -- -D warnings','cargo fmt --all -- --check','900 entry poses and all 29912 matrices, channel histories, scratch and publication/bounds words match prior independently ASM/track-verified report','inverse arrays match prior independently original-ASM-verified cache report','independent preparation/cache/mesh-cache ownership and ordered host event oracle on every call','nine integration tests cover cold buffers, inverse invalidation, cached preparation, editor, directors, split-array error copyback and unpaired scratch tails']}
(root/'analysis/reports/animation-entry-validation.json').write_text(json.dumps(result,indent=2)+'\n',encoding='utf-8')
evidence=root/'analysis/evidence.json';e=json.loads(evidence.read_text(encoding='utf-8-sig'));e['rust_tests']=365;e['animation_entry_validation']=result;evidence.write_text(json.dumps(e,indent=2,ensure_ascii=False)+'\n',encoding='utf-8')
note=('2026-10-08: Animations-Vollpose-Einstieg mit getrennter Quaternion-/Positionsspeicherung, Instanzvorbereitung, mesh-eigenem Referenzcache und gemeinsamem Scratch verbunden. '
      '900 Originaltrack-Posen / 29912 Matrizen und 900 Cachewiederholungen gegen unabhaengig gepruefte Vorberichte abgeglichen; Vorbereitung erfolgt auch vor Cache-Ruecksprung. '
      '365 Tests, Clippy und Format bestanden. Transform/Linkup-Hosts weiterhin geliefert, Root-only-Verbindung, Actor-Ticking und Skinning offen. Android zum Schluss. '
      'Details D:\\Rust Projects\\RepublicCommandoAndroid\\docs\\SKELETAL_ANIMATION_ENTRY.md.')
wiki=Path(r'C:\Users\brand\Documents\Main\05 KI\Project_Knowledge\projects\republic_commando_android')
for p in [root/'README.md',wiki/'index.md',wiki/'architecture/porting.md',wiki/'log.md']:
    if 'Animations-Vollpose-Einstieg mit getrennter Quaternion-/Positionsspeicherung' not in p.read_text(encoding='utf-8-sig'):
        with p.open('a',encoding='utf-8') as stream:stream.write('\n\n'+note+'\n')
print(json.dumps(dict(stats)))
