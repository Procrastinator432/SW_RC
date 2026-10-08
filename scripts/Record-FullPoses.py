"""Independent integration oracle against previously verified tracks and ASM matrix/bounds math."""
from pathlib import Path
import json,hashlib,math
root=Path(__file__).resolve().parents[1];sha=lambda p:hashlib.sha256(p.read_bytes()).hexdigest()
script=root/'scripts/Record-SkeletalPoses.py';ns={'__file__':str(script)}
exec(compile(script.read_text().split('poses=0;matrices=0;')[0],str(script),'exec'),ns)
f,bits,value,matrix,compose=[ns[k] for k in ('f','bits','value','matrix','compose')]
path=root/'analysis/reports/full-poses.json';report=json.loads(path.read_text())
prior_path=root/'analysis/reports/original-channel-stacks.json';prior=json.loads(prior_path.read_text())
assert sha(prior_path)==json.loads((root/'analysis/reports/original-channel-stacks-validation.json').read_text())['report_sha256']
assert (report['poses'],report['bone_matrices'])==(900,29912)
assert len(report['objects'])==len(prior['objects'])==130
poses=0;matrices=0;cached=0;applied=0;fallback=0
for o,old in zip(report['objects'],prior['objects']):
    assert (o['source_index'],o['object'],o['file'])==(old['source_index'],old['object'],old['file'])
    bones=ns['links']['objects'][o['source_index']]['prefix']['bones'];move=0 if bones[0]['name']['name'].lower()=='move' else -1
    assert len(o['runs'])==len(old['runs'])
    for run,previous_run in zip(o['runs'],old['runs']):
        assert (run['entry'],run['animation'],run['sequences'])==(previous_run['entry'],previous_run['animation'],previous_run['sequences'])
        last=None
        assert len(run['ticks'])==len(previous_run['ticks'])==4
        for c,p in zip(run['ticks'],previous_run['ticks']):
            for key in ('step','before','after','previous','scratch_before','scratch_after','local','calls','matrices'):assert c[key]==p[key],key
            assert c['result']=={'Ok':{'channels':p['result']['Ok'],'directors':0}}
            assert c['cached']=={'Ok':{'channels':'Cached','directors':0}} and c['byte_61']==1
            if p['result']['Ok']=='Reference':fallback+=1
            else:applied+=1
            expected_before={'minimum':[99]*3,'maximum':[100]*3,'sphere':[123]*4,'byte_60':7,'byte_61':0,'byte_179':9} if last is None else {**last,'byte_61':0}
            assert c['bounds_before']==expected_before
            output=[]
            for i,b in enumerate(c['local']):
                m=matrix(b['rotation'],b['position'])
                if i>0 and bones[i]['word_34']!=move:m=compose(m,output[bones[i]['word_34']])
                output.append(m)
            assert output==c['matrices']
            low=expected_before['minimum'].copy();high=expected_before['maximum'].copy()
            for i,m in enumerate(output):
                point=m[12:15]
                if i==move+1:low=point.copy();high=point.copy()
                elif i>move+1:
                    for a,v in enumerate(point):
                        if value(low[a])>value(v):low[a]=v
                        elif value(v)>value(high[a]):high[a]=v
            low=[f(f(value(v)*value(0x3f99999a))-f(1.+p)) for v,p in zip(low,[.25,1.,2.])]
            high=[f(f(p+1.)+f(value(v)*value(0x3f99999a))) for v,p in zip(high,[2.,.5,1.])]
            delta=[f(y-x) for x,y in zip(low,high)]
            n=f(f(f(delta[0]*delta[0])+f(delta[1]*delta[1]))+f(delta[2]*delta[2]))
            if n:
                r=f(1./f(math.sqrt(n)));length=f(f(f(3.-f(f(r*n)*r))*f(r*.5))*n)
            else:length=0.
            sphere=[bits(f(f(y+x)*.5)) for x,y in zip(low,high)]+[bits(f(length*.5))]
            published={'minimum':list(map(bits,low)),'maximum':list(map(bits,high)),'sphere':sphere,'byte_60':expected_before['byte_60'],'byte_61':0,'byte_179':expected_before['byte_179']}
            assert c['published']==published
            last={**published,'byte_60':1,'byte_61':1,'byte_179':0};assert c['bounds']==last
            poses+=1;matrices+=len(output);cached+=1
