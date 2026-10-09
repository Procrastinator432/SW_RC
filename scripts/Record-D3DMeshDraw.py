"""Original vertex-mesh loop, gates, material routing and complete uploaded draws.

Engine geometry/count generation, whole-mesh visibility/setup and IsWire answer
are inputs. Calls +a0/+d8/+cc return supplied answers. Cached wire shader must
already exist; lazy shader construction is excluded. SetMaterial is an explicit
boundary producing the supplied pass bank. Subsequent original D3D pass bodies
execute. COM/allocator/getter/fill/shader answers, SEH/logging/timing/destructors,
native error paths and actual GPU execution remain outside the replay.
"""
from pathlib import Path
import json,re,copy,collections
boundary_doc=__doc__
shared_path=Path(__file__).resolve().parent/'Record-D3DParticleDraw.py'
shared_src=shared_path.read_text()
exec(compile(shared_src.split('\nfixtures=json.loads')[0],str(shared_path),'exec'))
exec(compile(shared_src[shared_src.index('def interface('):shared_src.index('\ndef gate(')],str(shared_path),'exec'))
exec(compile(shared_src[shared_src.index('def handoff('):shared_src.index('\ndef caller_draws(')],str(shared_path),'exec'))
exec(compile(shared_src[shared_src.index('def runtime('):shared_src.index('\n# Explicit width regression:')],str(shared_path),'exec'))
__doc__=boundary_doc
assemblies.append('engine-vertmesh-loop.asm')
asm=(root/'analysis/decompiled/engine-vertmesh-loop.asm').read_text()
g['code'].update({int(a,16):(op,args.split(',') if args else []) for a,op,args in re.findall(r'^([0-9a-f]{8}) ([A-Z.]+)(?: (.*))?$',asm,re.M)})
addresses=sorted(g['code']);g['following']={a:addresses[i+1] for i,a in enumerate(addresses[:-1])}
fixtures=json.loads((root/'analysis/reports/d3d-mesh-draw.input.json').read_text());path=root/'analysis/reports/d3d-mesh-draw.json';report=json.loads(path.read_text())
assert path.read_bytes()==(root/'analysis/reports/d3d-mesh-draw-release.json').read_bytes()
for key,prior in [('source_sha256','d3d-particle-draw'),('passes_sha256','d3d-draw')]:
 assert fixtures[key]==report[key]==sha(root/('analysis/reports/'+prior+'.input.json'))==json.loads((root/('analysis/reports/'+prior+'-validation.json')).read_text())['input_sha256']

def material_call(vm,pc,offset,nargs,calls):
 n['call'](vm,pc,offset,nargs,calls,True);counts['material_interface_'+hex(offset)]+=1

