"""Replay original D3DDrv cache/lifecycle instructions against complete Rust images.

External COM Release is recorded at its ABI boundary. Function returns and the
index destructor's SEH prologue/epilogue are excluded; three base destructor RETs
are mapped to a common stop marker. No allocator or GPU implementation is assumed.
"""
from pathlib import Path
import json,re,collections
root=Path(__file__).resolve().parents[1]
s=root/'scripts/Record-D3DState.py';g={'__file__':str(s)}
exec(compile(s.read_text().split('\nfixtures=json.loads')[0],str(s),'exec'),g)
sha=g['sha'];counts=collections.Counter()
assemblies=['d3d-resource-cache.asm','d3d-resource-lifecycle.asm','d3d-index-resource.asm']
for name in assemblies:
 asm=(root/'analysis/decompiled'/name).read_text()
 g['code'].update({int(a,16):(op,args.split(',') if args else []) for a,op,args in re.findall(r'^([0-9a-f]{8}) ([A-Z.]+)(?: (.*))?$',asm,re.M)})
addresses=sorted(g['code']);g['following']={a:addresses[i+1] for i,a in enumerate(addresses[:-1])}
for a in [0x1002a235,0x1002a243,0x1002a250]:
 assert g['code'][a]==('RET',[]);g['code'][a]=('JMP',['0x7ffffff0'])
fixtures=json.loads((root/'analysis/reports/d3d-resource.input.json').read_text())
path=root/'analysis/reports/d3d-resource.json';report=json.loads(path.read_text())
assert path.read_bytes()==(root/'analysis/reports/d3d-resource-release.json').read_bytes()
assert fixtures['source_sha256']==report['source_sha256']==sha(root/'analysis/reports/d3d-source.input.json')
assert fixtures['source_sha256']==json.loads((root/'analysis/reports/d3d-source-validation.json').read_text())['input_sha256']
def put(vm,a,v):
 for k,b in enumerate(v.to_bytes(4,'little')):vm.m[a+k]=b
def word(vm,a):return int.from_bytes(bytes(vm.m[a+k] for k in range(4)),'little')
def hash_native(low):
 vm=g['Native']();put(vm,vm.r['ESP']+4,low);put(vm,vm.r['ESP']+8,0xfedcba98)
 vm.run(0x1002a110,0x1002a130);counts['hash_instructions']+=vm.steps;return vm.r['EAX']
hashes=[hash_native(v) for v in fixtures['hashes']]
assert hashes==report['hashes'] and set(hashes)==set(range(4096));counts['hashes']=len(hashes)
def initialize(c):
 vm=g['Native']();d=c['device'];put(vm,d+0xb0,c['head'])
 for i,v in enumerate(c['buckets']):put(vm,d+0xb4+i*4,v)
 for n in c['nodes']:
  for k,b in enumerate(n['bytes']):vm.m[n['address']+k]=b
 for h in [0x33000000,0x33001000]:put(vm,h,0x24000000)
 return vm
def image(vm,c):
 d=c['device'];return dict(device=d,head=word(vm,d+0xb0),buckets=[word(vm,d+0xb4+i*4) for i in range(4096)],nodes=[dict(address=n['address'],bytes=[vm.m[n['address']+k] for k in range(64)]) for n in c['nodes']])
def release(vm):
 counts['releases']+=1;vm.steps+=1
 return dict(receiver=vm.pop(),vtable_offset=8,arguments=[])
def unlink(vm,a):
 vm.r['ECX']=a;vm.run(0x1002a1d0,0x7ffffff0);counts['base_unlinks']+=1
