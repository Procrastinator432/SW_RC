"""Independent original x86 render/stage translation and D3D8 cache call replay."""
from pathlib import Path
import json,re,struct,hashlib,collections,functools
root=Path(__file__).resolve().parents[1]
s=root/'scripts/Record-MaterialState.py';old={'__file__':str(s)}
exec(compile(s.read_text().split('fixtures=json.loads')[0],str(s),'exec'),old)
sha=old['sha'];dll=old['dll'];raw=old['raw'];sections=old['sections']
def pe_word(address):
 rva=address-0x10000000
 for size,va,n,off in sections:
  if va<=rva<va+max(size,n):return struct.unpack_from('<I',raw,off+rva-va)[0]
 raise AssertionError(hex(address))
assert pe_word(0x100726c4)==0xc2c80000
asm='\n'.join((root/p).read_text() for p in ['analysis/decompiled/d3d-pass-translation.asm','analysis/decompiled/d3d-state-cache.asm'])
code={int(a,16):(op,args.split(',') if args else []) for a,op,args in re.findall(r'^([0-9a-f]{8}) ([A-Z.]+)(?: (.*))?$',asm,re.M)}
addresses=sorted(code);following={a:addresses[i+1] for i,a in enumerate(addresses[:-1])}
@functools.lru_cache(None)
def parsed(expr):
 terms=[];constant=0
 for part in expr.replace(' ','').replace('-','+-').split('+'):
  if not part:continue
  if '*' in part:
   reg,scale=part.split('*');assert reg in ['EAX','EBX','ECX','EDX','ESI','EDI','EBP','ESP'];terms.append((reg,int(scale,0)))
  elif part in ['EAX','EBX','ECX','EDX','ESI','EDI','EBP','ESP']:terms.append((part,1))
  else:constant+=int(part,0)
 return constant,terms
