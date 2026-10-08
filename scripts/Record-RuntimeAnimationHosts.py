"""Independent concrete-host integration oracle over original-verified prior reports."""
from pathlib import Path
import json,hashlib,re,collections
root=Path(__file__).resolve().parents[1];sha=lambda p:hashlib.sha256(p.read_bytes()).hexdigest()
path=root/'analysis/reports/runtime-animation-hosts.json';report=json.loads(path.read_text())
def checked(name,validation):
    p=root/'analysis/reports'/name;assert sha(p)==json.loads((root/'analysis/reports'/validation).read_text())['report_sha256'];return json.loads(p.read_text())
prior=checked('animation-entry.json','animation-entry-validation.json')
roots=checked('root-animation-entry.json','root-animation-entry-validation.json')
links=checked('original-skeletal-linkups.json','original-skeletal-linkups-validation.json')
stats=collections.Counter();assert (report['poses'],report['bone_matrices'],len(report['objects']))==(900,29912,130)
identity=[0x3f800000 if i//4==i%4 else 0 for i in range(16)]
for o,old,oldroot in zip(report['objects'],prior['objects'],roots['objects']):
    index=o['source_index'];assert index==old['source_index']==oldroot['source_index'];assert len(o['runs'])==len(old['runs'])==len(oldroot['runs'])
    source=links['objects'][index];built=False
    if o['runs']:stats['animated_meshes']+=1
    for run,previous,rroot,mapping in zip(o['runs'],old['runs'],oldroot['runs'],source['mappings']):
        assert run['entry']==previous['entry']==mapping['entry'] and run['animation']==previous['animation']==mapping['animation']
        assert run['sequences']==previous['sequences'] and run['inverse']==previous['inverse']
        mesh_names=[b['name']['name'].lower() for b in source['prefix']['bones']];anim_names=mapping['animation_names']
        computed=[anim_names.index(n) if n in anim_names else -1 for n in mesh_names]
        assert run['mapping']==computed==mapping['mapping']
        matched=sum(v>=0 for v in computed)
        for step,(c,p) in enumerate(zip(run['ticks'],previous['ticks'])):
            for key in ('step','previous','scratch_before','scratch_after','local','calls','bounds_before','bounds','published','matrices','mesh_to_world','byte_61'):assert c[key]==p[key],key
            assert c['mesh_to_world']==identity
            for state in ('before','after'):
                expected=[]
                for i,ch in enumerate(p[state]):
                    w=ch['words'].copy();w[0]=102 if i==1 else 101;w[17]=2 if i==1 else 1;expected.append({'words':w})
                assert c[state]==expected
            prep={'buffers':{'local_resized':False,'matrices_resized':step==0},'inverse_built':not built,'linkups':1}
            expected_result={'preparation':prep,'scratch_resized':False,'pose':p['result']['Ok']['pose']}
            assert c['result']=={'Ok':{'Full':expected_result}}
            if not built:stats['inverse_builds']+=1
            built=True
            cached={'preparation':{'buffers':{'local_resized':False,'matrices_resized':False},'inverse_built':False,'linkups':1},'scratch_resized':False,'pose':p['cached']['Ok']['pose']}
            assert c['cached']=={'Ok':{'Full':cached}}
            refreshed={'Rebuilt':{'matched':matched,'warning':matched==0}} if step==0 else 'SameLength'
            events=['Transform',{'Refresh':{'index':0,'result':refreshed}}]
            for channel in c['calls']:
                events += [{'Sequence':{'channel':channel,'identity':2 if channel==1 else 1}},{'Linkup':{'key':7,'index':0}}]
                stats['full_sequence_resolutions']+=1
            events += ['Transform',{'Refresh':{'index':0,'result':'SameLength'}}]
            assert c['events']==events
            stats['full_poses']+=1;stats['matrices']+=len(c['matrices']);stats['cached_repeats']+=1
        r=run['root'];expected_root=rroot['cases'][2]
        assert r['root']==expected_root['root'] and r['matrix']==expected_root['matrix']
        assert r['bounds']==r['bounds_before']==run['ticks'][-1]['bounds'] and r['tails_unchanged'] is True
        root_prep={'buffers':{'local_resized':False,'matrices_resized':False},'inverse_built':False,'linkups':1}
        assert r['result']=={'Root':{'preparation':root_prep,'sampled':expected_root['result']['sampled']}}
        events=['Transform',{'Refresh':{'index':0,'result':'SameLength'}},{'Lookup':{'name':101,'load':False,'result':1}},{'Sequence':{'channel':2,'identity':1}},{'Linkup':{'key':7,'index':0}}]
        if expected_root['time_bits']:
            events+=[{'RootSample':{'track':mapping['mapping'][0],'time_bits':expected_root['time_bits'][0]}}];stats['sampled_roots']+=1
        else:stats['reference_roots']+=1
        assert r['events']==events
        stats['root_evaluations']+=1;stats['editor_sequence_lookups']+=1;stats['rebuilt_linkups']+=1;stats['same_length_refreshes']+=8
        stats['transform_calls']+=9;stats['linkup_runs']+=1
assert stats['full_poses']==900 and stats['matrices']==29912 and stats['cached_repeats']==900 and stats['root_evaluations']==225 and stats['inverse_builds']==125
log=(root/'analysis/reports/runtime-hosts-tests.log').read_text();assert sum(map(int,re.findall(r'test result: ok\. (\d+) passed',log)))==386 and 'test result: FAILED' not in log
sources=['crates/rc-package/src/skeletal_runtime_hosts.rs','crates/rc-package/src/skeletal_runtime_hosts_tests.rs','crates/rc-package/src/skeletal_mesh.rs','crates/rc-package/src/skeletal_sequence.rs','crates/rc-package/src/skeletal_animation_entry.rs','crates/rc-inspect/src/bin/rc-runtime-hosts-check.rs','scripts/Record-RuntimeAnimationHosts.py']
result={'date':'2026-10-08','rust_tests':386,'scope':report['scope'],'completed_tasks':['loaded sequence identities and names connected to root/channel track sampling','real linkup refresh connected to shared mapping state consumed by both pose hosts','common frame entry checked with original tracks for full, cached and editor-root branches'],'counts':dict(stats),'source_sha256':{s:sha(root/s) for s in sources},'report_sha256':sha(path),'prior_full_report_sha256':sha(root/'analysis/reports/animation-entry.json'),'prior_root_report_sha256':sha(root/'analysis/reports/root-animation-entry.json'),'checks':['cargo test --workspace: 386 passed','cargo clippy --workspace --all-targets -- -D warnings','cargo fmt --all -- --check','900 full poses, 29912 matrices, all local/scratch/history/bounds words match prior independent original-track/ASM verification','225 actual linkup mappings independently rebuilt from original bone names; exact first-match and same-length behavior','225 editor root results and matrix words match prior independent original-root-track report','all concrete runtime events independently checked: preparation, shared refresh, cached sequence resolution, editor lookup, linkup selection and root sample time','eight tests cover loaded/null/unknown sequences, editor refresh, shared mappings, stale caches, duplicate/missing linkups and partial rotation writes']}
(root/'analysis/reports/runtime-animation-hosts-validation.json').write_text(json.dumps(result,indent=2)+'\n',encoding='utf-8')
evidence=root/'analysis/evidence.json';e=json.loads(evidence.read_text(encoding='utf-8-sig'));e['rust_tests']=386;e['runtime_animation_hosts_validation']=result;evidence.write_text(json.dumps(e,indent=2,ensure_ascii=False)+'\n',encoding='utf-8')
note=('2026-10-08: Drei Aufgaben abgeschlossen: geladene Sequenzen mit konkreten Root-/Kanal-Trackhosts verbunden, echte Linkup-Aktualisierung in gemeinsamem Mappingzustand angebunden und beide Pfade ueber den gemeinsamen Frame-Einstieg an Originaldaten geprueft. '
      '900 Vollposen / 29912 Matrizen, 900 Cachewiederholungen und 225 Editor-Root-Auswertungen; 225 Mapping-Neuaufbauten unabhaengig bestaetigt. '
      '386 Tests, Clippy und Format bestanden. Identitaeten/Transform diagnostisch vorgegeben, ein Linkup pro Diagnoselauf, keine Actor-/Script-Ticks oder Skinning. Android zum Schluss. '
      'Details D:\\Rust Projects\\RepublicCommandoAndroid\\docs\\SKELETAL_RUNTIME_HOSTS.md.')
wiki=Path(r'C:\Users\brand\Documents\Main\05 KI\Project_Knowledge\projects\republic_commando_android')
for p in [root/'README.md',wiki/'index.md',wiki/'architecture/porting.md',wiki/'log.md']:
    if 'Drei Aufgaben abgeschlossen: geladene Sequenzen mit konkreten Root-/Kanal-Trackhosts verbunden' not in p.read_text(encoding='utf-8-sig'):
        with p.open('a',encoding='utf-8') as stream:stream.write('\n\n'+note+'\n')
print(json.dumps(dict(stats)))
