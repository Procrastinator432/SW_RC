"""Independently check supplied track timing, SSE position arithmetic and write order."""
from pathlib import Path
from collections import Counter
import hashlib,json,math,struct
root=Path(__file__).resolve().parents[1]
f32=lambda x:struct.unpack('<f',struct.pack('<f',x))[0]
bits=lambda x:struct.unpack('<I',struct.pack('<f',x))[0]
value=lambda x:struct.unpack('<f',struct.pack('<I',x))[0]
report_path=root/'analysis/reports/skeletal-track.json'
report=json.loads(report_path.read_text())
assert len(report['probes'])==240
variants=['Timed','SingletonRotation','SingletonPosition','SingletonBoth','NoDurations','NegativeDurationCount','ZeroDurations','MissingPosition','DecodeBoundary','SlerpBoundary','TruncatedDurations','TruncatedRotations']
times=[-5.,-0.,0.,0.25,1.,2.,2.25,4.,5.,7.5,9.,10.,10.25,12.,15.,25.,100.,float('nan'),float('inf'),-float('inf')]
counts=Counter()
for row,p in enumerate(report['probes']):
    mode=row//20; time=value(p['time_bits']);t=p['track']
    assert p['variant']==variants[mode]
    assert p['time_bits']==bits(times[row%20])
    expected={'rotations':[[10,11,12],[20,21,22],[30,31,32]],'rotation_count_word':0xa0000003,
              'positions':[[0,-32767,32767],[32767,0,-32767],[-32767,32767,0]],'position_count_word':0xe0000003,
              'position_scale_bits':bits(2.),'durations':[2,3,5],'duration_count_word':0xa0000003}
    if mode in (1,3):expected['rotation_count_word']=0xe0000001;expected['rotations']=expected['rotations'][:1]
    if mode in (2,3):expected['position_count_word']=0xa0000001;expected['positions']=expected['positions'][:1]
    if mode in (4,5):expected['duration_count_word']=0 if mode==4 else 0x1fffffff;expected['durations']=[]
    if mode==6:expected['durations']=[0,0,0]
    if mode==7:expected['positions']=[]
    if mode==10:expected['durations']=[2]
    if mode==11:expected['rotations']=expected['rotations'][:1]
    assert t==expected
    out={'rotation':[7]*4,'position':[8]*3};assert p['before']==out
    events=[];error=None
    try:
        n=t['duration_count_word'] & 0x1fffffff
        if n & 0x10000000:n-=0x20000000
        index=0;residual=time;duration=0.;alpha=None
        if n>1:
            for index in range(n):
                if index>=len(t['durations']):raise ValueError('missing duration key')
                duration=float(t['durations'][index]);after=f32(residual-duration)
                if after<0.:break
                residual=after
            else:index=0;duration=float(t['durations'][0])
            if residual>0.:alpha=bits(f32(residual/duration)) if duration else bits(float('inf'))
        nxt=(index+1)%n if alpha is not None else index
        def decode(i):
            if i>=len(t['rotations']):raise ValueError('missing rotation key')
            key=t['rotations'][i];events.append({'decode':key})
            if mode==8:raise ValueError('unresolved rotation decode')
            return key+[42]
        single_r=t['rotation_count_word']&0x1fffffff==1
        if alpha is None or single_r:out['rotation']=decode(0 if single_r else index)
        else:
            b=decode(nxt);a=decode(index);events.append({'slerp':[a,b],'alpha_bits':alpha})
            if mode==9:raise ValueError('unresolved Slerp')
            out['rotation']=[a[0],b[0],alpha,43]
        def position(i):
            if i>=len(t['positions']):raise ValueError('missing position key')
            scale=f32(value(t['position_scale_bits'])*value(0x38000100))
            return [f32(v*scale) for v in t['positions'][i]]
        single_p=t['position_count_word']&0x1fffffff==1
        if alpha is None or single_p:out['position']=list(map(bits,position(0 if single_p else index)))
        else:
            b=position(nxt);a=position(index);factor=value(alpha)
            out['position']=[bits(f32(f32(f32(y-x)*factor)+x)) for x,y in zip(a,b)]
        result={'Ok':{'current':index,'next':nxt,'alpha_bits':alpha}}
    except ValueError as e:error=str(e);result={'Err':error}
    assert p['result']==result,(row,p['result'],result)
    assert p['after']==out,(row,p['after'],out)
    assert p['events']==events,(row,p['events'],events)
    counts[error or 'SuppliedRotationCompleted']+=1
