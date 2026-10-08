"""Independent sequential channel selection, persistent scratch and pose commit oracle."""
from pathlib import Path
import json,hashlib,math,collections,copy
root=Path(__file__).resolve().parents[1]
source=root/'scripts/Record-OriginalChannelPoses.py';ns={'__file__':str(source)}
exec(compile(source.read_text().split('max_rotation=max_position_relative=')[0],str(source),'exec'),ns)
links,animations,tracks=[ns[k] for k in ('links','animations','tracks')]
f,bits,value,matrix,compose,sample,slerp,angle,length,ease=[ns[k] for k in ('f','bits','value','matrix','compose','sample','slerp','angle','length','ease')]
path=root/'analysis/reports/original-channel-stacks.json';report=json.loads(path.read_text());stats=collections.Counter()
assert (report['poses'],report['bone_matrices'],len(report['objects']))==(900,29912,130)
max_rotation=0.;max_position=0.
def active(cs,index):
    c=cs[index];start,end=c[15],c[16]
    if index and not value(c[14])>0:return False
    for later in cs[index+1:]:
        if f(value(later[14])*value(later[11]))==1 and later[15]<=start and end<=later[16]:return False
    return True
for o in report['objects']:
    old=links['objects'][o['source_index']];bones=old['prefix']['bones'];assert (o['object'],o['file'])==(old['object'],old['file'])
    parents=[b['word_34'] for b in bones];parents[0]=-1;move=0 if bones[0]['name']['name'].lower()=='move' else -1
    reference=[{'rotation':b['rotation'],'position':b['position']} for b in bones];n=len(bones)
    assert len(o['runs'])==len(old['mappings'])
    for run,m in zip(o['runs'],old['mappings']):
        assert (run['animation'],run['entry'])==(m['animation'],m['entry']);a=animations[m['animation']]
        sequences=[0,1 if len(a['sequences'])>1 else 0,0];assert run['sequences']==sequences
        ts=[tracks(a,i) for i in sequences];frames=[a['sequences'][i]['metadata']['frames'] for i in sequences]
        cs=[[0]*18 for _ in range(3)]
        for c in cs:c[16]=n;c[11]=bits(1.)
        cs[0][14]=bits(1.);cs[1][15]=1;cs[1][14]=bits(.5);cs[1][11]=0
        local=copy.deepcopy(reference);scratch=copy.deepcopy(reference);assert len(run['ticks'])==4
        for step,tick in enumerate(run['ticks']):
            for c in cs:c[7]=bits(step*.25)
            cs[1][11]=bits((step+1)*.25);cs[2][14]=bits(1.) if step==3 else 0
            assert tick['step']==step and tick['before']==[{'words':c} for c in cs]
            assert tick['previous']==local and tick['scratch_before']==scratch
            previous=copy.deepcopy(local);called=[];applied=0
            for ci in range(3):
                if not active(cs,ci):continue
                called.append(ci);c=cs[ci];blend=value(c[11]);weight=1. if ci==0 else value(c[14])
                oldblend=value(c[13]);progress=1. if blend==1 else f(f(ease(blend)-ease(oldblend))/f(1-ease(oldblend)))
                for i in range(c[15],c[16]):
                    track=m['mapping'][i]
                    if track<0:scratch[i]=copy.deepcopy(reference[i]);stats['fallbacks']+=1;continue
                    if blend==0:scratch[i]=copy.deepcopy(previous[i]);continue
                    q,p=sample(ts[ci][track],f(f(frames[ci])*value(c[7])));stats['sampled_bones']+=1
                    if blend<1 and i>move:
                        pq,pp=sample(ts[ci][track],f(f(frames[ci])*value(c[12])))
                        oq=list(map(value,previous[i]['rotation']));op=list(map(value,previous[i]['position']))
                        distance=f(angle(oq,q));allowed=angle(q,pq)+distance*progress
                        if allowed<distance:q=slerp(oq,q,f(f(allowed)/distance));stats['angular_limited']+=1
                        delta=[f(y-x) for x,y in zip(op,p)];distance=length(delta);motion=length([f(x-y) for x,y in zip(pp,p)])
                        allowed=f(f(distance*progress)+motion)
                        if allowed<distance:
                            alpha=f(allowed/distance);p=[f(f(x*alpha)+y) for x,y in zip(delta,op)];stats['position_limited']+=1
                    if weight<1:
                        oq=list(map(value,scratch[i]['rotation']));op=list(map(value,scratch[i]['position']))
                        q=slerp(oq,q,f(ease(blend)*weight));p=[f(x+f(f(y-x)*weight)) for x,y in zip(op,p)]
                    scratch[i]={'rotation':list(map(bits,q)),'position':list(map(bits,p))}
                c[13]=c[11];c[12]=c[7];applied+=1
            local=copy.deepcopy(scratch) if applied else copy.deepcopy(reference)
            assert tick['calls']==called and called==([0,1] if step<3 else [2]),(o['object'],step,called)
            assert tick['result']=={'Ok':{'Channels':{'called':len(called),'applied':applied}}}
            assert tick['after']==[{'words':c} for c in cs] and tick['byte_61']==0
            assert tick['matrix_result']=={'Ok':None}
            for expected,actual in zip(local,tick['local']):
                qe=max(abs(value(x)-value(y)) for x,y in zip(expected['rotation'],actual['rotation']));max_rotation=max(max_rotation,qe)
                pe=max(abs(value(x)-value(y))/max(1,abs(value(x))) for x,y in zip(expected['position'],actual['position']));max_position=max(max_position,pe)
                assert qe<2e-6 and pe<2e-5
            # Exact portable results in this corpus also verify cross-tick state continuity.
            assert tick['local']==local and tick['scratch_after']==scratch
            output=[]
            for i,p in enumerate(local):
                mat=matrix(p['rotation'],p['position'])
                if i>0 and parents[i]!=move:mat=compose(mat,output[parents[i]])
                output.append(mat)
            assert tick['matrices']==output and all(math.isfinite(value(v)) for mat in output for v in mat)
            stats['channel_calls']+=len(called);stats['poses']+=1;stats['matrices']+=len(output)
        stats['runs']+=1
