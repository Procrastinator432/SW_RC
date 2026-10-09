"""Original resize helpers composed with the dynamic ring and binding instruction replay.

COM Create/Release/Evict answers are supplied at verified call boundaries. Stream
invalidation executes its original body and immediate SetStreamSource calls.
Nested helper frames preserve caller registers explicitly; SEH, logging, timing
and returns are omitted. Safe rollback errors do not execute native error paths.
"""
from pathlib import Path
import json,re,collections,copy
root=Path(__file__).resolve().parents[1];p=root/'scripts/Record-D3DDynamic.py';n={'__file__':str(p)};src=p.read_text()
exec(compile(src.split('\nfixtures=json.loads')[0],str(p),'exec'),n)
exec(compile(src[src.index('V=0x34000000'):src.index('\nfor c,q in zip(fixtures[\'cases\']')],str(p),'exec'),n)
g=n['g'];sha=n['sha'];put=n['put'];get=n['get'];D=n['D'];BASE=n['b']['BASE'];V=n['V'];I=n['I'];counts=collections.Counter();RF=0x27010000
assemblies=['d3d-resize-cache.asm','d3d-resize-index.asm','d3d-dynamic-vertex.asm']
for name in assemblies:
 asm=(root/'analysis/decompiled'/name).read_text();g['code'].update({int(a,16):(op,args.split(',') if args else []) for a,op,args in re.findall(r'^([0-9a-f]{8}) ([A-Z.]+)(?: (.*))?$',asm,re.M)})
addresses=sorted(g['code']);g['following']={a:addresses[i+1] for i,a in enumerate(addresses[:-1])}
fixtures=json.loads((root/'analysis/reports/d3d-resize.input.json').read_text());path=root/'analysis/reports/d3d-resize.json';report=json.loads(path.read_text())
assert path.read_bytes()==(root/'analysis/reports/d3d-resize-release.json').read_bytes()
assert report['source_sha256']==fixtures['source_sha256']==sha(root/'analysis/reports/d3d-dynamic.input.json')==json.loads((root/'analysis/reports/d3d-dynamic-validation.json').read_text())['input_sha256']
def old_world(w):
 rt=w['runtime'];return dict(vertex=rt['vertex'],index=rt['index'],device=rt['device'],deferred=rt['deferred'],state=w['state'],target=w['target'])
def world(vm,w):
 value=n['world'](vm,old_world(w));return dict(runtime={k:value[k] for k in ['vertex','index','device','deferred']},state=value['state'],target=value['target'])
def setup(vm,c):
 put(vm,BASE,D);put(vm,D+0x4210,c['stream_capacity']&0xffffffff)
 for k,off in [('hardware_vertices',0x40dc),('skip_eviction',0x4120),('address',0x46b4)]:put(vm,D+off,c['device'][k])
 put(vm,c['device']['address'],0x24000000)
def invalidate(vm,handle):
 saved=vm.r.copy();flags=(vm.z,vm.s,vm.o,vm.c);start=vm.steps;event=len(vm.events)
 vm.r.update(ECX=BASE,ESP=0x70020000);put(vm,vm.r['ESP']+4,handle);vm.run(0x10029fe0,0x1002a045)
 commands=[dict(receiver=0x23000000,**c) for c in vm.events[event:]]
 vm.r.update(saved);vm.z,vm.s,vm.o,vm.c=flags;counts['invalidation_calls']+=1;counts['invalidation_body_instructions']+=vm.steps-start;counts['immediate_unbinds']+=len(commands)
 return commands
def create(vm,pc,offset,commands,o,pos):
 args=n['call'](vm,pc,offset,5,commands);answer=o['creates'][pos];put(vm,args[4],answer['handle'])
 if answer['handle']:put(vm,answer['handle'],0x24000000)
 vm.r['EAX']=answer['hresult'];counts['create_calls']+=1;counts['system_pool_creates']+=int(args[3]==2)
 return pos+1
def eviction(vm,pc,commands,o,pos):
 n['call'](vm,pc,0x14,1,commands);vm.r['EAX']=o['evictions'][pos];assert vm.r['EAX']<0x80000000;counts['evictions']+=1;return pos+1
def release(vm,pc,commands):
 n['call'](vm,pc,8,0,commands);counts['releases']+=1
