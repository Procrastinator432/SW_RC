"""Original x86 instruction replay for HardwareShader material and sampler handoff."""
from pathlib import Path
import json,re,struct,hashlib,collections
root=Path(__file__).resolve().parents[1]
sha=lambda p:hashlib.sha256(p.read_bytes()).hexdigest()
dll=Path(r'D:\SteamLibrary\steamapps\common\Star Wars Republic Commando\GameData\System\d3ddrv.dll')
raw=dll.read_bytes();pe=struct.unpack_from('<I',raw,60)[0];optional=struct.unpack_from('<H',raw,pe+20)[0]
sections=[struct.unpack_from('<IIII',raw,pe+24+optional+40*i+8) for i in range(struct.unpack_from('<H',raw,pe+6)[0])]
def pe_string(address):
 rva=address-0x10000000
 for size,va,rawsize,off in sections:
  if va<=rva<va+max(size,rawsize):
   at=off+rva-va;return raw[at:raw.index(b'\0',at)].decode('ascii')
 raise AssertionError(hex(address))
assert b'd3d8.dll' in raw.lower()
asm=(root/'analysis/decompiled/hardware-material-state.asm').read_text()
code={int(a,16):(op,args.split(',') if args else []) for a,op,args in re.findall(r'^([0-9a-f]{8}) ([A-Z]+)(?: (.*))?$',asm,re.M)}
addresses=sorted(code);following={a:addresses[i+1] for i,a in enumerate(addresses[:-1])}
def memory(regions):
 m={}
 for region in regions:
  for k,b in enumerate(region['bytes']):
   a=region['address']+k;assert a not in m;m[a]=b
 return m
