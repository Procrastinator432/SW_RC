"""Independent original layout reads, material bindings, bank and vertex oracle."""
from pathlib import Path
import json,struct,re,math,hashlib,collections
root=Path(__file__).resolve().parents[1]
bits=lambda x:struct.unpack('<I',struct.pack('<f',x))[0]
value=lambda x:struct.unpack('<f',struct.pack('<I',x))[0]
f=lambda x:value(bits(x))
sha=lambda p:hashlib.sha256(p.read_bytes()).hexdigest()
transpose=lambda m:[list(row) for row in zip(*m)]
identity=[[bits(1.) if r==c else 0 for c in range(4)] for r in range(4)]
source=root/'scripts/Record-VertexShader.py';v={'__file__':str(source)}
exec(compile(source.read_text().split('textures=[]')[0],str(source),'exec'),v)
original=v['original'];props=v['props'];meta=v['meta'];properties=v['properties'];first=v['ns']['first']
def bindings(field):
    result=[]
    for p in props:
        if p['name']!=field:continue
        q,_=properties(p['raw'],meta,(0,0,0,0,len(p['raw']),0));raw=first(q,'Value');plane,_=properties(raw,meta,(0,0,0,0,len(raw),0))
        result.append({'slot':p['slot'],'kind':(first(q,'Type') or b'\0')[0],'value':[struct.unpack('<f',first(plane,a) or bytes(4))[0] for a in 'XYZW']})
    return result
vs=bindings('VSConstants');ps=bindings('PSConstants')
ctx={'root':root,'re':re,'f':f,'bits':bits,'value':value,'identity':[v for row in identity for v in row]}
s=(root/'scripts/Record-WorldDirectorPoses.py').read_text();exec(compile(s[s.index('def offset'):s.index('# Build expression')],'<Core inverse>','exec'),ctx)
def inverse(m):
    flat=ctx['native_inverse']([v for row in m for v in row]);return [flat[r*4:r*4+4] for r in range(4)]
composition={'root':root,'re':re,'struct':struct,'f':f,'bits':bits,'value':value}
s=(root/'scripts/Record-ShaderMatrices.py').read_text();exec(compile(s[s.index('asm_path=root/'):s.index('def fixture')],'<matrix instructions>','exec'),composition)
lights={'f':f,'bits':bits,'value':value}
s=(root/'scripts/Record-ShaderLights.py').read_text();exec(compile(s[s.index('def selection'):s.index('counts=collections')],'<selection/color>','exec'),lights)
positions={'root':root,'re':re,'f':f,'bits':bits,'value':value}
s=(root/'scripts/Record-LightPositions.py').read_text();exec(compile(s[s.index('def point'):s.index('context=')],'<position instructions>','exec'),positions)
scene={'__file__':str(root/'scripts/Record-SceneConstants.py')}
s=(root/'scripts/Record-SceneConstants.py').read_text();exec(compile(s[:s.index('path=root/')],'<PE constants>','exec'),scene)
memory=scene['memory']
for address,symbol in [(0x1006f304,b'?GEngineTime@@3MA\0'),(0x1006f0c8,b'?GIsEditor@@3HA\0'),(0x1006f354,b'?GCubemapManager@@3PAVUCubemapManager@@A\0')]:
    hint=memory(address);assert bytes(memory(0x10000000+hint+2+k)&255 for k in range(len(symbol)))==symbol
asm=(root/'analysis/decompiled/shader-constants.asm').read_text()
for marker in ['1001c1dd MOV EAX,[0x1006f304]','1001c238 MOV ECX,dword ptr [EAX + 0x2c]','1001e116 TEST EAX,EAX','1001e11a LEA ESI,[EAX + 0x54]','1001e1e8 MULSS XMM0,dword ptr [EAX + 0x138]','1001e2d4 MULSS XMM1,dword ptr [EBP + -0x6c]','1001e6a0 CMP byte ptr [EDX + 0x2a],0x13','1001df38 MOVSS XMM1,dword ptr [ECX + 0x2f4]']:
    assert marker in asm,marker