def vertex_resize(vm,size,c,o):
 saved=vm.r.copy();start=vm.steps;commands=[];setup(vm,c)
 vm.r.update(ESI=V,EBX=size,EDI=0,EBP=RF,ESP=0x70010000);put(vm,RF+8,size)
 vm.run(0x1002a62f,0x1002a637)
 if not vm.z:
  vm.run(0x1002a639,0x1002a643);assert vm.r['ECX']==BASE;handle=vm.pop();vm.steps+=1;commands+=invalidate(vm,handle)
  vm.run(0x1002a648,0x1002a64e);release(vm,0x1002a64e,commands);vm.run(0x1002a651,0x1002a654)
 vm.run(0x1002a654,0x1002a659)
 if not vm.z:
  vm.run(0x1002a65b,0x1002a665);assert vm.r['ECX']==BASE;handle=vm.pop();vm.steps+=1;commands+=invalidate(vm,handle)
  vm.run(0x1002a66a,0x1002a670);release(vm,0x1002a670,commands);vm.run(0x1002a673,0x1002a676)
 vm.run(0x1002a676,0x1002a68c)
 if vm.z:vm.run(0x1002a68e,0x1002a695)
 vm.run(0x1002a695,0x1002a6a0);response=0;evict=0
 while True:
  assert get(vm,RF+8)<3
  vm.run(0x1002a6a0,0x1002a6cb);response=create(vm,0x1002a6cb,0x5c,commands,o,response);vm.run(0x1002a6ce,0x1002a6d2);assert not vm.s
  vm.run(0x1002a6d4,0x1002a6ef);response=create(vm,0x1002a6ef,0x5c,commands,o,response);vm.run(0x1002a6f2,0x1002a6f6)
  if not vm.s:break
  vm.run(0x1002a6fc,0x1002a707)
  if not vm.z:vm.run(0x1002a709,0x1002a6a0)
  else:
   vm.run(0x1002a716,0x1002a721);evict=eviction(vm,0x1002a721,commands,o,evict);vm.run(0x1002a724,0x1002a726);assert not vm.s;vm.run(0x1002a73e,0x1002a6a0)
 plan=dict(usage=get(vm,RF-0x14),format=None,commands=commands);vm.r.update(saved);counts['vertex_resize_bodies']+=1
 return plan,vm.steps-start
def index_resize(vm,size,c,o):
 saved=vm.r.copy();start=vm.steps;commands=[];setup(vm,c)
 vm.r.update(ESI=I,EDI=size,EBX=0,EBP=RF,ESP=0x70010000);put(vm,RF+8,size)
 vm.run(0x1002a8bf,0x1002a8c7)
 if not vm.z:vm.run(0x1002a8c9,0x1002a8cc);release(vm,0x1002a8cc,commands);vm.run(0x1002a8cf,0x1002a8d2)
 vm.run(0x1002a8d2,0x1002a8f9)
 if vm.z:vm.run(0x1002a8fb,0x1002a902)
 vm.run(0x1002a902,0x1002a910);response=0;evict=0
 while True:
  assert get(vm,RF+8)<3
  vm.run(0x1002a910,0x1002a939);response=create(vm,0x1002a939,0x60,commands,o,response);vm.run(0x1002a93c,0x1002a940)
  if not vm.s:break
  vm.run(0x1002a946,0x1002a951)
  if not vm.z:vm.run(0x1002a953,0x1002a910)
  else:
   vm.run(0x1002a960,0x1002a96b);evict=eviction(vm,0x1002a96b,commands,o,evict);vm.run(0x1002a96e,0x1002a970);assert not vm.s;vm.run(0x1002a988,0x1002a910)
 plan=dict(usage=get(vm,RF-0x14),format=get(vm,RF-0x18),commands=commands);vm.r.update(saved);counts['index_resize_bodies']+=1
 return plan,vm.steps-start
current_plan=None
def nested_resize(vm,s,kind):
 global current_plan
 size=vm.pop();assert vm.r['ECX']==(V if kind=='vertex' else I);vm.steps+=1
 current_plan,_=globals()[kind+'_resize'](vm,size,s['context'],s['answers']['resize']);counts['integrated_resizes']+=1
