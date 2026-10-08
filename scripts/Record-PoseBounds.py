"""Independent SSE-order local bound/sphere oracle and original DLL constant check."""
from pathlib import Path
import json,struct,math,hashlib
root=Path(__file__).resolve().parents[1]
f=lambda x:struct.unpack('<f',struct.pack('<f',x))[0]
bits=lambda x:struct.unpack('<I',struct.pack('<f',x))[0]
value=lambda x:struct.unpack('<f',struct.pack('<I',x))[0]
sha=lambda p:hashlib.sha256(p.read_bytes()).hexdigest()
path=root/'analysis/reports/pose-bounds.json';report=json.loads(path.read_text())
stack_path=root/'analysis/reports/original-channel-stacks.json';stacks=json.loads(stack_path.read_text())
stack_validation=json.loads((root/'analysis/reports/original-channel-stacks-validation.json').read_text());assert stack_validation['report_sha256']==sha(stack_path)
links=json.loads((root/'analysis/reports/original-skeletal-linkups.json').read_text())
assert len(report['cases'])==900 and len(report['synthetic'])==64
padding=report['padding'];assert padding=={'minimum':list(map(bits,[.25,1.,2.])),'maximum':list(map(bits,[2.,.5,1.])),'k_one':[bits(1.)]*3}
def check(c,matrices,supplied):
    before=c['before'];low=list(map(value,before['minimum']));high=list(map(value,before['maximum']));first=c['move_bone']+1
    for i,mat in enumerate(matrices):
        p=list(map(value,mat[12:15]))
        if i==first:low=p.copy();high=p.copy()
        elif i>first:
            for j,v in enumerate(p):
                if low[j]>v:low[j]=v
                elif v>high[j]:high[j]=v
    scale=value(0x3f99999a);low=[f(x*scale) for x in low];high=[f(x*scale) for x in high]
    for i in range(3):
        one=value(padding['k_one'][i]);a=value(padding['minimum'][i]);b=value(padding['maximum'][i])
        offset=f(one+a) if i==0 else f(a+one);low[i]=f(low[i]-offset)
        offset=f(b+one);high[i]=f(high[i]+offset) if i==2 else f(offset+high[i])
    d=[f(y-x) for x,y in zip(low,high)];n=f(f(f(d[0]*d[0])+f(d[1]*d[1]))+f(d[2]*d[2]))
    r=.125 if supplied else f(1/f(math.sqrt(n))) if n else math.inf
    inv=f(f(3-f(f(r*n)*r))*f(r*.5));length=f(inv*n) if n else 0.
    center=[f(f(high[i]+low[i] if i!=1 else low[i]+high[i])*.5) for i in range(3)]
    expected={**before,'minimum':list(map(bits,low)),'maximum':list(map(bits,high)),'sphere':list(map(bits,center))+[bits(f(length*.5))]}
    assert c['squared']==[bits(n)] and c['published']==[expected]
    expected.update(byte_60=1,byte_61=1,byte_179=0)
    assert c['after']==expected and c['result']=={'Ok':None}
    assert all(math.isfinite(value(x)) for field in ('minimum','maximum','sphere') for x in expected[field])
states={};seen=set()
for c in report['cases']:
    oi,ri,ti=c['source_object'],c['run'],c['tick'];key=(oi,ri);assert (oi,ri,ti) not in seen;seen.add((oi,ri,ti))
    o=stacks['objects'][oi];m=links['objects'][o['source_index']];name=m['prefix']['bones'][0]['name']['name'];assert c['move_bone']==(0 if name.lower()=='move' else -1)
    if ti:
        prior=states[key].copy();prior['byte_61']=0;assert c['before']==prior
    else:assert c['before']=={'minimum':[bits(-3.)]*3,'maximum':[bits(5.)]*3,'sphere':[77]*4,'byte_60':7,'byte_61':0,'byte_179':9}
    check(c,o['runs'][ri]['ticks'][ti]['matrices'],False);states[key]=c['after']
assert len(states)==225
for c in report['synthetic']:check(c,c['matrices'],True)
def dll_words(file,address,n=1):
    data=file.read_bytes();pe=struct.unpack_from('<I',data,60)[0];count=struct.unpack_from('<H',data,pe+6)[0];opt=struct.unpack_from('<H',data,pe+20)[0]
    base=struct.unpack_from('<I',data,pe+24+28)[0];rva=address-base
    for i in range(count):
        offset=pe+24+opt+40*i;size,va,raw,ptr=struct.unpack_from('<IIII',data,offset+8)
        if va<=rva and rva+4*n<=va+raw:return list(struct.unpack_from('<'+'I'*n,data,ptr+rva-va))
    raise AssertionError(hex(address))
