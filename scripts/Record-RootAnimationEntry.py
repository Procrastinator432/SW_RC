"""Independent original-archive root sampling, preparation and matrix oracle."""
from pathlib import Path
import json,hashlib,re,math,collections
root=Path(__file__).resolve().parents[1];sha=lambda p:hashlib.sha256(p.read_bytes()).hexdigest()
script=root/'scripts/Record-OriginalChannelPoses.py';ns={'__file__':str(script)}
exec(compile(script.read_text().split('max_rotation=max_position_relative=')[0],str(script),'exec'),ns)
links,animations,tracks,f,bits,value,matrix,sample=[ns[k] for k in ('links','animations','tracks','f','bits','value','matrix','sample')]
path=root/'analysis/reports/root-animation-entry.json';report=json.loads(path.read_text())
inverse_path=root/'analysis/reports/reference-caches.json';inverse=json.loads(inverse_path.read_text());assert sha(inverse_path)==json.loads((root/'analysis/reports/reference-caches-validation.json').read_text())['report_sha256']
inverse_map={o['source_index']:o['inverse'] for o in inverse['objects']}
assert report['cases']==1575 and len(report['objects'])==130
stats=collections.Counter();max_rotation=0.;max_position=0.;seen=set()
for o in report['objects']:
    index=o['source_index'];assert index not in seen;seen.add(index);original=links['objects'][index];bones=original['prefix']['bones'];n=len(bones);built=False
    assert len(o['runs'])==len(original['mappings'])
    for run,mapping in zip(o['runs'],original['mappings']):
        assert (run['animation'],run['entry'],run['sequence'],run['track'])==(mapping['animation'],mapping['entry'],0,mapping['mapping'][0])
        a=animations[run['animation']];ts=tracks(a,0);frames=a['sequences'][0]['metadata']['frames'];track=run['track']
        assert run['inverse']==inverse_map[index] and len(run['cases'])==7
        for scenario,c in enumerate(run['cases']):
            assert c['scenario']==scenario and c['bone_count']==n and c['tails_zero'] is True
            frame=[0.,.25,.75,1.,-1.,math.nan,.25][scenario];normalized=0. if scenario in [0,4,5] else frame
            before=[0]*18;before[7]=bits(frame);before[11]=bits(.5);before[14]=bits(1.);before[16]=n
            assert c['before']==c['after']=={'words':before}
            sampled=scenario!=6 and bool(ts) and track>=0
            prep={'buffers':{'local_resized':scenario==0,'matrices_resized':scenario==0},'inverse_built':not built,'linkups':len(original['mappings'])}
            assert c['result']=={'preparation':prep,'sampled':int(sampled)}
            if not built:stats['inverse_builds']+=1
            built=True
            assert c['bounds']=={'minimum':[99]*3,'maximum':[100]*3,'sphere':[123]*4,'byte_60':7,'byte_61':0 if scenario==0 else 7,'byte_179':9}
            assert c['preparation_events']==['transform']+[f'linkup {i}' for i in range(len(original['mappings']))]
            if sampled:
                time=f(frames*normalized)
                assert c['time_bits']==[bits(time)]
                assert c['requests']==[{'channel':0,'sequence_token':0,'track':track,'frames':frames,'normalized_frame':normalized}]
                q,p=sample(ts[track],time);actualq=list(map(value,c['root']['rotation']));actualp=list(map(value,c['root']['position']))
                error=max(abs(x-y) for x,y in zip(q,actualq));max_rotation=max(max_rotation,error);assert error<2e-6
                error=max(abs(x-y)/max(1.,abs(x)) for x,y in zip(p,actualp));max_position=max(max_position,error);assert error<2e-5
                assert all(math.isfinite(v) for v in actualq+actualp)
                stats['sampled_roots']+=1
            else:
                assert c['requests']==c['time_bits']==[]
                assert c['root']=={'rotation':bones[0]['rotation'],'position':bones[0]['position']};stats['reference_roots']+=1
            assert c['matrix']==matrix(c['root']['rotation'],c['root']['position'])
            stats['cases']+=1;stats['transform_host_calls']+=1;stats['linkup_host_calls']+=len(original['mappings'])
        stats['linkup_runs']+=1