class Machine:
 def __init__(self,m,case):
  self.m=m.copy();self.case=case;self.r={k:0 for k in ['EAX','EBX','ECX','EDX','ESI','EDI','EBP','ESP']};self.r['ESP']=0x70000000
  self.z=False;self.c=False;self.diagnostic='previous';self.calls=0;self.steps=0
  for a in range(self.r['ESP']-128,self.r['ESP']+128):self.m[a]=0
 def address(self,arg):
  expr=arg[arg.index('[')+1:arg.index(']')]
  for reg,value in self.r.items():expr=expr.replace(reg,str(value))
  assert re.fullmatch(r'[0-9a-fx+*\- ]+',expr),expr
  return eval(expr,{'__builtins__':{}},{})&0xffffffff
 def bits(self,arg):return 8 if arg.startswith('byte ptr') or arg in ['AL','CL','DL','BL'] else 32
 def get(self,arg):
  if '[' in arg:
   a=self.address(arg);n=self.bits(arg)//8;return int.from_bytes(bytes(self.m[a+k] for k in range(n)),'little')
  if arg in self.r:return self.r[arg]
  if arg in ['AL','CL','DL','BL']:return self.r['E'+arg[0]+'X']&255
  return int(arg,0)&0xffffffff
 def put(self,arg,v):
  width=self.bits(arg);v&=(1<<width)-1
  if '[' in arg:
   a=self.address(arg)
   for k,b in enumerate(v.to_bytes(width//8,'little')):self.m[a+k]=b
  elif arg in self.r:self.r[arg]=v
  elif arg in ['AL','CL','DL','BL']:
   reg='E'+arg[0]+'X';self.r[reg]=(self.r[reg]&~255)|v
  else:raise AssertionError(arg)
 def push(self,v):self.r['ESP']-=4;self.put('dword ptr [ESP]',v)
 def pop(self):v=self.get('dword ptr [ESP]');self.r['ESP']+=4;return v
 def run(self,start):
  pc=start
  while True:
   self.steps+=1;assert self.steps<300,(hex(pc),self.case['id'])
   op,a=code[pc];nxt=following.get(pc)
   if op in ['MOV','MOVZX']:self.put(a[0],self.get(a[1]))
   elif op=='PUSH':self.push(self.get(a[0]))
   elif op=='POP':self.put(a[0],self.pop())
   elif op in ['SHL','SHR']:
    v=self.get(a[0]);shift=self.get(a[1]);self.put(a[0],v<<shift if op=='SHL' else v>>shift)
   elif op in ['XOR','AND','OR','SUB','CMP','TEST']:
    x=self.get(a[0]);y=self.get(a[1]);width=self.bits(a[0]);mask=(1<<width)-1
    v={'XOR':lambda:x^y,'AND':lambda:x&y,'OR':lambda:x|y,'SUB':lambda:x-y,'CMP':lambda:x-y,'TEST':lambda:x&y}[op]()&mask
    self.z=v==0;self.c=x<y if op in ['CMP','SUB'] else False
    if op not in ['CMP','TEST']:self.put(a[0],v)
   elif op=='DEC':v=(self.get(a[0])-1)&0xffffffff;self.put(a[0],v);self.z=v==0
   elif op in ['JZ','JNZ','JNC','JMP']:
    branch={'JZ':self.z,'JNZ':not self.z,'JNC':not self.c,'JMP':True}[op]
    if branch:nxt=int(a[0],0)
   elif op=='CALL':
    if a[0]=='0x1000dbe0':
     self.calls+=1;assert self.get('dword ptr [ESP]')==self.case['shader'] and self.get('dword ptr [ESP + 0x4]')==0
     if self.case['mutate']:
      r=self.case['renderer'];s=self.get(f'dword ptr [{r+0x9c0c}]');p=self.get(f'dword ptr [{s+0x304}]')
      self.put(f'dword ptr [{r+0x9fd0}]',self.get(f'dword ptr [{r+0x9fd0}]')^0x80000000)
      self.put(f'byte ptr [{p}]',self.get(f'byte ptr [{p}]')^0x5a);self.put(f'byte ptr [{s+0x324}]',self.get(f'byte ptr [{s+0x324}]')^0x80)
     self.r['EAX']=self.case['setup_return']&0xffffffff;self.r['ESP']+=8
    elif a[0] in ['dword ptr [0x1006f0c0]','dword ptr [0x1006f0d4]']:
     text=pe_string(self.pop());assert self.r['ECX']==0x71000000
     self.diagnostic=self.diagnostic+text if 'f0d4' in a[0] else text
    else:raise AssertionError((hex(pc),a))
   elif op=='RET':return
   else:raise AssertionError((hex(pc),op,a))
   pc=nxt
fixtures=json.loads((root/'analysis/reports/material-state.input.json').read_text())
path=root/'analysis/reports/material-state.json';report=json.loads(path.read_text())
assert path.read_bytes()==(root/'analysis/reports/material-state-release.json').read_bytes()
counts=collections.Counter();caps_paths=set();return_paths=set();masks=set();modes=set()
for case,p in zip(fixtures['materials'],report['materials'],strict=True):
 i=case['id'];assert p['id']==i
 if i>=512:assert 'capture_error' in p;counts['material_capture_errors']+=1;continue
 vm=Machine(memory(case['regions']),case);r=case['renderer'];h=case['shader']
 s=vm.get(f'dword ptr [{r+0x9c0c}]');device=vm.get(f'dword ptr [{r+4}]');address=vm.get(f'dword ptr [{s+0x304}]')
 def host():return {'renderer_flags':vm.get(f'dword ptr [{r+0x9fd0}]'),'pass':[vm.m[address+k] for k in range(28)],'active_passes':vm.get(f'byte ptr [{s+0x324}]')}
 assert p['before']==host()
 assert p['shader']=={'flags':vm.get(f'dword ptr [{h+0x92c}]'),'alpha_reference':vm.m[h+0x930],'source_blend':vm.m[h+0x931],'destination_blend':vm.m[h+0x932],'fallback':vm.get(f'dword ptr [{h+0x28}]')}
 caps=[vm.get(f'dword ptr [{device+o}]') for o in [0x4218,0x4220]];assert p['capabilities']==caps
 vm.r['ECX']=r;vm.put('dword ptr [ESP + 0x4]',h);vm.put('dword ptr [ESP + 0x8]',0x71000000 if case['diagnostic_present'] else 0);vm.put('dword ptr [ESP + 0xc]',0x72000000 if case['fallback_present'] else 0);vm.put('dword ptr [0x72000000]',0xdeadbeef)
 vm.run(0x1000fff0)
 assert p['host']==host() and p['status']==bool(vm.r['EAX']) and p['setup_calls']==vm.calls,(i,'native material')
 assert p['diagnostic']==vm.diagnostic and p['fallback']==vm.get('dword ptr [0x72000000]'),i
 counts['material_instruction_steps']+=vm.steps;counts['materials']+=1;counts['material_output_bytes']+=33
 caps_paths.add(tuple(bool(x) for x in caps));masks.add(p['shader']['flags']&31)
 if vm.calls:return_paths.add(case['setup_return'])
for case,p in zip(fixtures['samplers'],report['samplers'],strict=True):
 i=case['id'];assert p['id']==i
 if i>=3088:assert 'capture_error' in p;counts['sampler_capture_errors']+=1;continue
 vm=Machine(memory(case['regions']),case);stage=0x73000000
 for k,v in enumerate(case['stage']):vm.put(f'dword ptr [{stage+k*4}]',v)
 r=case['renderer'];t=case['texture'];res=case['resource']
 if t:
  d=vm.get(f'dword ptr [{r+4}]');filter=vm.get(f'dword ptr [{d+0x466c}]');word3c=vm.get(f'dword ptr [{res+0x3c}]') if res else 0
  assert p['texture']=={'address_u':vm.m[t+0x65],'address_v':vm.m[t+0x66],'flags_7c':vm.m[t+0x7c]}
  assert p['resource']=={'address':res,'word_3c':word3c} and p['filter']==filter
  modes.add((bool(res),bool(word3c)));vm.put(f'dword ptr [{r+0x9fd0}]',case['renderer_flags'])
  vm.r.update(ESI=t,EDI=r,EAX=res);vm.put('dword ptr [ESP + 0xc]',stage)
  vm.run(0x100044ec);expected_flags=vm.get(f'dword ptr [{r+0x9fd0}]')
 else:
  assert p['texture'] is None and p['resource']=={'address':0,'word_3c':0} and p['filter']==0
  vm.r['ECX']=r;vm.put('dword ptr [ESP + 0x4]',stage);vm.put('dword ptr [ESP + 0x8]',0)
  vm.run(0x100044b0);expected_flags=case['renderer_flags'];counts['null_textures']+=1
 assert p['stage']==[vm.get(f'dword ptr [{stage+k*4}]') for k in range(28)] and p['renderer_flags']==expected_flags,(i,'native sampler')
 counts['samplers']+=1;counts['sampler_words']+=28;counts['sampler_instruction_steps']+=vm.steps
assert caps_paths=={(False,False),(False,True),(True,False),(True,True)} and return_paths=={-1,0,3,-2} and masks==set(range(32))
assert modes=={(False,False),(True,False),(True,True)}
assert counts['material_capture_errors']==6 and counts['sampler_capture_errors']==2 and counts['null_textures']==16
# Every U byte and every selected V byte is exercised in all three resource paths.
coverage={(p['texture']['address_u'],p['texture']['address_v'],bool(p['resource']['address']),bool(p['resource']['word_3c'])) for p in report['samplers'] if p.get('texture')}
assert len(coverage)==256*4*3
log=root/'analysis/reports/material-state-tests.log';tests=sum(map(int,re.findall(r'test result: ok\. (\d+) passed',log.read_text())));assert tests==602,tests
sources=['crates/rc-package/src/hardware_material.rs','crates/rc-package/src/hardware_sampler.rs','crates/rc-package/src/hardware_state_snapshot.rs','crates/rc-package/src/shader_snapshot.rs','crates/rc-inspect/src/bin/rc-material-state-check.rs','scripts/Generate-MaterialState.py','scripts/Record-MaterialState.py','analysis/decompiled/hardware-material-state.c','analysis/decompiled/hardware-material-state.asm','analysis/decompiled/hardware-state-constructors.c','analysis/decompiled/hardware-state-constructors.asm']
validation={'date':'2026-10-09','counts':dict(counts),'address_u_bytes':256,'address_v_bytes':[0,1,2,255],'all_material_flag_patterns':32,'capability_patterns':4,'native_setup_return_values':sorted(return_paths),'debug_release_identical':True,'rust_tests':tests,'report_sha256':sha(path),'input_sha256':sha(root/'analysis/reports/material-state.input.json'),'original_d3ddrv_sha256':sha(dll),'source_sha256':{s:sha(root/s) for s in sources},'scope':report['scope'],'android':'deferred until end per user'}
(root/'analysis/reports/material-state-validation.json').write_text(json.dumps(validation,indent=2)+'\n')
ledger=root/'analysis/evidence.json';e=json.loads(ledger.read_text(encoding='utf-8'));e['material_state_validation']=validation;e['rust_tests']=tests;ledger.write_text(json.dumps(e,indent=2,ensure_ascii=False)+'\n',encoding='utf-8')
print(json.dumps({'counts':dict(counts),'rust_tests':tests}))
