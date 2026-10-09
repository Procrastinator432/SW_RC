"""Original x86 unused-stage/pass-completion and bounded hardware-pass replay."""
from pathlib import Path
import json,hashlib,collections,re,copy
root=Path(__file__).resolve().parents[1]
script=root/'scripts/Record-D3DState.py';g={'__file__':str(script)}
exec(compile(script.read_text().split('\nfixtures=json.loads')[0],str(script),'exec'),g)
sha=g['sha'];R=g['R'];D=g['D'];P=g['P'];FRAME=g['FRAME'];STATE=0x28000000
initialize=g['initialize'];state=g['state'];write=g['write'];flush=g['flush']
fixtures=json.loads((root/'analysis/reports/d3d-pass.input.json').read_text())
report_path=root/'analysis/reports/d3d-pass.json';report=json.loads(report_path.read_text())
assert report_path.read_bytes()==(root/'analysis/reports/d3d-pass-release.json').read_bytes()
source=root/'analysis/reports/d3d-state.input.json';previous=json.loads((root/'analysis/reports/d3d-state-validation.json').read_text())
assert sha(source)==fixtures['source_sha256']==report['source_sha256']==previous['input_sha256']

def setup(c,hardware):
 p=c['pass'];device=c['device']
 vm=initialize(dict(cache=c['cache'],**{'pass':p['header']},hardware=hardware,lod_bias=device['lod_bias']))
 vm.put(f'dword ptr [{R+0x9c0c}]',STATE)
 vm.put(f'dword ptr [{STATE+0x300}]',c['last_pass'])
 vm.put(f'dword ptr [{D+0x41e8}]',device['capacity'])
 vm.put(f'dword ptr [{P+0x20}]',p['color_write'])
 for s,stage in enumerate(p['stages']):
  write(vm,P+0x24+s*0x70,stage)
  if stage[0]:write(vm,stage[0]+0x38,c['resources'][s])
 return vm

def guard(vm):
 vm.r.update(EBX=R,EBP=FRAME)
 vm.put(f'dword ptr [{FRAME+8}]',P)
 vm.run(0x1001ed8e,0x1001edab)
 return vm.z # JZ at edab skips straight to epilogue, before any state writes

def tail(vm):
 vm.r.update(EBX=R,ESI=P,ECX=2,EDI=1)
 vm.run(0x1001f59f,0x1001f6ae)

def stages(vm):
 return [[vm.get(f'dword ptr [{P+0x24+s*0x70+k*4}]') for k in range(28)] for s in range(8)]

counts=collections.Counter();coverage=collections.defaultdict(set)
for c,q in zip(fixtures['cases'],report['probes'],strict=True):
 assert c['id']==q['id'] and c['material_id']==q['material_id'];active=c['pass']['header'][9];device=c['device'];capacity=device['capacity']
 if c['material_id'] is not None:
  prior=next(p for p in json.loads(source.read_text())['cases'] if p['material_id']==c['material_id'])
  assert c['pass']['header']==prior['pass'];counts['material_handoff_inputs']+=1
 vm=setup(c,c['hardware']);tail(vm)
 assert state(vm)==q['tail_state'],(c['id'],'tail cache')
 assert stages(vm)==q['tail_pass']['stages'],(c['id'],'tail packed stages')
 assert q['tail_last']==vm.get(f'dword ptr [{STATE+0x300}]')==P
 calls=flush(vm,capacity,c['stencil_gate'])
 assert calls==q['tail_calls'] and state(vm)==q['tail_flushed'],(c['id'],'tail calls')
 counts['tail_instructions']+=vm.steps;counts['tail_calls']+=len(calls)
 vm=setup(c,True);skipped=guard(vm)
 assert q['changed']==(not skipped)
 if not skipped:
  vm.r.update(EBX=R,ESI=P,EDI=1,EBP=FRAME)
  vm.put(f'dword ptr [{FRAME+0xc}]',device['cull_mode'])
  vm.run(0x1001efb7,0x1001f165)
  for s in range(active):
   vm.r.update(EDX=s,EBX=R,ESI=P,ECX=2,EBP=FRAME)
   vm.put(f'dword ptr [{FRAME+8}]',P);vm.put(f'dword ptr [{FRAME+0xc}]',s)
   vm.xmm['XMM0']=g['pe_word'](0x100726c4)
   vm.run(0x1001f176,0x1001f58e)
   counts['active_stages']+=1
  tail(vm)
 assert state(vm)==q['translated'],(c['id'],'whole pass')
 assert stages(vm)==q['pass']['stages'],(c['id'],'whole packed stages')
 assert q['last_pass']==vm.get(f'dword ptr [{STATE+0x300}]')==P
 calls=flush(vm,capacity,c['stencil_gate'])
 assert calls==q['calls'] and state(vm)==q['flushed'],(c['id'],'whole calls')
 # Original identity check must bypass an invalid changed active-stage count.
 vm.m[P+9]=255;before=state(vm);assert guard(vm)
 assert q['skipped'] and state(vm)==before==q['final']
 assert q['pass']['header'][9]==255
 counts['pipeline_instructions']+=vm.steps;counts['pipeline_calls']+=len(calls)
 counts['initial_identity_skips']+=int(skipped);counts['repeated_identity_skips']+=1
 counts['cases']+=1;counts['verified_packed_stage_words']+=8*28*2
 coverage['active'].add(active);coverage['capacity'].add(capacity);coverage['hardware_tail'].add(c['hardware'])
assert counts['cases']==226 and counts['material_handoff_inputs']==64 and coverage['active']==coverage['capacity']==set(range(9)) and len(coverage['hardware_tail'])==2
tests=sum(map(int,re.findall(r'test result: ok\. (\d+) passed',(root/'analysis/reports/d3d-pass-tests.log').read_text())))
assert tests==615,tests
sources=['crates/rc-package/src/d3d_pass.rs','crates/rc-package/src/d3d_state.rs','crates/rc-inspect/src/bin/rc-d3d-pass-check.rs','scripts/Generate-D3DPass.py','scripts/Record-D3DPass.py','scripts/Record-D3DState.py','scripts/Record-MaterialState.py','analysis/decompiled/d3d-pass-translation.asm','analysis/decompiled/d3d-state-cache.asm']
validation=dict(date='2026-10-09',counts=dict(counts),coverage={k:sorted(v) for k,v in coverage.items()},rust_tests=tests,debug_release_identical=True,original_d3ddrv_sha256=sha(g['dll']),report_sha256=sha(report_path),input_sha256=sha(root/'analysis/reports/d3d-pass.input.json'),source_sha256={s:sha(root/s) for s in sources},scope=report['scope'],android='deferred until end per user')
(root/'analysis/reports/d3d-pass-validation.json').write_text(json.dumps(validation,indent=2)+'\n')
ledger=root/'analysis/evidence.json';e=json.loads(ledger.read_text(encoding='utf-8'));e['d3d_pass_validation']=validation;e['rust_tests']=tests;ledger.write_text(json.dumps(e,indent=2,ensure_ascii=False)+'\n',encoding='utf-8')
print(json.dumps(dict(counts=dict(counts),rust_tests=tests)))
