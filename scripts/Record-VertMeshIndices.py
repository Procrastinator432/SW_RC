"""Original LOD face/collapse/index generation composed with mesh upload/draw.

Integer LOD selection, vertex animation/positions, source arrays and whole-mesh
setup/visibility remain inputs. Native array append helpers are explicit
allocator boundaries supplying count/pointer updates; allocation bodies are not
executed. Native u16 stores, section mutation and count writes execute. Existing
mesh material and D3D replay boundaries continue to apply. No real GPU execution.
"""
from pathlib import Path
import json,re,copy,struct
geometry_boundary=__doc__
mesh_shared_path=Path(__file__).resolve().parent/'Record-D3DMeshDraw.py'
mesh_shared_src=mesh_shared_path.read_text()
exec(compile(mesh_shared_src.split('\nfor c,actual in zip')[0],str(mesh_shared_path),'exec'))
__doc__=geometry_boundary
assemblies.append('engine-vertmesh-geometry.asm')
asm=(root/'analysis/decompiled/engine-vertmesh-geometry.asm').read_text()
g['code'].update({int(a,16):(op,args.split(',') if args else []) for a,op,args in re.findall(r'^([0-9a-f]{8}) ([A-Z.]+)(?: (.*))?$',asm,re.M)})
addresses=sorted(g['code']);g['following']={a:addresses[i+1] for i,a in enumerate(addresses[:-1])}
fixtures=json.loads((root/'analysis/reports/vertmesh-indices.input.json').read_text());path=root/'analysis/reports/vertmesh-indices.json';report=json.loads(path.read_text())
assert path.read_bytes()==(root/'analysis/reports/vertmesh-indices-release.json').read_bytes()
for key,prior in [('source_sha256','d3d-mesh-draw'),('passes_sha256','d3d-draw')]:
 assert fixtures[key]==report[key]==sha(root/('analysis/reports/'+prior+'.input.json'))==json.loads((root/('analysis/reports/'+prior+'-validation.json')).read_text())['input_sha256']
probe=g['Native']()
for reg in ['AX','BX','CX','DX','SI','DI','BP','SP']:
 probe.r['E'+reg]=0xabcd1234;assert probe.get(reg)==0x1234;probe.put(reg,0x9876);assert probe.r['E'+reg]==0xabcd9876
counts['word_register_regressions']=8

def geometry(vm,q):
 OBJ=0x39000000;SEC=0x3b000000;THRESH=0x68000000;FACES=0x69000000;COLLAPSE=0x6a000000;VERTEX=0x6b000000;INDICES=0x6c000000;COUNTS=0x3a000000
 put(vm,OBJ+0x1a0,SEC);put(vm,OBJ+0x1a4,q['packed_count']);put(vm,OBJ+0x94,THRESH);put(vm,OBJ+0x9c,FACES);put(vm,OBJ+0xa4,COLLAPSE);put(vm,OBJ+0xac,VERTEX)
 for j,v in enumerate(q['section_bytes']):vm.m[SEC+j]=v
 for values,address,stride in [(q['face_thresholds'],THRESH,2),(q['collapse'],COLLAPSE,2),(q['vertex_thresholds'],VERTEX,12)]:
  for j,v in enumerate(values):
   for k,bv in enumerate(struct.pack('<H',v)):vm.m[address+j*stride+k]=bv
 for j,words in enumerate(q['faces']):
  for k,bv in enumerate(struct.pack('<4H',*words)):vm.m[FACES+j*8+k]=bv
 vm.r.update(EBX=OBJ,EDI=q['lod']&0xffffffff,EBP=EF,ESP=0x70060000)
 put(vm,EF-0x20,q['lod']&0xffffffff);put(vm,EF-0x50,INDICES);put(vm,EF-0x4c,0)
 vm.run(0x1056036c,0x1056037a)
 while True:
  vm.run(0x1056037a,0x1056038b)
  if vm.s==vm.o:break
  vm.run(0x10560391,0x105603c2)
  while True:
   vm.run(0x105603c2,0x105603d8)
   if vm.s==vm.o:break
   vm.run(0x105603de,0x105603f0)
   # JG: unsigned WORD promoted to signed int, compared with signed LOD.
   if not vm.z and vm.s==vm.o:counts['filtered_faces']+=1
   else:
    vm.run(0x105603f6,0x10560410)
    while True:
     vm.run(0x10560410,0x10560413)
     if vm.s==vm.o:break
     vm.run(0x10560415,0x10560432);hops=0
     while vm.s==vm.o:
      vm.run(0x10560434,0x1056043e);vm.run(0x10560429,0x10560432);hops+=1;assert hops<=1024;counts['collapse_hops']+=1
     vm.run(0x10560440,0x10560454)
     assert vm.r['ECX']==EF-0x50;newcount=vm.pop();assert vm.pop()==0
     put(vm,EF-0x4c,newcount);vm.steps+=1;counts['index_array_appends']+=1
     vm.run(0x10560459,0x10560478)
    vm.run(0x1056047a,0x10560487);counts['emitted_faces']+=1
   vm.run(0x10560487,0x10560488)
  vm.run(0x1056048d,0x105604a1)
  assert vm.r['ECX']==EF-0x2c;newcount=vm.pop();assert vm.pop()==0
  put(vm,EF-0x28,newcount);put(vm,EF-0x2c,COUNTS);vm.steps+=1;counts['section_count_appends']+=1
  vm.run(0x105604a6,0x105604c6)
 n=get(vm,EF-0x28);size=get(vm,EF-0x4c)
 counts['generated_indices']+=size
 return dict(sections=dict(packed_count=q['packed_count'],bytes=[vm.m[SEC+j] for j in range(len(q['section_bytes']))],primitive_counts=[get(vm,COUNTS+j*4) for j in range(n)]),indices=[vm.get('word ptr ['+hex(INDICES+j*2)+']') for j in range(size)])

