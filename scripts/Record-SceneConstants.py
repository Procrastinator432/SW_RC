"""Independent replay of original scalar SSE instructions on supplied snapshots."""
from pathlib import Path
import struct, json, re, hashlib
root=Path(__file__).resolve().parents[1]
bits=lambda x:struct.unpack('<I',struct.pack('<f',x))[0]
value=lambda x:struct.unpack('<f',struct.pack('<I',x))[0]
f=lambda x:value(bits(x))
sha=lambda p:hashlib.sha256(p.read_bytes()).hexdigest()
# Read original PE constants directly, without loading/executing the DLL.
dll=Path(r'D:\SteamLibrary\steamapps\common\Star Wars Republic Commando\GameData\System\D3DDrv.dll')
pe=dll.read_bytes();h=struct.unpack_from('<I',pe,60)[0];opt=struct.unpack_from('<H',pe,h+20)[0];base=struct.unpack_from('<I',pe,h+52)[0]
def memory(address):
    for i in range(struct.unpack_from('<H',pe,h+6)[0]):
        at=h+24+opt+i*40;virtual,rva,raw,offset=struct.unpack_from('<IIII',pe,at+8)
        if rva<=address-base<rva+raw:return struct.unpack_from('<I',pe,offset+address-base-rva)[0]
    raise AssertionError(hex(address))
assert memory(0x1006fefc)==bits(3.) and memory(0x1006fa5c)==bits(.5)
assert memory(0x10072674)==0x322bcc77 and memory(0x1006f9a0)==bits(1.)
asm=[]
for line in (root/'analysis/decompiled/shader-constants.asm').read_text().splitlines():
    if not line or line.startswith('#'):continue
    a,instruction=line.split(' ',1);op,_,args=instruction.partition(' ');asm.append((int(a,16),op,args))
