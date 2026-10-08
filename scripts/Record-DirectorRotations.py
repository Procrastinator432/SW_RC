"""Independent portable-math oracle for native director rotation preparation."""
from pathlib import Path
import json,hashlib,math,struct,collections
root=Path(__file__).resolve().parents[1]
source=root/'scripts/Record-MatrixQuaternions.py';ns={'__file__':str(source)}
exec(compile(source.read_text().split("path=root/'analysis/reports/matrix-quaternions.json'")[0],str(source),'exec'),ns)
f,bits,value,convert=[ns[k] for k in ('f','bits','value','interpret')]
source=root/'scripts/Record-SkeletalPoses.py';pose_ns={'__file__':str(source)}
exec(compile(source.read_text().split('poses=0;matrices=0;')[0],str(source),'exec'),pose_ns)
matrix=pose_ns['matrix'];sha=lambda p:hashlib.sha256(p.read_bytes()).hexdigest()
path=root/'analysis/reports/director-rotations.json';report=json.loads(path.read_text())
source_path=root/'analysis/reports/world-director-poses.json';source=json.loads(source_path.read_text())
assert sha(source_path)==json.loads((root/'analysis/reports/world-director-poses-validation.json').read_text())['report_sha256']
assert len(report['cases'])==3600
def seed(n):return f(1/f(math.sqrt(n))) if n>0 else math.inf if n==0 else math.nan
def inverse(n):
    r=seed(n);return f(f(3-f(f(r*n)*r))*f(r*.5))
def slerp(a,b,t):
    a=list(map(value,a));b=list(map(value,b))
    dot=f(f(f(f(a[0]*b[0])+f(a[2]*b[2]))+f(a[1]*b[1]))+f(b[3]*a[3]));absolute=abs(dot)
    linear=not absolute<value(0x3f0ccccd)
    if linear:left=f(1-t);right=t
    else:
        angle=math.acos(absolute);inv=1/math.sin(angle)
        left=f(math.sin((1-t)*angle)*inv);right=f(math.sin(t*angle)*inv)
    if dot<0:right=f(0-right)
    q=[f(f(x*left)+f(y*right)) for x,y in zip(a,b)]
    if linear:
        n=f(f(f(f(q[3]*q[3])+f(q[2]*q[2]))+f(q[1]*q[1]))+f(q[0]*q[0]));inv=inverse(n);q=[f(v*inv) for v in q]
    return list(map(bits,q))
def angle_difference(a,b):
    a=list(map(value,a));b=list(map(value,b))
    d=abs(((a[3]*b[3]+a[2]*b[2])+a[1]*b[1])+a[0]*b[0]);d=d if d<=1 else 1.
    return f(2*math.acos(d))
def angle_fast(q):
    d=abs(value(q[3]));d=d if d<=1 else 1.;return f(2*math.acos(d))
def power(q,t):
    v=list(map(value,q));n=f(f(f(v[0]*v[0])+f(v[1]*v[1]))+f(v[2]*v[2]));r=f(inverse(n)*n) if n else 0.
    r=r if r<=1 else 1.;angle=2*math.asin(r)
    if not angle>0:return q.copy()
    scaled=t*angle*.5;factor=math.sin(scaled)/math.sin(angle*.5)
    return list(map(bits,[f(factor*x) for x in v[:3]]+[f(math.cos(scaled))]))
seen=set();counts=collections.Counter()
for c in report['cases']:
    si,scenario=c['source_case'],c['scenario'];key=(si,scenario);assert key not in seen;seen.add(key)
    original=source['cases'][si];assert original['transform']==0
    mats=original['matrices'];current=mats[0];supplied=mats[1%len(mats)]
    before=[0]*28;before[18]=0x100;before[19]=1;before[2:18]=supplied
    before[20]=bits(.3 if scenario>=2 else -1.);before[21]=bits(-1. if scenario==0 else 0.)
    before[26]=bits(.25);before[22:26]=[0,0,0,bits(1.)];before[27]=0xaabbcc00|int(scenario%2==1)
    assert c['before']=={'words':before};after=before.copy()
    if not after[27]&0xff:
        after[22:26]=convert(current);after[27]=(after[27]&0xffffff00)|1;counts['history_initializations']+=1
    q=convert(supplied);m=supplied.copy();scale=list(map(bits,[2.,.5,3.]));temporal=absolute=False
    if value(after[21])>=0:
        distance=angle_difference(after[22:26],q);budget=value(after[26])
        if distance>budget:
            q=slerp(after[22:26],q,f(budget/distance));m=matrix(q,supplied[12:15]);scale=[bits(1.)]*3;temporal=True
        after[26]=0;counts['budget_clears']+=1
    maximum=value(after[20])
    if maximum>=0:
        angle=angle_fast(q)
        if angle>maximum:
            q=power(q,f(maximum/angle));m=matrix(q,supplied[12:15]);scale=[bits(1.)]*3;absolute=True
    after[22:26]=q
    expected={'quaternion':q,'matrix':m,'scale':scale,'temporal_limited':temporal,'absolute_limited':absolute}
    assert c['after']=={'words':after},key
    assert c['result']=={'Ok':expected},key
    counts['temporal_limited']+=temporal;counts['absolute_limited']+=absolute;counts['prepared_rotations']+=1