def mesh(vm,q,index):
 m=q['materials'];raw=q['sections'];OBJ=m['mesh_address'];SEC=0x3b000000;COUNTS=0x3a000000;SLOTS=0x3d000000;SCENE=0x3e003000;VIEW=0x3e004000;DEBUG=0x3e002000
 assert OBJ==0x39000000 and m['renderer_address']==R
 for a in [OBJ,m['actor_object'],m['resolver'],R]:put(vm,a,0x24000000 if a!=R else 0x1006f8d8)
 put(vm,OBJ+0x1a0,SEC);put(vm,OBJ+0x1a4,raw['packed_count']);put(vm,OBJ+0xb4,SLOTS)
 for j,v in enumerate(raw['bytes']):vm.m[SEC+j]=v
 for j,v in enumerate(raw['primitive_counts']):put(vm,COUNTS+j*4,v)
 for j,v in enumerate(m['slots']):put(vm,SLOTS+j*8+4,v)
 put(vm,m['actor_address'],m['actor_object']);put(vm,m['actor_object']+0x1d0,m['override_packed_count'])
 put(vm,SCENE+4,VIEW);put(vm,DEBUG+0x6c,m['debug']);put(vm,0x108b9b28,m['wire_shader'])
 for off,v in [(8,m['actor_address']),(12,SCENE),(0x18,R),(-0x30,m['actor_address']),(-0x38,DEBUG),(-0x2c,COUNTS),(-0x24,index['offset'])]:put(vm,EF+off,v)
 vm.r.update(EBP=EF,ESP=0x70060000,EBX=OBJ);vm.run(0x105607b4,0x105607b6)
 out=[]
 while True:
  vm.run(0x105607b6,0x105607d0)
  if vm.s==vm.o:break
  j=vm.r['EDI'];assert j<256
  vm.run(0x105607d6,0x105607eb)
  if vm.z:
   counts['mesh_zero_count']+=1;vm.run(0x10560905,0x10560906);continue
  vm.run(0x105607f1,0x105607fd)
  if vm.z:
   counts['mesh_zero_word']+=1;vm.run(0x10560905,0x10560906);continue
  entry=vm.r['ESI'];word=lambda off:vm.get('word ptr ['+hex(entry+off)+']')
  section=dict(index=j,material_index=word(0),first_index=word(2),min_vertex=word(4),max_vertex=word(6),primitive_count=get(vm,COUNTS+j*4))
  calls=[];vm.run(0x10560803,0x1056080b);material_call(vm,0x1056080b,0xa0,1,calls);vm.r['EAX']=m['resolver']
  vm.run(0x10560811,0x10560824);material_call(vm,0x10560824,0xd8,2,calls);vm.r['EAX']=m['resolved'][j]
  vm.run(0x1056082a,0x10560832);assert vm.r['ECX']==VIEW;vm.steps+=1;vm.r['EAX']=int(m['wire'])
  vm.run(0x10560837,0x10560839);wire=not vm.z
  if not wire:
   vm.run(0x1056083b,0x10560843);wire=not vm.z
  if wire:
   vm.run(0x10560880,0x10560887);assert not vm.z
   vm.run(0x105608c7,0x105608d9);material_call(vm,0x105608d9,0x50,4,calls);counts['wire_sections']+=1
  else:
   vm.run(0x10560845,0x1056085b)
   if vm.s==vm.o:
    vm.run(0x10560877,0x1056087e);vm.run(0x105608d4,0x105608d9);material_call(vm,0x105608d9,0x50,4,calls);counts['resolved_sections']+=1
   else:
    vm.run(0x1056085d,0x10560860);material_call(vm,0x10560860,0xcc,1,calls);vm.r['EAX']=m['overrides'][section['material_index']]
    vm.run(0x10560866,0x10560872);material_call(vm,0x10560872,0x50,4,calls);counts['override_sections']+=1
  token=calls[-1]['arguments'][0];selection=dict(material=token,calls=calls)
  assert calls[0]==dict(receiver=OBJ,vtable_offset=0xa0,arguments=[m['actor_address']])
  assert calls[1]==dict(receiver=m['resolver'],vtable_offset=0xd8,arguments=[m['slots'][section['material_index']],m['actor_address']])
  vm.run(0x105608dc,0x105608ff);args=interface(vm,0x105608ff,0x94,5)
  d=dict(zip(['primitive','start','primitive_count','min_vertex','max_vertex'],args),indexed=True)
  assert get(vm,STATE+0x484)!=0
  saved=vm.r.copy();bank=next(v for v in q['bank'] if v['material']==token)
  submission=dict(last_pass=get(vm,STATE+0x300),counters=draw['counters'](vm),passes=bank['passes'])
  rendered,mutated=b['draw_sequence'](vm,submission,q['context'],d)
  for value in mutated:
   for material in q['bank']:
    for value2 in material['passes']:
     if value2['pass']['address']==value['pass']['address']:value2['pass']=copy.deepcopy(value['pass'])
  counts['material_passes']+=len(rendered);counts['draws']+=1
  out.append(dict(section=section,selection=selection,draw=d,rendered=rendered))
  vm.r.update(saved);vm.run(0x10560905,0x10560906)
 return out