assert seen==set(inverse_map) and stats['cases']==1575 and stats['linkup_runs']==225 and stats['inverse_builds']==125
asm=(root/'analysis/decompiled/skeletal-root-frame.asm').read_text()
for marker in ['1050a4bb JZ 0x1050a615','1050a4d5 MOV dword ptr [ECX],ESI','1050a5d3 CALL 0x10500fc0','1050a60e MOVSD.REP ES:EDI,ESI','1050a610 JMP 0x1050b28b']:assert marker in asm
log=(root/'analysis/reports/root-entry-tests.log').read_text();assert sum(map(int,re.findall(r'test result: ok\. (\d+) passed',log)))==371 and 'test result: FAILED' not in log
sources=['crates/rc-package/src/skeletal_animation_entry.rs','crates/rc-package/src/skeletal_animation_entry_tests.rs','crates/rc-package/src/skeletal_root_pose.rs','crates/rc-inspect/src/bin/rc-root-entry-check.rs','scripts/Record-RootAnimationEntry.py','scripts/Record-OriginalChannelPoses.py','analysis/decompiled/skeletal-root-frame.asm']
result={'date':'2026-10-08','rust_tests':371,'scope':report['scope'],'counts':dict(stats),'max_rotation_error':max_rotation,'max_position_relative_error':max_position,'source_sha256':{s:sha(root/s) for s in sources},'report_sha256':sha(path),'inverse_report_sha256':sha(inverse_path),'checks':['cargo test --workspace: 371 passed','cargo clippy --workspace --all-targets -- -D warnings','cargo fmt --all -- --check','1575 root-entry cases independently sampled from original archive tracks (rotation tolerance 2e-6, position relative tolerance 2e-5)','all root matrices bit-exact against independent original ASM constructor oracle using actual sampled quaternion','preparation events, cache invalidations, unchanged bounds and blocked/reference paths independently checked','CLI guards all nonroot array tails remain zero after cold preparation','six new tests cover cache bypass, cold initialization, root partial errors, preparation errors, empty skeleton and last eligible sample wins']}
(root/'analysis/reports/root-animation-entry-validation.json').write_text(json.dumps(result,indent=2)+'\n',encoding='utf-8')
evidence=root/'analysis/evidence.json';e=json.loads(evidence.read_text(encoding='utf-8-sig'));e['rust_tests']=371;e['root_animation_entry_validation']=result;evidence.write_text(json.dumps(e,indent=2,ensure_ascii=False)+'\n',encoding='utf-8')
note=('2026-10-08: Root-only-Einstieg mit gemeinsamer Animations-Instanzvorbereitung verbunden; Root-Auswertung vor Vollpose-Cachegate, partielle Sampling-Schreibzugriffe und unveraenderte Bounds-Abschlussflags erhalten. '
      '1575 Originaltrack-Faelle in 225 Linkuplaeufen unabhaengig geprueft, inklusive Frame-Clamp, NaN und Blockierung. '
      '371 Tests, Clippy und Format bestanden. Portable x87-/Quaternion-Policy und gelieferte Runtime-Hosts, echtes Actor-Ticking und Skinning offen. Android zum Schluss. '
      'Details D:\\Rust Projects\\RepublicCommandoAndroid\\docs\\SKELETAL_ROOT_ENTRY.md.')
wiki=Path(r'C:\Users\brand\Documents\Main\05 KI\Project_Knowledge\projects\republic_commando_android')
for p in [root/'README.md',wiki/'index.md',wiki/'architecture/porting.md',wiki/'log.md']:
    if 'Root-only-Einstieg mit gemeinsamer Animations-Instanzvorbereitung verbunden' not in p.read_text(encoding='utf-8-sig'):
        with p.open('a',encoding='utf-8') as stream:stream.write('\n\n'+note+'\n')
print(json.dumps(dict(stats)));print('max rotation error',max_rotation,'max position relative error',max_position)
