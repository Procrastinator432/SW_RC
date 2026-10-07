"""Independent prepared-channel model; host values are supplied, not CPU emulation."""
from pathlib import Path
import hashlib,json,struct,math,collections
root=Path(__file__).resolve().parents[1]
def value(b):return struct.unpack('<f',struct.pack('<I',b))[0]
def bits(f):return struct.unpack('<I',struct.pack('<f',f))[0]
def f(x):return value(bits(x))
def div(a,b):
    if b==0:return math.nan if a==0 else math.copysign(math.inf,a*b if b else a)
    return f(a/b)
def ease(t):
    if not t<=.5:
        r=f(1-t);return f(1-f(f(r*r)*2))
    return f(f(t*t)*2)
def pose(x):return {'rotation':[0,0,0,bits(1)],'position':[bits(x),0,0]}
path=root/'analysis/reports/skeletal-channel.json';report=json.loads(path.read_text());stats=collections.Counter()
for c in report['cases']:
    words=c['before']['words'].copy();out=[pose(2),pose(2)];events=[];failed=False;valid=c['frames'] is not None
    def event(e):
        events.append(e)
        if len(events)==c['failure']:raise RuntimeError('host boundary')
    def sample(track,time):event(['sample',track,bits(time)]);return pose(10)
    def slerp(a,b,t):event(['slerp',a,b,bits(t)]);return [11,22,33,44]
    def length(d):
        x,y,z=d;n=f(f(f(z*z)+f(y*y))+f(x*x));event(['seed',bits(n)]);r=.125
        refined=f(f(3-f(f(r*n)*r))*f(r*.5));return 0. if n==0 else f(refined*n)
    try:
        if valid:
            weight=1. if c['index']==0 else value(words[14]);blend=value(words[11]);prev=value(words[13])
            progress=1. if blend==1 else div(f(ease(blend)-ease(prev)),f(1-ease(prev)))
            for bone,track in enumerate(c['mapping']):
                if track<0:out[bone]=pose(20);continue
                if blend==0:out[bone]=pose(0);continue
                target=sample(track,6.)
                if blend<1 and bone>c['move_bone']:
                    earlier=sample(track,2.);old=pose(0)
                    event(['angular',old['rotation'],target['rotation'],earlier['rotation'],bits(progress)])
                    if c['angular']:target['rotation']=slerp(old['rotation'],target['rotation'],.25)
                    a=list(map(value,old['position']));b=list(map(value,target['position']));p=list(map(value,earlier['position']))
                    delta=[f(y-x) for x,y in zip(a,b)];distance=length(delta);motion=length([f(x-y) for x,y in zip(p,b)])
                    allowed=f(f(distance*progress)+motion)
                    if allowed<distance:
                        alpha=div(allowed,distance);target['position']=[bits(f(f(x*alpha)+y)) for x,y in zip(delta,a)]
                if not weight<1:out[bone]=target
                else:
                    out[bone]['rotation']=slerp(out[bone]['rotation'],target['rotation'],f(ease(blend)*weight))
                    a=list(map(value,out[bone]['position']));b=list(map(value,target['position']))
                    out[bone]['position']=[bits(f(x+f(f(y-x)*weight))) for x,y in zip(a,b)]
        words[13]=words[11];words[12]=words[7]
    except RuntimeError:failed=True
    expected={'Err':'host boundary'} if failed else {'Ok':valid}
    assert c['result']==expected,(c,expected)
    assert c['after']['words']==words
    assert c['scratch']==out,(c,out)
    assert c['events']==events,(c,events)
    stats['failure' if failed else 'applied' if valid else 'missing_sequence']+=1
assert len(report['cases'])==3072
asm=(root/'analysis/decompiled/skeletal-apply-channel.asm').read_text()
for marker in ('1050146b JZ 0x10501b94','10501622 JL 0x10501b1e','10501634 JP 0x1050168b',
               '105016b2 JBE 0x105019da','10501743 FADDP','105019e3 JBE 0x10501ad9',
               '10501a36 MULSS XMM0,XMM2','10501b9b MOV dword ptr [ESI + 0x34],EAX'):
    assert marker in asm,marker
sha=lambda p:hashlib.sha256(p.read_bytes()).hexdigest()
sources=['crates/rc-package/src/skeletal_channel.rs','crates/rc-inspect/src/bin/rc-skeletal-channel-probe.rs','scripts/Record-SkeletalChannel.py','analysis/decompiled/skeletal-apply-channel.c','analysis/decompiled/skeletal-apply-channel.asm','analysis/decompiled/quaternion-angle-diff.c','analysis/decompiled/quaternion-angle-diff.asm']
result={'date':'2026-10-07','rust_tests':269,'cases':3072,'outcomes':dict(stats),'scope':report['scope'],
        'report_sha256':sha(path),'source_sha256':{s:sha(root/s) for s in sources},
        'checks':['cargo test --workspace: 269 passed','cargo clippy --workspace --all-targets -- -D warnings','cargo fmt --all -- --check','independent Python model compares all outputs, history words and ordered host events in 3072 cases','original ASM verifies copy/blend branches, quaternion eased weight and raw history writes'],
        'remaining':['real track host integration','native x87 AngleDiffFast/angular budget','full ApplyAnimation preparation/directors/bounds','skinning','Android validation at end']}
(root/'analysis/reports/skeletal-channel-validation.json').write_text(json.dumps(result,indent=2)+'\n',encoding='utf-8')
path=root/'analysis/evidence.json';e=json.loads(path.read_text(encoding='utf-8-sig'));e['rust_tests']=269;e['skeletal_channel_validation']=result;path.write_text(json.dumps(e,indent=2,ensure_ascii=False)+'\n',encoding='utf-8')
note=('2026-10-07: ApplyAnimChannel auf vorbereiteten Puffern rekonstruiert. Referenz-/Vorposen-Fallback, '
      'Frameklammerung, progressives Übergangslimit, unterschiedliche Rotations-/Positionsgewichte und '
      'History-Schreibreihenfolge ergänzt. 3072 Fälle unabhängig inklusive Host-Ereignissen und Fehlergrenzen '
      'geprüft; 269 Workspace-Tests, Clippy und Format bestanden. Track-/Slerp-/x87-Winkelbudget-/RSQRT-Hostwerte '
      'hier explizit vorgegeben; noch kein Nachweis vollständigen Original-Kanalplaybacks. Android zum Schluss. '
      'Details D:\\Rust Projects\\RepublicCommandoAndroid\\docs\\SKELETAL_CHANNEL.md.')
wiki=Path(r'C:\Users\brand\Documents\Main\05 KI\Project_Knowledge\projects\republic_commando_android')
for path in [root/'README.md',wiki/'index.md',wiki/'architecture/porting.md',wiki/'log.md']:
    if 'ApplyAnimChannel auf vorbereiteten Puffern rekonstruiert' not in path.read_text(encoding='utf-8-sig'):
        with path.open('a',encoding='utf-8') as stream:stream.write('\n\n'+note+'\n')
print(json.dumps({'cases':3072,'outcomes':dict(stats),'rust_tests':269}))
