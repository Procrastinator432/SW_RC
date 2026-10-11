"""Original GetFrame cache gate/reset/zero/metadata and GetSequence body replay.
Logical 10332670 resizing with supplied stable storage is a boundary; original
descriptor flag/count commit executes. Mesh +ac uses supplied
binding answers. Original scheduler/decode/geometry/upload/draw instructions follow.
No native allocator implementation, asset sequence loading, GPU or Android execution.
"""
from pathlib import Path
import json,copy,re
instance_boundary=__doc__
instance_animation_path=Path(__file__).resolve().parent/'Record-VertMeshAnimation.py'
instance_animation_source=instance_animation_path.read_text()
exec(compile(instance_animation_source.split('\nfor i,(q,value)',1)[0],str(instance_animation_path),'exec'))
__doc__=instance_boundary
fixtures=json.loads((root/'analysis/reports/vertmesh-instance.input.json').read_text())
path=root/'analysis/reports/vertmesh-instance.json';report=json.loads(path.read_text())
assert path.read_bytes()==(root/'analysis/reports/vertmesh-instance-release.json').read_bytes()
assert fixtures['source_sha256']==report['source_sha256']==sha(root/'analysis/reports/vertmesh-animation.json')==json.loads((root/'analysis/reports/vertmesh-animation-validation.json').read_text())['report_sha256']
asm=(root/'analysis/decompiled/vertmesh-sequence.asm').read_text()+'\n'+(root/'analysis/decompiled/vertmesh-cache-array.asm').read_text()
g['code'].update({int(a,16):(op,args.split(',') if args else []) for a,op,args in re.findall(r'^([0-9a-f]{8}) ([A-Z0-9.]+)(?: (.*))?$',asm,re.M)})
addresses=sorted(g['code']);g['following']={a:addresses[i+1] for i,a in enumerate(addresses[:-1])}
assert engine_word(0x10653578)==0x3f800000
assert 'param_1[1] ^ (param_1[1] ^ param_2) & 0x1fffffff' in (root/'analysis/decompiled/vertmesh-cache-array.c').read_text()

def cache_prepare(vm,q):
 start=vm.steps;INSTANCE=0x76000000;MESH=0x77000000;CP=0x78400000;CN=0x78500000;FB=0x28010000
 state=q['cache'];vc=q['vertex_count'];mesh_id=q['mesh'];resizes=[]
 # Identity is an opaque raw pointer; current mesh is placed at that supplied address.
 MESH=mesh_id
 for off,key in [(0x98,'cached_mesh'),(0x9c,'word_9c'),(0x90,'word_90'),(0x8c,'word_8c'),(0x94,'last_count')]:put(vm,INSTANCE+off,state[key]&0xffffffff)
 for j,v in enumerate(state['flags']):vm.m[INSTANCE+0x60+j]=v
 for off,key,address in [(0x7c,'positions',CP),(0x84,'normals',CN)]:
  put(vm,INSTANCE+off,address);put(vm,INSTANCE+off+4,state[key]['packed_count'])
  for i,words in enumerate(state[key]['values']):
   for j,w in enumerate(words):put(vm,address+i*12+j*4,w)
 put(vm,INSTANCE+0x54,MESH);put(vm,MESH+0x5c,vc);put(vm,FB-0x18,MESH);put(vm,0x10653578,engine_word(0x10653578))
 vm.r.update(EBX=INSTANCE,ESI=MESH,EBP=FB,ESP=0x70080000)
 def resize():
  descriptor=vm.r['ECX'];assert descriptor in [INSTANCE+0x7c,INSTANCE+0x84]
  count=vm.pop();assert vm.pop()==0;old=get(vm,descriptor+4)&0x1fffffff;address=get(vm,descriptor)
  # Allocator contents are unspecified; poison growth to prove native zero stores.
  for i in range(old*12,count*12):vm.m[address+i]=0xa5
  saved=vm.r.copy();flags=(vm.z,vm.s,vm.o,vm.c)
  vm.r.update(EDI=descriptor,EBX=count)
  vm.run(0x103327ca,0x103327dc) # original packed-count/flag commit
  vm.r.update(saved);vm.z,vm.s,vm.o,vm.c=flags
  resizes.append([0 if descriptor==INSTANCE+0x7c else 1,count]);vm.steps+=1;counts['cache_array_boundaries']+=1
 vm.run(0x1055f0c5,0x1055f0cf);initialized=vm.z;vm.steps+=1
 if not initialized:
  vm.run(0x1055f0d1,0x1055f0d7);initialized=not vm.z;vm.steps+=1
 changed=state['cached_mesh']!=mesh_id
 if initialized:
  vm.run(0x1055f0dd,0x1055f0e3);vm.steps+=1
  if not vm.z:
   vm.run(0x1055f0e5,0x1055f104);resize()
   vm.run(0x1055f109,0x1055f125);resize()
  vm.run(0x1055f12a,0x1055f141);resize()
  vm.run(0x1055f146,0x1055f191);resize()
  vm.run(0x1055f196,0x1055f1f2)
  counts['cache_initializations']+=1
 else:counts['cache_reuses']+=1
 result=copy.deepcopy(state)
 for off,key in [(0x98,'cached_mesh'),(0x9c,'word_9c'),(0x90,'word_90'),(0x8c,'word_8c'),(0x94,'last_count')]:result[key]=get(vm,INSTANCE+off)
 if result['last_count']&0x80000000:result['last_count']-=1<<32
 for off,key in [(0x7c,'positions'),(0x84,'normals')]:
  packed=get(vm,INSTANCE+off+4);address=get(vm,INSTANCE+off)
  result[key]=dict(packed_count=packed,values=[[get(vm,address+i*12+j*4) for j in range(3)] for i in range(packed&0x1fffffff)])
 counts['cache_preparations']+=1;counts['cache_instructions']+=vm.steps-start
 return dict(initialized=initialized,mesh_changed=changed,resizes=resizes,cache=result)

