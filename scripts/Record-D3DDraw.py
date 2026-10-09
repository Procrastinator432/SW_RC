"""Replay original x86 draw, fog and bounded pass loop against Rust plans."""
from pathlib import Path
import json,re,collections,copy,struct
root=Path(__file__).resolve().parents[1]
s=root/'scripts/Record-D3DComplete.py';env={'__file__':str(s)};src=s.read_text()
exec(compile(src.split('\nfixtures=json.loads')[0],str(s),'exec'),env)
exec(compile(src[src.index('empty=dict'):src.index('\ncounts=')],str(s),'exec'),env)
g=env['g'];sha=env['sha'];BASE=env['BASE'];D=env['D'];R=env['R'];FRAME=env['FRAME'];STATE=env['STATE'];write=env['write'];state=env['state']
counts=collections.Counter();env['counts']=counts
assemblies=['d3d-pass-consumer.asm','d3d-draw-continuation.asm','d3d-draw-entry.asm','draw-color.asm']
for name in assemblies:
 asm=(root/'analysis/decompiled'/name).read_text()
 g['code'].update({int(a,16):(op,args.split(',') if args else []) for a,op,args in re.findall(r'^([0-9a-f]{8}) ([A-Z.]+)(?: (.*))?$',asm,re.M)})
addresses=sorted(g['code']);g['following']={a:addresses[i+1] for i,a in enumerate(addresses[:-1])}
engine=Path('D:/SteamLibrary/steamapps/common/Star Wars Republic Commando/GameData/System/Engine.dll')
def pe_reader(path):
 data=path.read_bytes();pe=struct.unpack_from('<I',data,60)[0];opt=pe+24;size=struct.unpack_from('<H',data,pe+20)[0]
 sections=[struct.unpack_from('<IIII',data,opt+size+40*i+8) for i in range(struct.unpack_from('<H',data,pe+6)[0])]
 def offset(rva):
  for virtual,va,raw_size,at in sections:
   if va<=rva<va+max(virtual,raw_size):return at+rva-va
  raise AssertionError(hex(rva))
 def word(rva):return struct.unpack_from('<I',data,offset(rva))[0]
 def string(rva):
  at=offset(rva);return data[at:data.index(b'\0',at)].decode('ascii')
 return data,opt,offset,word,string
data,opt,offset,word,string=pe_reader(g['dll']);descriptor=struct.unpack_from('<I',data,opt+104)[0];target=None
while word(descriptor+12):
 lookup=word(descriptor) or word(descriptor+16);iat=word(descriptor+16);i=0
 while word(lookup+i*4):
  thunk=word(lookup+i*4)
  if 0x10000000+iat+i*4==0x1006f3e8:
   assert not thunk&0x80000000;target=(string(word(descriptor+12)),string(thunk+2))
  i+=1
 descriptor+=20
assert target==('Engine.dll','??BFColor@@QBEKXZ'),target
data,opt,offset,word,string=pe_reader(engine);export=struct.unpack_from('<I',data,opt+96)[0];names=word(export+32);ordinals=word(export+36);functions=word(export+28)
matches=[i for i in range(word(export+24)) if string(word(names+i*4))==target[1]];assert len(matches)==1
ordinal=struct.unpack_from('<H',data,offset(ordinals+matches[0]*2))[0];getter=word(functions+ordinal*4)
image_base=struct.unpack_from('<I',data,opt+28)[0]
assert image_base+getter==0x10338670 and data[offset(getter):offset(getter)+3]==bytes.fromhex('8b01c3')
fixtures=json.loads((root/'analysis/reports/d3d-draw.input.json').read_text());path=root/'analysis/reports/d3d-draw.json';report=json.loads(path.read_text())
assert path.read_bytes()==(root/'analysis/reports/d3d-draw-release.json').read_bytes()
assert fixtures['source_sha256']==report['source_sha256']==sha(root/'analysis/reports/d3d-complete.input.json')
assert fixtures['mesh_sha256']==report['mesh_sha256']==sha(root/'analysis/reports/skeletal-draw.json')==json.loads((root/'analysis/reports/skeletal-draw-validation.json').read_text())['report_sha256']
sample=json.loads((root/'analysis/reports/d3d-complete.input.json').read_text())['cases'][0]['complete']
def put(vm,a,v):vm.put(f'dword ptr [{a}]',v)
def get(vm,a):return vm.get(f'dword ptr [{a}]')
def initialize(c,counters,draw):
 vm=env['initialize'](c);put(vm,R+0x9c0c,STATE);put(vm,STATE+0x484,int(draw['indexed']))
 for key,off in [('draw_passes',0xfec0),('submitted_primitives',0xfc90),('submitted_vertices',0xfcb8)]:put(vm,D+off,counters[key])
 return vm