def replay(p,start,end,mode):
    stack={-0x6c+4*(r*4+c):value(p['inverse'][r][c]) for r in range(4) for c in range(4)}
    registers={};eax=0;out={}
    eye=p['editor_eye'] if p['editor'] else p['runtime_eye']
    eye_base=0x138 if p['editor'] else 0x194
    def address(op):
        m=re.fullmatch(r'dword ptr \[(EBP|EAX|ECX|ESI)(?: \+ (-?0x[0-9a-f]+))?\]',op)
        assert m,op
        offset=int(m[2],16) if m[2] else 0
        if offset>=2**31:offset-=2**32
        return m[1],offset
    def operand(op):
        if op.startswith('XMM'):return registers[op]
        absolute=re.fullmatch(r'dword ptr \[(0x[0-9a-f]+)\]',op)
        if absolute:return value(memory(int(absolute[1],16)))
        base,offset=address(op)
        if base=='EBP':return stack[offset]
        if mode=='eye':
            assert base=='EAX';return value(eye[(eax+offset-eye_base)//4])
        if base=='ECX':return value(p['object'][0][(offset-0x2c)//4])
        assert base=='ESI';return value(p['light']['radius'][(offset-0x30)//4])
    for a,op,args in asm:
        if not start<=a<=end:continue
        if op=='ADD' and args.startswith('EAX,'):
            eax+=int(args.split(',')[1],16);continue
        if op not in ('MOVSS','MOVAPS','MULSS','ADDSS','SUBSS','DIVSS','XORPS','RSQRTSS','CMPNEQSS','ANDPS'):continue
        dst,src=args.split(',',1)
        if not dst.startswith('XMM'):
            base,offset=address(dst)
            if base=='EBP':stack[offset]=operand(src)
            else:out[offset]=bits(operand(src))
            continue
        if op=='XORPS':registers[dst]=0.;continue
        if op=='RSQRTSS':
            assert bits(operand(src))==p['squared'];registers[dst]=value(p['seed']);continue
        if op in ('CMPNEQSS','ANDPS'):
            # All corpus squared lengths are finite, positive: native mask is all ones.
            assert value(p['squared'])>0.;continue
        v=operand(src)
        if op=='MULSS':v=f(registers[dst]*v)
        elif op=='ADDSS':v=f(registers[dst]+v)
        elif op=='SUBSS':v=f(registers[dst]-v)
        elif op=='DIVSS':v=f(registers[dst]/v)
        registers[dst]=v
    if mode=='eye':
        offset=-0x110 if p['editor'] else -0xbc
        return [bits(stack[offset+4*k]) for k in range(3)]+[bits(1.)]
    return [out[0],out[4],p['light']['radius'][1],bits(1.)]
path=root/'analysis/reports/scene-constants.json';report=json.loads(path.read_text())
assert path.read_bytes()==(root/'analysis/reports/scene-constants-release.json').read_bytes()
assert len(report['probes'])==1024
for i,p in enumerate(report['probes']):
    assert p['index']==i and p['editor']==(i%2==0)
    assert p['object']==[[bits(f((2. if r==c else 0.)+(i+r*7+c*3)*.001953125)) for c in range(4)] for r in range(4)]
    assert p['inverse']==[[0x7fc12345 if c==3 else bits(f(((i+r*11+c*5)%31)*.125-1.)) for c in range(4)] for r in range(4)]
    eye_words=list(map(bits,[i*.125,-2.5,3.25]));assert p['runtime_eye']==eye_words and p['editor_eye']==[eye_words[2],eye_words[0],eye_words[1]]
    assert p['fog']==list(map(bits,[i*.25,i*.25+(4. if i%2==0 else -2.)]))
    for s,light in enumerate(p['lights']):
        assert light['kind']==(19 if (i+s)%2==0 else 7) and light['actor_present']
        assert light['radius']==list(map(bits,[(i+s)*.125+4.,s*.25]))
    expected=[]
    for light in p['lights']:
        q={**p,'light':light}
        if light['kind']==19:
            outer,inner=map(value,light['radius']);v=bits(f(outer-inner));result=[0x322bcc77,0x322bcc77,v,bits(1.)]
        else:result=replay(q,0x1001e6cd,0x1001e79a,'radius')
        expected.append(result)
    eye=replay(p,0x1001e1e3 if p['editor'] else 0x1001e2ae,0x1001e28f if p['editor'] else 0x1001e35a,'eye')
    start,end=map(value,p['fog']);reciprocal=f(1./f(end-start));fog=[*p['fog'],bits(f(end*reciprocal)),bits(reciprocal)]
    expected.extend([eye,fog,eye,[0x7fc12345,0x80000000,7,i]])
    assert p['bank']=={'count':7,'words':expected},i
    assert p['inverse_calls']==[p['object']],i
sources=['crates/rc-package/src/shader_scene.rs','crates/rc-package/src/shader_lights.rs','crates/rc-package/src/shader_constants.rs','crates/rc-inspect/src/bin/rc-scene-constant-check.rs','scripts/Record-SceneConstants.py','analysis/decompiled/shader-constants.asm']
result={'date':'2026-10-08','scope':'Native instruction replay, supplied snapshots, host RSQRTSS seeds; no engine execution or portable seed bit parity claim','contexts':1024,'light_radius_registers':4096,'eye_registers':2048,'fog_registers':1024,'bank_words':32768,'inverse_calls':1024,'debug_release_identical':True,'original_dll_sha256':sha(dll),'source_sha256':{s:sha(root/s) for s in sources},'android':'deferred until end per user'}
tests=sum(map(int,re.findall(r'test result: ok\. (\d+) passed',(root/'analysis/reports/scene-constants-tests.log').read_text())))
assert tests==575,tests
result['rust_tests']=tests
(root/'analysis/reports/scene-constants-validation.json').write_text(json.dumps(result,indent=2)+'\n')
ledger=root/'analysis/evidence.json';evidence=json.loads(ledger.read_text(encoding='utf-8'));evidence['rust_tests']=tests;evidence['scene_constants_validation']=result;ledger.write_text(json.dumps(evidence,indent=2,ensure_ascii=False)+'\n',encoding='utf-8')
print(json.dumps(result,indent=2))