path=root/'analysis/reports/shader-snapshots.json';report=json.loads(path.read_text());fixtures=json.loads((root/'analysis/reports/shader-snapshots.input.json').read_text())['cases']
assert path.read_bytes()==(root/'analysis/reports/shader-snapshots-release.json').read_bytes()
assert report['vs']==vs and report['ps']==ps and len(vs)==20 and len(ps)==2
assert len(report['probes'])==len(fixtures)==264
counts=collections.Counter();bank={'count':-2,'words':[[0]*4 for _ in range(96)]};bank['words'][17]=[bits(.25)]*4;pixel={'count':-3,'words':[[0]*4 for _ in range(8)]};state={'last_time':None,'samples':[.5]*3}
for case,p in zip(fixtures,report['probes']):
    i=case['id'];assert p['id']==i
    if i>=256:assert case['expected'] is None and isinstance(p['capture_error'],str);counts['capture_errors']+=1;continue
    # Independent random-access dictionary, rather than the Rust region reader.
    mem={}
    for region in case['regions']:
        for k,byte in enumerate(region['bytes']):
            at=region['address']+k;assert at not in mem;mem[at]=byte
    def word(address):return struct.unpack('<I',bytes(mem[address+k] for k in range(4)))[0]
    def words(address,n):return [word(address+4*k) for k in range(n)]
    def matrix(address):return [words(address+16*r,4) for r in range(4)]
    renderer=case['renderer'];globals=case['globals'];render_state=word(renderer+0x9c0c);viewport=word(renderer+8)
    editor=word(globals['editor'])!=0;time=word(globals['engine_time']);manager=word(globals['cubemap_manager']);actor=word(manager+0x2c) if manager else 0
    camera=word(viewport+0x184) if not editor else 0;editor_actor=word(viewport+0x30) if editor else 0
    slots=[]
    for k in range(4):
        source=word(render_state+0x328+4*k)
        if source==0:slots.append(None);continue
        la=word(source)
        if la==0:
            slots.append({'actor_present':False,'kind':0,'position':[0]*3,'cone':0,'color':[0]*4,'brightness':0,'direction':[0]*3,'radius':[0]*2,'flags':[0]*2});counts['null_light_actors']+=1;continue
        slots.append({'actor_present':True,'kind':mem[la+0x2a],'cone':mem[la+0x39],'color':words(source+8,4),'position':words(source+0x18,3),'direction':words(source+0x24,3),'radius':words(source+0x30,2),'flags':words(source+0x38,2),'brightness':word(source+0x48)})
    h={'object':matrix(render_state+0x2c),'view':matrix(render_state+0x6c),'projection':matrix(render_state+0xac),'camera':matrix(camera+0x54) if camera else None,'time':time,'editor':editor,'actor_scale':words(actor+0x1b4,3) if actor else None,'editor_eye':words(editor_actor+0x138,3) if editor else [0]*3,'runtime_eye':words(camera+0x194,3) if camera else None,'fog':words(render_state+0x2f0,2),'lighting':{'slots':slots,'alpha_gate':bool(actor and word(actor+0x64)&0x2000==0),'ambient_bgra':word(render_state+0x148)}}
    assert p['host']==h==case['expected'],i
    assert p['addresses']=={'renderer':renderer,'state':render_state,'viewport':viewport,'camera':camera,'editor_actor':editor_actor,'cubemap_actor':actor} and p['direct_snapshot_equal']
    q=p['pipeline'];assert q['before']=={'vertex':bank,'pixel':pixel,'flicker':state},i
    calls=[];draws=[];error=None;obj_inv=None;bank={'count':32,'words':[row[:] for row in bank['words']]}
    for b in vs:
        slot=b['slot'];kind=b['kind'];plane=b['value']
        if kind==0:continue
        if kind==1:bank['words'][slot]=list(map(bits,plane))
        elif kind==3:bank['words'][slot:slot+4]=composition['original'](h['object'],h['view'],h['projection'])[1]
        elif kind==4:bank['words'][slot:slot+4]=transpose(h['object'])
        elif kind==14:
            selected=lights['select'](h['lighting'],0)
            if 'Err' in selected:error=selected['Err'];break
            light=selected['Ok']
            if light is None:bank['words'][slot]=[bits(10000000.)]*3+[0]
            else:
                calls.append(h['object']);obj_inv=inverse(h['object']);point=positions['point'](light,h['object']);bank['words'][slot]=positions['transformed'](point,obj_inv,light['kind']==19)
        elif kind==12:
            if h['camera'] is not None:cam=h['camera']
            else:calls.append(h['view']);cam=inverse(h['view'])
            bank['words'][slot]=cam[3]
        elif kind==9:bank['words'][slot]=[bits(f(math.cos(f(math.fmod(value(time),120.))*(plane[0] or 1.))))]*4
        elif kind==21:
            selected=lights['select'](h['lighting'],2)
            if 'Err' in selected:error=selected['Err'];break
            bank['words'][slot]=lights['color'](selected['Ok'],h['lighting']['alpha_gate'])
        elif kind==27:
            current=value(time)
            if state['last_time'] is None:state['last_time']=current
            if state['last_time']!=current:
                state['last_time']=current;draws=[(i*73+j*127)%32768 for j in range(3)];state['samples']=[f(r*value(0x38000100)) for r in draws]
            a,bb,c=state['samples'];amp=plane[1];base=f(1.-amp)
            bank['words'][slot]=[bits(1. if test<=plane[0] else f(f(amp*other)+base)) for test,other in [(a,c),(bb,a),(c,bb)]]+[bits(1.)]
        else:raise AssertionError(kind)
    if error is None:
        pixel={'count':2,'words':[row[:] for row in pixel['words']]}
        for b in ps:assert b['kind']==1;pixel['words'][b['slot']]=list(map(bits,b['value']))
        vertices=[[0.]*4 for _ in range(16)];vertices[0]=[0.,0.,.125,1.];vertices[1]=[1.,0.,0.,0.];vertices[2]=[.25,.75,0.,1.]
        out,defined,_=v['algorithm'](vertices,[[value(x) for x in row] for row in bank['words']],False)
        assert q['vertex_output']=={'Ok':{'words':[[bits(x) for x in row] for row in out],'defined':defined}},(i,'vertex')
        counts['vertex_outputs']+=1
    else:assert q['vertex_output'] is None;counts['expected_dispatch_errors']+=1
    assert q['status']==({'Ok':None} if error is None else {'Err':error}),i
    assert q['vertex']==bank and q['pixel']==pixel and q['flicker']==state and q['rng']==draws and q['inverse_calls']==calls,(i,'bank/state')
    counts['snapshots']+=1;counts['bank_words']+=(96+8)*4*2;counts['inverse_callbacks']+=len(calls);counts['rng_draws']+=len(draws);counts['editor_snapshots']+=editor;counts['runtime_null_camera']+=not editor and camera==0
