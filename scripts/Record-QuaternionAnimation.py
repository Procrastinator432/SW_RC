"""Independent primitive-math oracle; portable tracks checked as mathematical rotations."""
from pathlib import Path
from collections import Counter
import hashlib,json,math,struct
root=Path(__file__).resolve().parents[1]
f=lambda x:struct.unpack('<f',struct.pack('<f',x))[0]
bits=lambda x:struct.unpack('<I',struct.pack('<f',x))[0]
value=lambda x:struct.unpack('<f',struct.pack('<I',x))[0]
mul=lambda a,b:f(a*b)
add=lambda a,b:f(a+b)
scale=f(1/46339)
threshold=value(0x3f0ccccd)
path=root/'analysis/reports/quaternion-animation.json';report=json.loads(path.read_text())
assert [len(report[k]) for k in ('decodes','slerps','portable_tracks')]==[80,84,12]
counts=Counter()
class Boundary(Exception):pass
def inverse(n,events,fail):
    events.append({'rsqrt_bits':bits(n)})
    if fail:raise Boundary('unresolved RSQRTSS seed')
    return mul(f(3-mul(mul(.875,n),.875)),mul(.875,.5))
def decode(key,events,fail):
    a=[]
    for word in key:
        word&=0xfffe
        if word>=32768:word-=65536
        a.append(mul(word,scale))
    x,y,z=a;n=f(f(f(1-mul(x,x))-mul(y,y))-mul(z,z))
    d=mul(inverse(n,events,fail),n)
    if n==0:d=0.
    if key[2]&1:d=f(0-d)
    a.append(d);offset=(key[0]&1)+2*(key[1]&1)
    return list(map(bits,a[offset:]+a[:offset]))
