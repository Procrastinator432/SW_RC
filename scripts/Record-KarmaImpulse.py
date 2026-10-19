"""Replay the exported x87 accumulator instructions with supplied COM/body data.
VM/model lookup and process FPU control are explicit boundaries. Bounded float
inputs use f64 register arithmetic; every original FSTP rounds to float32.
"""
import argparse, hashlib, json, random, re, struct
from pathlib import Path

p=argparse.ArgumentParser(description=__doc__)
p.add_argument('engine',type=Path);p.add_argument('instructions',type=Path);p.add_argument('output',type=Path)
a=p.parse_args();raw=a.engine.read_bytes();pe=struct.unpack_from('<I',raw,60)[0]
base=struct.unpack_from('<I',raw,pe+52)[0];optional=struct.unpack_from('<H',raw,pe+20)[0]
sections=[struct.unpack_from('<IIII',raw,pe+24+optional+40*i+8) for i in range(struct.unpack_from('<H',raw,pe+6)[0])]
def word(address):
 for _,va,count,offset in sections:
  if va<=address-base<va+count:return struct.unpack_from('<I',raw,offset+address-base-va)[0]
 raise ValueError(hex(address))
def bits(v):return struct.unpack('<I',struct.pack('<f',v))[0]
def number(v):return struct.unpack('<f',struct.pack('<I',v))[0]
assert word(0x1067eacc)==bits(.02)
code=[(int(pc,16),op,args) for pc,op,args in re.findall(r'^([0-9a-f]{8}) ([A-Z0-9]+)(?: (.*))?$',a.instructions.read_text(),re.M) if 0x10596582<=int(pc,16)<=0x1059662a]
assert len(code)==40, len(code)
def replay(old_linear,old_angular,impulse,position,center):
 mem={('ESP',i*4):v for i,v in enumerate(center)}
 mem.update({('ESP',0x14+i*4):v for i,v in enumerate(impulse)})
 mem.update({('ESP',0x20+i*4):v for i,v in enumerate(position)})
 mem.update({('EDX',0x170+i*4):v for i,v in enumerate(old_linear)})
 mem.update({('EDX',0x180+i*4):v for i,v in enumerate(old_angular)})
 stack=[]
 def address(s):
  m=re.fullmatch(r'(?:float|dword) ptr \[(ESP|EDX)(?: \+ (0x[0-9a-f]+))?\]',s);assert m,s
  return m[1],int(m[2] or '0',0)
 def value(s):return stack[int(s[2:])] if s.startswith('ST') else mem[address(s)]
 for _,op,s in code:
  if op=='MOV':continue # pending impulse flag
  if op=='FLD':stack.insert(0,value(s))
  elif op=='FSUB':stack[0]-=value(s)
  elif op=='FMUL':stack[0]*=value(s)
  elif op=='FADD':stack[0]+=value(s)
  elif op=='FSUBP':v=stack.pop(0);stack[0]-=v
  elif op=='FSTP':mem[address(s)]=number(bits(stack.pop(0)))
  elif op=='FXCH':stack[0],stack[1]=stack[1],stack[0]
  else:raise ValueError((op,s))
 assert not stack
 return [[bits(mem[('EDX',offset+i*4)]) for i in range(3)] for offset in [0x170,0x180]]
rng=random.Random(0x4b41524d);rows=[]
for i in range(256):
 vectors=[[number(bits(rng.uniform(-m,m))) for _ in range(3)] for m in [1000,100000,1000,100,100]]
 result=replay(*vectors)
 rows.append(dict(input=[[bits(v) for v in vec] for vec in vectors],linear=result[0],angular=result[1]))
out=dict(engine_sha256=hashlib.sha256(raw).hexdigest(),instruction_sha256=hashlib.sha256(a.instructions.read_bytes()).hexdigest(),scale_bits=word(0x1067eacc),boundary=__doc__,frames=rows)
a.output.parent.mkdir(parents=True,exist_ok=True);a.output.write_text(json.dumps(out,separators=(',',':'))+'\n')
print(json.dumps(dict(frames=len(rows),engine_sha256=out['engine_sha256'],scale_bits=out['scale_bits'])))
