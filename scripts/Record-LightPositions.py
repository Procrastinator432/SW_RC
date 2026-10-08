"""Native instruction replay for light positions/cache and full original shader bindings."""
from pathlib import Path
import json,struct,re,math,hashlib,collections
root=Path(__file__).resolve().parents[1]
sha=lambda p:hashlib.sha256(p.read_bytes()).hexdigest()
value=lambda x:struct.unpack('<f',struct.pack('<I',x))[0]
bits=lambda x:struct.unpack('<I',struct.pack('<f',x))[0]
f=lambda x:value(bits(x))
transpose=lambda m:[list(r) for r in zip(*m)]
def matrix(i):return [[bits(f((2. if r==c else 0.)+(i+r*7+c*3)*.001953125)) for c in range(4)] for r in range(4)]
def inverse_fixture(i):return [[0x7fc10000+i if c==3 else bits(f(((i+r*11+c*5)%31)*.125-1.)) for c in range(4)] for r in range(4)]
def light(i,slot):return {'actor_present':True,'kind':19 if (i+slot)%2==0 else 7,'position':list(map(bits,[i*.125,slot-2.,3.5])),'direction':list(map(bits,[.125,(slot+1)*.25,-.5])),'cone':0,'color':[0]*4,'brightness':0,'flags':[0,0]}
def point(light,object):return [f(value(object[3][k])-f(value(light['direction'][k])*65365.)) for k in range(3)] if light['kind']==19 else list(map(value,light['position']))
asm=[]
for line in (root/'analysis/decompiled/shader-constants.asm').read_text().splitlines():
    if line and not line.startswith('#'):
        a,instruction=line.split(' ',1);op,_,args=instruction.partition(' ');asm.append((int(a,16),op,args))