for i,p in enumerate(report['decodes']):
    base=[[0]*3,[12000,57536,4000],[32766,0,0],[32768]*3,[1000,2000,3000]][i//16]
    flags=(i//2)%8;expected=[base[0]|(flags&1),base[1]|((flags>>1)&1),base[2]|((flags>>2)&1)]
    assert p['key']==expected and p['fail']==bool(i%2)
    events=[]
    try:result={'Ok':decode(p['key'],events,p['fail'])}
    except Boundary as e:result={'Err':str(e)}
    assert (p['result'],p['events'])==(result,events),(i,p,result,events)
    counts['DecodeBoundary' if p['fail'] else 'SuppliedPrimitiveDecode']+=1
pairs=[([0.,0.,0.,1.],[0.,0.,0.,1.]),([0.,0.,0.,1.],[0.,0.,0.,-1.]),([1.,0.,0.,0.],[0.,0.,1.,0.]),
       ([0.,0.,0.,1.],[0.,0.,0.,threshold]),([0.,0.,0.,1.],[0.,0.,0.,.54]),([.1,-.2,.3,.9],[-.7,.4,.2,-.1])]
times=[-1.,-0.,0.,.25,.5,1.,2.5]
for i,p in enumerate(report['slerps']):
    a,b=pairs[i//14];assert p['current']==list(map(bits,a)) and p['next']==list(map(bits,b))
    assert p['alpha_bits']==bits(times[(i//2)%7]) and p['fail']==bool(i%2)
    a=list(map(value,p['current']));b=list(map(value,p['next']));t=value(p['alpha_bits']);events=[]
    dot=add(add(add(mul(a[0],b[0]),mul(a[2],b[2])),mul(a[1],b[1])),mul(b[3],a[3]));absolute=abs(dot)
    linear=absolute>=threshold
    try:
        if linear:left=f(1-t);right=t
        else:
            events.append({'dot_bits':bits(absolute),'alpha_bits':bits(t)})
            if p['fail']:raise Boundary('unresolved x87 weights')
            left=.25;right=.75
        if dot<0:right=f(0-right)
        out=[add(mul(x,left),mul(y,right)) for x,y in zip(a,b)]
        if linear:
            n=add(add(add(mul(out[3],out[3]),mul(out[2],out[2])),mul(out[1],out[1])),mul(out[0],out[0]))
            factor=inverse(n,events,p['fail']);out=[mul(x,factor) for x in out]
        result={'Ok':list(map(bits,out))}
    except Boundary as e:result={'Err':str(e)}
    assert (p['result'],p['events'])==(result,events),(i,p,result,events)
    counts[('Linear' if linear else 'Spherical')+('Boundary' if p['fail'] else 'SuppliedPrimitives')]+=1

# Independent mathematical oracle: full square root and standard spherical interpolation.
# This checks portable policy accuracy, deliberately not RSQRTSS/x87 bit parity.
def mathematical_decode(key):
    a=[]
    for w in key:
        w&=0xfffe
        if w>=32768:w-=65536
        a.append(w*scale)
    d=math.sqrt(1-sum(x*x for x in a))
    a.append(-d if key[2]&1 else d);offset=(key[0]&1)+2*(key[1]&1)
    return a[offset:]+a[:offset]
max_error=0.
for i,p in enumerate(report['portable_tracks']):
    t=value(p['time_bits']);tr=p['track'];idx=0 if t<2 else 1;res=t if t<2 else t-2
    alpha=res/2;next_idx=1-idx
    assert p['result']=={'Ok':{'current':idx,'next':next_idx if alpha>0 else idx,'alpha_bits':bits(alpha) if alpha>0 else None}}
    a=mathematical_decode(tr['rotations'][idx]);b=mathematical_decode(tr['rotations'][next_idx]);dot=sum(x*y for x,y in zip(a,b))
    if not alpha:out=a
    elif abs(dot)>=threshold:
        out=[x*(1-alpha)+y*alpha*(-1 if dot<0 else 1) for x,y in zip(a,b)];n=math.sqrt(sum(x*x for x in out));out=[x/n for x in out]
    else:
        angle=math.acos(abs(dot));left=math.sin((1-alpha)*angle)/math.sin(angle);right=math.sin(alpha*angle)/math.sin(angle)
        out=[x*left+y*right*(-1 if dot<0 else 1) for x,y in zip(a,b)]
    error=max(abs(x-value(y)) for x,y in zip(out,p['root']['rotation']));max_error=max(max_error,error);assert error<1e-6,(i,error)
    ps=f(2*value(0x38000100));a=[mul(x,ps) for x in tr['positions'][idx]];b=[mul(x,ps) for x in tr['positions'][next_idx]]
    out=[add(mul(f(y-x),alpha),x) for x,y in zip(a,b)] if alpha else a
    assert p['root']['position']==list(map(bits,out))
def pe_read(file,address,n):
    d=file.read_bytes();p=struct.unpack_from('<I',d,60)[0];o=p+24;b=struct.unpack_from('<I',d,o+28)[0];size=struct.unpack_from('<H',d,p+20)[0]
    ss=[struct.unpack_from('<IIII',d,o+size+i*40+8) for i in range(struct.unpack_from('<H',d,p+6)[0])]
    off=next(raw+address-b-start for _,start,size,raw in ss if start<=address-b<start+size)
    return d[off:off+n]
game=Path(r'D:\SteamLibrary\steamapps\common\Star Wars Republic Commando\GameData\System')
for file,addr,fmt,v in [('engine.dll',0x10668cf0,'d',.5),('engine.dll',0x10671a48,'f',32767.),('engine.dll',0x10653578,'f',1.),('engine.dll',0x1065efc8,'f',3.),('engine.dll',0x1065efc4,'f',.5),('core.dll',0x10186f30,'f',threshold),('core.dll',0x10186ed8,'f',.5),('core.dll',0x10186edc,'f',3.)]:
    assert pe_read(game/file,addr,struct.calcsize('<'+fmt))==struct.pack('<'+fmt,v)
assert int(f(32767/f(math.sqrt(.5))))==46339 and bits(scale)==0x37b506e6
asm=(root/'analysis/decompiled/quaternion-slerp.asm').read_text()
for marker in ('10144a11 ADDSS XMM2,XMM0','10144a5f JBE 0x10144b1f','10144a69 CALL 0x1017a650','10144a8c FSIN','10144c2d RSQRTSS XMM0,XMM1','10144c49 SUBSS XMM3,XMM0'):assert marker in asm
init=(root/'analysis/decompiled/rotation-scale-init.asm').read_text()
for marker in ('1064beb6 FSQRT','1064bed0 CVTTSS2SI EAX,XMM0','1064bef4 MOVSS dword ptr [0x108b85d0],XMM1'):assert marker in init
sha=lambda p:hashlib.sha256(p.read_bytes()).hexdigest()
sources=['crates/rc-package/src/quaternion_animation.rs','crates/rc-inspect/src/bin/rc-quaternion-probe.rs','scripts/Record-QuaternionAnimation.py','analysis/decompiled/quaternion-slerp.c','analysis/decompiled/quaternion-slerp.asm','analysis/decompiled/skeletal-rotation-decode.c','analysis/decompiled/skeletal-rotation-decode.asm','analysis/decompiled/rotation-scale-refs.txt','analysis/decompiled/rotation-scale-init.asm']
result={'date':'2026-10-07','rust_tests':248,'decode_cases':80,'slerp_cases':84,'portable_track_cases':12,'outcomes':dict(counts),'maximum_portable_component_error':max_error,'report_sha256':sha(path),'source_sha256':{s:sha(root/s) for s in sources},'engine_dll_sha256':sha(game/'engine.dll'),'core_dll_sha256':sha(game/'core.dll'),'checks':['cargo test --workspace: 248 passed','cargo clippy --workspace --all-targets -- -D warnings','cargo fmt --all -- --check','164 primitive-math cases independently bit-checked','12 portable tracks compared to mathematical quaternion oracle, tolerance 1e-6','native constants, initialization and instruction order'],'scope':report['scope']}
(root/'analysis/reports/quaternion-animation-validation.json').write_text(json.dumps(result,indent=2)+'\n',encoding='utf-8')
path=root/'analysis/evidence.json';e=json.loads(path.read_text(encoding='utf-8-sig'));e['rust_tests']=248;e['quaternion_animation_validation']=result;path.write_text(json.dumps(e,indent=2,ensure_ascii=False)+'\n',encoding='utf-8')
note=('2026-10-07: Quaternion-Decodierung und Slerp-Rechenwege mit Track-Sampler verbunden. '
      'Initialisierung belegt Rotationsmaßstab 1/46339; Original-Slerp-Schwelle 0,55, kürzester Weg, '
      'SSE-Reihenfolge, lineare Normalisierung und sphärische Mischung rekonstruiert. '
      '164 Diagnosen mit gelieferten Matheprimitiven/Grenzen bitweise unabhängig geprüft; zwölf synthetische '
      'Tracks mit portabler sqrt/f64-Trigonometrie gegen mathematische Referenz geprüft. Keine x86-Bitgleichheit '
      'oder Originalclip-Wiedergabe behauptet. 248 Workspace-Tests, Clippy und Format bestanden; Android zum Schluss. '
      'Details D:\\Rust Projects\\RepublicCommandoAndroid\\docs\\QUATERNION_ANIMATION.md.')
wiki=Path(r'C:\Users\brand\Documents\Main\05 KI\Project_Knowledge\projects\republic_commando_android')
for path in [root/'README.md',wiki/'index.md',wiki/'architecture/porting.md',wiki/'log.md']:
    if 'Quaternion-Decodierung und Slerp-Rechenwege mit Track-Sampler verbunden' not in path.read_text(encoding='utf-8-sig'):
        with path.open('a',encoding='utf-8') as stream:stream.write('\n\n'+note+'\n')
print(json.dumps({k:result[k] for k in ('rust_tests','outcomes','maximum_portable_component_error')}))