def counters(vm):return {k:get(vm,D+o) for k,o in [('draw_passes',0xfec0),('submitted_primitives',0xfc90),('submitted_vertices',0xfcb8)]}
def frame(vm,draw):
 vm.r.update(ESI=R,EBP=FRAME,EDI=draw['primitive_count'],EBX=draw['min_vertex'],ESP=0x70000000)
 for off,key in [(8,'primitive'),(12,'start'),(16,'primitive_count'),(20,'min_vertex'),(24,'max_vertex')]:put(vm,FRAME+off,draw[key])
def dispatch(vm,draw):
 frame(vm,draw);vm.r['ECX']=D+0xfebc;vm.run(0x10020dbb,0x10020dbe);start=len(vm.events);vm.run(0x10020dbe,0x10020e10)
 calls=vm.events[start:];assert len(calls)<=1;return calls[0] if calls else None
def load_pass(vm,p):
 a=p['pass']['address'];raw=p['pass']
 for k,b in enumerate(raw['header']):vm.m[a+k]=b
 put(vm,a+0x1c,p['fog_color']);put(vm,a+0x20,raw['color_write'])
 for i,w in enumerate(raw['stages']):write(vm,a+0x24+i*0x70,w)
def raw_pass(vm,a):return dict(address=a,header=[vm.m[a+k] for k in range(28)],color_write=get(vm,a+0x20),stages=[[get(vm,a+0x24+i*0x70+k*4) for k in range(28)] for i in range(8)])
def fog_begin(vm):
 vm.r.update(ESI=R,EBP=FRAME);vm.run(0x10020d1b,0x10020d5e);return get(vm,FRAME-0x1c)!=0
def finish(vm,draw,index):
 frame(vm,draw);put(vm,FRAME-0x14,index);vm.run(0x10021063,0x100210b2)
for c,q in zip(fixtures['draws'],report['draws'],strict=True):
 vm=initialize(sample,c['counters'],c['draw']);put(vm,FRAME-0x1c,0);call=dispatch(vm,c['draw']);finish(vm,c['draw'],0)
 assert q==dict(id=c['id'],call=call,counters=counters(vm)),('draw',c['id'])
 counts['draw_cases']+=1;counts['draw_instructions']+=vm.steps;counts['draw_calls']+=int(call is not None)
for c,q in zip(fixtures['fog'],report['fog'],strict=True):
 initial=copy.deepcopy(sample);initial['deferred']['states']=c['cache'];vm=initialize(initial,fixtures['draws'][0]['counters'],fixtures['draws'][0]['draw'])
 load_pass(vm,c['pass']);put(vm,STATE+0x300,c['pass']['pass']['address']);vm.m[STATE+0x17]=c['enabled'];put(vm,STATE+0x2f8,c['restore'])
 overridden=fog_begin(vm);applied=state(vm);vm.r.update(ESI=R,EBP=FRAME);vm.run(0x10021063,0x10021096)
 assert q==dict(id=c['id'],overridden=overridden,applied=applied,final=state(vm)),('fog',c['id'])
 counts['fog_cases']+=1;counts['fog_instructions']+=vm.steps;counts['fog_overrides']+=int(overridden)