class Native(old['Machine']):
 def __init__(self):
  super().__init__({},{});self.s=False;self.o=False;self.events=[];self.xmm={}
 def bits(self,arg):
  if arg.startswith('word ptr'):return 16
  return 8 if arg=='AH' else super().bits(arg)
 def get(self,arg):
  if arg in self.xmm:return self.xmm[arg]
  return (self.r['EAX']>>8)&255 if arg=='AH' else super().get(arg)
 def address(self,arg):
  c,t=parsed(arg[arg.index('[')+1:arg.index(']')]);return (c+sum(self.r[r]*scale for r,scale in t))&0xffffffff
 def run(self,start,stop):
  pc=start;local_steps=0
  while pc!=stop:
   self.steps+=1;local_steps+=1;assert local_steps<10000,hex(pc)
   op,a=code[pc];nxt=following.get(pc)
   if op in ['MOV','MOVZX']:self.put(a[0],self.get(a[1]))
   elif op=='MOVSS':
    if a[0].startswith('XMM'):self.xmm[a[0]]=self.get(a[1])
    else:self.put(a[0],self.get(a[1]))
   elif op=='XORPS':
    assert a[0]==a[1];self.xmm[a[0]]=0
   elif op=='NOP':pass
   elif op in ['STOSD.REP','STOSB.REP']:
    assert a==['ES:EDI'] # native ABI has clear direction flag
    unit=4 if op=='STOSD.REP' else 1;kind='dword' if unit==4 else 'byte'
    for _ in range(self.r['ECX']):
     self.put(f'{kind} ptr [EDI]',self.r['EAX']);self.r['EDI']+=unit
    self.r['ECX']=0
   elif op in ['MOVSD.REP','MOVSB.REP']:
    assert a==['ES:EDI','ESI'] # native ABI has clear direction flag
    unit=4 if op=='MOVSD.REP' else 1;kind='dword' if unit==4 else 'byte'
    for _ in range(self.r['ECX']):
     self.put(f'{kind} ptr [EDI]',self.get(f'{kind} ptr [ESI]'));self.r['ESI']+=unit;self.r['EDI']+=unit
    self.r['ECX']=0
   elif op=='COMISS':
    x=struct.unpack('<f',struct.pack('<I',self.xmm[a[0]]))[0];y=struct.unpack('<f',struct.pack('<I',self.get(a[1])))[0]
    unordered=x!=x or y!=y;self.z=unordered or x==y;self.c=unordered or x<y;self.s=self.o=False
   elif op=='LEA':self.put(a[0],self.address(a[1]))
   elif op=='PUSH':self.push(self.get(a[0]))
   elif op=='POP':self.put(a[0],self.pop())
   elif op in ['SHL','SHR','SAR']:
    x=self.get(a[0]);shift=self.get(a[1])&31
    if op=='SAR' and x&(1<<(self.bits(a[0])-1)):x-=1<<self.bits(a[0])
    v=x<<shift if op=='SHL' else x>>shift;self.put(a[0],v)
   elif op in ['ADD','SUB','CMP','XOR','AND','OR','TEST','SBB']:
    x=self.get(a[0]);y=self.get(a[1]);width=self.bits(a[0]);mask=(1<<width)-1;sign=1<<(width-1);carry=int(self.c)
    v={'ADD':lambda:x+y,'SUB':lambda:x-y,'CMP':lambda:x-y,'XOR':lambda:x^y,'AND':lambda:x&y,'OR':lambda:x|y,'TEST':lambda:x&y,'SBB':lambda:x-y-carry}[op]()&mask
    self.z=v==0;self.s=bool(v&sign)
    if op in ['SUB','CMP','SBB']:self.c=x<y+(carry if op=='SBB' else 0);self.o=bool((x^y)&(x^v)&sign)
    elif op=='ADD':self.c=x+y>mask;self.o=bool((~(x^y))&(x^v)&sign)
    else:self.c=self.o=False
    if op not in ['CMP','TEST']:self.put(a[0],v)
   elif op in ['INC','DEC','NEG']:
    x=self.get(a[0]);width=self.bits(a[0]);mask=(1<<width)-1;v=({'INC':x+1,'DEC':x-1,'NEG':-x}[op])&mask
    self.put(a[0],v);self.z=v==0;self.s=bool(v&(1<<(width-1)))
    if op=='NEG':self.c=x!=0;self.o=x==1<<(width-1)
    else:self.o=(x==(1<<(width-1))-1 if op=='INC' else x==1<<(width-1))
   elif op=='IMUL':self.put(a[0],self.get(a[0])*self.get(a[1]) if len(a)==2 else self.get(a[1])*self.get(a[2]))
   elif op=='CDQ':self.r['EDX']=0xffffffff if self.r['EAX']&0x80000000 else 0
   elif op in ['DIV','IDIV']:
    numerator=(self.r['EDX']<<32)|self.r['EAX'];divisor=self.get(a[0]);assert divisor!=0
    if op=='IDIV':
     if numerator&(1<<63):numerator-=1<<64
     if divisor&(1<<31):divisor-=1<<32
     quotient=abs(numerator)//abs(divisor)
     if (numerator<0)!=(divisor<0):quotient=-quotient
     remainder=numerator-quotient*divisor;assert -(1<<31)<=quotient<(1<<31)
    else:quotient,remainder=divmod(numerator,divisor);assert quotient<1<<32
    self.r['EAX']=quotient&0xffffffff;self.r['EDX']=remainder&0xffffffff
   elif op in ['SETNZ','SETZ']:self.put(a[0],int(not self.z) if op=='SETNZ' else int(self.z))
   elif op=='CMOVZ':
    if self.z:self.put(a[0],self.get(a[1]))
   elif op in ['JZ','JNZ','JNC','JMP','JBE','JL','JLE','JGE','JNS']:
    branch={'JZ':self.z,'JNZ':not self.z,'JNC':not self.c,'JMP':True,'JBE':self.c or self.z,'JL':self.s!=self.o,'JLE':self.z or self.s!=self.o,'JGE':self.s==self.o,'JNS':not self.s}[op]
    if branch:nxt=int(a[0],0)
   elif op=='CALL':
    if a[0]=='dword ptr [0x1006f3e8]':
     self.run(0x10338670,0x10338672);self.steps+=1 # original FColor MOV, then no-argument RET
     pc=nxt;continue
    match=re.fullmatch(r'dword ptr \[[A-Z]+ \+ (0x[0-9a-f]+)\]',a[0]);assert match,(hex(pc),a)
    offset=int(match[1],16);assert offset in [0xc8,0xfc,0xf4,0x94,0x130,0x14c,0x160,0x154,0xb0,0xb8,0x16c,0x118,0x11c],hex(offset)
    assert self.pop()==0x23000000
    args=[self.pop() for _ in range({0xc8:2,0xfc:3,0xf4:2,0x94:2,0x130:1,0x14c:3,0x160:1,0x154:2,0xb0:2,0xb8:2,0x16c:3,0x118:3,0x11c:5}[offset])]
    if offset==0x94:
     self.events.append({'vtable_offset':offset,'transform':args[0],'matrix':[self.get(f'dword ptr [{args[1]+k*4}]') for k in range(16)]})
    elif offset==0xb0:self.events.append({'vtable_offset':offset,'index':args[0],'words':[self.get(f'dword ptr [{args[1]+k*4}]') for k in range(26)]})
    elif offset==0x16c:self.events.append({'vtable_offset':offset,'first':args[0],'count':args[2],'words':[self.get(f'dword ptr [{args[1]+k*4}]') for k in range(args[2]*4)]})
    else:self.events.append({'vtable_offset':offset,'arguments':args})
    self.r['EAX']=0x80004005 # ignored failure HRESULT, cache write already happened
   else:raise AssertionError((hex(pc),op,a))
   pc=nxt
