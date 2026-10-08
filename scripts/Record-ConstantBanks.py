"""Native span table and independent raw matrix/camera/persistent-bank oracle."""
from pathlib import Path
import json,struct,math,re
root=Path(__file__).resolve().parents[1]
s=root/'scripts/Record-ShaderScalars.py';n={'__file__':str(s)}
exec(compile(s.read_text().split('pn,data,meta,e=')[0],str(s),'exec'),n)
f=n['f'];bits=n['bits'];sha=n['sha'];memory=n['memory'];original=n['original']
def native_span(kind):
    if not 2<=kind<=34:return 1
    index=memory(0x1001eccc+kind-2,1)[0];target=struct.unpack('<I',memory(0x1001ecc0+index*4,4))[0]
    return {0x1001c1b4:4,0x1001c1b9:2,0x1001c1be:1}[target]
assert [kind for kind in range(256) if native_span(kind)==4]==[2,3,4,5,6,7,32]
assert [kind for kind in range(256) if native_span(kind)==2]==[34]
path=root/'analysis/reports/constant-banks.json';report=json.loads(path.read_text())
def matrix(seed):
    def rotate(v,count):return ((v<<count)|(v>>(32-count if count else 32)))&0xffffffff
    return [[rotate((seed+(r*4+c)*0x10203)&0xffffffff,r+c) for c in range(4)] for r in range(4)]
