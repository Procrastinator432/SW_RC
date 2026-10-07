"""Independent original-track channel oracle, portable math policy and ASM matrices."""
from pathlib import Path
import json,math,hashlib,collections
root=Path(__file__).resolve().parents[1]
# Only definitions: no earlier validation loops, evidence rewrites or wiki append.
source=root/'scripts/Record-SkeletalPoses.py';ns={'__file__':str(source)}
exec(compile(source.read_text().split('poses=0;matrices=0;')[0],str(source),'exec'),ns)
links,animations,tracks=ns['links'],ns['animations'],ns['tracks']
f,bits,value,matrix,compose=[ns[k] for k in ('f','bits','value','matrix','compose')]
path=root/'analysis/reports/original-channel-poses.json';report=json.loads(path.read_text())
assert (report['poses'],report['bone_matrices'],len(report['objects']))==(900,29912,130)
def inverse(n):
    r=f(1/f(math.sqrt(n))) if n else math.inf
    return f(f(3-f(f(r*n)*r))*f(r*.5))
def decode(key):
    a=[]
    for word in key:
        w=word&0xfffe;w=w-65536 if w>=32768 else w
        a.append(f(w*f(1/46339)))
    n=f(f(f(1-f(a[0]*a[0]))-f(a[1]*a[1]))-f(a[2]*a[2]))
    d=f(inverse(n)*n) if n else 0.
    a.append(f(0-d) if key[2]&1 else d);shift=(key[0]&1)+2*(key[1]&1)
    return a[shift:]+a[:shift]
def slerp(a,b,t):
    dot=f(f(f(f(a[0]*b[0])+f(a[2]*b[2]))+f(a[1]*b[1]))+f(b[3]*a[3]));absolute=abs(dot)
    linear=absolute>=value(0x3f0ccccd)
    if linear:left,right=f(1-t),t
    else:
        angle=math.acos(absolute);inv=1/math.sin(angle)
        left=f(math.sin((1-t)*angle)*inv);right=f(math.sin(t*angle)*inv)
    if dot<0:right=f(0-right)
    out=[f(f(x*left)+f(y*right)) for x,y in zip(a,b)]
    if linear:
        n=f(f(f(f(out[3]*out[3])+f(out[2]*out[2]))+f(out[1]*out[1]))+f(out[0]*out[0]));inv=inverse(n)
        out=[f(x*inv) for x in out]
    return out
def sample(track,time):
    scale,pos,rot,dur=track;current=0;res=time;alpha=None
    if len(dur)>1:
        for current,d in enumerate(dur):
            after=f(res-d)
            if after<0:break
            res=after
        else:current=0;d=dur[0]
        if res>0:alpha=f(res/d)
    nxt=(current+1)%len(dur) if alpha is not None else current
    q=decode(rot[0 if len(rot)==1 else current])
    if alpha is not None and len(rot)!=1:q=slerp(q,decode(rot[nxt]),alpha)
    ps=f(scale*value(0x38000100));p=[f(x*ps) for x in pos[0 if len(pos)==1 else current]]
    if alpha is not None and len(pos)!=1:
        b=[f(x*ps) for x in pos[nxt]];p=[f(f(f(y-x)*alpha)+x) for x,y in zip(p,b)]
    return q,p
def angle(a,b):
    dot=abs(((a[3]*b[3]+a[2]*b[2])+a[1]*b[1])+a[0]*b[0]);return 2*math.acos(min(dot,1))
def length(d):
    x,y,z=d;n=f(f(f(z*z)+f(y*y))+f(x*x));return f(inverse(n)*n) if n else 0.