def action(vm,c,a):
 kind=a['kind'];vm.r['ESP']=0x70000000;addr=a.get('address');counts[kind]+=1
 if kind=='lookup':
  low,high=a['key'];vm.r.update(ESI=c['device'],EBX=low,EDI=high,EAX=hash_native(low))
  vm.run(0x10015077,0x10015099);result=vm.r['EAX'] or None
  counts['lookup_hits' if result else 'lookup_misses']+=1;return result
 if kind=='insert':
  vm.r['ECX']=addr;put(vm,vm.r['ESP']+4,c['device']);put(vm,vm.r['ESP']+8,0x35000000)
  put(vm,0x35000004,a['key'][0]);put(vm,0x35000008,a['key'][1]);vm.run(0x1002a140,0x1002a1c7)
  vm.r['ESI']=addr;k=a['resource_kind'];counts['insert_'+k]+=1
  if k=='Vertex':vm.run(0x1002a334,0x1002a34a)
  if k=='Index':vm.run(0x1002a7e4,0x1002a7f4)
  return None
 if kind=='unlink':unlink(vm,addr);return None
 commands=[]
 if kind=='reset_vertex':
  vm.r['ECX']=addr;vm.run(0x1002a351,0x1002a358)
  if not vm.z:
   vm.run(0x1002a35a,0x1002a35d);commands.append(release(vm));vm.run(0x1002a360,0x1002a367)
  vm.run(0x1002a367,0x1002a36c)
  if not vm.z:
   vm.run(0x1002a36e,0x1002a371);commands.append(release(vm));vm.run(0x1002a374,0x1002a37b)
 elif kind=='destroy_index':
  vm.r['ESI']=addr;vm.run(0x1002a81d,0x1002a830)
  if not vm.z:
   vm.run(0x1002a832,0x1002a835);commands.append(release(vm))
  vm.run(0x1002a838,0x1002a842);unlink(vm,addr)
 else:raise AssertionError(kind)
 return commands
for c,q in zip(fixtures['cases'],report['probes'],strict=True):
 assert c['id']==q['id'];vm=initialize(c['cache'])
 for i,(a,step) in enumerate(zip(c['actions'],q['steps'],strict=True)):
  expected=action(vm,c['cache'],a)
  assert step['result']==expected,('result',c['id'],i,a,step['result'],expected)
  assert step['cache']==image(vm,c['cache']),('image',c['id'],i,a)
  counts['steps']+=1
 assert q['cache']==image(vm,c['cache']);counts['cases']+=1;counts['lifecycle_instructions']+=vm.steps
for c,q in zip(fixtures['errors'],report['errors'],strict=True):
 assert c['id']==q['id'] and 'Err' in q['result'] and q['cache']==c['cache'];counts['safe_errors']+=1
assert counts['cases']==64 and counts['steps']==1408 and counts['safe_errors']==12
tests=sum(map(int,re.findall(r'test result: ok\. (\d+) passed',(root/'analysis/reports/d3d-resource-tests.log').read_text())));assert tests==701,tests
sources=['crates/rc-package/src/d3d_resource.rs','crates/rc-inspect/src/bin/rc-d3d-resource-check.rs','scripts/Generate-D3DResource.py','scripts/Record-D3DResource.py','scripts/Record-D3DState.py','scripts/Record-MaterialState.py','analysis/decompiled/d3d-resource-cache.c','analysis/decompiled/d3d-resource-lifecycle.c','analysis/decompiled/d3d-resource-unlink.c']+['analysis/decompiled/'+p for p in assemblies]
validation=dict(date='2026-10-09',counts=dict(counts),rust_tests=tests,debug_release_identical=True,original_d3ddrv_sha256=sha(g['dll']),input_sha256=sha(root/'analysis/reports/d3d-resource.input.json'),report_sha256=sha(path),source_sha256={p:sha(root/p) for p in sources},scope=report['scope'],replay_boundary=__doc__,android='deferred until end per user')
(root/'analysis/reports/d3d-resource-validation.json').write_text(json.dumps(validation,indent=2)+'\n')
ledger=root/'analysis/evidence.json';e=json.loads(ledger.read_text(encoding='utf-8'));e['d3d_resource_validation']=validation;e['rust_tests']=tests;ledger.write_text(json.dumps(e,indent=2,ensure_ascii=False)+'\n',encoding='utf-8')
print(json.dumps(dict(counts=dict(counts),rust_tests=tests)))
