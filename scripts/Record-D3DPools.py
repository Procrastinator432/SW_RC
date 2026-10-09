"""Original lazy pool, constructor, resource insertion and scratch upload replay.

Captured allocation bytes and nonnull allocator results are explicit answers.
The original base constructor, dynamic constructors, initial resize helpers,
device slot selection/storage, ring/scratch uploads and renderer bindings execute.
SEH, logging, timing, native error paths and returns remain excluded. No live GPU
or allocator is represented. Safe port errors are checked only for atomicity.
"""
from pathlib import Path
import json,re,copy,collections
root=Path(__file__).resolve().parents[1];p=root/'scripts/Record-D3DScratch.py';src=p.read_text();s={'__file__':str(p)}
exec(compile(src.split('\nfixtures=json.loads')[0],str(p),'exec'),s)
exec(compile(src[src.index('def array('):src.index('\nfor c,q in zip(fixtures[\'cases\']')],str(p),'exec'),s)
r=s['r'];n=s['n'];g=s['g'];put=s['put'];get=s['get'];sha=s['sha'];D=s['D'];R=n['R'];V=n['V'];FRAME=n['FRAME'];PF=0x27030000;CF=0x27040000;counts=collections.Counter()
assemblies=['d3d-pools.asm','d3d-resource-lifecycle.asm','d3d-dynamic-index.asm']
for name in assemblies:
 asm=(root/'analysis/decompiled'/name).read_text();g['code'].update({int(a,16):(op,args.split(',') if args else []) for a,op,args in re.findall(r'^([0-9a-f]{8}) ([A-Z.]+)(?: (.*))?$',asm,re.M)})
addresses=sorted(g['code']);g['following']={a:addresses[i+1] for i,a in enumerate(addresses[:-1])}
fixtures=json.loads((root/'analysis/reports/d3d-pools.input.json').read_text());path=root/'analysis/reports/d3d-pools.json';report=json.loads(path.read_text())
assert path.read_bytes()==(root/'analysis/reports/d3d-pools-release.json').read_bytes()
assert fixtures['source_sha256']==report['source_sha256']==sha(root/'analysis/reports/d3d-scratch.input.json')==json.loads((root/'analysis/reports/d3d-scratch-validation.json').read_text())['input_sha256']

def allocation(vm,pc,q,kind):
 calls=[];args=n['call'](vm,pc,4,1,calls,True);size=72 if kind=='vertex' else 64;assert args==[size]
 image=q['pool']['allocation'];assert len(image['bytes'])==size and image['address']!=0
 for j,v in enumerate(image['bytes']):vm.m[image['address']+j]=v
 nodes.append(dict(address=image['address'],size=size));vm.r['EAX']=image['address'];counts['allocations']+=1

def constructor(vm,q,kind,pc):
 start=vm.steps;address=vm.r['ECX'];owner=vm.pop();source=vm.pop();width=vm.pop() if kind=='index' else None;vm.steps+=1
 assert owner==D and source==q['source']['address'];assert g['code'][pc]==('CALL',['0x1002c120' if kind=='vertex' else '0x1002c460'])
 saved=vm.r.copy();put(vm,source+4,q['key'][0]);put(vm,source+8,q['key'][1])
 # Original base constructor initializes both lists and all base fields.
 vm.r.update(ECX=address,ESP=0x70040000);put(vm,vm.r['ESP']+4,D);put(vm,vm.r['ESP']+8,source);vm.run(0x1002a140,0x1002a1c7)
 vm.r.update(ESI=address,EBP=CF,ESP=0x70050000);put(vm,CF+0x10,width or 0)
 if kind=='vertex':
  vm.run(0x1002c155,0x1002c17a);size=vm.pop();assert size==0x20000;vm.steps+=1
  plan,_=r['vertex_resize'](vm,size,q['context'],q['pool']['creates']);counts['vertex_constructors']+=1
 else:
  vm.run(0x1002c495,0x1002c4b7);size=vm.pop();assert size==0x4000;vm.steps+=1
  plan,_=r['index_resize'](vm,size,q['context'],q['pool']['creates']);counts['index_constructors']+=1
 vm.r.update(saved);vm.r['EAX']=address;counts['constructor_instructions']+=vm.steps-start
 return dict(allocation_size=72 if kind=='vertex' else 64,address=address,key=q['key'],resize=plan)

