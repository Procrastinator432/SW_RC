"""Original x86 shader/stream/index tail and combined deferred cache replay."""
from pathlib import Path
import json,re,collections
root=Path(__file__).resolve().parents[1]
script=root/'scripts/Record-D3DState.py';g={'__file__':str(script)}
exec(compile(script.read_text().split('\nfixtures=json.loads')[0],str(script),'exec'),g)
sha=g['sha'];BASE=g['BASE'];D=g['D'];write=g['write'];state=g['state']
fixtures=json.loads((root/'analysis/reports/d3d-bindings.input.json').read_text())
path=root/'analysis/reports/d3d-bindings.json';report=json.loads(path.read_text())
assert path.read_bytes()==(root/'analysis/reports/d3d-bindings-release.json').read_bytes()
source=root/'analysis/reports/d3d-transforms.json';old_validation=json.loads((root/'analysis/reports/d3d-transforms-validation.json').read_text())
assert sha(source)==fixtures['source_sha256']==report['source_sha256']==old_validation['report_sha256']
prior=json.loads(source.read_text())['probes']
empty=dict(render=[0]*32,stages=[[0]*21 for _ in range(8)],textures=[0]*8)

def initialize(cache,b,transforms=None):
 vm=g['initialize'](dict(cache=cache,**{'pass':[0]*28},hardware=True,lod_bias=0))
 for key,delta in [('desired',0),('applied',0xa34)]:
  v=b[key];write(vm,BASE+delta+0x628,[v['vertex_shader'],v['pixel_shader']])
  write(vm,BASE+delta+0x630,[x for p in v['streams'] for x in p]);write(vm,BASE+delta+0x6b0,v['indices'])
 if transforms is not None:
  for s,m in enumerate(transforms['words']):write(vm,BASE+0x324+s*0x40,m)
  vm.put(f'dword ptr [{BASE+0x624}]',transforms['mask'])
 return vm
def bindings(vm):
 result={}
 for key,delta in [('desired',0),('applied',0xa34)]:
  get=lambda a:vm.get(f'dword ptr [{BASE+delta+a}]')
  result[key]=dict(vertex_shader=get(0x628),pixel_shader=get(0x62c),streams=[[get(0x630+s*8+k*4) for k in range(2)] for s in range(16)],indices=[get(0x6b0),get(0x6b4)])
 return result
def matrices(vm):return dict(words=[[vm.get(f'dword ptr [{BASE+0x324+s*0x40+k*4}]') for k in range(16)] for s in range(11)],mask=vm.get(f'dword ptr [{BASE+0x624}]'))
def deferred(vm):return dict(states=state(vm),transforms=matrices(vm),bindings=bindings(vm))
def tail(vm,dirty,cap):
 vm.put(f'dword ptr [{BASE+0x146c}]',dirty);vm.put(f'dword ptr [{D+0x4210}]',cap)
 vm.r.update(ESI=BASE,ESP=0x70000000);start=len(vm.events)
 vm.run(0x10029dd9,0x10029ef0);return vm.events[start:]
def flush(vm,texture_cap,stream_cap,gate):
 dirty=vm.get(f'dword ptr [{BASE+0x146c}]')
 vm.put(f'dword ptr [{D+0x41e8}]',texture_cap);vm.put(f'dword ptr [{D+0x54}]',int(gate))
 vm.r.update(ESI=BASE,EBP=0,ESP=0x70000000);start=len(vm.events)
 vm.run(0x10028f8a,0x10029808);before=vm.events[start:];start=len(vm.events)
 vm.put('dword ptr [ESP + 0x10]',0);vm.run(0x100298a4,0x10029a7a);transforms=vm.events[start:];start=len(vm.events)
 if dirty&0x10:vm.run(0x10029b6f,0x10029bc3)
 after=vm.events[start:]
 calls=tail(vm,dirty,stream_cap)
 vm.r.update(ESI=BASE,ECX=0);vm.run(0x10029efd,0x10029f03);vm.run(0x10029f75,0x10029f7b)
 return dict(states=dict(before=before,transforms=transforms,after=after),bindings=calls)

