"""Original x86 light, resolved pixel selection and complete bounded pass/cache replay."""
from pathlib import Path
import json,re,collections
root=Path(__file__).resolve().parents[1]
s=root/'scripts/Record-D3DBindings.py';ns={'__file__':str(s)};code=s.read_text()
exec(compile(code.split('\nfixtures=json.loads')[0],str(s),'exec'),ns)
exec(compile(code[code.index('\ndef initialize('):code.index('\ncounts=')],str(s),'exec'),ns)
g=ns['g'];sha=ns['sha'];BASE=ns['BASE'];D=ns['D'];P=g['P'];R=g['R'];FRAME=g['FRAME'];STATE=0x28000000
write=ns['write'];state=ns['state'];binding_tail=ns['tail']
fixtures=json.loads((root/'analysis/reports/d3d-complete.input.json').read_text())
path=root/'analysis/reports/d3d-complete.json';report=json.loads(path.read_text())
assert path.read_bytes()==(root/'analysis/reports/d3d-complete-release.json').read_bytes()
for kind,filename,validation in [('bindings','d3d-bindings.input.json','d3d-bindings-validation.json'),('passes','d3d-transforms.input.json','d3d-transforms-validation.json')]:
 v=json.loads((root/'analysis/reports'/validation).read_text());assert sha(root/'analysis/reports'/filename)==fixtures['source_'+kind+'_sha256']==report['source_'+kind+'_sha256']==v['input_sha256']
assert g['pe_word'](0x1006f9a0)==0x3f800000
empty=dict(render=[0]*32,stages=[[0]*21 for _ in range(8)],textures=[0]*8)
empty_b=dict(vertex_shader=0,pixel_shader=0,streams=[[0,0] for _ in range(16)],indices=[0,0])
identity=[0x3f800000,0,0,0,0,0x3f800000,0,0,0,0,0x3f800000,0]
def initialize(c):
 d=c['deferred'];vm=ns['initialize'](d['states'],d['bindings'],d['transforms']);l=c['lights']
 for i,w in enumerate(l['words']):write(vm,BASE+0x6d8+i*0x68,w)
 write(vm,BASE+0xa18,l['enabled']);write(vm,BASE+0x144c,l['applied_enabled']);return vm
def lights(vm):return dict(words=[[vm.get(f'dword ptr [{BASE+0x6d8+s*0x68+k*4}]') for k in range(26)] for s in range(8)],enabled=[vm.get(f'dword ptr [{BASE+0xa18+s*4}]') for s in range(8)],applied_enabled=[vm.get(f'dword ptr [{BASE+0x144c+s*4}]') for s in range(8)])
def complete(vm):return dict(deferred=ns['deferred'](vm),lights=lights(vm))
def light_tail(vm,dirty,cap):
 vm.put(f'dword ptr [{BASE+0x146c}]',dirty);vm.put(f'dword ptr [{D+0x41f4}]',cap)
 vm.r.update(ESI=BASE,ESP=0x70000000);start=len(vm.events)
 if dirty&0x80:vm.run(0x10029cb8,0x10029d3e)
 return vm.events[start:]
def flush(vm,texture_cap,stream_cap,light_cap,gate):
 vm.r.update(ESI=BASE);vm.run(0x10028f2c,0x10028f36)
 if vm.z:
  counts['zero_dirty_entry_skips']+=1
  return dict(states=dict(before=[],transforms=[],after=[]),lights=[],bindings=[])
 dirty=vm.get(f'dword ptr [{BASE+0x146c}]');vm.put(f'dword ptr [{D+0x41e8}]',texture_cap);vm.put(f'dword ptr [{D+0x54}]',int(gate))
 vm.r.update(ESI=BASE,EBP=0,ESP=0x70000000);start=len(vm.events)
 vm.run(0x10028f8a,0x10029808);before=vm.events[start:];start=len(vm.events)
 vm.put('dword ptr [ESP + 0x10]',0);vm.run(0x100298a4,0x10029a7a);transforms=vm.events[start:];start=len(vm.events)
 if dirty&0x10:vm.run(0x10029b6f,0x10029bc3)
 after=vm.events[start:];light_calls=light_tail(vm,dirty,light_cap);binding_calls=binding_tail(vm,dirty,stream_cap)
 vm.r.update(ESI=BASE,ECX=0);vm.run(0x10029efd,0x10029f03);vm.run(0x10029f75,0x10029f7b)
 return dict(states=dict(before=before,transforms=transforms,after=after),lights=light_calls,bindings=binding_calls)
