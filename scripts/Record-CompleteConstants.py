"""Independent original-instruction and mixed-bank oracle for all kinds 0..34."""
from pathlib import Path
import json,struct,math,re,hashlib,collections
root=Path(__file__).resolve().parents[1]
bits=lambda x:struct.unpack('<I',struct.pack('<f',x))[0]
value=lambda x:struct.unpack('<f',struct.pack('<I',x))[0]
f=lambda x:value(bits(x))
sha=lambda p:hashlib.sha256(p.read_bytes()).hexdigest()
transpose=lambda m:[list(row) for row in zip(*m)]
identity=[[bits(1.) if r==c else 0 for c in range(4)] for r in range(4)]
# Import read-only instruction interpreters, stopping before their report mutations.
scene={'__file__':str(root/'scripts/Record-SceneConstants.py')}
s=(root/'scripts/Record-SceneConstants.py').read_text();exec(compile(s[:s.index("path=root/")],'<scene instructions>','exec'),scene)
asm=scene['asm'];memory=scene['memory']
hint=memory(0x1006f354)
symbol=b'?GCubemapManager@@3PAVUCubemapManager@@A\0'
assert bytes(memory(0x10000000+hint+2+k)&255 for k in range(len(symbol)))==symbol
for kind,target in [(30,0x1001dcb0),(34,0x1001e09c)]:
    index=memory(0x1001ed44+kind-1)&255
    assert memory(0x1001ecf0+index*4)==target
assert any(a==0x1001c238 and args=='ECX,dword ptr [EAX + 0x2c]' for a,op,args in asm)
assert any(a==0x1001dcba and args=='EAX,0x1b4' for a,op,args in asm)
ctx={'root':root,'re':re,'f':f,'bits':bits,'value':value,'identity':[v for row in identity for v in row]}
s=(root/'scripts/Record-WorldDirectorPoses.py').read_text();exec(compile(s[s.index('def offset'):s.index('# Build expression')],'<Core inverse>','exec'),ctx)
def inverse(m):
    flat=ctx['native_inverse']([v for row in m for v in row]);return [flat[r*4:r*4+4] for r in range(4)]