for c,q in zip(fixtures['cases'],report['probes'],strict=True):
 vm=initialize(c['complete'],c['counters'],c['draw']);context=c['context'];device=context['device'];put(vm,STATE+0x300,c['last_pass']);vm.m[STATE+0x324]=len(c['passes']);vm.m[STATE+0x17]=context['fog_enabled'];put(vm,STATE+0xf0,device['cull_mode']);put(vm,STATE+0x2f8,context['restore_fog']);put(vm,D+0x4140,device['lod_bias']);put(vm,D+0x41e8,device['capacity'])
 loaded=set()
 for i,p in enumerate(c['passes']):
  a=p['pass']['address'];put(vm,STATE+0x304+i*4,a)
  if a not in loaded:load_pass(vm,p);loaded.add(a)
  else:counts['aliased_slots']+=1
 results=[]
 for i,p in enumerate(c['passes']):
  a=p['pass']['address'];env['P']=a;vm.r.update(ESI=R,ECX=i,EBP=FRAME,ESP=0x70000000);vm.run(0x10020cf0,0x10020cff);assert not (vm.z or vm.s==vm.o)
  vm.run(0x10020d05,0x10020d16);assert get(vm,vm.r['ESP'])==a and get(vm,vm.r['ESP']+4)==device['cull_mode'];vm.r['ESP']+=8
  for j,w in enumerate(p['pass']['stages']):
   if w[0]:write(vm,w[0]+0x38,p['resources'][j])
  skipped=env['guard'](vm);immediate=None
  if not skipped:
   immediate=env['select'](vm,int.from_bytes(bytes(p['pass']['header'][:4]),'little'),p['shader'],True)
   vm.r.update(EBX=R,ESI=a,EDI=1,EBP=FRAME);put(vm,FRAME+12,device['cull_mode']);vm.run(0x1001efb7,0x1001f165)
   for j in range(vm.m[a+9]):
    vm.r.update(EDX=j,EBX=R,ESI=a,ECX=2,EBP=FRAME);put(vm,FRAME+8,a);put(vm,FRAME+12,j);vm.xmm['XMM0']=g['pe_word'](0x100726c4);vm.run(0x1001f176,0x1001f58e);counts['active_stages']+=1
   vm.r.update(EBX=R,ESI=a,ECX=2,EDI=1);vm.run(0x1001f59f,0x1001f6ae)
  overridden=fog_begin(vm);commands=env['flush'](vm,device['capacity'],context['stream_capacity'],context['light_capacity'],context['stencil_gate']);call=dispatch(vm,c['draw']);finish(vm,c['draw'],i)
  results.append(dict(address=a,selection=dict(changed=not skipped,immediate=immediate),commands=commands,draw=call,fog_override=overridden))
  assert results[-1]==q['result'][i],('sequence pass',c['id'],i)
  counts['processed_passes']+=1;counts['identity_skips']+=int(skipped);counts['sequence_draw_calls']+=int(call is not None);counts['sequence_fog_overrides']+=int(overridden)
 vm.r.update(ESI=R,ECX=len(c['passes']));vm.run(0x10020cf0,0x10020cff);assert vm.z or vm.s==vm.o
 mutated=copy.deepcopy(c['passes'])
 for p in mutated:p['pass']=raw_pass(vm,p['pass']['address'])
 assert q==dict(id=c['id'],section=c['section'],result=results,complete=env['complete'](vm),last_pass=get(vm,STATE+0x300),counters=counters(vm),passes=mutated),('sequence final',c['id'])
 counts['sequence_cases']+=1;counts['sequence_instructions']+=vm.steps;counts['empty_sequences']+=int(not c['passes'])
expected_sections=[dict(primitive=5,start=s['first_index'],primitive_count=s['triangle_count'],min_vertex=s['min_vertex'],max_vertex=s['max_vertex'],indexed=True) for s in fixtures['mesh_sections']]
assert report['section_draws']==expected_sections and sum(s['primitive_count'] for s in expected_sections)==3500
for c in fixtures['cases']:
 if c['section'] is not None:assert c['draw']==expected_sections[c['section']]
assert counts['draw_cases']==480 and counts['fog_cases']==524 and counts['sequence_cases']==195
tests=sum(map(int,re.findall(r'test result: ok\. (\d+) passed',(root/'analysis/reports/d3d-draw-tests.log').read_text())));assert tests==655,tests
sources=['crates/rc-package/src/d3d_draw.rs','crates/rc-package/src/d3d_complete.rs','crates/rc-package/src/skeletal_draw.rs','crates/rc-inspect/src/bin/rc-d3d-draw-check.rs','scripts/Generate-D3DDraw.py','scripts/Record-D3DDraw.py','scripts/Record-D3DComplete.py','scripts/Record-D3DBindings.py','scripts/Record-D3DState.py','scripts/Record-MaterialState.py','scripts/ghidra/ExportInstructionRange.java']+['analysis/decompiled/'+s for s in assemblies]
validation=dict(date='2026-10-09',counts=dict(counts),rust_tests=tests,debug_release_identical=True,original_d3ddrv_sha256=sha(g['dll']),original_engine_sha256=sha(engine),color_import=dict(library=target[0],symbol=target[1],iat=0x1006f3e8,address=image_base+getter,bytes='8b01c3'),input_sha256=sha(root/'analysis/reports/d3d-draw.input.json'),report_sha256=sha(path),source_sha256={s:sha(root/s) for s in sources},mesh_sections=3,mesh_triangles=3500,scope=report['scope'],android='deferred until end per user')
(root/'analysis/reports/d3d-draw-validation.json').write_text(json.dumps(validation,indent=2)+'\n')
ledger=root/'analysis/evidence.json';e=json.loads(ledger.read_text(encoding='utf-8'));e['d3d_draw_validation']=validation;e['rust_tests']=tests;ledger.write_text(json.dumps(e,indent=2,ensure_ascii=False)+'\n',encoding='utf-8')
print(json.dumps(dict(counts=dict(counts),rust_tests=tests)))