for c,actual in zip(fixtures['cases'],report['probes'],strict=True):
 assert c['id']==actual['id'];rt=c['runtime'];oldw=dict(runtime=rt['buffers'],state=rt['state'],target=rt['vertex_target']);n['I']=r['I']=0x34000100
 vm,old=p['initial'](oldw);p['nodes']=[dict(address=a['address'],size=len(a['bytes'])) for a in rt['buffers']['cache']['nodes']];s['extent']=len(rt['buffers']['scratch']['bytes'])
 put(vm,0x1006f014,0x2f000000);put(vm,0x2f000000,0x2f000100);put(vm,0x2f000100,0x2f000200)
 vertex_bytes=copy.deepcopy(rt['vertex_target']);index_bytes=copy.deepcopy(rt['index_target']);put(vm,STATE+0x300,rt['last_pass'])
 for i,w in enumerate(rt['lights']['words']):b['write'](vm,BASE+0x6d8+i*0x68,w)
 b['write'](vm,BASE+0xa18,rt['lights']['enabled']);b['write'](vm,BASE+0x144c,rt['lights']['applied_enabled'])
 for k,off in [('draw_passes',0xfec0),('submitted_primitives',0xfc90),('submitted_vertices',0xfcb8)]:put(vm,D+off,rt['counters'][k])
 for j,(q,step) in enumerate(zip(c['requests'],actual['steps'],strict=True)):
  q=copy.deepcopy(q);start=vm.steps
  upload_request=dict(caller=dict(kind='vert_mesh'),vertex=q['vertex'],index=q['index'])
  vertex,index=handoff(vm,upload_request);q['index']=upload_request['index']
  upload_plan=dict(vertex=vertex,index=index,draws=[],rendered=[])
  value=dict(upload=upload_plan,sections=mesh(vm,q,index))
  if c['id']==48:
   assert len(value['sections'])==2 and all(v['rendered'] for v in value['sections'])
   assert runtime(vm,oldw,old)!=c['runtime']
   counts['late_error_prefix_draws']+=2
  assert step['result']==dict(Ok=value),('result',c['id'],j,step['result'],value)
  assert step['request']==q,('request',c['id'],j)
  assert step['runtime']==runtime(vm,oldw,old),('runtime',c['id'],j)
  counts['instructions']+=vm.steps-start;counts['requests']+=1
 assert actual['runtime']==runtime(vm,oldw,old);counts['cases']+=1
for c,q in zip(fixtures['errors'],report['errors'],strict=True):
 assert c['id']==q['id'] and q['runtime']==c['runtime']
 for original,step in zip(c['requests'],q['steps'],strict=True):assert 'Err' in step['result'] and step['runtime']==c['runtime'] and step['request']==original
 counts['safe_errors']+=1
assert counts['cases']==49 and counts['requests']==145 and counts['safe_errors']==16
assert report['errors'][12]['steps'][0]['result']==dict(Err='Invalid bounded draw pass address/stage count')
tests=sum(map(int,re.findall(r'test result: ok\. (\d+) passed',(root/'analysis/reports/d3d-mesh-draw-tests.log').read_text())));assert tests==825,tests
sources=set(json.loads((root/'analysis/reports/d3d-particle-draw-validation.json').read_text())['source_sha256'])
sources.update(['crates/rc-package/src/lib.rs','crates/rc-package/src/d3d_mesh_draw.rs','crates/rc-inspect/src/bin/rc-d3d-mesh-draw-check.rs','scripts/Generate-D3DMeshDraw.py','scripts/Record-D3DMeshDraw.py','analysis/decompiled/engine-vertmesh-loop.asm'])
validation=dict(date='2026-10-09',counts=dict(counts),rust_tests=tests,debug_release_identical=True,original_d3ddrv_sha256=sha(g['dll']),original_engine_sha256=sha(draw['engine']),input_sha256=sha(root/'analysis/reports/d3d-mesh-draw.input.json'),report_sha256=sha(path),source_sha256={a:sha(root/a) for a in sorted(sources)},scope=report['scope'],replay_boundary=__doc__,android='deferred until end per user')
(root/'analysis/reports/d3d-mesh-draw-validation.json').write_text(json.dumps(validation,indent=2)+'\n');print(json.dumps(dict(counts=dict(counts),rust_tests=tests)))
