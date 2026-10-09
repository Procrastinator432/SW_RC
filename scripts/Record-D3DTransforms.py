"""Independent original x86 stage-matrix, transform flush and hardware-pass replay."""
from pathlib import Path
import json,re,collections
root=Path(__file__).resolve().parents[1]
script=root/'scripts/Record-D3DState.py';g={'__file__':str(script)}
exec(compile(script.read_text().split('\nfixtures=json.loads')[0],str(script),'exec'),g)
sha=g['sha'];R=g['R'];D=g['D'];P=g['P'];FRAME=g['FRAME'];BASE=g['BASE'];STATE=0x28000000
state=g['state'];write=g['write']
fixtures=json.loads((root/'analysis/reports/d3d-transforms.input.json').read_text())
path=root/'analysis/reports/d3d-transforms.json';report=json.loads(path.read_text())
assert path.read_bytes()==(root/'analysis/reports/d3d-transforms-release.json').read_bytes()
source=root/'analysis/reports/d3d-pass.input.json';previous=json.loads((root/'analysis/reports/d3d-pass-validation.json').read_text())
assert sha(source)==fixtures['source_sha256']==report['source_sha256']==previous['input_sha256']
prior=json.loads(source.read_text())['cases']

def setup(c):
 p=c.get('pass',dict(header=[0]*28));d=c.get('device',dict(lod_bias=0))
 vm=g['initialize'](dict(cache=c['cache'],**{'pass':p['header']},hardware=True,lod_bias=d['lod_bias']))
 for s,matrix in enumerate(c['transforms']['words']):write(vm,BASE+0x324+s*0x40,matrix)
 vm.put(f'dword ptr [{BASE+0x624}]',c['transforms']['mask'])
 if 'pass' in c:
  vm.put(f'dword ptr [{R+0x9c0c}]',STATE);vm.put(f'dword ptr [{STATE+0x300}]',c['last_pass'])
  vm.put(f'dword ptr [{D+0x41e8}]',d['capacity']);vm.put(f'dword ptr [{P+0x20}]',p['color_write'])
  for s,stage in enumerate(p['stages']):
   write(vm,P+0x24+s*0x70,stage)
   if stage[0]:write(vm,stage[0]+0x38,c['resources'][s])
 return vm

def matrices(vm):return dict(words=[[vm.get(f'dword ptr [{BASE+0x324+s*0x40+k*4}]') for k in range(16)] for s in range(11)],mask=vm.get(f'dword ptr [{BASE+0x624}]'))
def guard(vm):
 vm.r.update(EBX=R,EBP=FRAME);vm.put(f'dword ptr [{FRAME+8}]',P)
 vm.run(0x1001ed8e,0x1001edab);return vm.z
def flush(vm,capacity,gate):
 vm.put(f'dword ptr [{D+0x41e8}]',capacity);vm.put(f'dword ptr [{D+0x54}]',int(gate))
 vm.r.update(ESI=BASE,EBP=0,ESP=0x70000000);start=len(vm.events)
 vm.run(0x10028f8a,0x10029808);before=vm.events[start:];start=len(vm.events)
 vm.put('dword ptr [ESP + 0x10]',0)
 vm.run(0x100298a4,0x10029a7a);transforms=vm.events[start:];start=len(vm.events)
 assert vm.get('dword ptr [ESP + 0x10]')==len(transforms)
 if vm.get(f'dword ptr [{BASE+0x146c}]')&0x10:vm.run(0x10029b6f,0x10029bc3)
 after=vm.events[start:]
 vm.r.update(ESI=BASE,ECX=0)
 vm.run(0x10029efd,0x10029f03) # clear all matrix mask bits, even gated-out ones
 vm.run(0x10029f75,0x10029f7b)
 return dict(before=before,transforms=transforms,after=after)