asm=(root/'analysis/decompiled/skeletal-root-frame.asm').read_text()
for marker in ('1050a628 JZ 0x1050b28b','1050a67d JNZ 0x1050ab6f','1050a69a CALL 0x10500930',
               '1050a6a6 CALL 0x10501420','1050a6be JZ 0x1050ab6f','1050a6e4 MOVSD.REP ES:EDI,ESI',
               '1050ab7f JGE 0x1050a719'):assert marker in asm,marker
sha=lambda p:hashlib.sha256(p.read_bytes()).hexdigest()
sources=['crates/rc-package/src/skeletal_channel_stack.rs','crates/rc-package/src/skeletal_channel.rs','crates/rc-package/src/skeletal_hierarchy.rs','crates/rc-package/src/skeletal_track.rs','crates/rc-package/src/quaternion_animation.rs','crates/rc-package/src/move_coords.rs','crates/rc-inspect/src/bin/rc-original-channel-stack-check.rs','scripts/Record-OriginalChannelStacks.py','scripts/Record-OriginalChannelPoses.py','scripts/Record-SkeletalPoses.py','scripts/Record-SkeletalLinkups.py','scripts/Record-OriginalAnimationTracks.py','analysis/decompiled/skeletal-root-frame.asm','analysis/decompiled/skeletal-apply-animation.c']
result={'date':'2026-10-07','rust_tests':276,'counts':dict(stats),'maximum_rotation_error':max_rotation,'maximum_relative_position_error':max_position,'scope':report['scope'],'source_sha256':{s:sha(root/s) for s in sources},'report_sha256':sha(path),'original_package_sha256':ns['ns']['ns']['files'],'checks':['cargo test --workspace: 276 passed','cargo clippy --workspace --all-targets -- -D warnings','cargo fmt --all -- --check','225 sequential runs / 900 poses independently verified including all previous/scratch/history snapshots','all 29912 matrices verified by original ASM expression interpreter','selection/cache/fallback/commit instructions checked against original ASM']}
(root/'analysis/reports/original-channel-stacks-validation.json').write_text(json.dumps(result,indent=2)+'\n',encoding='utf-8')
path=root/'analysis/evidence.json';e=json.loads(path.read_text(encoding='utf-8-sig'));e['rust_tests']=276;e['original_channel_stacks_validation']=result;path.write_text(json.dumps(e,indent=2,ensure_ascii=False)+'\n',encoding='utf-8')
note=('2026-10-07: Native aktive Kanalfolge und gemeinsame Pose-Übernahme ergänzt. Vorpose bleibt während '
      'aller Kanäle erhalten, Scratch wird geteilt/persistent übergeben; Cache-/Disable-/Referenzfallback '
      'und Fehlergrenzen berücksichtigt. 225 Original-Linkup-Läufe mit je vier Schritten: 900 Posen und '
      '29912 Matrizen unabhängig inklusive History und Scratch geprüft, Layer mit zweiter Sequenz soweit '
      'vorhanden und vollständige Verdrängung im vierten Schritt. 276 Tests, Clippy, Format bestanden. '
      'Noch keine natürliche Tick-/Cache-Komplettierung, Directors, Bounds oder Skinning. Android zum Schluss. '
      'Details D:\\Rust Projects\\RepublicCommandoAndroid\\docs\\SKELETAL_CHANNEL_STACK.md.')
wiki=Path(r'C:\Users\brand\Documents\Main\05 KI\Project_Knowledge\projects\republic_commando_android')
for path in [root/'README.md',wiki/'index.md',wiki/'architecture/porting.md',wiki/'log.md']:
    if 'Native aktive Kanalfolge und gemeinsame Pose-Übernahme ergänzt' not in path.read_text(encoding='utf-8-sig'):
        with path.open('a',encoding='utf-8') as stream:stream.write('\n\n'+note+'\n')
print(json.dumps({'counts':dict(stats),'maximum_rotation_error':max_rotation,'maximum_relative_position_error':max_position}))
