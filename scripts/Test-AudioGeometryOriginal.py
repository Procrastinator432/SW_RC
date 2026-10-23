"""Execute original PC audio geometry queries with supplied BSPs and trace callback.
Requires unicorn==2.1.4 and pefile==2024.8.26. No original bytes exported.
"""
import argparse,hashlib,json,random,struct
from pathlib import Path
import pefile
from unicorn import Uc,UC_ARCH_X86,UC_MODE_32,UC_HOOK_CODE
from unicorn.x86_const import *
p=argparse.ArgumentParser();p.add_argument('--game-data',type=Path,required=True);p.add_argument('--output',type=Path,required=True);p.add_argument('--verify-fixture',type=Path);p.add_argument('--models',type=Path);a=p.parse_args()
u=Uc(UC_ARCH_X86,UC_MODE_32);hashes={}
for name in ['engine','msvcr71']:
 b=(a.game_data/'System'/(name+'.dll')).read_bytes();pe=pefile.PE(data=b);hashes[name]=hashlib.sha256(b).hexdigest();u.mem_map(pe.OPTIONAL_HEADER.ImageBase,(pe.OPTIONAL_HEADER.SizeOfImage+4095)&~4095);u.mem_write(pe.OPTIONAL_HEADER.ImageBase,pe.get_memory_mapped_image())
assert hashes=={'engine':'f8b3fc9d62414828a1c31d8b5f3b2bcb951f7c0f142fdf2a9846c98d7b5d35a5','msvcr71':'8094af5ee310714caebccaeee7769ffb08048503ba478b879edfef5f1a24fefe'}
u.mem_map(0,0x1000);u.mem_map(0x5000000,0x40000);u.mem_map(0x6000000,0x1000000)
def put(at,fmt,*v):u.mem_write(at,struct.pack('<'+fmt,*v))
def get(at,fmt='I'):return struct.unpack('<'+fmt,u.mem_read(at,struct.calcsize('<'+fmt)))[0]
model,nodes,start,end,level,actor,vtable,stack,stub,result= [0x5000000+x for x in [0,0x2000,0x6000,0x6100,0x8000,0xa000,0xc000,0x20000,0x30000,0x31000]]
put(0x10650c4c,'I',0x7c34406f)
u.mem_write(stub,b'\xd9\x1d'+struct.pack('<I',result)+b'\x90')
nodes=0x6000000
trace_calls=[]
def hook(machine,at,size,data):
 if at in [stub+0x100,stub+0x120]:
  sp=machine.reg_read(UC_X86_REG_ESP);trace_calls.append(dict(flags=get(sp+20),extent=list(struct.unpack('<3f',machine.mem_read(get(sp+24),12))),actor=get(sp+8)==actor))
u.hook_add(UC_HOOK_CODE,hook)
def run(at,stop=stub):
 u.reg_write(UC_X86_REG_FPCW,0x37f);u.reg_write(UC_X86_REG_FPSW,0);u.reg_write(UC_X86_REG_FPTAG,0xffff);u.reg_write(UC_X86_REG_MXCSR,0x1f80)
 u.emu_start(at,stop,count=100000);assert u.reg_read(UC_X86_REG_EIP)==stop,hex(u.reg_read(UC_X86_REG_EIP))
def call(at,args,ctx=model,stop=stub):
 put(stack,'I'+'I'*len(args),stub,*args);u.reg_write(UC_X86_REG_ESP,stack);u.reg_write(UC_X86_REG_ECX,ctx);run(at,stop);return u.reg_read(UC_X86_REG_EAX)
# Includes the actual bundled CRT power implementation, without a Python pow callback.
powers=[]
for n in range(256):
 put(level+0xf0+3*64+7,'B',n);call(0x10446180,[3,7],level,stub+6);powers.append(get(result))
r=random.Random(61001);cases=[]
f32=lambda x:struct.unpack('<f',struct.pack('<f',x))[0]
for case in range(512):
 ns=[]
 for i in range(7 if case%8 else 0):
  axis=i%3;plane=[0.,0.,0.,r.choice([0.,-10.,10.,.0001])];plane[axis]=r.choice([-1.,1.])
  ns.append(dict(plane=plane,back=2*i+1 if 2*i+1<7 else -1,front=2*i+2 if 2*i+2<7 else -1,flags=r.choice([0,1,2,4,32,33,255]),zones=[r.randrange(4),r.randrange(4)],leaves=[i*2,i*2+1]))
 root_outside=bool(case%2);zone_actors=[0,101,0,103] if case%3 else []
 put(model+0x58,'II',nodes,len(ns));put(model+0xd4,'I',int(root_outside));put(model+0xe4,'I',len(zone_actors))
 for i in range(4):put(model+0xe8+i*24,'I',zone_actors[i] if zone_actors else 0)
 for i,n in enumerate(ns):
  at=nodes+i*100;put(at,'4f',*n['plane']);put(at+0x20,'ii',n['back'],n['front']);put(at+0x44,'BBBB',*n['zones'],3,n['flags']);put(at+0x48,'ii',*n['leaves'])
 point=[f32(r.choice([0.,-0.,.0001,-.0001,10.,-10.,r.uniform(-30,30)])) for _ in range(3)]
 other=[f32(r.uniform(-40,40)) for _ in range(3)]
 if case%5==0:other=point[:]
 put(start,'3f',*point);put(end,'3f',*other)
 clear=bool(call(0x10552a60,[start,end])&255)
 call(0x105538e0,[result,999,start]);region=dict(actor=get(result),leaf=get(result+4,'i'),zone=get(result+8)&255)
 cases.append(dict(bsp=dict(nodes=ns,root_outside=root_outside,zone_actors=zone_actors),start=point,end=other,clear=clear,region=region))