def get_sequence(vm,q):
 start=vm.steps;INSTANCE=0x76000000;CHANNEL=0x7c000000;GLOBAL=0x7d000000
 channels=copy.deepcopy(q['channels']);put(vm,INSTANCE+0x64,CHANNEL);put(vm,INSTANCE+0x54,q['mesh']);put(vm,q['mesh'],0x7e000000)
 put(vm,0x106502f4,GLOBAL);put(vm,GLOBAL,int(q['editor']))
 for i,c in enumerate(channels):
  for j,w in enumerate(c['words']):put(vm,CHANNEL+i*0x48+j*4,w)
 vm.r.update(ECX=INSTANCE,ESP=0x70080000);vm.push(0);vm.push(0xdead0000)
 if q['editor']:
  vm.run(0x1044faf0,0x1044fb12)
  assert vm.r['ECX']==q['mesh'];name=vm.pop();assert vm.pop()==0
  vm.r['EAX']=next((identity for n,identity in q['bindings'] if n==name),0)
  vm.steps+=1;counts['sequence_lookup_boundaries']+=1
  vm.run(0x1044fb18,0x1044fb1b)
 else:vm.run(0x1044faf0,0x1044fb1b)
 vm.run(0x1044fb1b,0x1044fb1f);identity=vm.r['EAX'];assert vm.pop()==0xdead0000;assert vm.pop()==0;vm.steps+=1
 for i,c in enumerate(channels):c['words']=[get(vm,CHANNEL+i*0x48+j*4) for j in range(18)]
 counts['sequence_calls']+=1;counts['sequence_instructions']+=vm.steps-start
 return identity,channels

# Preserve selected opaque pointer identity through the existing scheduler memory setup.
instance_schedule_source=instance_animation_source.split('def schedule(vm,q):',1)[1].split('\n# Execute',1)[0]
instance_schedule_source='def schedule(vm,q):'+instance_schedule_source.replace('SEQ=0x7b000000','SEQ=active_sequence_identity')
exec(compile(instance_schedule_source,str(__file__)+'::selected-sequence-scheduler','exec'))
exec(compile(animation_decode_source.replace("vm.r['EAX']=0x7b000000 if animation_input['clock']['sequence'] is not None else 0", "vm.r['EAX']=active_sequence_identity"),str(__file__)+'::selected-sequence-decode','exec'))

def instance_frame(vm,q):
 global active_sequence_identity
 prepared=cache_prepare(vm,dict(cache=q['cache'],mesh=q['mesh'],vertex_count=q['animation']['clock']['vertex_count']))
 identity,channels=get_sequence(vm,q);active_sequence_identity=identity
 a=copy.deepcopy(q['animation']);vc=a['clock']['vertex_count'];state=prepared['cache']
 a['clock']['sequence']=None if identity==0 else next(s['sequence'] for s in q['sequences'] if s['identity']==identity)
 a['clock']['time']=channels[0]['words'][7]
 a['cache']=dict(positions=state['positions']['values'][:vc],normals=state['normals']['values'][:vc],last_count=state['last_count'],flags=state['flags'])
 animated=animated_frame(vm,a);cache=copy.deepcopy(state);frame_cache=animated['frame']['cache']
 for key in ['positions','normals']:cache[key]['values'][:vc]=copy.deepcopy(frame_cache[key])
 cache['last_count']=frame_cache['last_count'];cache['flags']=frame_cache['flags'].copy()
 counts['instance_frames']+=1
 return dict(preparation=prepared,cache=cache,channels=channels,sequence_identity=identity,animation=animated)