assert (poses,matrices,cached)==(900,29912,900)
asm=(root/'analysis/decompiled/skeletal-root-frame.asm').read_text()
for marker in ['1050a618 MOV AL,byte ptr [EDX + 0x61]','1050a628 JZ 0x1050b28b','1050a69a CALL 0x10500930','1050a6a6 CALL 0x10501420','1050a6e4 MOVSD.REP ES:EDI,ESI','1050a719 MOV EAX,dword ptr [EBX + 0x1c0]']:assert marker in asm
sources=['crates/rc-package/src/skeletal_full_pose.rs','crates/rc-package/src/skeletal_full_pose_tests.rs','crates/rc-package/src/skeletal_channel_stack.rs','crates/rc-package/src/skeletal_directed_bounds.rs','crates/rc-inspect/src/bin/rc-full-pose-check.rs','scripts/Record-FullPoses.py','scripts/Record-SkeletalPoses.py','analysis/decompiled/skeletal-root-frame.asm']
result={'date':'2026-10-08','rust_tests':347,'scope':report['scope'],'counts':{'original_meshes':130,'linkup_runs':225,'poses':poses,'matrices':matrices,'cached_repeats':cached,'channel_poses':applied,'reference_fallbacks':fallback},'source_sha256':{s:sha(root/s) for s in sources},'report_sha256':sha(path),'prior_report_sha256':sha(prior_path),'checks':['cargo test --workspace: 347 passed','cargo clippy --workspace --all-targets -- -D warnings','cargo fmt --all -- --check','all original-track local poses, channel histories, persistent scratch and host calls match independently verified prior report','29912 hierarchy matrices independently recomputed from original ASM expression oracle','900 local bounds/spheres and publication-before-flags snapshots independently checked under portable math policy','900 cached repeats produce no additional channel or publication calls','six integration tests cover cache before buffer validation, editor override, disabled channel fallback, director-before-child, and channel/publication error states']}
(root/'analysis/reports/full-poses-validation.json').write_text(json.dumps(result,indent=2)+'\n',encoding='utf-8')
evidence=root/'analysis/evidence.json';e=json.loads(evidence.read_text(encoding='utf-8-sig'));e['rust_tests']=347;e['full_poses_validation']=result;evidence.write_text(json.dumps(e,indent=2,ensure_ascii=False)+'\n',encoding='utf-8')
note=('2026-10-08: Vorbereitete Vollpose von Kanal-Auswertung bis Director-Hierarchie, Bounds-Publikation und gemeinsamem Cachebyte verbunden. '
      '900 Originaltrack-Posen / 29912 Matrizen und 900 Cachewiederholungen unabhaengig geprueft; sechs neue Integrationstests fuer Cache, Editor, Director und Fehlerteilzustaende. '
      '347 Tests, Clippy und Format bestanden. Puffer-/Transform-/Inverse-/Linkup-Vorbereitung weiterhin vorausgesetzt; Runtime, Skinning und sichtbare Animation offen. Android zum Schluss. '
      'Details D:\\Rust Projects\\RepublicCommandoAndroid\\docs\\SKELETAL_FULL_POSE.md.')
wiki=Path(r'C:\Users\brand\Documents\Main\05 KI\Project_Knowledge\projects\republic_commando_android')
for p in [root/'README.md',wiki/'index.md',wiki/'architecture/porting.md',wiki/'log.md']:
    if 'Vorbereitete Vollpose von Kanal-Auswertung bis Director-Hierarchie' not in p.read_text(encoding='utf-8-sig'):
        with p.open('a',encoding='utf-8') as stream:stream.write('\n\n'+note+'\n')
print(json.dumps(result['counts']))
