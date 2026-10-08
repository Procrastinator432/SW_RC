"""Original PE constants, x87 remainder/cosine and independent shared flicker oracle."""
from pathlib import Path
import json,struct,math,re
root=Path(__file__).resolve().parents[1]
s=root/'scripts/Record-Hologram.py';n={'__file__':str(s)}
exec(compile(s.read_text().split("data,meta=ns['package']")[0],str(s),'exec'),n)
f=n['f'];bits=n['bits'];sha=n['sha'];original=n['ns'];properties=n['properties']
path=root/'analysis/reports/shader-scalars.json';report=json.loads(path.read_text())
dll=original['game']/'System/D3DDrv.dll';pe=dll.read_bytes();h=struct.unpack_from('<I',pe,60)[0];opt=struct.unpack_from('<H',pe,h+20)[0];base=struct.unpack_from('<I',pe,h+52)[0]
def memory(address,size):
    for i in range(struct.unpack_from('<H',pe,h+6)[0]):
        at=h+24+opt+40*i;virtual,rva,raw,offset=struct.unpack_from('<IIII',pe,at+8)
        if rva<=address-base<rva+max(virtual,raw):
            delta=address-base-rva
            return pe[offset+delta:offset+delta+size] if delta+size<=raw else bytes(size)
    raise AssertionError(hex(address))
assert struct.unpack('<d',memory(0x10072690,8))[0]==120.
assert memory(0x10072370,4)==struct.pack('<I',0x38000100)
assert memory(0x1006f9a0,4)==struct.pack('<f',1.) and memory(0x1006ff00,4)==bytes(4)
assert all(memory(a,4)==struct.pack('<f',.5) for a in [0x100801a0,0x1008019c,0x10080198])
assert memory(0x100846fc,4)==memory(0x100846f8,4)==bytes(4)
# The original helper thunk imports _CIfmod rather than a positive-wrap operator.
assert memory(0x1006c238,6)==bytes.fromhex('ff25b8f40610')
hint=struct.unpack('<I',memory(0x1006f4b8,4))[0];assert memory(base+hint+2,8)==b'_CIfmod\0'
pn,data,meta,e=original['asset']('HardwareShaders.Hologram.DynamicHologram');props,_=properties(data,meta,e);bindings=[]
for p in props:
    if p['name']!='VSConstants':continue
    q,_=properties(p['raw'],meta,(0,0,0,0,len(p['raw']),0));raw=n['first'](q,'Value');plane,_=properties(raw,meta,(0,0,0,0,len(raw),0))
    bindings.append({'slot':p['slot'],'kind':(n['first'](q,'Type') or b'\0')[0],'value':[struct.unpack('<f',n['first'](plane,a) or bytes(4))[0] for a in 'XYZW']})
assert len(bindings)==len(report['bindings'])==20
for a,b in zip(bindings,report['bindings']):
    assert a['slot']==b['slot'] and a['kind']==b['kind'] and list(map(bits,a['value']))==list(map(bits,b['value']))
flicker=[b for b in bindings if b['kind']==27];assert [b['slot'] for b in flicker]==[21,22,25]
input_bytes=(root/'analysis/reports/shader-scalars.input.bin').read_bytes();native=(root/'analysis/reports/shader-scalars.x87.bin').read_bytes()
assert len(input_bytes)==len(native)==1024*8
special=[0.,-0.,f(119.999),120.,-120.,240.5,-121.25,struct.unpack('<f',bytes.fromhex('ffff7f7f'))[0],-struct.unpack('<f',bytes.fromhex('ffff7f7f'))[0],65536.125]
for i,p in enumerate(report['time_probes']):
    time=special[i] if i<len(special) else f(f(i-512.25)*f(.37));rate=[0.,1.,-1.,f(.1),2.,32.,64.][i%7]
    assert bits(p['time'])==bits(time) and bits(p['rate'])==bits(rate)
    assert input_bytes[i*8:i*8+8]==struct.pack('<ff',time,rate)
    wrapped=f(math.fmod(time,120.));cosine=f(math.cos(wrapped*(rate if rate!=0. else 1.)))
    native_time,native_cos=struct.unpack_from('<II',native,i*8)
    assert p['time_words']==[native_time]*4==[bits(wrapped)]*4,i
    assert p['cos_words']==[native_cos]*4==[bits(cosine)]*4,i
assert len(report['time_probes'])==1024
last=None;samples=[.5]*3;draw_count=0;factor=struct.unpack('<f',memory(0x10072370,4))[0]
for i,p in enumerate(report['flicker_probes']):
    time=f((i//4%8)*.125);binding=flicker[i%3];plane=binding['value'];assert p['time']==time and p['slot']==binding['slot'] and list(map(bits,p['plane']))==list(map(bits,plane))
    before={'last_time':last,'samples':samples[:]};assert p['before']==before
    if last is None:last=time
    draws=[]
    if last!=time:
        last=time;draws=[(i*73+j*127)%32768 for j in range(3)];samples=[f(x*factor) for x in draws]
    assert p['draws']==draws;draw_count+=len(draws)
    assert p['after']['last_time']==last and list(map(bits,p['after']['samples']))==list(map(bits,samples))
    threshold,amp=plane[:2];base_value=f(1.-amp);a,b,c=samples
    values=[1. if test<=threshold else f(f(amp*other)+base_value) for test,other in [(a,c),(b,a),(c,b)]]+[1.]
    assert p['words']==list(map(bits,values)),i
assert len(report['flicker_probes'])==1024 and draw_count==765
tests=sum(map(int,re.findall(r'test result: ok\. (\d+) passed',(root/'analysis/reports/shader-scalars-tests.log').read_text())));assert tests==550,tests
sources=['crates/rc-package/src/material_constants.rs','crates/rc-inspect/src/bin/rc-constant-check.rs','scripts/Probe-ShaderScalars.rs','scripts/Record-ShaderScalars.py','analysis/decompiled/shader-constants.c','analysis/decompiled/shader-constants.asm']
original['hashes'][str(dll)]=sha(dll)
validation={'report_sha256':sha(path),'x87_input_sha256':sha(root/'analysis/reports/shader-scalars.input.bin'),'x87_output_sha256':sha(root/'analysis/reports/shader-scalars.x87.bin'),'original_sha256':original['hashes'],'source_sha256':{s:sha(root/s) for s in sources},'native_dispatcher':'1001c130 / 1001c260','constant_cases':[8,9,27],'time_cosine_probes':1024,'flicker_transitions':1024,'random_draws':765,'original_flicker_slots':[21,22,25],'rust_tests':tests,'scope':report['scope']}
(root/'analysis/reports/shader-scalars-validation.json').write_text(json.dumps(validation,indent=2)+'\n')
e=root/'analysis/evidence.json';evidence=json.loads(e.read_text());evidence['rust_tests']=tests;evidence['shader_scalars_validation']=validation;e.write_text(json.dumps(evidence,indent=2,ensure_ascii=False)+'\n',encoding='utf-8')
print(json.dumps({k:validation[k] for k in ['time_cosine_probes','flicker_transitions','random_draws','rust_tests']}))