def transformed(p,inv,directional):
    registers={'XMM0':p[1],'XMM1':p[2],'XMM2':p[0]} if directional else {}
    stack={-0x6c+(r*4+c)*4:value(inv[r][c]) for r in range(4) for c in range(4)}
    start,end,base=(0x1001e8da,0x1001e980,-0xc8) if directional else (0x1001e9c6,0x1001ea76,-0xd8)
    def operand(op):
        if op.startswith('XMM'):return registers[op]
        m=re.fullmatch(r'dword ptr \[(EBP|ESI) \+ (-?0x[0-9a-f]+)\]',op);assert m,op
        offset=int(m[2],16);offset=offset-2**32 if offset>=2**31 else offset
        return stack[offset] if m[1]=='EBP' else p[(offset-0x18)//4]
    for address,op,args in asm:
        if not start<=address<=end or op not in ('MOVSS','MOVAPS','MULSS','ADDSS','XORPS'):continue
        dst,src=args.split(',',1)
        if not dst.startswith('XMM'):
            m=re.fullmatch(r'dword ptr \[EBP \+ (0x[0-9a-f]+)\]',dst);assert m,dst
            stack[int(m[1],16)-2**32]=operand(src);continue
        if op=='XORPS':registers[dst]=0.;continue
        v=operand(src)
        if op=='MULSS':v=f(registers[dst]*v)
        elif op=='ADDSS':v=f(registers[dst]+v)
        registers[dst]=v
    return [bits(stack[base+4*k]) for k in range(3)]+[bits(1.)]
context={'root':root,'re':re,'f':f,'bits':bits,'value':value,'identity':[bits(1.) if r==c else 0 for r in range(4) for c in range(4)]}
s=(root/'scripts/Record-WorldDirectorPoses.py').read_text();exec(compile(s[s.index('def offset'):s.index('# Build expression')],'<native inverse>','exec'),context)
native_inverse=lambda m:[context['native_inverse']([v for row in m for v in row])[r*4:r*4+4] for r in range(4)]
s=(root/'scripts/Record-ShaderMatrices.py').read_text();composition={'root':root,'re':re,'struct':struct,'f':f,'bits':bits,'value':value}
exec(compile(s[s.index('asm_path=root/'):s.index('def fixture')],'<matrix instructions>','exec'),composition)
s=(root/'scripts/Record-ShaderLights.py').read_text();lights={'f':f,'bits':bits,'value':value}
exec(compile(s[s.index('def selection'):s.index('counts=collections')],'<light selection>','exec'),lights)
path=root/'analysis/reports/light-positions.json';report=json.loads(path.read_text());data=(root/'analysis/reports/light-positions.input.bin').read_bytes();sse=(root/'analysis/reports/light-positions.sse.bin').read_bytes()
assert len(data)==1024*104 and len(sse)==1024*28
assert (root/'analysis/reports/light-positions-release.json').read_bytes()==path.read_bytes()
assert (root/'analysis/reports/light-positions-release.input.bin').read_bytes()==data
counts=collections.Counter()
for i,p in enumerate(report['probes']):
    object=matrix(i);inv=inverse_fixture(i);slots=[]
    for slot in range(4):
        l=light(i,slot);mode=(i>>(slot*2))&3
        if mode==0:l=None
        elif mode==1:l['actor_present']=False
        slots.append(l)
    start=4 if i%2==0 else 0;inverse_slot=0 if i%2==0 else 4;fail=i%31==0
    assert p['index']==i and p['object']==object and p['inverse']==inv and p['slots']==slots and p['positions']==start and p['inverse_slot']==inverse_slot and p['fail']==fail
    before={'words':[[0x7fc12345,0x80000000,7,i] for _ in range(96)],'count':-2};assert p['before']==before
    after={'words':[row[:] for row in before['words']],'count':10};calls=[];cached=False;camera=False;error=None;slot=0
    while slot<10:
        if slot==inverse_slot:
            if not cached:
                calls.append(object)
                if fail:error='fixture inverse failure';break
                cached=True
            after['words'][slot:slot+4]=transpose(inv);slot+=4;continue
        if start<=slot<start+4:
            selected=lights['select']({'slots':slots},slot-start)
            if 'Err' in selected:error=selected['Err'];break
            l=selected['Ok']
            if l is None:after['words'][slot]=[0x4b189680]*3+[0]
            else:
                if not cached:
                    calls.append(object)
                    if fail:error='fixture inverse failure';break
                    cached=True
                after['words'][slot]=transformed(point(l,object),inv,l['kind']==19)
        else:
            if not camera:
                calls.append(matrix(2000))
                if fail:error='fixture inverse failure';break
                camera=True
            after['words'][slot]=inv[3][:]
        slot+=1
    assert p['after']==after and p['calls']==calls and p['error']==error,(i,'bank')
    scalar=report['scalar'][i];l=light(i,0);pt=point(l,object);out=transformed(pt,inv,l['kind']==19)
    assert scalar=={'light':l,'point':list(map(bits,pt)),'words':out}
    raw=[l['kind']]+l['position']+l['direction']+object[3][:3]+[v for row in inv for v in row]
    assert data[i*104:(i+1)*104]==struct.pack('<26I',*raw)
    assert sse[i*28:(i+1)*28]==struct.pack('<7I',*map(bits,pt),*out),(i,'SSE')
    counts['position_contexts']+=1;counts['position_bank_words']+=96*4*2;counts['inverse_callbacks']+=len(calls);counts['expected_dispatch_errors']+=error is not None;counts['sse_words']+=7
# Full original material and its original vertex algorithm, imported read-only.
source=root/'scripts/Record-VertexShader.py';v={'__file__':str(source)}
exec(compile(source.read_text().split('textures=[]')[0],str(source),'exec'),v)
original=v['original'];props=v['props'];meta=v['meta'];properties=v['properties'];first=v['ns']['first']
hpath=root/'analysis/reports/hologram-constants.json';h=json.loads(hpath.read_text())
assert (root/'analysis/reports/hologram-constants-release.json').read_bytes()==hpath.read_bytes()
assert h['shader']=='HardwareShaders.Hologram.DynamicHologram' and h['source']==first(props,'VertexShaderText')
def bindings(field):
    result=[]
    for p in props:
        if p['name']!=field:continue
        q,_=properties(p['raw'],meta,(0,0,0,0,len(p['raw']),0));raw=first(q,'Value');plane,_=properties(raw,meta,(0,0,0,0,len(raw),0))
        result.append({'slot':p['slot'],'kind':(first(q,'Type') or b'\0')[0],'value':[struct.unpack('<f',first(plane,a) or bytes(4))[0] for a in 'XYZW']})
    return result
vs=bindings('VSConstants');ps=bindings('PSConstants');assert vs==h['bindings'] and ps==h['pixel_bindings'] and len(vs)==20 and len(ps)==2
words=lambda m:[[bits(x) for x in row] for row in m]
object=words([[2.,0.,0.,0.],[0.,1.,0.,0.],[0.,0.,.5,0.],[.125,-.25,.5,1.]])
view=words([[1.,0.,0.,0.],[0.,1.,0.,0.],[0.,0.,1.,0.],[0.,0.,0.,1.]])
projection=words([[0.,0.,f(.4),f(.3)],[f(.9),0.,0.,0.],[0.,f(.9),0.,0.],[0.,0.,.5,1.]])
camera=words([[1.,0.,0.,0.],[0.,1.,0.,0.],[0.,0.,1.,0.],[4.,0.,0.,1.]])
assert (h['object'],h['view'],h['projection'],h['camera'])==(object,view,projection,camera)
inv=native_inverse(object);clip=composition['original'](object,view,projection)[1]
bank={'words':[[0]*4 for _ in range(96)],'count':-2};bank['words'][17]=[bits(.25)]*4;pixel={'words':[[0]*4 for _ in range(8)],'count':-3};state={'last_time':None,'samples':[.5]*3}
for i,p in enumerate(h['probes']):
    time=(i//4)*.125;kind=19 if i%2==0 else 7
    assert p['index']==i and p['time']==time and p['light_kind']==kind and p['before']==bank and p['pixel_before']==pixel and p['flicker_before']==state
    draws=[];bank={'words':[row[:] for row in bank['words']],'count':32};pixel={'words':[row[:] for row in pixel['words']],'count':2}
    l={'kind':kind,'position':list(map(bits,[2.,3.,4.])),'direction':list(map(bits,[.125,.25,-.5]))}
    for b in vs:
        slot=b['slot'];k=b['kind'];plane=b['value']
        if k==1:bank['words'][slot]=list(map(bits,plane))
        elif k==3:bank['words'][slot:slot+4]=clip
        elif k==4:bank['words'][slot:slot+4]=transpose(object)
        elif k==14:bank['words'][slot]=transformed(point(l,object),inv,kind==19)
        elif k==12:bank['words'][slot]=camera[3][:]
        elif k==9:bank['words'][slot]=[bits(f(math.cos(f(math.fmod(time,120.))*(plane[0] or 1.))))]*4
        elif k==21:bank['words'][slot]=list(map(bits,[.375,.75,1.125,1.]))
        elif k==27:
            if state['last_time'] is None:state['last_time']=time
            if state['last_time']!=time:
                state['last_time']=time
                for sample in range(3):
                    rand=(i*73+len(draws)*127)%32768;draws.append(rand);state['samples'][sample]=f(rand*value(0x38000100))
            a,bb,c=state['samples'];amp=plane[1];base=f(1.-amp)
            bank['words'][slot]=[bits(1. if test<=plane[0] else f(f(amp*other)+base)) for test,other in [(a,c),(bb,a),(c,bb)]]+[bits(1.)]
        else:assert k==0,k
    for b in ps:assert b['kind']==1;pixel['words'][b['slot']]=list(map(bits,b['value']))
    assert p['after']==bank and p['pixel_after']==pixel and p['draws']==draws and p['flicker_after']==state and p['inverse_calls']==[object],(i,'material bank')
    vertices=[[0.]*4 for _ in range(16)];vertices[0]=[(i%8)*.0625-.25,((i//8)%8)*.0625-.25,.125,1.];vertices[1]=[1.,0.,0.,0.];vertices[2]=[.25,.75,0.,1.]
    assert p['vertices']==vertices
    out,defined,_=v['algorithm'](vertices,[[value(x) for x in row] for row in bank['words']],False)
    assert p['output_words']==words(out) and p['defined']==defined,(i,'vertex')
    counts['material_dispatches']+=2;counts['material_bank_words']+=(96+8)*4*2;counts['original_vertex_outputs']+=1;counts['material_rng_draws']+=len(draws)
assert len(h['probes'])==256 and len(report['probes'])==1024
tests=sum(map(int,re.findall(r'test result: ok\. (\d+) passed',(root/'analysis/reports/light-positions-tests.log').read_text())));assert tests==571,tests
s=root/'scripts/Record-ShaderScalars.py';native={'__file__':str(s)}
exec(compile(s.read_text().split('pn,data,meta,e=')[0],str(s),'exec'),native)
memory=native['memory']
assert memory(0x10072670,4)==struct.pack('<f',65365.) and memory(0x1007266c,4)==struct.pack('<f',10000000.)
for kind,target in [(7,0x1001d5f7),(14,0x1001e467),(17,0x1001e467),(20,0x1001e467),(23,0x1001e467)]:
    index=memory(0x1001ed44+kind-1,1)[0];assert struct.unpack('<I',memory(0x1001ecf0+index*4,4))[0]==target
symbol=b'?Inverse@FMatrix@@QBE?AV1@XZ\0';hint=struct.unpack('<I',memory(0x1006f250,4))[0];assert memory(0x10000000+hint+2,len(symbol))==symbol
exports=json.loads((root/'analysis/reports/binaries/System__core.dll.json').read_text())['exports'];assert next(e['address'] for e in exports if e['name']==symbol[:-1].decode())=='10143b40'
original['hashes'][str(native['dll'])]=sha(native['dll']);core=original['game']/'System/core.dll';original['hashes'][str(core)]=sha(core)
sources=['crates/rc-package/src/shader_lights.rs','crates/rc-package/src/shader_constants.rs','crates/rc-package/src/skeletal_matrix_inverse.rs','crates/rc-inspect/src/bin/rc-light-position-check.rs','crates/rc-inspect/src/bin/rc-hologram-constant-check.rs','scripts/Probe-LightPositions.rs','scripts/Record-LightPositions.py','analysis/decompiled/shader-constants.asm','analysis/decompiled/skeletal-matrix-inverse.asm']
sources+=['crates/rc-inspect/src/assets.rs','crates/rc-package/src/vertex_shader.rs','crates/rc-package/src/shader_matrix_orders.rs','scripts/Record-WorldDirectorPoses.py','scripts/Record-ShaderMatrices.py','scripts/Record-VertexShader.py','scripts/Record-ShaderLights.py','scripts/Record-ShaderScalars.py']
validation={'counts':dict(counts),'rust_tests':tests,'position_report_sha256':sha(path),'material_report_sha256':sha(hpath),'debug_release_identical':True,'original_sha256':original['hashes'],'input_sha256':sha(root/'analysis/reports/light-positions.input.bin'),'sse_sha256':sha(root/'analysis/reports/light-positions.sse.bin'),'source_sha256':{s:sha(root/s) for s in sources},'scope':{'positions':report['scope'],'material':h['scope']}}
(root/'analysis/reports/light-positions-validation.json').write_text(json.dumps(validation,indent=2)+'\n')
e=root/'analysis/evidence.json';evidence=json.loads(e.read_text());evidence['rust_tests']=tests;evidence['light_positions_validation']=validation;e.write_text(json.dumps(evidence,indent=2,ensure_ascii=False)+'\n',encoding='utf-8')
print(json.dumps({'counts':dict(counts),'rust_tests':tests}))