def commit_instance(q,value):
 q['cache']=copy.deepcopy(value['cache']);q['channels']=copy.deepcopy(value['channels'])
 a=q['animation'];a['bytes']=value['animation']['frame']['bytes'].copy();a['cache']=copy.deepcopy(value['animation']['frame']['cache'])
 a['clock']['time']=q['channels'][0]['words'][7]
 a['clock']['sequence']=None if value['sequence_identity']==0 else next(s['sequence'] for s in q['sequences'] if s['identity']==value['sequence_identity'])

for i,(q,value) in enumerate(zip(fixtures['caches'],report['caches'],strict=True)):
 observed=cache_prepare(g['Native'](),q);assert value==dict(Ok=observed),(i,difference(value,dict(Ok=observed)))
 counts['cache_probes']+=1
for i,(q,value) in enumerate(zip(fixtures['frames'],report['frames'],strict=True)):
 observed=instance_frame(g['Native'](),q);assert value==dict(Ok=observed),(i,difference(value,dict(Ok=observed)))
 counts['instance_probes']+=1
for key in ['cache_errors','frame_errors']:
 assert len(fixtures[key])==len(report[key]) and all('Err' in v for v in report[key]);counts[key]=len(report[key])

instance_loop='\nfor c,actual in zip'+frame_indices_source.split('\nfor c,actual in zip',1)[1].split('\ntests=sum(',1)[0]
old="q=copy.deepcopy(q);start=vm.steps;generated=geometry(vm,q['geometry']);qm=q['mesh'];qm['sections']=generated['sections']"
new="""q=copy.deepcopy(q);outer=q;start=vm.steps;instanced=instance_frame(vm,outer['instance']);decoded=instanced['animation']['frame']
  commit_instance(outer['instance'],instanced);q=outer['geometry']
  source=q['mesh']['vertex']['source'];source.update(stride=32,size_before=len(decoded['bytes']),size_again=len(decoded['bytes']),payload=decoded['bytes'].copy())
  generated=geometry(vm,q['geometry']);qm=q['mesh'];qm['sections']=generated['sections']"""
assert instance_loop.count(old)==1;instance_loop=instance_loop.replace(old,new)
old="  assert step['result']==dict(Ok=value)"
assert instance_loop.count(old)==1;instance_loop=instance_loop.replace(old,"  value=dict(instance=instanced,draw=value);outer['geometry']=q;q=outer\n"+old)
instance_loop=instance_loop.replace("counts['safe_errors']==12","counts['safe_errors']==8")
exec(compile(instance_loop,str(__file__)+'::composed-loop','exec'))
tests=sum(map(int,re.findall(r'test result: ok\. (\d+) passed',(root/'analysis/reports/vertmesh-instance-tests.log').read_text())));assert tests==920,tests
sources=set(json.loads((root/'analysis/reports/vertmesh-animation-validation.json').read_text())['source_sha256'])
sources.update(['crates/rc-package/src/vertmesh_instance.rs','crates/rc-package/src/skeletal_sequence.rs','crates/rc-package/src/mesh_animation.rs','crates/rc-inspect/src/bin/rc-vertmesh-instance-check.rs','scripts/Generate-VertMeshInstance.py','scripts/Record-VertMeshInstance.py','analysis/decompiled/vertmesh-sequence.asm','analysis/decompiled/vertmesh-sequence.c','analysis/decompiled/vertmesh-cache-array.c','analysis/decompiled/vertmesh-cache-array.asm','docs/VERTMESH_INSTANCE.md','analysis/finalize-vertmesh-instance.py'])
validation=dict(date='2026-10-10',counts=dict(counts),rust_tests=tests,debug_release_identical=True,original_engine_sha256=sha(draw['engine']),original_d3ddrv_sha256=sha(g['dll']),input_sha256=sha(root/'analysis/reports/vertmesh-instance.input.json'),report_sha256=sha(path),source_sha256={a:sha(root/a) for a in sorted(sources)},scope=report['scope'],replay_boundary=instance_boundary,android='deferred until end per user')
(root/'analysis/reports/vertmesh-instance-validation.json').write_text(json.dumps(validation,indent=2)+'\n');print(json.dumps(dict(counts=dict(counts),rust_tests=tests)))