assert counts['temporal_limited']>0 and counts['absolute_limited']>0
# Verify gate constants directly in both original DLLs.
dll_word=ns['dll_word'];core=ns['core'];engine=core.with_name('engine.dll')
assert dll_word(core,0x10186c1c)==dll_word(engine,0x1065efcc)==bits(0.)
asm=(root/'analysis/decompiled/skeletal-director.asm').read_text()
for marker in ['105020b3 MOV AL,byte ptr [EBX + 0x6c]','105020e3 MOV byte ptr [EBX + 0x6c],0x1','1050212b JBE 0x105021df','10502136 DIVSS XMM0,dword ptr [ESP + 0x10]','105021e2 MOVSS dword ptr [EBX + 0x68],XMM0','10502215 JBE 0x105022c7','10502220 DIVSS XMM0,dword ptr [ESP + 0x10]','105022d8 MOV dword ptr [EDX],EAX','105022f2 MOV dword ptr [EDX + 0xc],ECX']:assert marker in asm,marker
power_asm=(root/'analysis/decompiled/skeletal-quat-from-matrix.asm').read_text().split('# 10146d40')[1]
assert '10146d46 CALL 0x10146b80' in power_asm and '10146d5d JBE 0x10146dbb' in power_asm and '10146d6f FSTP float ptr [ESP + 0x10]' in power_asm
sources=['crates/rc-package/src/skeletal_director_rotation.rs','crates/rc-package/src/skeletal_director_rotation_tests.rs','crates/rc-inspect/src/bin/rc-director-rotation-check.rs','scripts/Record-DirectorRotations.py','scripts/Record-MatrixQuaternions.py','analysis/decompiled/skeletal-director.asm','analysis/decompiled/skeletal-quat-angles.c','analysis/decompiled/skeletal-quat-angles.asm','analysis/decompiled/skeletal-quat-from-matrix.asm']
result={'date':'2026-10-07','rust_tests':312,'scope':report['scope'],'counts':dict(counts),'source_sha256':{s:sha(root/s) for s in sources},'report_sha256':sha(path),'source_report_sha256':sha(source_path),'binary_sha256':{str(p):sha(p) for p in [core,engine]},'checks':['cargo test --workspace: 312 passed','cargo clippy --workspace --all-targets -- -D warnings','cargo fmt --all -- --check','3600 rotation preparation results, matrices, scales and mutated director snapshots independently recomputed bit-exact under portable math policy','original ASM branch/write markers and zero gate constants verified','eight tests include ordered limits, NaN gates, padding-byte preservation and six successive failure snapshots','transcendentals and x87 values approximate with f64; no claim of native x86 bit identity']}
(root/'analysis/reports/director-rotations-validation.json').write_text(json.dumps(result,indent=2)+'\n',encoding='utf-8')
path=root/'analysis/evidence.json';e=json.loads(path.read_text(encoding='utf-8-sig'));e['rust_tests']=312;e['director_rotations_validation']=result;path.write_text(json.dumps(e,indent=2,ensure_ascii=False)+'\n',encoding='utf-8')
note=('2026-10-07: Director-Rotationsvorbereitung mit History, zeitlicher und absoluter Winkelbegrenzung rekonstruiert. '
      'History-Initialisierung mit Byte-Padding-Erhalt, Budget-Reset, begrenzungsbedingter Matrixneuaufbau/Skalenreset und Abschluss-History. '
      '3600 Originaltrack-abgeleitete Diagnosefaelle inklusive mutierter Snapshots unabhaengig unter portabler Math-Policy bitgenau geprueft. '
      'Acht neue Tests; 312 Tests, Clippy und Format bestanden. Anwendung auf Knochen/Vorfahren und Runtimebindung offen; Android zum Schluss. '
      'Details D:\\Rust Projects\\RepublicCommandoAndroid\\docs\\SKELETAL_DIRECTOR_ROTATION.md.')
wiki=Path(r'C:\Users\brand\Documents\Main\05 KI\Project_Knowledge\projects\republic_commando_android')
for path in [root/'README.md',wiki/'index.md',wiki/'architecture/porting.md',wiki/'log.md']:
    if 'Director-Rotationsvorbereitung mit History, zeitlicher und absoluter Winkelbegrenzung' not in path.read_text(encoding='utf-8-sig'):
        with path.open('a',encoding='utf-8') as stream:stream.write('\n\n'+note+'\n')
print(json.dumps(dict(counts)))