# Full dispatch function, original BSP query; external general trace explicitly supplied.
put(level,'I',vtable);put(level+0x78,'I',model)
for value in [0,1]:u.mem_write(stub+0x100+value*0x20,b'\xb8'+struct.pack('<I',value)+b'\xc2\x18\x00')
# Use a guaranteed solid empty model so BSP returns false and bypass returns true.
put(model+0x5c,'I',0);put(model+0xd4,'I',0)
policies=[]
for mode in range(256):
 for mover in [False,True]:
  for trace in [False,True]:
   put(actor+0x24,'I',0x10758fc8 if mover else 0);put(actor+0x1e7,'B',mode)
   put(vtable+0xe4,'I',stub+0x100+int(trace)*0x20);trace_calls.clear()
   audible=bool(call(0x104468b0,[start,end,actor],level));policies.append(dict(mode=mode,mover=mover,trace=trace,audible=audible,trace_called=bool(trace_calls)))
   if trace_calls:assert trace_calls==[dict(flags=0x286,extent=[0.,0.,0.],actor=True)]
null_actor=bool(call(0x104468b0,[start,end,0],level));assert not null_actor
# Compose exact original zone factors with the existing original radius regions.
sequences=[]
source,frame=actor,level+0x5000
for sequence in range(16):
 base=[10.,100.][sequence%2];put(source+0x7c,'ff',base,base);put(source+0x84,'d',0.);steps=[]
 for tick in range(32):
  clear=(tick//4+sequence)%3==0;distance=(tick//3+sequence)%8;now=f32(tick/60+sequence*.25)
  put(frame-0x14,'f',base);put(0x1076390c,'f',now);u.reg_write(UC_X86_REG_ESI,source);u.reg_write(UC_X86_REG_EBP,frame)
  if clear:run(0x103d26a9,0x103d26b3)
  else:
   put(frame-0x18,'I',powers[distance]);u.reg_write(UC_X86_REG_EAX,source+0x500);run(0x103d267f,0x103d26a7)
  u.reg_write(UC_X86_REG_ECX,source+0x500);run(0x103d26f6,0x103d2771)
  steps.append(dict(clear=clear,distance=distance,now=now,priority_bits=get(source+0x78),current_bits=get(source+0x7c),target_bits=get(source+0x80),changed_at_bits=get(source+0x84,'Q')))
 sequences.append(dict(base_radius=base,steps=steps))
map_results=[]
if a.models:
 for path in sorted(a.models.glob('geo_*.json')):
  m=json.loads(path.read_text(encoding='utf-8'));bsp=m['bsp'];ns=bsp['nodes'];assert len(ns)*100<0x1000000
  put(model+0x58,'II',nodes,len(ns));put(model+0xd4,'I',int(bsp['root_outside']));put(model+0xe4,'I',len(bsp['zone_actors']))
  for i in range(64):put(model+0xe8+i*24,'I',(bsp['zone_actors'][i] if i<len(bsp['zone_actors']) else 0)&0xffffffff)
  for i,n in enumerate(ns):
   at=nodes+i*100;put(at,'4f',*n['plane']);put(at+0x20,'ii',n['back'],n['front']);put(at+0x44,'BBBB',*n['zones'],3,n['flags']);put(at+0x48,'ii',*n['leaves'])
  for q in m['queries']:
   put(start,'3f',*q['start']);put(end,'3f',*q['end']);assert bool(call(0x10552a60,[start,end])&255)==q['clear'],(path,q)
   call(0x105538e0,[result,999,start]);region=dict(actor=get(result,'i'),leaf=get(result+4,'i'),zone=get(result+8)&255);assert region==q['region'],(path,region,q['region'])
  map_results.append(dict(map=m['map'],queries=len(m['queries']),nodes=len(ns),all_original_queries_equal=True))
out=dict(binary_sha256=hashes,scope='Full original FastLineCheck, PointRegion, IsAudibleAt (general trace supplied), CalculateRadiusMultiplier with bundled CRT',zone_multiplier_bits=powers,radius_sequences=sequences,geometry=cases,policies=policies,null_actor_uses_bsp=True)
if a.verify_fixture:assert json.loads(a.verify_fixture.read_text(encoding='utf-8'))==out
a.output.parent.mkdir(parents=True,exist_ok=True);a.output.write_text(json.dumps(out,separators=(',',':'))+'\n',encoding='utf-8');print(json.dumps(dict(passed=True,geometry_cases=len(cases),policy_cases=len(policies),zone_factors=len(powers),radius_steps=sum(len(s['steps']) for s in sequences),maps=map_results)))
if a.models:(a.output.parent/'map-original-validation.json').write_text(json.dumps(dict(passed=True,maps=map_results),indent=2)+'\n',encoding='utf-8')
