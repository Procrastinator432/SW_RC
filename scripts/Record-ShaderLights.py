"""Independent native sparse light-table, color/spot equations and SSE validation."""
from pathlib import Path
import struct,json,re,hashlib,collections
root=Path(__file__).resolve().parents[1]
sha=lambda p:hashlib.sha256(p.read_bytes()).hexdigest()
value=lambda x:struct.unpack('<f',struct.pack('<I',x))[0]
bits=lambda x:struct.unpack('<I',struct.pack('<f',x))[0]
f=lambda x:value(bits(x))
s=root/'scripts/Record-ShaderScalars.py';n={'__file__':str(s)}
exec(compile(s.read_text().split('pn,data,meta,e=')[0],str(s),'exec'),n)
memory=n['memory']
for address,word in [(0x10072684,0x3c008081),(0x10072678,0x3dcccccd),(0x1007267c,0x3f89999a),(0x10072680,0x3a800142),(0x1006ff08,0x40000000),(0x10072398,0x7f7fffff)]:
    assert memory(address,4)==struct.pack('<I',word)
# Both passes write index EAX at [EBP + EAX*8 - 600], not count*8.
assert memory(0x1001e49b,7)==memory(0x1001e4d3,7)==bytes.fromhex('8984c500faffff')
for kind,target in [(15,0x1001e467),(18,0x1001e467),(21,0x1001e467),(24,0x1001e467),(26,0x1001e3f2),(28,0x1001e467),(29,0x1001e467)]:
    entry=memory(0x1001ed44+kind-1,1)[0]
    assert struct.unpack('<I',memory(0x1001ecf0+entry*4,4))[0]==target
hint=struct.unpack('<I',memory(0x1006f24c,4))[0]
symbol=b'??DFPlane@@QBE?AV0@M@Z\0';assert memory(0x10000000+hint+2,len(symbol))==symbol
plane=(root/'analysis/decompiled/shader-light-plane.asm').read_text()
assert plane.count('MULSS')==4 and '1011497d MULSS XMM1,XMM0' in plane
path=root/'analysis/reports/shader-lights.json';report=json.loads(path.read_text())
data=(root/'analysis/reports/shader-lights.input.bin').read_bytes()
sse=(root/'analysis/reports/shader-lights.sse.bin').read_bytes()
release=root/'analysis/reports/shader-lights-release.json'
assert release.read_bytes()==path.read_bytes()
assert (root/'analysis/reports/shader-lights-release.input.bin').read_bytes()==data
regression={}
for before,after in [('constant-banks.json','constant-banks-light-regression.json'),('shader-matrices.json','shader-matrices-light-regression.json'),('shader-matrices.input.bin','shader-matrices-light-regression.input.bin')]:
    prior=root/'analysis/reports'/before;current=root/'analysis/reports'/after
    assert prior.read_bytes()==current.read_bytes(),('regression',before)
    regression[before]={'prior_sha256':sha(prior),'current_sha256':sha(current),'byte_identical':True}