def select(vm,kind,choice,initialized):
 vm.put(f'dword ptr [{P}]',kind);vm.put(f'dword ptr [{P+0x3a8}]',int(choice['hardware']))
 vm.put('dword ptr [0x10084734]',1) # static container already allocated; allocation external
 vm.put('dword ptr [0x10084700]',int(initialized));write(vm,0x10084704,identity if initialized else [0xcccccccc]*12)
 vm.put('dword ptr [0x1006f9a0]',g['pe_word'](0x1006f9a0))
 vm.r.update(EBX=R,ESI=P,EBP=FRAME,EDI=1);start=len(vm.events)
 if choice['hardware'] or kind==0:vm.run(0x1001ee08,0x1001efb7)
 else:
  assert choice['resolved'] is not None
  vm.put('dword ptr [0x29000004]',choice['resolved']);vm.r['EAX']=0x29000000
  vm.run(0x1001ee2d,0x1001efb7) # resume after external GetPixelShader
 calls=vm.events[start:];assert len(calls)<=1
 return calls[0] if calls else None
def guard(vm):
 vm.r.update(EBX=R,EBP=FRAME);vm.put(f'dword ptr [{FRAME+8}]',P);vm.run(0x1001ed8e,0x1001edab);return vm.z
zero_plan=dict(states=dict(before=[],transforms=[],after=[]),lights=[],bindings=[])
counts=collections.Counter();coverage=collections.defaultdict(set)
for c,q in zip(fixtures['lights'],report['lights'],strict=True):
 assert c['id']==q['id'];d=dict(states=dict(desired=empty,applied=empty,dirty=c['dirty']),bindings=dict(desired=empty_b,applied=empty_b),transforms=dict(words=[[0]*16 for _ in range(11)],mask=0))
 vm=initialize(dict(deferred=d,lights=c['lights']));calls=light_tail(vm,c['dirty'],c['capacity']);assert calls==q['calls'] and lights(vm)==q['applied']
 repeated=light_tail(vm,c['dirty'],c['capacity']);assert repeated==q['repeated']
 expanded=light_tail(vm,0x80,8);assert expanded==q['expanded'] and lights(vm)==q['final']
 counts['light_cases']+=1;counts['light_instructions']+=vm.steps;counts['light_calls']+=len(calls)+len(repeated)+len(expanded)
 coverage['light_masks'].add(sum((1<<s) for s in range(8) if c['lights']['enabled'][s]!=0));coverage['light_capacities'].add(c['capacity'])
for c,q in zip(fixtures['selectors'],report['selectors'],strict=True):
 assert c['id']==q['id'];invalid=not c['choice']['hardware'] and c['kind']!=0 and c['choice']['resolved'] is None
 if invalid:
  assert 'Err' in q['result'] and q['bindings']==c['bindings'] and q['dirty']==c['dirty'];counts['expected_missing_handle_errors']+=1
 else:
  vm=ns['initialize'](dict(desired=empty,applied=empty,dirty=c['dirty']),c['bindings']);immediate=select(vm,c['kind'],c['choice'],c['initialized'])
  assert q['result']==dict(Ok=immediate) and q['bindings']==ns['bindings'](vm) and q['dirty']==state(vm)['dirty'],(c['id'],'selection')
  counts['selection_instructions']+=vm.steps;counts['immediate_uploads']+=int(immediate is not None)
 counts['selection_cases']+=1;coverage['shader_kinds'].add(c['kind']);coverage['hardware'].add(c['choice']['hardware']);coverage['constant_initialized'].add(c['initialized'])