BASE=0x2200e7f0;R=0x21000000;D=0x22000000;P=0x26000000;FRAME=0x27000000
def write(vm,address,values):
 for k,v in enumerate(values):vm.put(f'dword ptr [{address+4*k}]',v)
def values(vm,applied=False):
 delta=0xa34 if applied else 0
 get=lambda a,n:[vm.get(f'dword ptr [{a+delta+4*k}]') for k in range(n)]
 return {'render':get(BASE+4,32),'stages':[get(BASE+0x84+s*0x54,21) for s in range(8)],'textures':get(BASE+0x6b8,8)}
def state(vm):return {'desired':values(vm),'applied':values(vm,True),'dirty':vm.get(f'dword ptr [{BASE+0x146c}]')}
def initialize(case):
 vm=Native();vm.put(f'dword ptr [{R+4}]',D);vm.put(f'dword ptr [{BASE}]',D);vm.put(f'dword ptr [{D+0x46b4}]',0x23000000);vm.put('dword ptr [0x23000000]',0x24000000)
 vm.put('dword ptr [0x1006f0c8]',0x25000000);vm.put('dword ptr [0x25000000]',0)
 vm.put('dword ptr [0x100726c4]',pe_word(0x100726c4));vm.put(f'dword ptr [{D+0x4140}]',case['lod_bias'])
 for k,b in enumerate(case['pass']):vm.m[P+k]=b
 vm.put(f'dword ptr [{P+0x3a8}]',int(case['hardware']))
 for key,delta in [('desired',0),('applied',0xa34)]:
  v=case['cache'][key];write(vm,BASE+4+delta,v['render']);write(vm,BASE+0x6b8+delta,v['textures'])
  for s in range(8):write(vm,BASE+0x84+s*0x54+delta,v['stages'][s])
 vm.put(f'dword ptr [{BASE+0x146c}]',case['cache']['dirty'])
 return vm
def flush(vm,capacity,stencil_gate):
 vm.put(f'dword ptr [{D+0x41e8}]',capacity);vm.put(f'dword ptr [{D+0x54}]',int(stencil_gate))
 vm.r.update(ESI=BASE,EBP=0,ESP=0x70000000);start=len(vm.events)
 vm.run(0x10028f8a,0x10029808)
 if vm.get(f'dword ptr [{BASE+0x146c}]')&0x10:vm.run(0x10029b6f,0x10029bc3)
 vm.r.update(ESI=BASE,ECX=0);vm.run(0x10029f75,0x10029f7b)
 return vm.events[start:]