composition={'root':root,'re':re,'struct':struct,'f':f,'bits':bits,'value':value}
s=(root/'scripts/Record-ShaderMatrices.py').read_text();exec(compile(s[s.index('asm_path=root/'):s.index('def fixture')],'<matrix instructions>','exec'),composition)
lights={'f':f,'bits':bits,'value':value}
s=(root/'scripts/Record-ShaderLights.py').read_text();exec(compile(s[s.index('def selection'):s.index('counts=collections')],'<light equations>','exec'),lights)
positions={'root':root,'struct':struct,'re':re,'f':f,'bits':bits,'value':value}
s=(root/'scripts/Record-LightPositions.py').read_text();exec(compile(s[s.index('def point'):s.index('context=')],'<light position instructions>','exec'),positions)
def draw(p):
    if p['actor_scale'] is not None:return p['actor_scale']+[0]
    stack={};registers={};masks={};eax=0;outputs={};seed_index=0
    def addr(op):
        m=re.fullmatch(r'dword ptr \[(EBP|EAX|ECX)(?: \+ (-?0x[0-9a-f]+))?\]',op);assert m,op
        offset=int(m[2],16) if m[2] else 0
        if offset>=2**31:offset-=2**32
        return m[1],offset
    def operand(op):
        if op.startswith('XMM'):return registers[op]
        absolute=re.fullmatch(r'dword ptr \[(0x[0-9a-f]+)\]',op)
        if absolute:return value(memory(int(absolute[1],16)))
        base,offset=addr(op)
        if base=='EBP':return stack[offset]
        assert base=='EAX';index=(eax+offset-0x2c)//4
        return value(p['object'][index//4][index%4])
    for a,op,args in asm:
        if not 0x1001dcfb<=a<=0x1001df24:continue
        if op=='MOV' and args=='EAX,dword ptr [EDX + 0x9c0c]':eax=0;continue
        if op=='ADD' and args.startswith('EAX,'):eax+=int(args.split(',')[1],16);continue
        if op not in ('MOVSS','MOVAPS','MULSS','ADDSS','SUBSS','XORPS','RSQRTSS','CMPNEQSS','ANDPS'):continue
        dst,src=args.split(',',1)
        if not dst.startswith('XMM'):
            if dst=='dword ptr [ECX + EDI*0x1]':outputs[0]=bits(operand(src));continue
            base,offset=addr(dst)
            if base=='EBP':stack[offset]=operand(src)
            else:assert base=='ECX';outputs[offset]=bits(operand(src))
            continue
        if op=='XORPS':registers[dst]=0.;continue
        if op=='RSQRTSS':
            assert bits(operand(src))==p['draw_squared'][seed_index]
            registers[dst]=value(p['draw_seeds'][seed_index]);seed_index+=1;continue
        if op=='CMPNEQSS':masks[dst]=registers[dst]!=operand(src);continue
        if op=='ANDPS':
            if not masks[src]:registers[dst]=0.
            continue
        v=operand(src)
        if op=='MULSS':v=f(registers[dst]*v)
        elif op=='ADDSS':v=f(registers[dst]+v)
        elif op=='SUBSS':v=f(registers[dst]-v)
        registers[dst]=v
    assert seed_index==3
    return [outputs[k*4] for k in range(4)]
path=root/'analysis/reports/complete-constants.json';report=json.loads(path.read_text());data=(root/'analysis/reports/complete-constants.input.bin').read_bytes();native=(root/'analysis/reports/complete-constants.x87.bin').read_bytes()
assert path.read_bytes()==(root/'analysis/reports/complete-constants-release.json').read_bytes()
assert data==(root/'analysis/reports/complete-constants-release.input.bin').read_bytes()
assert len(report['probes'])==256 and len(data)==256*8 and len(native)==256*36
counts=collections.Counter();previous=None;state=None
for n,p in enumerate(report['probes']):
    i,phase=divmod(n,2);time=f(i*.125+phase*.03125);rate=[0.,-0.,f(.1),2.,-1.,f(33.3),f(.37),64.][i%8]
    assert p['index']==i and p['pass']==phase and p['time']==bits(time) and p['rate']==bits(rate)
    assert p['object']==[[bits(f((2. if r==c else 0.)+(i+r*7+c*3)*.001953125)) for c in range(4)] for r in range(4)]
    assert p['actor_scale']==([0x80000000,0x7fc12345,i] if i%3==0 else None) and p['editor']==(i%2==0)
    source={'slots':[{'radius':list(map(bits,[4.,2.])),'kind':19 if (i+s)%2==0 else 7,'actor_present':True,'position':list(map(bits,[2.,3.,4.])),'direction':list(map(bits,[.125,.25,-.5])),'cone':[37,0,255,3][s],'color':list(map(bits,[.25,.5,.75,1.])),'brightness':bits(.75),'flags':[1,0] if i%3==0 else [0,0]} for s in range(4)],'alpha_gate':i%3==1,'ambient_bgra':0x99ff8040}
    source['alpha_gate']=i%3==0 and i%2==0
    assert p['lighting']==source
    portable=[]
    for r in range(3):
        squares=[f(value(p['object'][r][c])**2) for c in range(3)]
        q=f(f(squares[1]+squares[2])+squares[0]) if r==0 else f(f(squares[0]+squares[1])+squares[2])
        assert p['draw_squared'][r]==bits(q)
        seed=f(1./f(math.sqrt(q)));product=f(f(seed*q)*seed)
        portable.append(bits(f(f(f(3.-product)*f(seed*.5))*q)))
    assert p['portable_draw']==(p['actor_scale']+[0] if p['actor_scale'] is not None else portable+[bits(1.)])
    expected_bindings=[{'kind':0,'words':[0]*4} for _ in range(96)];slot=0
    for kind in (range(35) if phase==0 else reversed(range(35))):
        span=4 if kind in [2,3,4,5,6,7,32] else 2 if kind==34 else 1
        words=[bits(f(.1)),bits(f(.4)),0x7fc00000,0x7f800000] if kind==27 else [0x80000000,0x7fc12345,i,phase] if kind==1 else [bits(rate),0x7fc12345,7,8]
        expected_bindings[slot]={'kind':kind,'words':words}
        for k in range(slot+1,slot+span):expected_bindings[k]['kind']=255
        slot+=span
    assert p['bindings']==expected_bindings
    if phase==0:previous=[[0x7fc12345,0x80000000,7,i] for _ in range(96)];state={'last_time':None,'samples':[.5]*3}
    assert p['before']=={'count':-2,'words':previous} and p['flicker_before']==state
    result=[row[:] for row in previous];count=max(k for k,b in enumerate(expected_bindings) if b['kind'])+1
    assert count==(57 if phase==0 else 56)
    slot=0;object_cache=None;camera_cache=None;calls=[];rng=[]
    composed=composition['original'](p['object'],identity,identity)
    wrapped=f(math.fmod(time,120.));argument=f(wrapped*rate);cosine=bits(f(math.cos(argument)));sine=math.sin(argument)
    rotation=[[cosine,bits(f(-sine)),0,0],[bits(f(sine)),cosine,0,0]]
    assert data[n*8:n*8+8]==struct.pack('<ff',time,rate)
    assert struct.unpack_from('<9I',native,n*36)==tuple([bits(argument)]+[v for row in rotation for v in row]),(n,'rotation x87')
    while slot<count:
        binding=expected_bindings[slot];kind=binding['kind'];counts['case_'+str(kind)]+=1;rows=None;output=None
        if kind in [7,14,17,20,23,33] and object_cache is None:calls.append(p['object']);object_cache=inverse(p['object'])
        if kind in [5,12] and camera_cache is None:
            if p['editor']:calls.append(identity);camera_cache=inverse(identity)
            else:camera_cache=[row[:] for row in identity];camera_cache[3]=list(map(bits,[4.,5.,6.,1.]))
        if kind==0:pass
        elif kind==1:output=binding['words']
        elif kind in [2,3,32]:rows=composed[{2:0,3:1,32:2}[kind]]
        elif kind in [4,6,7,5]:rows=transpose(p['object'] if kind==4 else identity if kind==6 else object_cache if kind==7 else camera_cache)
        elif kind==12:output=camera_cache[3]
        elif kind in [8,9,10,11]:
            arg=wrapped*(rate if rate!=0 else 1.)
            v=wrapped if kind==8 else math.cos(arg) if kind==9 else math.sin(arg) if kind==10 else math.tan(arg)
            output=[bits(f(v))]*4
        elif kind==13:output=[bits(f(math.cos(wrapped)*500.)),bits(f(math.sin(wrapped)*500.)),0,bits(1.)]
        elif 14<=kind<=25:
            l=source['slots'][(kind-14)//3];sub=(kind-14)%3
            if sub==0:output=positions['transformed'](positions['point'](l,p['object']),object_cache,l['kind']==19)
            elif sub==1:output=lights['color'](l,source['alpha_gate'])
            elif l['kind']==19:output=[0x322bcc77,0x322bcc77,bits(2.),bits(1.)]
            else:
                q={**p,'inverse':identity,'editor_eye':[0,0,0],'runtime_eye':[0,0,0],'light':l,'squared':p['light_squared'],'seed':p['light_seed']}
                output=scene['replay'](q,0x1001e6cd,0x1001e79a,'radius')
        elif kind==26:output=lights['ambient'](source['ambient_bgra'])
        elif kind==27:
            if state['last_time'] is None:state['last_time']=time
            if state['last_time']!=time:
                state['last_time']=time;rng=[(i*73+j*127)%32768 for j in range(3)];state['samples']=[f(r*value(0x38000100)) for r in rng]
            threshold,amp=map(value,binding['words'][:2]);base=f(1.-amp);a,b,c=state['samples']
            output=[bits(1. if test<=threshold else f(f(amp*other)+base)) for test,other in [(a,c),(b,a),(c,b)]]+[bits(1.)]
        elif kind==28:output=lights['direction'](source['slots'][0])
        elif kind==29:output=lights['cone'](source['slots'][0])
        elif kind==30:output=draw(p)
        elif kind==31:
            reciprocal=f(1./6.);output=[bits(2.),bits(8.),bits(f(8.*reciprocal)),bits(reciprocal)]
        elif kind==33:output=scene['replay']({**p,'inverse':object_cache,'editor_eye':list(map(bits,[1.,2.,3.])),'runtime_eye':list(map(bits,[7.,8.,9.]))},0x1001e1e3 if p['editor'] else 0x1001e2ae,0x1001e28f if p['editor'] else 0x1001e35a,'eye')
        elif kind==34:rows=rotation
        else:raise AssertionError(kind)
        if rows is not None:result[slot:slot+len(rows)]=rows;slot+=len(rows)
        else:
            if output is not None:result[slot]=output
            slot+=1
    assert p['after']=={'count':count,'words':result},(n,'bank')
    assert p['inverse_calls']==calls and p['rng']==rng and p['flicker_after']==state,(n,'host/state')
    previous=result;counts['bank_words']+=96*4*2;counts['inverse_calls']+=len(calls);counts['rng_draws']+=len(rng)
assert set(int(k[5:]) for k in counts if k.startswith('case_'))==set(range(35))
tests=sum(map(int,re.findall(r'test result: ok\. (\d+) passed',(root/'analysis/reports/complete-constants-tests.log').read_text())));assert tests==585,tests
sources=['crates/rc-package/src/material_constants.rs','crates/rc-package/src/shader_scene.rs','crates/rc-package/src/shader_constants.rs','crates/rc-package/src/shader_lights.rs','crates/rc-package/src/skeletal_matrix_inverse.rs','analysis/decompiled/skeletal-matrix-inverse.asm','crates/rc-inspect/src/bin/rc-complete-constant-check.rs','scripts/Record-CompleteConstants.py','scripts/Probe-RotatorConstants.rs','analysis/decompiled/shader-constants.asm','scripts/Record-SceneConstants.py','scripts/Record-ShaderMatrices.py','scripts/Record-LightPositions.py','scripts/Record-ShaderLights.py','scripts/Record-WorldDirectorPoses.py']
validation={'date':'2026-10-08','dispatches':256,'native_kinds':list(range(35)),'counts':dict(counts),'rust_tests':tests,'debug_release_identical':True,'rotator_x87_words':256*8,'scope':report['scope'],'original_dll_sha256':sha(scene['dll']),'report_sha256':sha(path),'x87_sha256':sha(root/'analysis/reports/complete-constants.x87.bin'),'source_sha256':{s:sha(root/s) for s in sources},'android':'deferred until end per user'}
validation['original_core_sha256']=sha(scene['dll'].parent/'core.dll')
(root/'analysis/reports/complete-constants-validation.json').write_text(json.dumps(validation,indent=2)+'\n')
ledger=root/'analysis/evidence.json';evidence=json.loads(ledger.read_text(encoding='utf-8'));evidence['rust_tests']=tests;evidence['complete_constants_validation']=validation;ledger.write_text(json.dumps(evidence,indent=2,ensure_ascii=False)+'\n',encoding='utf-8')
print(json.dumps({'dispatches':256,'rust_tests':tests,'counts':dict(counts)}))