for c,q in zip(fixtures['cases'],report['probes'],strict=True):
 assert c['id']==q['id'] and c['material_id']==q['material_id'];vm=initialize(c['complete']);p=c['pass'];device=c['device']
 for k,b in enumerate(p['header']):vm.m[P+k]=b
 vm.put(f'dword ptr [{P+0x20}]',p['color_write']);vm.put(f'dword ptr [{R+0x9c0c}]',STATE);vm.put(f'dword ptr [{STATE+0x300}]',c['last_pass'])
 vm.put(f'dword ptr [{D+0x41e8}]',device['capacity']);vm.put(f'dword ptr [{D+0x4140}]',device['lod_bias'])
 for s,stage in enumerate(p['stages']):
  write(vm,P+0x24+s*0x70,stage)
  if stage[0]:write(vm,stage[0]+0x38,c['resources'][s])
 skipped=guard(vm);immediate=None
 if not skipped:
  immediate=select(vm,int.from_bytes(bytes(p['header'][:4]),'little'),c['choice'],c['initialized'])
  vm.r.update(EBX=R,ESI=P,EDI=1,EBP=FRAME);vm.put(f'dword ptr [{FRAME+0xc}]',device['cull_mode']);vm.run(0x1001efb7,0x1001f165)
  for s in range(p['header'][9]):
   vm.r.update(EDX=s,EBX=R,ESI=P,ECX=2,EBP=FRAME);vm.put(f'dword ptr [{FRAME+8}]',P);vm.put(f'dword ptr [{FRAME+0xc}]',s);vm.xmm['XMM0']=g['pe_word'](0x100726c4)
   vm.run(0x1001f176,0x1001f58e);counts['active_stages']+=1
  vm.r.update(EBX=R,ESI=P,ECX=2,EDI=1);vm.run(0x1001f59f,0x1001f6ae)
 assert q['pass_plan']==dict(changed=not skipped,immediate=immediate) and q['translated']==complete(vm),(c['id'],'pass')
 assert q['translated_pass']==dict(address=P,header=[vm.m[P+k] for k in range(28)],color_write=vm.get(f'dword ptr [{P+0x20}]'),stages=[[vm.get(f'dword ptr [{P+0x24+s*0x70+k*4}]') for k in range(28)] for s in range(8)])
 assert q['last_pass']==vm.get(f'dword ptr [{STATE+0x300}]')==P
 plan=flush(vm,device['capacity'],c['stream_capacity'],c['light_capacity'],c['stencil_gate']);assert plan==q['plan'] and complete(vm)==q['flushed'],(c['id'],'pass flush')
 repeated=flush(vm,device['capacity'],c['stream_capacity'],c['light_capacity'],c['stencil_gate']);assert repeated==q['repeated']==zero_plan and complete(vm)==q['flushed']
 vm.m[P+9]=255;assert guard(vm) and q['skipped']==dict(changed=False,immediate=None)
 vm.put(f'dword ptr [{BASE+0x146c}]',0xff);expanded=flush(vm,8,16,8,True);assert expanded==q['expanded'] and complete(vm)==q['final'],(c['id'],'expanded pass')
 counts['pass_cases']+=1;counts['pass_instructions']+=vm.steps;counts['pass_immediate_uploads']+=int(immediate is not None);counts['initial_identity_skips']+=int(skipped)
 if c['material_id'] is not None:counts['material_handoff_inputs']+=1
 for pl in [plan,expanded]:counts['pass_deferred_calls']+=len(pl['states']['before'])+len(pl['states']['transforms'])+len(pl['states']['after'])+len(pl['lights'])+len(pl['bindings'])
for c,q in zip(fixtures['flushes'],report['flushes'],strict=True):
 assert c['id']==q['id'];vm=initialize(c['complete']);plan=flush(vm,c['texture_capacity'],c['stream_capacity'],c['light_capacity'],c['stencil_gate'])
 assert plan==q['plan'] and complete(vm)==q['flushed'],(c['id'],'all-group flush')
 repeated=flush(vm,c['texture_capacity'],c['stream_capacity'],c['light_capacity'],c['stencil_gate']);assert repeated==q['repeated']==zero_plan and complete(vm)==q['final']
 counts['flush_cases']+=1;counts['flush_instructions']+=vm.steps;counts['flush_calls']+=len(plan['states']['before'])+len(plan['states']['transforms'])+len(plan['states']['after'])+len(plan['lights'])+len(plan['bindings'])
 coverage['dirty_masks'].add(c['complete']['deferred']['states']['dirty'])
assert counts['light_cases']==308 and counts['selection_cases']==64 and counts['expected_missing_handle_errors']==6 and counts['pass_cases']==482 and counts['flush_cases']==256 and counts['material_handoff_inputs']==64
assert coverage['light_masks']==set(range(256)) and coverage['dirty_masks']==set(range(256)) and len(coverage['shader_kinds'])==4 and len(coverage['hardware'])==len(coverage['constant_initialized'])==2
assert counts['zero_dirty_entry_skips']>738
tests=sum(map(int,re.findall(r'test result: ok\. (\d+) passed',(root/'analysis/reports/d3d-complete-tests.log').read_text())));assert tests==642,tests
sources=['crates/rc-package/src/d3d_complete.rs','crates/rc-package/src/d3d_bindings.rs','crates/rc-package/src/d3d_transforms.rs','crates/rc-package/src/d3d_state.rs','crates/rc-inspect/src/bin/rc-d3d-complete-check.rs','scripts/Generate-D3DComplete.py','scripts/Record-D3DComplete.py','scripts/Record-D3DBindings.py','scripts/Record-D3DState.py','scripts/Record-MaterialState.py','analysis/decompiled/d3d-pass-translation.asm','analysis/decompiled/d3d-state-cache.asm']
validation=dict(date='2026-10-09',counts=dict(counts),coverage={k:sorted(v) for k,v in coverage.items()},rust_tests=tests,debug_release_identical=True,original_d3ddrv_sha256=sha(g['dll']),report_sha256=sha(path),input_sha256=sha(root/'analysis/reports/d3d-complete.input.json'),source_sha256={s:sha(root/s) for s in sources},scope=report['scope'],android='deferred until end per user')
(root/'analysis/reports/d3d-complete-validation.json').write_text(json.dumps(validation,indent=2)+'\n')
ledger=root/'analysis/evidence.json';e=json.loads(ledger.read_text(encoding='utf-8'));e['d3d_complete_validation']=validation;e['rust_tests']=tests;ledger.write_text(json.dumps(e,indent=2,ensure_ascii=False)+'\n',encoding='utf-8')
print(json.dumps(dict(counts=dict(counts),rust_tests=tests)))
