"""Prepared SelectSound/appSRand instruction replay, not the complete game DLL.

Requires unicorn and pefile. Binds the engine appSRand import to original core
code; only the empty-group diagnostic is skipped. Synthetic objects supply
original field layouts. No original binary bytes are distributed in fixtures.
"""
import argparse, hashlib, json, random, struct
from pathlib import Path
import pefile
from unicorn import Uc, UC_ARCH_X86, UC_MODE_32, UC_HOOK_CODE
from unicorn.x86_const import *

parser=argparse.ArgumentParser()
parser.add_argument('--game-data',type=Path,required=True)
parser.add_argument('--output',type=Path,required=True)
args=parser.parse_args()
uc=Uc(UC_ARCH_X86,UC_MODE_32)
hashes={}
for name in ['engine','core']:
    path=args.game_data/'System'/(name+'.dll')
    data=path.read_bytes();p=pefile.PE(data=data)
    base=p.OPTIONAL_HEADER.ImageBase;size=(p.OPTIONAL_HEADER.SizeOfImage+4095)&~4095
    uc.mem_map(base,size);uc.mem_write(base,p.get_memory_mapped_image())
    hashes[name]=hashlib.sha256(data).hexdigest()
assert hashes == {'engine':'f8b3fc9d62414828a1c31d8b5f3b2bcb951f7c0f142fdf2a9846c98d7b5d35a5','core':'4f69fc7aae44560b692538565e41885e79b4ee4afe93bc8400fd13ba207c8709'}, 'Unsupported original DLL revision'
u32=lambda v:struct.pack('<I',v)
f32=lambda v:struct.pack('<f',v)
read32=lambda a:struct.unpack('<I',uc.mem_read(a,4))[0]
assert read32(0x101baeb0)==0x3f800000,'Unexpected appSRand constant'
uc.mem_write(0x10650344,u32(0x10110250))
uc.mem_map(0x4000000,0x10000);uc.mem_map(0x5000000,0x100000);uc.mem_map(0x6000000,4096)
draws=0
def hook(machine,address,size,user):
    global draws
    if address==0x10110250:draws+=1
    if address==0x103d1f91:machine.reg_write(UC_X86_REG_EIP,0x103d1fb3)
uc.hook_add(UC_HOOK_CODE,hook)
pointer=lambda sound:0 if sound is None else 0x5010000+sound*256
def execute(group,state,seed,campaign,flags):
    global draws
    draws=0;obj=0x5000000;array=obj+0x1000
    uc.mem_write(obj,bytes(256))
    uc.mem_write(obj+0x70,u32(array)+u32(len(group['members'])))
    uc.mem_write(obj+0x78,u32(pointer(group['first'])))
    uc.mem_write(obj+0x84,bytes([int(group['in_order'])]))
    uc.mem_write(obj+0x88,u32(state['flags'])+u32(pointer(state['rendered']))+f32(state['total'])+u32(state['which']))
    for i,m in enumerate(group['members']):
        uc.mem_write(array+i*4,u32(pointer(m['sound']) if m else 0))
        if m:
            addr=pointer(m['sound']);uc.mem_write(addr,bytes(256))
            uc.mem_write(addr+0x34,bytes([m['battle'],m['minimum'],m['maximum'],0])+f32(m['likelihood']))
    uc.mem_write(0x101be1b8,u32(seed))
    sp=0x4008000;uc.mem_write(sp,u32(0x6000000)+u32(campaign)+u32(flags))
    uc.reg_write(UC_X86_REG_ESP,sp);uc.reg_write(UC_X86_REG_ECX,obj)
    uc.reg_write(UC_X86_REG_FPCW,0x37f);uc.reg_write(UC_X86_REG_FPSW,0);uc.reg_write(UC_X86_REG_FPTAG,0xffff)
    uc.reg_write(UC_X86_REG_MXCSR,0x1f80)
    uc.emu_start(0x103d1f80,0x6000000,count=20000)
    assert uc.reg_read(UC_X86_REG_EIP)==0x6000000
    result=uc.reg_read(UC_X86_REG_EAX)
    selected=None if result==0 else (result-0x5010000)//256
    rendered=read32(obj+0x8c)
    return dict(selected=selected,seed=read32(0x101be1b8),draws=draws,state=dict(
        flags=read32(obj+0x88),total=struct.unpack('<f',uc.mem_read(obj+0x90,4))[0],
        which=read32(obj+0x94),rendered=None if rendered==0 else (rendered-0x5010000)//256))
r=random.Random(573901);cases=[]
for i in range(320):
    count=0 if i==0 else r.randint(1,8)
    members=[]
    for j in range(count):
        members.append(None if r.random()<0.12 else dict(sound=j,battle=r.randrange(3),minimum=r.choice([0,3,5]),maximum=r.choice([10,255]),likelihood=r.choice([0.,0.125,0.5,1.,3.,7.,0.3333333432674408])))
    if i%3==0:
        for m in filter(None,members):m.update(battle=0,minimum=0,maximum=255)
    group=dict(source='prepared-original-fixture',in_order=i%5==0,first=99 if i%7==0 else None,members=members)
    state=dict(flags=1,total=0.,which=0,rendered=None);seed=r.getrandbits(32)
    for repeat in range(3):
        campaign=r.choice([0,3,5,10,11,255]);flags=r.choice([0,0x200,0x201])
        expected=execute(group,state,seed,campaign,flags)
        cases.append(dict(group=group,state=state,seed=seed,campaign=campaign,flags=flags,expected=expected))
        state=expected['state'];seed=expected['seed']
result=dict(scope='Prepared original x86 SelectSound and appSRand; empty diagnostic skipped; no full engine execution',binary_sha256=hashes,entry='103d1f80',random_entry='10110250',cases=cases)
args.output.parent.mkdir(parents=True,exist_ok=True)
args.output.write_text(json.dumps(result,separators=(',',':'))+'\n',encoding='utf-8')
print(json.dumps(dict(passed=True,cases=len(cases),binary_sha256=hashes)))