def ease(t):return f(f(t*t)*2) if t<=.5 else f(1-f(f(f(1-t)*f(1-t))*2))
max_rotation=max_position_relative=0.;stats=collections.Counter();file_hashes={}
for o in report['objects']:
    old=links['objects'][o['source_index']];bones=old['prefix']['bones'];assert o['object']==old['object'] and o['file']==old['file']
    parents=[b['word_34'] for b in bones];parents[0]=-1;move=0 if bones[0]['name']['name'].lower()=='move' else -1
    reference=[(list(map(value,b['rotation'])),list(map(value,b['position']))) for b in bones]
    assert len(o['cases'])==4*len(old['mappings'])
    for ci,c in enumerate(o['cases']):
        m=old['mappings'][ci//4];animation=animations[m['animation']];s=animation['sequences'][0]['metadata'];ts=tracks(animation,0)
        assert c['animation']==m['animation'] and c['sequence']==0 and c['sequence_name']==s['name']['name'] and c['entry']==m['entry']
        kind,index,blend,weight=[('Replace',0,1.,.5),('Layer',1,1.,.5),('Transition',0,.25,.5),('LayerTransition',1,.25,.5)][ci%4]
        assert c['kind']==kind and c['channel_index']==index
        words=[0]*18;words[7]=bits(.75);words[12]=bits(.25);words[11]=bits(blend);words[14]=bits(weight);words[16]=len(bones)
        assert c['before']['words']==words;words[13]=words[11];words[12]=words[7];assert c['after']['words']==words
        assert c['result']=={'Ok':True} and c['matrix_result']=={'Ok':None}
        output=[];effective=1. if index==0 else weight;progress=ease(blend)
        for i,track in enumerate(m['mapping']):
            aq,ap=reference[i]
            if track<0:q,p=aq,ap;stats['fallbacks']+=1
            else:
                q,p=sample(ts[track],f(f(s['frames'])*.75));stats['sampled']+=1
                if blend<1 and i>move:
                    pq,pp=sample(ts[track],f(f(s['frames'])*.25));distance=f(angle(aq,q));allowed=angle(q,pq)+distance*progress
                    if allowed<distance:q=slerp(aq,q,f(f(allowed)/distance));stats['angular_limited']+=1
                    delta=[f(y-x) for x,y in zip(ap,p)];distance=length(delta);motion=length([f(x-y) for x,y in zip(pp,p)]);allowed=f(f(distance*progress)+motion)
                    if allowed<distance:
                        alpha=f(allowed/distance);p=[f(f(x*alpha)+y) for x,y in zip(delta,ap)];stats['position_limited']+=1
                if effective<1:
                    q=slerp(aq,q,f(ease(blend)*effective));p=[f(x+f(f(y-x)*effective)) for x,y in zip(ap,p)]
            actual=c['local'][i];actual_q=list(map(value,actual['rotation']));actual_p=list(map(value,actual['position']))
            assert all(math.isfinite(v) for v in actual_q+actual_p)
            error=max(abs(x-y) for x,y in zip(q,actual_q));max_rotation=max(max_rotation,error);assert error<2e-6,(o['object'],ci,i,'rotation',error)
            error=max(abs(x-y)/max(1,abs(x)) for x,y in zip(p,actual_p));max_position_relative=max(max_position_relative,error);assert error<2e-5,(o['object'],ci,i,'position',error)
            if track<0:assert actual=={'rotation':bones[i]['rotation'],'position':bones[i]['position']}
            a=matrix(actual['rotation'],actual['position']);parent=parents[i]
            if i>0 and parent!=move:a=compose(a,output[parent])
            output.append(a)
        assert c['matrices']==output
        assert all(math.isfinite(value(v)) for a in output for v in a)
        stats['poses']+=1;stats['matrices']+=len(output)
assert (stats['poses'],stats['matrices'])==(900,29912)
sha=lambda p:hashlib.sha256(p.read_bytes()).hexdigest()
sources=['crates/rc-package/src/skeletal_channel.rs','crates/rc-inspect/src/bin/rc-original-channel-check.rs','scripts/Record-OriginalChannelPoses.py','scripts/Record-SkeletalPoses.py','scripts/Record-SkeletalLinkups.py','scripts/Record-OriginalAnimationTracks.py','analysis/decompiled/quaternion-angle-diff.asm','analysis/decompiled/skeletal-apply-channel.asm']
result={'date':'2026-10-07','rust_tests':271,'counts':dict(stats),'maximum_rotation_error':max_rotation,'maximum_relative_position_error':max_position_relative,'scope':report['scope'],'report_sha256':sha(path),'source_sha256':{s:sha(root/s) for s in sources},'original_package_sha256':ns['ns']['files'],'checks':['cargo test --workspace: 271 passed','cargo clippy --workspace --all-targets -- -D warnings','cargo fmt --all -- --check','900 channel poses independently recomputed from original archive tracks','all 29912 matrices verified via original ASM expression interpreter','rotation tolerance 2e-6; position relative tolerance 2e-5; all components finite']}
(root/'analysis/reports/original-channel-poses-validation.json').write_text(json.dumps(result,indent=2)+'\n',encoding='utf-8')
path=root/'analysis/evidence.json';e=json.loads(path.read_text(encoding='utf-8-sig'));e['rust_tests']=271;e['original_channel_poses_validation']=result;path.write_text(json.dumps(e,indent=2,ensure_ascii=False)+'\n',encoding='utf-8')
note=('2026-10-07: Echte Originaltracks an ApplyAnimChannel angebunden. Austauschbare Quaternion- und '
      'Winkelbudget-Policy; portable f64-Näherung der x87-Winkelrechnung ausdrücklich gekennzeichnet. '
      '900 Kanal-Diagnoseposen mit 29912 Knochenmatrizen aus 225 Original-Linkups unabhängig geprüft: '
      'Replace, Layer, Transition und LayerTransition. Vorpose/Scratch sind definierte Referenz-Snapshots, '
      'noch keine automatische Animationslaufzeit. 271 Tests, Clippy und Format bestanden. '
      'Details D:\\Rust Projects\\RepublicCommandoAndroid\\docs\\ORIGINAL_CHANNEL_POSES.md. Android zum Schluss.')
wiki=Path(r'C:\Users\brand\Documents\Main\05 KI\Project_Knowledge\projects\republic_commando_android')
for path in [root/'README.md',wiki/'index.md',wiki/'architecture/porting.md',wiki/'log.md']:
    if 'Echte Originaltracks an ApplyAnimChannel angebunden' not in path.read_text(encoding='utf-8-sig'):
        with path.open('a',encoding='utf-8') as stream:stream.write('\n\n'+note+'\n')
print(json.dumps({'counts':dict(stats),'maximum_rotation_error':max_rotation,'maximum_relative_position_error':max_position_relative}))