tests=sum(map(int,re.findall(r'test result: ok\. (\d+) passed',(root/'analysis/reports/shader-snapshots-tests.log').read_text())));assert tests==587,tests
sources=['crates/rc-package/src/shader_snapshot.rs','crates/rc-package/src/shader_constants.rs','crates/rc-package/src/shader_lights.rs','crates/rc-package/src/skeletal_matrix_inverse.rs','crates/rc-package/src/vertex_shader.rs','crates/rc-inspect/src/bin/rc-shader-snapshot-check.rs','scripts/Generate-ShaderSnapshots.py','scripts/Record-ShaderSnapshots.py','analysis/decompiled/shader-constants.asm','scripts/Record-VertexShader.py','scripts/Record-WorldDirectorPoses.py','scripts/Record-ShaderMatrices.py','scripts/Record-ShaderLights.py','scripts/Record-LightPositions.py','scripts/Record-SceneConstants.py']
validation={'date':'2026-10-09','counts':dict(counts),'rust_tests':tests,'debug_release_identical':True,'input_sha256':sha(root/'analysis/reports/shader-snapshots.input.json'),'report_sha256':sha(path),'original_dll_sha256':sha(scene['dll']),'source_sha256':{s:sha(root/s) for s in sources},'scope':report['scope'],'android':'deferred until end per user'}
(root/'analysis/reports/shader-snapshots-validation.json').write_text(json.dumps(validation,indent=2)+'\n')
ledger=root/'analysis/evidence.json';evidence=json.loads(ledger.read_text(encoding='utf-8'));evidence['rust_tests']=tests;evidence['shader_snapshot_validation']=validation;ledger.write_text(json.dumps(evidence,indent=2,ensure_ascii=False)+'\n',encoding='utf-8')
print(json.dumps({'counts':dict(counts),'rust_tests':tests}))
