"""Replay original PC ambient distance branches and non-streamed fade-out.
Geometry/actor lookup is supplied; no engine allocator or original bytes exported.
Requires unicorn==2.1.4 and pefile==2024.8.26.
"""
import argparse,hashlib,json,random,struct
from pathlib import Path
import pefile
from unicorn import Uc,UC_ARCH_X86,UC_MODE_32,UC_HOOK_CODE
from unicorn.x86_const import *
p=argparse.ArgumentParser();p.add_argument('--game-data',type=Path,required=True);p.add_argument('--output',type=Path,required=True);p.add_argument('--verify-fixture',type=Path);a=p.parse_args()
u=Uc(UC_ARCH_X86,UC_MODE_32);hashes={}
for name in ['engine','core']:
 b=(a.game_data/'System'/(name+'.dll')).read_bytes();pe=pefile.PE(data=b);hashes[name]=hashlib.sha256(b).hexdigest();u.mem_map(pe.OPTIONAL_HEADER.ImageBase,(pe.OPTIONAL_HEADER.SizeOfImage+4095)&~4095);u.mem_write(pe.OPTIONAL_HEADER.ImageBase,pe.get_memory_mapped_image())
assert hashes=={'engine':'f8b3fc9d62414828a1c31d8b5f3b2bcb951f7c0f142fdf2a9846c98d7b5d35a5','core':'4f69fc7aae44560b692538565e41885e79b4ee4afe93bc8400fd13ba207c8709'}
u.mem_map(0x5000000,0x10000)
def put(at,fmt,*v):u.mem_write(at,struct.pack('<'+fmt,*v))
def get(at,fmt='I'):return struct.unpack('<'+fmt,u.mem_read(at,struct.calcsize('<'+fmt)))[0]
put(0x106503b8,'I',0x101ba518);assert get(0x101ba518,'f')==100 and get(0x10668e18,'f')==10
stops=set()
def hook(machine,address,size,data):
 if address in stops:machine.emu_stop()
u.hook_add(UC_HOOK_CODE,hook)
def run(start,ends):
 global stops
 stops=set(ends);u.reg_write(UC_X86_REG_FPCW,0x37f);u.reg_write(UC_X86_REG_FPSW,0);u.reg_write(UC_X86_REG_FPTAG,0xffff);u.reg_write(UC_X86_REG_MXCSR,0x1f80)
 u.emu_start(start,0,count=2000);at=u.reg_read(UC_X86_REG_EIP);assert at in stops,hex(at);return at
f32=lambda v:struct.unpack('<f',struct.pack('<f',v))[0]
params,source,actor,listener,frame,stack,stub,result=[0x5000000+n for n in [0,0x1000,0x2000,0x3000,0x5000,0x7000,0x9000,0x9100]]
put(source+0x14,'I',actor);put(source+0xa4,'I',0);put(actor+0x8c,'I',1)
r=random.Random(60001);cases=[]
for i in range(800):
 radius=r.choice([0.,1.,10.,100.,1024.]);scale=r.choice([0.,.1,.5,1.]);position=[f32(r.uniform(-10000,10000)) for _ in range(3)];eye=[f32(v+r.uniform(-radius*200,radius*200)) for v in position]
 if i%4==0:
  eye=position[:];eye[0]=f32(eye[0]+f32(radius*100*scale)+r.choice([0.,.001,10.,10.001]))
 put(params+0x58,'f',radius);put(actor+0x138,'3f',*position);put(listener,'3f',*eye)
 u.reg_write(UC_X86_REG_EBP,frame);u.reg_write(UC_X86_REG_EBX,actor+0x138);u.reg_write(UC_X86_REG_EAX,params);put(frame+0x14,'I',listener);put(frame-0x14,'f',scale)
 admit=run(0x103d7a48,[0x103d7aac,0x103d7b4c])==0x103d7aac
 u.reg_write(UC_X86_REG_EDI,source);u.reg_write(UC_X86_REG_EAX,params);put(frame-0x20,'f',scale);put(frame-0x40,'3f',*eye)
 release=run(0x103d8459,[0x103d8510,0x103d8527])==0x103d8510
 u.reg_write(UC_X86_REG_EDI,source);u.reg_write(UC_X86_REG_EAX,params)
 queued=run(0x103d8633,[0x103d86b2,0x103d86b9])==0x103d86b2
 cases.append(dict(location=position,listener=eye,radius=radius,occlusion=scale,admit=admit,release=release,release_queued=queued))
# Original linear curve setter and complete GetFadeMultiplier for 18 ticks.
put(source+0x50,'f',1.);put(source+0xa4,'I',2);put(source+0x9c,'f',.3);put(source+0xa0,'f',0.)
put(stack,'IffI',stub,0.,.3,0);u.reg_write(UC_X86_REG_ESP,stack);u.reg_write(UC_X86_REG_ECX,source+0x40);run(0x103d1cf0,[stub])
u.mem_write(stub+16,b'\xd9\x1d'+struct.pack('<I',result)+b'\x90');fade=[]
for tick in range(1,19):
 put(stack,'If',stub+16,1/60);u.reg_write(UC_X86_REG_ESP,stack);u.reg_write(UC_X86_REG_ECX,source);run(0x103d2810,[stub+22])
 fade.append(dict(tick=tick,value_bits=get(result),elapsed_bits=get(source+0xa0)))
data=dict(binary_sha256=hashes,scope='Prepared original admission/active-release/queued-release branches; geometry supplied; non-streamed linear fade controller',distances=cases,fade=fade)
if a.verify_fixture:assert json.loads(a.verify_fixture.read_text(encoding='utf-8'))==data
a.output.parent.mkdir(parents=True,exist_ok=True);a.output.write_text(json.dumps(data,separators=(',',':'))+'\n',encoding='utf-8');print(json.dumps(dict(passed=True,distance_cases=len(cases),original_branches=len(cases)*3,fade_ticks=18)))