transpose=lambda m:[list(row) for row in zip(*m)]
banks=[{'words':[[0x7fc12345,0x80000000,0xdeadbeef,7] for _ in range(size)],'count':marker} for size,marker in [(8,-3),(96,-2)]]
flicker={'last_time':None,'samples':[.5]*3};factor=struct.unpack('<f',memory(0x10072370,4))[0]
counts={'updates':0,'raw_words_checked':0,'inverse_calls':0,'random_draws':0,'errors':0,'matrix_uploads':0,'eye_uploads':0}
for i,probe in enumerate(report['probes']):
    which=i%2;bank=banks[which];size=len(bank['words']);reset=i%17==0
    if reset:bank['count']=-3 if size==8 else -2
    assert probe['index']==i and probe['bank']==which and probe['reset']==reset
    assert probe['before']==bank and probe['flicker_before']==flicker
    bindings=[{'kind':0,'words':[0]*4} for _ in range(size)];mode=i//2%7
    if mode==0:
        for slot,kind in [(0,4),(4,12),(5,8)]:bindings[slot]['kind']=kind
        bindings[6]={'kind':9,'words':[bits(f(.1)),0,0,0]};bindings[7]={'kind':27,'words':[bits(f(.1)),bits(f(.4)),0,0]}
    elif mode==1:
        bindings[0]['kind']=12;bindings[1]['kind']=5;bindings[5]={'kind':27,'words':[bits(f(.99)),bits(f(.3)),0,0]};bindings[6]['kind']=9;bindings[7]={'kind':1,'words':[0x7fc01000+i,0x80000000,i,0]}
    elif mode==2:bindings[0]['kind']=6;bindings[4]['kind']=5
    elif mode==3:bindings[0]['kind']=4;bindings[1]['kind']=3;bindings[4]['kind']=12;bindings[7]={'kind':1,'words':[i]*4}
    elif mode==4:bindings[0]={'kind':1,'words':[0xdead0000+i]*4};bindings[2]['kind']=35
    elif mode==6:bindings[0]['kind']=12;bindings[1]['kind']=5;bindings[5]['kind']=12
    assert bindings==probe['bindings']
    host={'object_to_world':matrix(0x3f800000+i),'world_to_camera':matrix(0x40000000+i),'camera_to_world':matrix(0x7fc00000+i) if i%4 else None,'editor':i%3==0,'engine_time':f(i//4*.125)}
    assert host==probe['host'];inverse_result=matrix(0x12340000+i);inverse_error=i%29==0
    assert inverse_result==probe['inverse_result'] and inverse_error==probe['inverse_error']
    if bank['count']<0:
        limit=96 if bank['count']==-2 else 8
        last=max([j for j,b in enumerate(bindings[:limit]) if b['kind']]+[0]);bank['count']=last+native_span(bindings[last]['kind'])
    assert bank['count']<=size
    slot=0;cached=None;calls=[];draws=[];error=None
    while slot<bank['count']:
        b=bindings[slot];kind=b['kind'];rows=None
        if kind in (4,6):rows=transpose(host['object_to_world'] if kind==4 else host['world_to_camera'])
        elif kind in (5,12):
            if cached is None:
                if not host['editor'] and host['camera_to_world'] is not None:cached=host['camera_to_world']
                elif not host['editor'] and kind==5:
                    error='Native CameraToWorld runtime null dereference excluded by host contract';break
                else:
                    calls.append(host['world_to_camera'])
                    if inverse_error:error='fixture inverse unavailable';break
                    cached=inverse_result
            if kind==12:bank['words'][slot]=cached[3][:];counts['eye_uploads']+=1
            else:rows=transpose(cached)
        elif kind==1:bank['words'][slot]=b['words'][:]
        elif kind==8:bank['words'][slot]=[bits(f(math.fmod(host['engine_time'],120.)))]*4
        elif kind==9:
            rate=struct.unpack('<f',struct.pack('<I',b['words'][0]))[0] or 1.;time=f(math.fmod(host['engine_time'],120.))
            bank['words'][slot]=[bits(f(math.cos(rate*time)))]*4
        elif kind==27:
            time=host['engine_time'];threshold,amp=[struct.unpack('<f',struct.pack('<I',x))[0] for x in b['words'][:2]]
            if flicker['last_time'] is None:flicker['last_time']=time
            if flicker['last_time']!=time:
                flicker['last_time']=time
                for j in range(3):
                    value=(i*97+len(draws)*37)%32768;draws.append(value);flicker['samples'][j]=f(value*factor)
            a,bb,c=flicker['samples'];base=f(1.-amp)
            bank['words'][slot]=[bits(1. if test<=threshold else f(f(amp*other)+base)) for test,other in [(a,c),(bb,a),(c,bb)]]+[bits(1.)]
        elif kind!=0:error=f'Unreconstructed shader constant kind {kind} at slot {slot}';break
        if rows is not None:
            assert slot+4<=size;bank['words'][slot:slot+4]=rows;slot+=4;counts['matrix_uploads']+=1
        else:slot+=1
    assert probe['after']==bank,(i,'bank')
    assert probe['error']==error and probe['inverse_calls']==calls and probe['draws']==draws,(i,'callbacks')
    assert probe['flicker_after']==flicker,(i,'flicker')
    counts['updates']+=1;counts['raw_words_checked']+=size*4*2;counts['inverse_calls']+=len(calls);counts['random_draws']+=len(draws);counts['errors']+=error is not None
assert len(report['probes'])==counts['updates']==1024
assert counts['inverse_calls']>0 and counts['random_draws']>0 and counts['errors']>0
tests=sum(map(int,re.findall(r'test result: ok\. (\d+) passed',(root/'analysis/reports/constant-banks-tests.log').read_text())));assert tests==560,tests
sources=['crates/rc-package/src/shader_constants.rs','crates/rc-inspect/src/bin/rc-constant-bank-check.rs','scripts/Record-ConstantBanks.py','analysis/decompiled/shader-constants.c','analysis/decompiled/shader-constants.asm']
original['hashes'][str(n['dll'])]=sha(n['dll'])
validation={'report_sha256':sha(path),'counts':counts,'native_cases':[1,4,5,6,8,9,12,27],'native_span_table':'1001ecc0 / 1001eccc','source_sha256':{s:sha(root/s) for s in sources},'original_sha256':original['hashes'],'rust_tests':tests,'scope':report['scope']}
(root/'analysis/reports/constant-banks-validation.json').write_text(json.dumps(validation,indent=2)+'\n')
e=root/'analysis/evidence.json';evidence=json.loads(e.read_text());evidence['rust_tests']=tests;evidence['constant_banks_validation']=validation;e.write_text(json.dumps(evidence,indent=2,ensure_ascii=False)+'\n',encoding='utf-8')
print(json.dumps({'counts':counts,'rust_tests':tests}))