for c,actual in zip(fixtures['cases'],report['probes'],strict=True):
 assert c['id']==actual['id'];rt=c['runtime'];oldw=dict(runtime=rt['buffers'],state=rt['state'],target=rt['vertex_target']);n['I']=r['I']=0x34000100
 vm,old=p['initial'](oldw);p['nodes']=[dict(address=a['address'],size=len(a['bytes'])) for a in rt['buffers']['cache']['nodes']];s['extent']=len(rt['buffers']['scratch']['bytes'])
 put(vm,0x1006f014,0x2f000000);put(vm,0x2f000000,0x2f000100);put(vm,0x2f000100,0x2f000200)
 vertex_bytes=copy.deepcopy(rt['vertex_target']);index_bytes=copy.deepcopy(rt['index_target']);put(vm,STATE+0x300,rt['last_pass'])
 for i,w in enumerate(rt['lights']['words']):b['write'](vm,BASE+0x6d8+i*0x68,w)
 b['write'](vm,BASE+0xa18,rt['lights']['enabled']);b['write'](vm,BASE+0x144c,rt['lights']['applied_enabled'])
 for k,off in [('draw_passes',0xfec0),('submitted_primitives',0xfc90),('submitted_vertices',0xfcb8)]:put(vm,D+off,rt['counters'][k])
 for j,(q,step) in enumerate(zip(c['requests'],actual['steps'],strict=True)):
  q=copy.deepcopy(q);start=vm.steps;generated=geometry(vm,q['geometry']);qm=q['mesh'];qm['sections']=generated['sections']
  payload=list(b''.join(struct.pack('<H',i) for i in generated['indices']))
  qm['index']['source'].update(width=2,size=len(payload),payload=payload);qm['index']['reported_size']=len(payload)
  upload_request=dict(caller=dict(kind='vert_mesh'),vertex=qm['vertex'],index=qm['index'])
  vertex,index=handoff(vm,upload_request);qm['index']=upload_request['index']
  upload_plan=dict(vertex=vertex,index=index,draws=[],rendered=[])
  value=dict(geometry=generated,draw=dict(upload=upload_plan,sections=mesh(vm,qm,index)))
  assert step['result']==dict(Ok=value),('result',c['id'],j,step['result'],value)
  assert step['request']==q,('request',c['id'],j)
  observed=runtime(vm,oldw,old)
  def difference(a,b,path=''):
   if isinstance(a,dict):
    for k in a:
     if a[k]!=b[k]:return difference(a[k],b[k],path+'.'+k)
   elif isinstance(a,list):
    if len(a)!=len(b):return path+'.length',len(a),len(b)
    for k in range(len(a)):
     if a[k]!=b[k]:return difference(a[k],b[k],path+'['+str(k)+']')
   return path,a,b
  assert step['runtime']==observed,('runtime',c['id'],j,difference(step['runtime'],observed))
  counts['instructions']+=vm.steps-start;counts['requests']+=1
 assert actual['runtime']==runtime(vm,oldw,old);counts['cases']+=1
for c,q in zip(fixtures['errors'],report['errors'],strict=True):
 assert c['id']==q['id'] and q['runtime']==c['runtime']
 for original,step in zip(c['requests'],q['steps'],strict=True):assert 'Err' in step['result'] and step['runtime']==c['runtime'] and step['request']==original
 counts['safe_errors']+=1
assert counts['cases']==50 and counts['requests']==146 and counts['safe_errors']==12
tests=sum(map(int,re.findall(r'test result: ok\. (\d+) passed',(root/'analysis/reports/vertmesh-indices-tests.log').read_text())));assert tests==840,tests
sources=set(json.loads((root/'analysis/reports/d3d-mesh-draw-validation.json').read_text())['source_sha256'])
sources.update(['crates/rc-package/src/vertmesh_indices.rs','crates/rc-inspect/src/bin/rc-vertmesh-indices-check.rs','scripts/Generate-VertMeshIndices.py','scripts/Record-VertMeshIndices.py','analysis/decompiled/engine-vertmesh-geometry.asm'])
validation=dict(date='2026-10-10',counts=dict(counts),rust_tests=tests,debug_release_identical=True,original_d3ddrv_sha256=sha(g['dll']),original_engine_sha256=sha(draw['engine']),input_sha256=sha(root/'analysis/reports/vertmesh-indices.input.json'),report_sha256=sha(path),source_sha256={a:sha(root/a) for a in sorted(sources)},scope=report['scope'],replay_boundary=__doc__,android='deferred until end per user')
(root/'analysis/reports/vertmesh-indices-validation.json').write_text(json.dumps(validation,indent=2)+'\n');print(json.dumps(dict(counts=dict(counts),rust_tests=tests)))