def ensure(vm,q,kind):
 start=vm.steps;source=q['source']['address'];vm.r.update(ESI=R,EDI=source,EBX=0 if kind=='vertex' else 1,EBP=PF,ESP=0x70030000)
 put(vm,PF+12,source);put(vm,PF+8,source);initialized=None
 if kind=='vertex':
  vm.run(0x100205bf,0x100205cc)
  if vm.z:
   vm.run(0x100205ce,0x100205da);allocation(vm,0x100205da,q,kind)
   vm.run(0x100205dd,0x100205eb);assert not vm.z
   vm.run(0x100205ed,0x100205f2);initialized=constructor(vm,q,kind,0x100205f2)
   vm.run(0x100205f7,0x10020604)
  else:vm.run(0x10020617,0x1002061a)
  assert get(vm,D+0x40bc)==V
 else:
  vm.r['EAX']=q['source']['width'];vm.run(0x10020b20,0x10020b26);large=vm.z
  if large:
   vm.run(0x10020b28,0x10020b30)
   if vm.z:
    vm.run(0x10020b32,0x10020b3e);allocation(vm,0x10020b3e,q,kind)
    vm.run(0x10020b41,0x10020b4c);assert not vm.z
    vm.run(0x10020b4e,0x10020b55);initialized=constructor(vm,q,kind,0x10020b55)
    vm.run(0x10020b5e,0x10020b6a)
   vm.run(0x10020b6a,0x10020b73)
  else:
   vm.run(0x10020b75,0x10020b7d)
   if vm.z:
    vm.run(0x10020b7f,0x10020b8b);allocation(vm,0x10020b8b,q,kind)
    vm.run(0x10020b8e,0x10020b99);assert not vm.z
    vm.run(0x10020b9b,0x10020ba2);initialized=constructor(vm,q,kind,0x10020ba2)
    vm.run(0x10020bab,0x10020bb7)
   vm.run(0x10020bb7,0x10020bc0)
  assert vm.r['EBX']==n['I']
 counts['lazy_instructions']+=vm.steps-start;counts['pool_misses' if initialized else 'pool_hits']+=1
 return initialized

def initial(w):
 rt=w['runtime'];old=dict(vertex=dict(address=V,handles=[0,0],capacity=0,cursor=0,active=0,source=0),index=dict(address=0x34000100,handle=0,capacity=0,cursor=0,width=0),device=rt['device'],deferred=rt['deferred'],state=w['state'],target=w['target'])
 vm=n['initialize'](old);c=rt['cache'];assert c['device']==D;put(vm,D+0xb0,c['head'])
 for j,v in enumerate(c['buckets']):put(vm,D+0xb4+j*4,v)
 for image in c['nodes']:
  for j,v in enumerate(image['bytes']):vm.m[image['address']+j]=v
  for off in ([0x30,0x34] if len(image['bytes'])==72 else [0x30]):
   handle=get(vm,image['address']+off)
   if handle:put(vm,handle,0x24000000)
 for off,v in [(0x40bc,rt['pools']['vertex']),(0x40c0,rt['pools']['indices'][0]),(0x40c4,rt['pools']['indices'][1])]:put(vm,D+off,v)
 buf=rt['scratch'];put(vm,D+0xe7e8,buf['address']);put(vm,D+0xe7ec,buf['packed_count'])
 for j,v in enumerate(buf['bytes']):vm.m[buf['address']+j]=v
 put(vm,0x1006f014,0x26000000);put(vm,0x26000000,0x26000100);put(vm,0x26000100,0x26000200)
 return vm,old

def world(vm,w,old):
 value=n['world'](vm,old);pointer=get(vm,D+0xe7e8)
 cache=dict(device=D,head=get(vm,D+0xb0),buckets=[get(vm,D+0xb4+j*4) for j in range(4096)],nodes=[dict(address=image['address'],bytes=[vm.m[image['address']+j] for j in range(image['size'])]) for image in nodes])
 pools=dict(vertex=get(vm,D+0x40bc),indices=[get(vm,D+0x40c0),get(vm,D+0x40c4)])
 scratch=dict(address=pointer,packed_count=get(vm,D+0xe7ec),bytes=[vm.m.get(pointer+j,0) for j in range(s['extent'])])
 return dict(runtime=dict(cache=cache,pools=pools,device=value['device'],deferred=value['deferred'],scratch=scratch),state=value['state'],target=value['target'])