fixtures=json.loads((root/'analysis/reports/d3d-state.input.json').read_text());path=root/'analysis/reports/d3d-state.json';report=json.loads(path.read_text())
assert path.read_bytes()==(root/'analysis/reports/d3d-state-release.json').read_bytes()
source=root/'analysis/reports/material-state.json';previous=json.loads(source.read_text());previous_validation=json.loads((root/'analysis/reports/material-state-validation.json').read_text())
assert sha(source)==fixtures['source_material_report_sha256']==report['source_material_report_sha256']==previous_validation['report_sha256']
counts=collections.Counter();coverage=collections.defaultdict(set)
for c,p in zip(fixtures['cases'],report['probes'],strict=True):
 assert c['id']==p['id'] and c['material_id']==p['material_id']
 if c['material_id'] is not None:
  q=previous['materials'][c['material_id']];assert q['status'] and c['pass']==q['host']['pass'];counts['material_handoff_inputs']+=1
 vm=initialize(c);vm.r.update(EBX=R,ESI=P,EDI=1,EBP=FRAME);vm.put(f'dword ptr [{FRAME+0xc}]',c['cull_mode'])
 vm.run(0x1001efb7,0x1001f165);assert p['render']==state(vm),(c['id'],'render')
 stage_address=P+0x24+c['index']*0x70;write(vm,stage_address,c['stage'])
 if c['stage'][0]:write(vm,c['stage'][0]+0x38,c['resource'])
 vm.put(f'dword ptr [{FRAME+8}]',P);vm.put(f'dword ptr [{FRAME+0xc}]',c['index'])
 vm.r.update(EDX=c['index'],EBX=R,ESI=P,ECX=2,EBP=FRAME);vm.xmm['XMM0']=pe_word(0x100726c4)
 vm.run(0x1001f176,0x1001f58e)
 assert p['translated']==state(vm),(c['id'],'stage')
 assert p['stage']==[vm.get(f'dword ptr [{stage_address+4*k}]') for k in range(28)]
 counts['translation_instruction_steps']+=vm.steps;vm.steps=0
 calls=flush(vm,c['capacity'],c['stencil_gate']);assert p['calls']==calls and p['flushed']==state(vm),(c['id'],'flush')
 vm.put(f'dword ptr [{BASE+0x146c}]',0x13);repeated=flush(vm,c['capacity'],c['stencil_gate']);assert p['repeated']==repeated==[],(c['id'],'repeat')
 vm.put(f'dword ptr [{BASE+0x146c}]',0x13);expanded=flush(vm,8,True);assert p['expanded']==expanded and p['final']==state(vm),(c['id'],'expanded')
 counts['flush_instruction_steps']+=vm.steps;counts['calls']+=len(calls)+len(expanded);counts['cases']+=1;counts['verified_cache_words']+=(32+8*21+8)*2*4+4;counts['stage_words']+=28
 coverage['flags'].add(c['pass'][4]&63);coverage['fill_selector'].add(struct.unpack('<I',bytes(c['pass'][12:16]))[0]);coverage['capacity'].add(c['capacity']);coverage['hardware'].add(c['hardware']);coverage['lod'].add(c['stage'][1])
assert counts['cases']==352 and counts['material_handoff_inputs']==96 and len(coverage['flags'])==64 and set(range(3))<=coverage['fill_selector'] and coverage['capacity']==set(range(9)) and len(coverage['lod'])==7
log=root/'analysis/reports/d3d-state-tests.log';tests=sum(map(int,re.findall(r'test result: ok\. (\d+) passed',log.read_text())));assert tests==608,tests
sources=['crates/rc-package/src/d3d_state.rs','crates/rc-inspect/src/bin/rc-d3d-state-check.rs','scripts/Generate-D3DState.py','scripts/Record-D3DState.py','scripts/Record-MaterialState.py','scripts/ghidra/ExportIndirectCalls.java','analysis/decompiled/d3d-pass-translation.asm','analysis/decompiled/d3d-state-cache.asm','analysis/decompiled/d3d-pass-translation.c','analysis/decompiled/d3d-state-cache.c']
validation={'date':'2026-10-09','counts':dict(counts),'debug_release_identical':True,'rust_tests':tests,'original_d3ddrv_sha256':sha(dll),'ignored_native_hresult':0x80004005,'report_sha256':sha(path),'input_sha256':sha(root/'analysis/reports/d3d-state.input.json'),'material_state_report_sha256':sha(source),'source_sha256':{s:sha(root/s) for s in sources},'scope':report['scope'],'android':'deferred until end per user'}
(root/'analysis/reports/d3d-state-validation.json').write_text(json.dumps(validation,indent=2)+'\n')
ledger=root/'analysis/evidence.json';e=json.loads(ledger.read_text(encoding='utf-8'));e['d3d_state_validation']=validation;e['rust_tests']=tests;ledger.write_text(json.dumps(e,indent=2,ensure_ascii=False)+'\n',encoding='utf-8')
print(json.dumps({'counts':dict(counts),'rust_tests':tests}))