n['resize']=nested_resize
for c,q in zip(fixtures['invalidations'],report['invalidations'],strict=True):
 w=old_world(fixtures['cases'][0]['world']);w['deferred']=c['deferred'];vm=n['initialize'](w);put(vm,BASE,D);put(vm,D+0x4210,c['capacity']&0xffffffff);put(vm,D+0x46b4,c['device']);put(vm,c['device'],0x24000000)
 start=vm.steps;commands=invalidate(vm,c['handle']);expected=dict(id=c['id'],commands=commands,deferred=n['b']['deferred'](vm));assert q==expected,('invalidation',c['id']);counts['standalone_invalidation_instructions']+=vm.steps-start;counts['invalidation_probes']+=1
for c,q in zip(fixtures['resizes'],report['resizes'],strict=True):
 initial=copy.deepcopy(fixtures['cases'][0]['world']);initial['runtime']=c['runtime'];vm=n['initialize'](old_world(initial));plan,steps=globals()[c['kind']+'_resize'](vm,c['size'],c['context'],c['answers'])
 assert q==dict(id=c['id'],result=dict(Ok=plan),runtime=world(vm,initial)['runtime']),('resize',c['id'])
 counts['standalone_resize_instructions']+=steps;counts['standalone_resizes']+=1
for c,q in zip(fixtures['cases'],report['probes'],strict=True):
 assert c['id']==q['id'];vm=n['initialize'](old_world(c['world']))
 for j,(step,actual) in enumerate(zip(c['steps'],q['steps'],strict=True)):
  current_plan=None;setup(vm,step['context']);s=copy.deepcopy(step);s['responses']=s['answers']['lock']
  upload=n[step['kind']](vm,s)
  if step['kind']=='vertex':n['bind_vertex'](vm,s,upload);pool=None
  else:pool=n['bind_index'](vm,s,upload)
  expected=dict(result=dict(Ok=dict(transfer=dict(resize=current_plan,upload=upload),pool=pool)),world=world(vm,c['world']))
  assert actual==expected,('integration',c['id'],j,actual['result'],expected['result'])
  counts['steps']+=1
 assert q['world']==world(vm,c['world']);counts['cases']+=1
for c,q in zip(fixtures['errors'],report['errors'],strict=True):
 assert c['id']==q['id'] and q['world']==c['world']
 for step in q['steps']:assert 'Err' in step['result'] and step['world']==c['world']
 counts['safe_errors']+=1
assert counts['invalidation_probes']==24 and counts['standalone_resizes']==96 and counts['cases']==48 and counts['steps']==192 and counts['safe_errors']==12
counts['integrated_instructions']=sum(v for k,v in n['counts'].items() if k.endswith('_instructions'))
counts['total_instructions']=counts['integrated_instructions']+counts['standalone_resize_instructions']+counts['standalone_invalidation_instructions']
assert counts['immediate_unbinds']>0 and counts['system_pool_creates']>0 and counts['evictions']>0
tests=sum(map(int,re.findall(r'test result: ok\. (\d+) passed',(root/'analysis/reports/d3d-resize-tests.log').read_text())));assert tests==746,tests
sources=['crates/rc-package/src/d3d_resize.rs','crates/rc-package/src/d3d_dynamic.rs','crates/rc-package/src/d3d_bindings.rs','crates/rc-package/src/d3d_upload.rs','crates/rc-inspect/src/bin/rc-d3d-resize-check.rs','scripts/Generate-D3DResize.py','scripts/Record-D3DResize.py','scripts/Record-D3DDynamic.py','scripts/Record-D3DBuffers.py','scripts/Record-D3DState.py','scripts/Record-MaterialState.py','analysis/decompiled/d3d-resize-cache.c','analysis/decompiled/d3d-dynamic-buffers.c']+['analysis/decompiled/'+name for name in assemblies+['d3d-dynamic-ring.asm','d3d-dynamic-setup.asm']]
validation=dict(date='2026-10-09',counts=dict(counts),rust_tests=tests,debug_release_identical=True,original_d3ddrv_sha256=sha(g['dll']),input_sha256=sha(root/'analysis/reports/d3d-resize.input.json'),report_sha256=sha(path),source_sha256={p:sha(root/p) for p in sources},scope=report['scope'],replay_boundary=__doc__,android='deferred until end per user')
(root/'analysis/reports/d3d-resize-validation.json').write_text(json.dumps(validation,indent=2)+'\n');print(json.dumps(dict(counts=dict(counts),rust_tests=tests)))