counts=collections.Counter();coverage=collections.defaultdict(set)
for c,q in zip(fixtures['cases'],report['probes'],strict=True):
 assert c['id']==q['id'] and c['material_id']==q['material_id']
 if c['material_id'] is not None:
  old=next(p for p in prior if p['material_id']==c['material_id']);assert old['pass']['header']==c['pass']['header'];counts['material_handoff_inputs']+=1
 vm=setup(c);skipped=guard(vm);assert q['changed']==(not skipped)
 active=c['pass']['header'][9];device=c['device']
 if not skipped:
  vm.r.update(EBX=R,ESI=P,EDI=1,EBP=FRAME);vm.put(f'dword ptr [{FRAME+0xc}]',device['cull_mode'])
  vm.run(0x1001efb7,0x1001f165)
  for s in range(active):
   vm.r.update(EDX=s,EBX=R,ESI=P,ECX=2,EBP=FRAME)
   vm.put(f'dword ptr [{FRAME+8}]',P);vm.put(f'dword ptr [{FRAME+0xc}]',s);vm.xmm['XMM0']=g['pe_word'](0x100726c4)
   vm.run(0x1001f176,0x1001f58e);counts['active_stages']+=1
   counts['matrix_copies']+=int(bool(c['pass']['stages'][s][4]&0x40))
  vm.r.update(EBX=R,ESI=P,ECX=2,EDI=1);vm.run(0x1001f59f,0x1001f6ae)
 assert q['translated']==state(vm) and q['translated_matrices']==matrices(vm),(c['id'],'translation')
 assert q['pass']['stages']==[[vm.get(f'dword ptr [{P+0x24+s*0x70+k*4}]') for k in range(28)] for s in range(8)]
 assert q['last_pass']==vm.get(f'dword ptr [{STATE+0x300}]')==P
 plan=flush(vm,device['capacity'],c['stencil_gate'])
 assert plan==q['plan'] and state(vm)==q['flushed'] and matrices(vm)==q['flushed_matrices'],(c['id'],'pipeline flush')
 vm.m[P+9]=255;assert guard(vm) and q['skipped']
 assert state(vm)==q['final'] and matrices(vm)==q['final_matrices']
 counts['pipeline_instructions']+=vm.steps;counts['passes']+=1;counts['pipeline_transform_calls']+=len(plan['transforms'])
 counts['pipeline_state_calls']+=len(plan['before'])+len(plan['after']);counts['initial_identity_skips']+=int(skipped)
 coverage['stage_patterns'].add(sum((1<<s) for s in range(active) if c['pass']['stages'][s][4]&0x40))
 coverage['capacity'].add(device['capacity'])
for c,q in zip(fixtures['flush_cases'],report['flushes'],strict=True):
 assert c['id']==q['id'];vm=setup(c);plan=flush(vm,c['capacity'],c['stencil_gate'])
 assert plan==q['plan'] and state(vm)==q['flushed'] and matrices(vm)==q['flushed_matrices'],(c['id'],'mask flush')
 repeated=flush(vm,c['capacity'],c['stencil_gate']);assert repeated==q['repeated']==dict(before=[],transforms=[],after=[])
 assert state(vm)==q['final'] and matrices(vm)==q['final_matrices']
 counts['flush_instructions']+=vm.steps;counts['flush_cases']+=1;counts['flush_transform_calls']+=len(plan['transforms'])
 counts['flush_state_calls']+=len(plan['before'])+len(plan['after'])
 coverage['masks'].add(c['transforms']['mask']&0x7ff);coverage['gates'].add(c['cache']['dirty']&0x60)
assert counts['passes']==482 and counts['flush_cases']==2092 and counts['material_handoff_inputs']==64
assert coverage['stage_patterns']==set(range(256)) and coverage['masks']==set(range(2048)) and coverage['gates']=={0,0x20,0x40,0x60} and coverage['capacity']==set(range(9))
tests=sum(map(int,re.findall(r'test result: ok\. (\d+) passed',(root/'analysis/reports/d3d-transforms-tests.log').read_text())))
assert tests==622,tests
sources=['crates/rc-package/src/d3d_transforms.rs','crates/rc-package/src/d3d_state.rs','crates/rc-package/src/d3d_pass.rs','crates/rc-inspect/src/bin/rc-d3d-transforms-check.rs','scripts/Generate-D3DTransforms.py','scripts/Record-D3DTransforms.py','scripts/Record-D3DState.py','scripts/Record-MaterialState.py','analysis/decompiled/d3d-pass-translation.asm','analysis/decompiled/d3d-state-cache.asm']
validation=dict(date='2026-10-09',counts=dict(counts),coverage={k:len(v) for k,v in coverage.items()},rust_tests=tests,debug_release_identical=True,original_d3ddrv_sha256=sha(g['dll']),report_sha256=sha(path),input_sha256=sha(root/'analysis/reports/d3d-transforms.input.json'),source_sha256={s:sha(root/s) for s in sources},scope=report['scope'],android='deferred until end per user')
(root/'analysis/reports/d3d-transforms-validation.json').write_text(json.dumps(validation,indent=2)+'\n')
ledger=root/'analysis/evidence.json';e=json.loads(ledger.read_text(encoding='utf-8'));e['d3d_transforms_validation']=validation;e['rust_tests']=tests;ledger.write_text(json.dumps(e,indent=2,ensure_ascii=False)+'\n',encoding='utf-8')
print(json.dumps(dict(counts=dict(counts),rust_tests=tests)))