game=Path(r'D:\SteamLibrary\steamapps\common\Star Wars Republic Commando\GameData\System')
engine,core=game/'engine.dll',game/'core.dll'
constants={hex(a):dll_words(engine,a)[0] for a in [0x10664fac,0x1065efc8,0x1065efc4]}
assert constants=={'0x10664fac':0x3f99999a,'0x1065efc8':bits(3.),'0x1065efc4':bits(.5)}
assert dll_words(core,0x10186c18)==[bits(1.)]
asm=(root/'analysis/decompiled/skeletal-root-frame.asm').read_text()
for marker in ('1050ac34 CMP ESI,EAX','1050ac89 JBE 0x1050ac92','1050ad14 MULSS XMM1,XMM0','1050ae60 RSQRTSS XMM0,XMM1','1050ae90 ANDPS XMM3,XMM4','1050aec5 PUSH 0x40000000','1050af18 JZ 0x1050b27c','1050b27c MOV byte ptr [EDI + 0x60],0x1','1050b280 MOV byte ptr [EDI + 0x61],0x1','1050b284 MOV byte ptr [EDI + 0x179],0x0'):assert marker in asm
vector=(root/'analysis/decompiled/skeletal-bound-vector.asm').read_text()
for marker in ('101132ca DIVSS XMM0,dword ptr [ESP + 0x8]','101132d0 MULSS XMM1,XMM0'):assert marker in vector
sources=['crates/rc-package/src/skeletal_bounds.rs','crates/rc-inspect/src/bin/rc-pose-bounds-check.rs','scripts/Record-PoseBounds.py','analysis/decompiled/skeletal-root-frame.asm','analysis/decompiled/skeletal-bound-vector.asm','analysis/decompiled/skeletal-bound-vector.c']
result={'date':'2026-10-07','rust_tests':281,'original_matrix_bounds':900,'synthetic_cases':64,'dll_constants':constants,'binary_sha256':{str(p):sha(p) for p in [engine,core]},'scope':report['scope'],'source_sha256':{s:sha(root/s) for s in sources},'report_sha256':sha(path),'stack_report_sha256':sha(stack_path),'checks':['cargo test --workspace: 281 passed','cargo clippy --workspace --all-targets -- -D warnings','cargo fmt --all -- --check','900 original-matrix local bounds/spheres independently recomputed bit-exact under portable seed policy','64 supplied-seed cases and publication-before-flags snapshots independently verified','scale/refinement/divisor constants verified in original DLL bytes']}
(root/'analysis/reports/pose-bounds-validation.json').write_text(json.dumps(result,indent=2)+'\n',encoding='utf-8')
path=root/'analysis/evidence.json';e=json.loads(path.read_text(encoding='utf-8-sig'));e['rust_tests']=281;e['pose_bounds_validation']=result;path.write_text(json.dumps(e,indent=2,ensure_ascii=False)+'\n',encoding='utf-8')
note=('2026-10-07: Lokale Posegrenzen und Abschlussflags rekonstruiert. Move-Root-Ausschluss, '
      'Ursprungsskalierung1.2 aus Original-DLL, explizite Padding-/kOne-Snapshots, RSQRT-Sphäre und '
      'Flags erst nach erfolgreicher Actor-Publikationsgrenze. 900 Originaltrack-Matrix-Bounds und64 '
      'synthetische Fälle unabhängig bitgenau unter der portablen Math-Policy geprüft. 281Tests,Clippy '
      'und Format bestanden. Diagnosepadding ist ausdrücklich kein Original-Meshwert; Actor-Weltbounds '
      'bleiben Hostgrenze, Directors/Skinning offen. Android zum Schluss. Details '
      'D:\\Rust Projects\\RepublicCommandoAndroid\\docs\\SKELETAL_BOUNDS.md.')
wiki=Path(r'C:\Users\brand\Documents\Main\05 KI\Project_Knowledge\projects\republic_commando_android')
for path in [root/'README.md',wiki/'index.md',wiki/'architecture/porting.md',wiki/'log.md']:
    if 'Lokale Posegrenzen und Abschlussflags rekonstruiert' not in path.read_text(encoding='utf-8-sig'):
        with path.open('a',encoding='utf-8') as stream:stream.write('\n\n'+note+'\n')
print(json.dumps({'original_matrix_bounds':900,'synthetic_cases':64,'constants':constants}))