for c,q in zip(fixtures['cases'],report['probes'],strict=True):
 assert c['id']==q['id'];w=c['world'];vm,old=initial(w);nodes=[dict(address=image['address'],size=len(image['bytes'])) for image in w['runtime']['cache']['nodes']];s['extent']=len(w['runtime']['scratch']['bytes'])
 for j,(step,actual) in enumerate(zip(c['steps'],q['steps'],strict=True)):
  kind=step['kind'];request=copy.deepcopy(step['request']);request['answers']=request['upload'];request['responses']=request['upload']['lock']
  if kind=='index':n['I']=r['I']=0x34000200 if request['source']['width']==4 else 0x34000100
  r['setup'](vm,request['context']);initialized=ensure(vm,request,kind)
  r['current_plan']=None;s['current_scratch']=None;before=vm.steps
  upload=n[kind](vm,request)
  if kind=='vertex':
   # Preserve the original wrapper return slot before its renderer handoff.
   saved=vm.r.copy();vm.r.update(ESI=R,EDI=request['source']['address'],EBP=PF,EAX=upload['allocation']['element_offset']);vm.run(0x10020630,0x10020637);vm.r.update(saved)
   n['bind_vertex'](vm,request,upload);vm.r['EBP']=PF;vm.run(0x100207e7,0x100207ea)
  else:
   n['bind_index'](vm,request,upload);vm.r['EBP']=FRAME;vm.run(0x10020c63,0x10020c66)
  offset=vm.r['EAX'];assert offset==upload['allocation']['element_offset'];counts['upload_binding_instructions']+=vm.steps-before
  transfer=dict(initialization=initialized,transfer=dict(resize=r['current_plan'],scratch=s['current_scratch'],upload=upload),offset=offset)
  assert actual['result']==dict(Ok=transfer),('result',c['id'],j,actual['result'],transfer)
  assert actual['world']==world(vm,w,old),('world',c['id'],j)
  counts['steps']+=1
 assert q['world']==world(vm,w,old);counts['cases']+=1
for c,q in zip(fixtures['errors'],report['errors'],strict=True):
 assert c['id']==q['id'] and q['world']==c['world']
 for step in q['steps']:assert 'Err' in step['result'] and step['world']==c['world']
 counts['safe_errors']+=1
assert counts['cases']==48 and counts['steps']==384 and counts['safe_errors']==12
assert counts['pool_hits']>0 and counts['pool_misses']==counts['allocations']
counts['total_instructions']=counts['lazy_instructions']+counts['upload_binding_instructions']
counts['scratch_growths']=s['counts']['scratch_growths'];counts['scratch_reuses']=s['counts']['scratch_reuses'];counts['upload_resizes']=r['counts']['integrated_resizes'];counts['original_creates']=r['counts']['create_calls']
tests=sum(map(int,re.findall(r'test result: ok\. (\d+) passed',(root/'analysis/reports/d3d-pools-tests.log').read_text())));assert tests==780,tests
sources=['crates/rc-package/src/d3d_pools.rs','crates/rc-package/src/d3d_resource.rs','crates/rc-package/src/lib.rs','crates/rc-inspect/src/bin/rc-d3d-pools-check.rs','scripts/Generate-D3DPools.py','scripts/Record-D3DPools.py','analysis/decompiled/d3d-pools.c']+['analysis/decompiled/'+name for name in assemblies]
sources=sorted(set(sources)|set(json.loads((root/'analysis/reports/d3d-scratch-validation.json').read_text())['source_sha256']))
validation=dict(date='2026-10-09',counts=dict(counts),rust_tests=tests,debug_release_identical=True,original_d3ddrv_sha256=sha(g['dll']),input_sha256=sha(root/'analysis/reports/d3d-pools.input.json'),report_sha256=sha(path),source_sha256={p:sha(root/p) for p in sources},scope=report['scope'],replay_boundary=__doc__,android='deferred until end per user')
(root/'analysis/reports/d3d-pools-validation.json').write_text(json.dumps(validation,indent=2)+'\n');print(json.dumps(dict(counts=dict(counts),rust_tests=tests)))