assert len(data)==1024*24 and len(sse)==1024*44
def light(i,slot):
    color=[0x7fc12345 if c==3 else 0x80000000 if (i+slot+c)%17==0 else ((i*7919+slot*127+c*31)&0x807fffff)|((118+(i+slot+c)%13)<<23)|(((i+slot+c)%2)<<31) for c in range(4)]
    return {'actor_present':True,'cone':1+(i+slot*37)%255,'color':color,'brightness':bits(f(i%23*.125-1.)),'direction':[0x7fc10000+i,0x80000000,slot*19+i],'flags':[[0,0],[1,0],[0,0x80000000],[7,9]][i//256%4]}
def lighting(i):
    slots=[]
    for slot in range(4):
        mode=(i>>(slot*2))&3;l=light(i,slot)
        if mode==0:l=None
        elif mode==1:l['actor_present']=False
        elif mode==2:l['cone']=0
        slots.append(l)
    return {'slots':slots,'alpha_gate':i%2==0,'ambient_bgra':i*0x1020304&0xffffffff}
def selection(host):
    indices=[-1]*4;count=0
    for spot in [True,False]:
        for source,l in enumerate(host['slots']):
            if l is not None and l['actor_present'] and (l['cone']!=0)==spot:
                indices[source]=source;count+=1
    return {'count':count,'indices':indices}
def select(host,index):
    cache=selection(host)
    if index>=cache['count']:return {'Ok':None}
    if cache['indices'][index]==-1:return {'Err':'Native sparse light table would address slot -1'}
    return {'Ok':host['slots'][cache['indices'][index]]}
def color(l,gate):
    if l is None:return [0]*4
    rgb=[bits(f(f(value(x)*2.)*value(l['brightness']))) for x in l['color'][:3]]
    return rgb+[bits(1.) if not gate and l['flags']==[0,0] else 0]
def ambient(bgra):return [bits(f(((bgra>>shift)&255)*value(0x3c008081))) for shift in [16,8,0]]+[bits(2.)]
def direction(l):return l['direction']+[bits(1.)] if l is not None and l['cone'] else [bits(1.),0,0,bits(1.)]
def cone(l):
    if l is None or not l['cone']:return [0,0,0,bits(1.)]
    complement=f(1.-f(l['cone']*value(0x3a800142)));x=f(f(complement*complement)*value(0x3f89999a));y=f(x*value(0x3dcccccd))
    return list(map(bits,[x,y,f(1./y),1.]))
def constant(host,kind):
    if kind==26:return {'Ok':ambient(host['ambient_bgra'])}
    result=select(host,0 if kind>=28 else (kind-15)//3)
    if 'Err' in result:return result
    l=result['Ok']
    return {'Ok':direction(l) if kind==28 else cone(l) if kind==29 else color(l,host['alpha_gate'])}
counts=collections.Counter();patterns=set();cone_bytes=set()
for i,p in enumerate(report['probes']):
    host=lighting(i);cache=selection(host)
    assert p['index']==i and p['lighting']==host and p['cache']==cache
    assert p['selection']==[select(host,index) for index in range(4)]
    patterns.add(tuple((i>>(slot*2))&3 for slot in range(4)))
    counts['source_contexts']+=1;counts['selection_checks']+=4
    for kind,d in zip([15,18,21,24,26,28,29],p['dispatches']):
        before={'words':[[0x7fc12345,0x80000000,7,i] for _ in range(8)],'count':-3}
        after={'words':[row[:] for row in before['words']],'count':2};after['words'][0]=[i]*4
        result=constant(host,kind)
        if 'Ok' in result:after['words'][1]=result['Ok']
        else:counts['sparse_dispatch_errors']+=1
        assert d['kind']==kind and d['before']==before and d['after']==after and d['error']==result.get('Err'),(i,kind)
        counts['isolated_dispatches']+=1;counts['bank_words_checked']+=64
    combined={'words':[[99]*4 for _ in range(8)],'count':7};error=None
    for slot,kind in enumerate([15,18,21,24,26,28,29]):
        result=constant(host,kind)
        if 'Err' in result:error=result['Err'];break
        combined['words'][slot]=result['Ok']
    assert p['combined']['first_error']==error and p['combined']['after_first']==combined
    empty={**host,'slots':[None]*4}
    for slot,kind in enumerate([15,18,21,24,26,28,29]):combined['words'][slot]=constant(empty,kind)['Ok']
    assert p['combined']['second_error'] is None and p['combined']['after_second']==combined
    counts['combined_dispatches']+=2;counts['bank_words_checked']+=64
    scalar=report['scalar'][i];l=light(i,0);l['cone']=i%256;cone_bytes.add(l['cone']);bgra=host['ambient_bgra']
    c=color(l,host['alpha_gate']);a=ambient(bgra);spot=cone(l)
    assert scalar=={'light':l,'alpha_gate':host['alpha_gate'],'bgra':bgra,'color':c,'ambient':a,'cone':spot}
    assert data[i*24:(i+1)*24]==struct.pack('<6I',*l['color'][:3],l['brightness'],bgra,l['cone'])
    assert sse[i*44:(i+1)*44]==struct.pack('<11I',*c[:3],*a,*spot),(i,'SSE')
    counts['sse_words_checked']+=11;counts['scalar_probes']+=1
for mode,p in enumerate(report['errors']):
    l=light(3,0)
    if mode==0:l['color'][0]=0x7fc12345
    elif mode==1:l['brightness']=0x7f800000
    elif mode==2:l['color'][0]=0x7f7fffff;l['brightness']=0
    else:l['color'][0]=bits(f(value(0x7f7fffff)*.25));l['brightness']=bits(8.)
    assert p['mode']==mode and p['light']==l
    after={'words':[[7]*4 for _ in range(8)],'count':2};after['words'][0]=ambient(0x99ff0080)
    assert p['after']==after
    assert p['error']==('Nonfinite light brightness excluded by host contract' if mode==1 else 'Nonfinite light color arithmetic excluded by host contract')
    counts['expected_arithmetic_errors']+=1
assert len(patterns)==256 and cone_bytes==set(range(256))
assert counts['source_contexts']==1024 and counts['expected_arithmetic_errors']==4
tests=sum(map(int,re.findall(r'test result: ok\. (\d+) passed',(root/'analysis/reports/shader-lights-tests.log').read_text())));assert tests==566,tests
sources=['crates/rc-package/src/shader_lights.rs','crates/rc-package/src/shader_constants.rs','crates/rc-inspect/src/bin/rc-shader-light-check.rs','scripts/Probe-ShaderLights.rs','scripts/Record-ShaderLights.py','analysis/decompiled/shader-constants.asm','analysis/decompiled/shader-constants.c','analysis/decompiled/shader-light-plane.asm','analysis/decompiled/shader-light-plane.c']
core=n['original']['game']/'System/core.dll'
validation={'counts':dict(counts),'rust_tests':tests,'source_patterns':len(patterns),'cone_bytes':len(cone_bytes),'report_sha256':sha(path),'release_report_sha256':sha(release),'debug_release_identical':True,'regression':regression,'input_sha256':sha(root/'analysis/reports/shader-lights.input.bin'),'sse_sha256':sha(root/'analysis/reports/shader-lights.sse.bin'),'original_sha256':{str(n['dll']):sha(n['dll']),str(core):sha(core)},'source_sha256':{s:sha(root/s) for s in sources},'scope':report['scope']}
(root/'analysis/reports/shader-lights-validation.json').write_text(json.dumps(validation,indent=2)+'\n')
e=root/'analysis/evidence.json';evidence=json.loads(e.read_text());evidence['rust_tests']=tests;evidence['shader_lights_validation']=validation
e.write_text(json.dumps(evidence,indent=2,ensure_ascii=False)+'\n',encoding='utf-8')
print(json.dumps({'counts':dict(counts),'source_patterns':len(patterns),'cone_bytes':len(cone_bytes),'rust_tests':tests}))
