"""Execute the original PC OpenAL source coordinate/reference-radius conversion.
Only the selected non-local, falloff-enabled arithmetic regions are replayed.
Requires unicorn==2.1.4 and pefile==2024.8.26; no DLL bytes are written out.
"""
import argparse,hashlib,json,random,struct
from pathlib import Path
import pefile
from unicorn import Uc,UC_ARCH_X86,UC_MODE_32
from unicorn.x86_const import *

p=argparse.ArgumentParser();p.add_argument('--game-data',type=Path,required=True);p.add_argument('--output',type=Path,required=True);args=p.parse_args()
b=(args.game_data/'System/alaudio.dll').read_bytes();digest=hashlib.sha256(b).hexdigest()
assert digest=='60e0c8495a94b297c36bc96f29e4c64a208f020e1ac55f5d6848d5697cff0dcd','Unsupported original audio DLL'
pe=pefile.PE(data=b);u=Uc(UC_ARCH_X86,UC_MODE_32);u.mem_map(pe.OPTIONAL_HEADER.ImageBase,(pe.OPTIONAL_HEADER.SizeOfImage+4095)&~4095);u.mem_write(pe.OPTIONAL_HEADER.ImageBase,pe.get_memory_mapped_image());u.mem_map(0x5000000,0x10000)
f32=lambda v:struct.unpack('<f',struct.pack('<f',v))[0]
r=random.Random(59002);cases=[]
for i in range(512):
 position=[f32(r.uniform(-1e6,1e6)) for _ in range(3)];radius=f32(r.choice([0,15,100,1024,1e6]))
 u.mem_write(0x500005c,struct.pack('<3f',*position));u.mem_write(0x500007c,struct.pack('<f',radius))
 u.reg_write(UC_X86_REG_ESI,0x5000000);u.reg_write(UC_X86_REG_EBP,0x5008000);u.reg_write(UC_X86_REG_EAX,0);u.reg_write(UC_X86_REG_MXCSR,0x1f80)
 u.emu_start(0x10006335,0x1000636a,count=100)
 assert u.reg_read(UC_X86_REG_EIP)==0x1000636a
 actual=struct.unpack('<3I',u.mem_read(0x5008000-0x3c,12))
 scale=f32(.01);expected=struct.unpack('<3I',struct.pack('<3f',*[f32(position[j]*scale) for j in [0,2,1]]));assert actual==expected
 u.emu_start(0x1000642c,0x10006435,count=100)
 radius_bits=u.reg_read(UC_X86_REG_XMM0)&0xffffffff;assert radius_bits==struct.unpack('<I',struct.pack('<f',radius*scale))[0]
 cases.append(dict(position=position,radius=radius,openal_position_bits=actual,reference_radius_bits=radius_bits))
report=dict(passed=True,scope='Original FALSource Update selected coordinate/reference-radius SSE regions only; not OpenAL rendering/HRTF',binary_sha256=digest,cases=cases)
args.output.parent.mkdir(parents=True,exist_ok=True);args.output.write_text(json.dumps(report,separators=(',',':'))+'\n',encoding='utf-8');print(json.dumps(dict(passed=True,original_cases=len(cases))))
