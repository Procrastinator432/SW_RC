"""Original PE/jump-table evidence, independent math and offline x87 checks."""
from pathlib import Path
import json,struct,math,re,hashlib
root=Path(__file__).resolve().parents[1]
bits=lambda x:struct.unpack('<I',struct.pack('<f',x))[0]
value=lambda x:struct.unpack('<f',struct.pack('<I',x))[0]
f=lambda x:value(bits(x))
sha=lambda p:hashlib.sha256(p.read_bytes()).hexdigest()
dll=Path(r'D:\SteamLibrary\steamapps\common\Star Wars Republic Commando\GameData\System\D3DDrv.dll')
pe=dll.read_bytes();h=struct.unpack_from('<I',pe,60)[0];opt=struct.unpack_from('<H',pe,h+20)[0];base=struct.unpack_from('<I',pe,h+52)[0]
def memory(address,size):
    for i in range(struct.unpack_from('<H',pe,h+6)[0]):
        at=h+24+opt+i*40;virtual,rva,raw,offset=struct.unpack_from('<IIII',pe,at+8)
        if rva<=address-base<rva+raw:
            delta=address-base-rva;assert delta+size<=raw
            return pe[offset+delta:offset+delta+size]
    raise AssertionError(hex(address))
assert memory(0x10072688,4)==struct.pack('<f',500.)
assert memory(0x10072690,8)==struct.pack('<d',120.)
assert memory(0x1006f9a0,4)==struct.pack('<f',1.) and memory(0x1006ff00,4)==bytes(4)
for kind,target in [(10,0x1001dff9),(11,0x1001e04a),(13,0x1001e3b1)]:
    index=memory(0x1001ed44+kind-1,1)[0]
    assert struct.unpack('<I',memory(0x1001ecf0+index*4,4))[0]==target
asm=(root/'analysis/decompiled/shader-constants.asm').read_text()
for instruction in ['1001e02c FMUL float ptr [EBP + -0x20]','1001e037 FSIN','1001e087 FPTAN','1001e08a FSTP ST0','1001e3b6 FCOS','1001e3d7 FMUL float ptr [0x10072688]','1001e3e2 FSIN','1001e3e4 FMUL float ptr [0x10072688]']:
    assert instruction in asm,instruction
path=root/'analysis/reports/wave-constants.json';report=json.loads(path.read_text())
data=(root/'analysis/reports/wave-constants.input.bin').read_bytes();native=(root/'analysis/reports/wave-constants.x87.bin').read_bytes()
assert path.read_bytes()==(root/'analysis/reports/wave-constants-release.json').read_bytes()
assert data==(root/'analysis/reports/wave-constants-release.input.bin').read_bytes()
assert len(data)==2048*8 and len(native)==2048*20 and len(report['probes'])==2048
special=[0.,-0.,f(119.999),120.,-120.,240.5,-121.25,value(0x7f7fffff),-value(0x7f7fffff),65536.125,value(0x3fc90fdb),-value(0x3fc90fdb)]
bank_words=0
for i,p in enumerate(report['probes']):
    time=special[i] if i<len(special) else f(f(i-512.25)*f(.37)) if i<1024 else f(f(value(0x3fc90fdb)*(i%39-19.))+f((i%17-8.)*f(.000001)))
    rate=[0.,-0.,1.,-1.,f(.1),2.,32.,64.][i%8]
    assert p['index']==i and p['time']==bits(time) and p['frequency']==bits(rate),i
    assert data[8*i:8*i+8]==struct.pack('<ff',time,rate),i
    wrapped=f(math.fmod(time,120.));argument=wrapped*(rate if rate!=0. else 1.)
    sine=bits(f(math.sin(argument)));tangent=bits(f(math.tan(argument)))
    circle=[bits(f(math.cos(wrapped)*500.)),bits(f(math.sin(wrapped)*500.)),0,bits(1.)]
    raw=struct.unpack_from('<5I',native,i*20)
    assert raw==(bits(wrapped),sine,tangent,circle[0],circle[1]),(i,'x87/math',raw)
    length=8 if i%2==0 else 96
    words=[[0x7fc12345,0x80000000,7,i] for _ in range(length)]
    assert p['before']=={'count':-3 if length==8 else -2,'words':words},i
    words[1]=[sine]*4;words[2]=[tangent]*4;words[3]=circle;words[4]=[0x80000000,0x7fc12345,i,9]
    assert p['after']=={'count':5,'words':words},(i,'bank')
    bank_words+=length*4*2
tests=sum(map(int,re.findall(r'test result: ok\. (\d+) passed',(root/'analysis/reports/wave-constants-tests.log').read_text())))
assert tests==579,tests
sources=['crates/rc-package/src/material_constants.rs','crates/rc-package/src/shader_constants.rs','crates/rc-inspect/src/bin/rc-wave-constant-check.rs','scripts/Probe-WaveConstants.rs','scripts/Record-WaveConstants.py','analysis/decompiled/shader-constants.asm']
result={'date':'2026-10-08','cases':[10,11,13],'contexts':2048,'register_outputs':6144,'x87_output_words':10240,'bank_before_after_words':bank_words,'debug_release_identical':True,'rust_tests':tests,'scope':report['scope'],'x87_scope':'037f control word, original instruction order on supplied finite fixtures; observed f32 equality only, no universal transcendental/Android parity claim','original_sha256':sha(dll),'report_sha256':sha(path),'input_sha256':sha(root/'analysis/reports/wave-constants.input.bin'),'x87_sha256':sha(root/'analysis/reports/wave-constants.x87.bin'),'source_sha256':{s:sha(root/s) for s in sources},'android':'deferred until end per user'}
(root/'analysis/reports/wave-constants-validation.json').write_text(json.dumps(result,indent=2)+'\n')
ledger=root/'analysis/evidence.json';evidence=json.loads(ledger.read_text(encoding='utf-8'));evidence['rust_tests']=tests;evidence['wave_constants_validation']=result;ledger.write_text(json.dumps(evidence,indent=2,ensure_ascii=False)+'\n',encoding='utf-8')
print(json.dumps({k:result[k] for k in ['contexts','register_outputs','x87_output_words','bank_before_after_words','rust_tests']}))