counts=collections.Counter();coverage=collections.defaultdict(set)
for c,q in zip(fixtures['tails'],report['tails'],strict=True):
 assert c['id']==q['id'];vm=initialize(dict(desired=empty,applied=empty,dirty=c['dirty']),c['bindings'])
 calls=tail(vm,c['dirty'],c['stream_capacity']);assert calls==q['calls'] and bindings(vm)==q['applied'],(c['id'],'tail')
 repeated=tail(vm,c['dirty'],c['stream_capacity']);assert repeated==q['repeated']==[]
 expanded=tail(vm,12,16);assert expanded==q['expanded'] and bindings(vm)==q['final'],(c['id'],'expanded tail')
 counts['tail_cases']+=1;counts['tail_instructions']+=vm.steps;counts['tail_calls']+=len(calls)+len(expanded)
 coverage['stream_capacities'].add(c['stream_capacity']);coverage['binding_gates'].add(c['dirty'])
 for call in calls+expanded:
  coverage['binding_offsets'].add(call['vtable_offset'])
  if call['vtable_offset']==0x14c:coverage['stream_slots'].add(call['arguments'][0])
for c,q in zip(fixtures['cases'],report['probes'],strict=True):
 assert c['id']==q['id'] and c['material_id']==q['material_id'];d=c['deferred']
 if c['id']<len(prior):
  p=prior[c['id']];assert d['transforms']==p['translated_matrices']
  for k in ['desired','applied']:assert d['states'][k]==p['translated'][k]
 if c['material_id'] is not None:counts['material_handoff_inputs']+=1
 vm=initialize(d['states'],d['bindings'],d['transforms'])
 plan=flush(vm,c['texture_capacity'],c['stream_capacity'],c['stencil_gate'])
 assert plan==q['plan'] and deferred(vm)==q['flushed'],(c['id'],'combined flush')
 repeated=flush(vm,c['texture_capacity'],c['stream_capacity'],c['stencil_gate'])
 assert repeated==q['repeated']==dict(states=dict(before=[],transforms=[],after=[]),bindings=[])
 assert deferred(vm)==q['flushed']
 vm.put(f'dword ptr [{BASE+0x146c}]',12);expanded=flush(vm,8,16,True)
 assert expanded==q['expanded'] and deferred(vm)==q['final'],(c['id'],'combined expanded')
 counts['combined_cases']+=1;counts['combined_instructions']+=vm.steps
 for pl in [plan,expanded]:
  counts['combined_binding_calls']+=len(pl['bindings']);counts['combined_transform_calls']+=len(pl['states']['transforms'])
  counts['combined_state_calls']+=len(pl['states']['before'])+len(pl['states']['after'])
 coverage['combined_dirty_masks'].add(d['states']['dirty']);coverage['texture_capacities'].add(c['texture_capacity'])
assert counts['tail_cases']==704 and counts['combined_cases']==610 and counts['material_handoff_inputs']==64
assert coverage['binding_gates']=={0,4,8,12} and coverage['binding_offsets']=={0x130,0x14c,0x160,0x154}
assert coverage['stream_slots']==set(range(16)) and coverage['combined_dirty_masks']==set(range(128)) and coverage['texture_capacities']==set(range(9))
assert len(coverage['stream_capacities'])==22 and min(coverage['stream_capacities'])==-2147483648 and max(coverage['stream_capacities'])==2147483647
tests=sum(map(int,re.findall(r'test result: ok\. (\d+) passed',(root/'analysis/reports/d3d-bindings-tests.log').read_text())))
assert tests==629,tests
sources=['crates/rc-package/src/d3d_bindings.rs','crates/rc-package/src/d3d_state.rs','crates/rc-package/src/d3d_transforms.rs','crates/rc-inspect/src/bin/rc-d3d-bindings-check.rs','scripts/Generate-D3DBindings.py','scripts/Record-D3DBindings.py','scripts/Record-D3DState.py','scripts/Record-MaterialState.py','analysis/decompiled/d3d-state-cache.asm']
validation=dict(date='2026-10-09',counts=dict(counts),coverage={k:sorted(v) for k,v in coverage.items()},rust_tests=tests,debug_release_identical=True,original_d3ddrv_sha256=sha(g['dll']),report_sha256=sha(path),input_sha256=sha(root/'analysis/reports/d3d-bindings.input.json'),source_sha256={s:sha(root/s) for s in sources},scope=report['scope'],android='deferred until end per user')
(root/'analysis/reports/d3d-bindings-validation.json').write_text(json.dumps(validation,indent=2)+'\n')
ledger=root/'analysis/evidence.json';e=json.loads(ledger.read_text(encoding='utf-8'));e['d3d_bindings_validation']=validation;e['rust_tests']=tests;ledger.write_text(json.dumps(e,indent=2,ensure_ascii=False)+'\n',encoding='utf-8')
print(json.dumps(dict(counts=dict(counts),rust_tests=tests)))
