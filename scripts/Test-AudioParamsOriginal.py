"""Replay only the original PC PlayUSound volume/pitch instruction region.
Requires unicorn==2.1.4 and pefile==2024.8.26. No original code is distributed.
"""
import argparse,hashlib,json,random,struct
from pathlib import Path
import pefile
from unicorn import Uc,UC_ARCH_X86,UC_MODE_32,UC_HOOK_CODE
from unicorn.x86_const import *
p=argparse.ArgumentParser();p.add_argument('--game-data',type=Path,required=True);p.add_argument('--output',type=Path,required=True);args=p.parse_args()
u=Uc(UC_ARCH_X86,UC_MODE_32);hashes={}
for name in ['engine','core']:
 b=(args.game_data/'System'/(name+'.dll')).read_bytes();pe=pefile.PE(data=b);base=pe.OPTIONAL_HEADER.ImageBase
 u.mem_map(base,(pe.OPTIONAL_HEADER.SizeOfImage+4095)&~4095);u.mem_write(base,pe.get_memory_mapped_image());hashes[name]=hashlib.sha256(b).hexdigest()
assert hashes=={'engine':'f8b3fc9d62414828a1c31d8b5f3b2bcb951f7c0f142fdf2a9846c98d7b5d35a5','core':'4f69fc7aae44560b692538565e41885e79b4ee4afe93bc8400fd13ba207c8709'}
u.mem_map(0x4000000,0x10000);u.mem_map(0x5000000,0x10000)
u.mem_write(0x10650344,struct.pack('<I',0x10110250))
read32=lambda a:struct.unpack('<I',u.mem_read(a,4))[0]
draws=0;accepted=False
def hook(machine,address,size,data):
 global draws,accepted
 if address==0x10110250:draws+=1
 if address in [0x103d5f2c,0x103d600d,0x103d6065]:
  accepted=address==0x103d5f2c;machine.emu_stop()
u.hook_add(UC_HOOK_CODE,hook)
r=random.Random(58001);cases=[]
for i in range(800):
 volumes=sorted(r.choices([0.,0.00001,0.0001,0.05,0.35,0.75,1.,2.,16.],k=2));pitches=sorted(r.choices([0.,0.1,0.5,0.8,1.,1.5,2.,8.],k=2))
 seed=r.getrandbits(32);params=dict(source='prepared-original-fixture',min_volume=volumes[0],max_volume=volumes[1],min_pitch=pitches[0],max_pitch=pitches[1],radius=1024.,non_positional=False,disable_falloff=False,disable_occlusion=False,category=2)
 for at,v in [(0x34,volumes[1]),(0x38,volumes[0]),(0x3c,pitches[1]),(0x40,pitches[0])]:u.mem_write(0x5000000+at,struct.pack('<f',v))
 u.mem_write(0x101be1b8,struct.pack('<I',seed));u.reg_write(UC_X86_REG_EBX,0x5000000);u.reg_write(UC_X86_REG_EBP,0x4008000);u.reg_write(UC_X86_REG_ESP,0x4007000)
 u.reg_write(UC_X86_REG_FPCW,0x37f);u.reg_write(UC_X86_REG_FPSW,0);u.reg_write(UC_X86_REG_FPTAG,0xffff);u.reg_write(UC_X86_REG_MXCSR,0x1f80)
 draws=0;accepted=False;u.emu_start(0x103d5e56,0,count=10000)
 result=dict(accepted=accepted,seed=read32(0x101be1b8),draws=draws)
 if accepted:result.update(volume_bits=read32(0x4008000-0x28),pitch_bits=read32(0x4008000+0x14))
 cases.append(dict(params=params,seed=seed,expected=result))
args.output.parent.mkdir(parents=True,exist_ok=True);args.output.write_text(json.dumps(dict(scope='Prepared PlayUSound parameter instruction region; no complete engine/source allocation/spatial host',binary_sha256=hashes,cases=cases),separators=(',',':'))+'\n',encoding='utf-8')
print(json.dumps(dict(passed=True,cases=len(cases),audible=sum(c['expected']['accepted'] for c in cases))))