asm=(root/'analysis/decompiled/skeletal-get-rot-pos.asm').read_text()
for marker in ('10500fd8 SAR EAX,0x3','10501019 MOVSS XMM0,dword ptr [ESI + 0x10]',
               '105010a2 JA 0x105010b8','105010bb JBE 0x10501239','105010c8 XOR EBX,EBX',
               '1050111b CALL 0x104fdc60','1050112d CALL 0x104fdc60','105011e2 SUBSS XMM0,XMM4',
               '105003f6 CMP dword ptr [EAX + 0x4],EDX'):assert marker in asm,marker
rotation_asm=(root/'analysis/decompiled/skeletal-rotation-decode.asm').read_text()
assert '104fdd11 RSQRTSS XMM0,XMM1' in rotation_asm
game=Path(r'D:\SteamLibrary\steamapps\common\Star Wars Republic Commando\GameData\System\engine.dll')
data=game.read_bytes();pe=struct.unpack_from('<I',data,60)[0];o=pe+24
base=struct.unpack_from('<I',data,o+28)[0];size=struct.unpack_from('<H',data,pe+20)[0]
sec=[struct.unpack_from('<IIII',data,o+size+i*40+8) for i in range(struct.unpack_from('<H',data,pe+6)[0])]
offset=next(raw+0x10664f44-base-start for _,start,n,raw in sec if start<=0x10664f44-base<start+n)
assert data[offset:offset+4]==struct.pack('<I',0x38000100)
sha=lambda p:hashlib.sha256(p.read_bytes()).hexdigest()
sources=['crates/rc-package/src/skeletal_track.rs','crates/rc-inspect/src/bin/rc-skeletal-track-probe.rs',
         'analysis/decompiled/skeletal-get-rot-pos.c','analysis/decompiled/skeletal-get-rot-pos.asm',
         'analysis/decompiled/skeletal-rotation-decode.c','analysis/decompiled/skeletal-rotation-decode.asm','scripts/Record-SkeletalTrack.py']
validation={'date':'2026-10-07','rust_tests':242,'synthetic_track_cases':240,'outcomes':dict(counts),
            'report_sha256':sha(report_path),'engine_dll_sha256':sha(game),'source_sha256':{s:sha(root/s) for s in sources},
            'checks':['cargo test --workspace: 242 passed','cargo clippy --workspace --all-targets -- -D warnings','cargo fmt --all -- --check',
                      '240 independent key/position/output/event calculations','native ASM branches, MOVSS float scale and PE constant'],
            'scope':report['scope']}
(root/'analysis/reports/skeletal-track-validation.json').write_text(json.dumps(validation,indent=2)+'\n',encoding='utf-8')
path=root/'analysis/evidence.json';evidence=json.loads(path.read_text(encoding='utf-8-sig'))
evidence['rust_tests']=242;evidence['skeletal_track_validation']=validation
path.write_text(json.dumps(evidence,ensure_ascii=False,indent=2)+'\n',encoding='utf-8')
note=('2026-10-07: GetRotPos-Schlüsselwahl, Positionsdecodierung/-interpolation und geordnete Linkup-Suche ergänzt. '
      'Originalassembler belegt Floatmaßstab, signierte Daueranzahl, einmaligen Zeitdurchlauf statt Modulo, '
      'Singleton-Keys und Rotation-vor-Position-Schreibreihenfolge. 240 synthetische Trackfälle unabhängig geprüft; '
      'Quaternion-Decodierung/Slerp ausdrücklich geliefert oder offen, RSQRTSS in Originaldecodierung identifiziert. '
      '242 Workspace-Tests, Clippy und Format bestanden. Originaltracks, x87-Framezeit und Vollpose offen; Android zum Schluss. '
      'Details D:\\Rust Projects\\RepublicCommandoAndroid\\docs\\SKELETAL_TRACK.md.')
wiki=Path(r'C:\Users\brand\Documents\Main\05 KI\Project_Knowledge\projects\republic_commando_android')
for path in [root/'README.md',wiki/'index.md',wiki/'architecture/porting.md',wiki/'log.md']:
    if 'GetRotPos-Schlüsselwahl, Positionsdecodierung/-interpolation und geordnete Linkup-Suche ergänzt' not in path.read_text(encoding='utf-8-sig'):
        with path.open('a',encoding='utf-8') as f:f.write('\n\n'+note+'\n')
print(json.dumps({'cases':240,'outcomes':dict(counts),'rust_tests':242}))
