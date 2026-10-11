"""Joined original preprocessing/Project/AbsMax/LOD/GetFrame/geometry/upload/draw replay.
Actual prepared stream address and count pointer feed original GetFrame bodies.
Matrices and stored FPTAN, logical allocators, mesh +ac, material/COM answers supplied.
Transform setup, asset loading, allocator ownership, live GPU and Android excluded.
Safe port errors checked for full request/runtime rollback, not executed natively.
"""
from pathlib import Path
import json,copy,re,collections
render_boundary=__doc__
render_preprocess_path=Path(__file__).resolve().parent/'Record-VertMeshPreprocess.py'
render_preprocess_source=render_preprocess_path.read_text()
exec(compile(render_preprocess_source.split('\nfor x,y in zip',1)[0],str(render_preprocess_path),'exec'))
render_camera_function=camera
render_shared_code=g['code'].copy();render_regressions=counts.copy()
render_instance_path=Path(__file__).resolve().parent/'Record-VertMeshInstance.py'
render_instance_source=render_instance_path.read_text()
exec(compile(render_instance_source.split('\nfor i,(q,value)',1)[0],str(render_instance_path),'exec'))
__doc__=render_boundary
g['code'].update(render_shared_code)
addresses=sorted(g['code']);g['following']={a:addresses[i+1] for i,a in enumerate(addresses[:-1])}
counts.update(render_regressions)
fixtures=json.loads((root/'analysis/reports/vertmesh-render.input.json').read_text())
path=root/'analysis/reports/vertmesh-render.json';report=json.loads(path.read_text())
assert path.read_bytes()==(root/'analysis/reports/vertmesh-render-release.json').read_bytes()
assert fixtures['source_sha256']==report['source_sha256']==sha(root/'analysis/reports/vertmesh-instance.json')==json.loads((root/'analysis/reports/vertmesh-instance-validation.json').read_text())['report_sha256']

# Keep the original caller's mesh, stream and count-pointer addresses through GetFrame.
render_decode_source=animation_decode_source.replace("vm.r['EAX']=0x7b000000 if animation_input['clock']['sequence'] is not None else 0", "vm.r['EAX']=active_sequence_identity")
for old,new in [('MESH=0x77000000','MESH=active_mesh_identity'),('OUT=0x80000000','OUT=active_stream_address'),('COUNT=0x7a000000','COUNT=active_count_pointer')]:
 assert old in render_decode_source;render_decode_source=render_decode_source.replace(old,new)
exec(compile(render_decode_source,str(__file__)+'::caller-address-getframe','exec'))
exec(compile(instance_schedule_source.replace('MESH=0x77000000','MESH=active_mesh_identity'),str(__file__)+'::caller-mesh-scheduler','exec'))

# Reuse the Project/AbsMax-enhanced camera function source with only the GetFrame
# answer boundary replaced by the proved instance body and actual memory writeback.
render_lod_source=(root/'scripts/Record-VertMeshLod.py').read_text()
render_camera_source='def camera(vm,q):'+render_lod_source.split('def camera(vm,q):',1)[1].split('\nfor c,actual in zip',1)[0]
old="put(vm,EF-0x18,c['tan_fov']);assert vm.pop()==EF-0x48 and vm.pop()==ACTOR+0x138\n vm.steps+=1;vm.r['EAX']=PROJ"
new="put(vm,EF-0x18,c['tan_fov']);project_call(vm,SCENE,EF-0x48,ACTOR+0x138)"
assert render_camera_source.count(old)==1;render_camera_source=render_camera_source.replace(old,new)
old="put(vm,EF-0x18,c['actor_abs_max']);vm.steps+=2"
assert render_camera_source.count(old)==1;render_camera_source=render_camera_source.replace(old,"abs_call(vm,ACTOR+0x1b4,EF-0x18)")
render_camera_source=render_camera_source.replace(' c=q[', ' global active_mesh_identity,active_stream_address,active_count_pointer,render_instance_output\n c=q[',1)
old=" put(vm,EF-0x34,answer['count_after']);counts['vertex_callbacks']+=1"
new=""" active_mesh_identity=q['instance']['mesh'];active_stream_address=answer['source'];active_count_pointer=answer['count_pointer']
 a=q['instance']['animation'];a.update(requested_count=selected,mode=0,stride=32,wedges=q['geometry']['geometry']['vertex_thresholds'].copy(),bytes=q['preprocessing']['state']['stream'].copy())
 render_instance_output=instance_frame(vm,q['instance']);commit_instance(q['instance'],render_instance_output)
 answer['count_after']=render_instance_output['animation']['frame']['count']
 assert get(vm,active_count_pointer)==answer['count_after']
 assert [vm.m[active_stream_address+i] for i in range(len(a['bytes']))]==render_instance_output['animation']['frame']['bytes']
 counts['vertex_callbacks']+=1;counts['actual_stream_callbacks']+=1"""
assert render_camera_source.count(old)==1;render_camera_source=render_camera_source.replace(old,new)
exec(compile(render_camera_source,str(__file__)+'::real-getframe-camera','exec'))

render_loop='\nfor c,actual in zip'+render_lod_source.split('\nfor c,actual in zip',1)[1].split('\ntests=sum(',1)[0]
old="q=copy.deepcopy(q);start=vm.steps;lod,gate,vertex_call=camera(vm,q)"
new="""q=copy.deepcopy(q);start=vm.steps;prepared=preprocess(vm,q['preprocessing'])
  q['preprocessing']['state']=copy.deepcopy(prepared['state']);state=prepared['state'];q['gates']['packed_sections']=state['packed_sections']
  geom=q['geometry']['geometry'];geom['packed_count']=state['packed_sections'];geom['section_bytes']=state['section_bytes'].copy()
  if prepared['processed']:
   inp=q['preprocessing'];geom['faces']=copy.deepcopy(inp['faces'][:inp['packed_faces']&0x1fffffff]);geom['vertex_thresholds']=[w[0]&65535 for w in inp['wedges'][:inp['packed_wedges']&0x1fffffff]]
  matrix(vm,q['vertex']['scene'],0x114,q['world_to_screen']);actor=q['geometry']['mesh']['materials']['actor_address']
  for k,v in enumerate(q['actor_position']):put(vm,actor+0x138+4*k,v)
  for k,v in enumerate(q['actor_draw_scale']):put(vm,actor+0x1b4+4*k,v)
  lod,gate,vertex_call=camera(vm,q);projected=[get(vm,EF-0x48+4*k) for k in range(4)]
  absolute=get(vm,EF-0x18);q['camera']['projected_w']=projected[3];q['camera']['actor_abs_max']=absolute
  counts['gate_'+gate]+=1"""
assert render_loop.count(old)==1;render_loop=render_loop.replace(old,new)
old="   qg=q['geometry'];qg['geometry']['lod']=lod['selected'];generated=geometry(vm,qg['geometry']);qm=qg['mesh'];qm['sections']=generated['sections']"
new="""   qg=q['geometry'];qg['geometry']['lod']=lod['selected']
   decoded=render_instance_output['animation']['frame'];source=qg['mesh']['vertex']['source']
   source.update(stride=32,size_before=len(decoded['bytes']),size_again=len(decoded['bytes']),payload=decoded['bytes'].copy())
   generated=geometry(vm,qg['geometry']);qm=qg['mesh'];qm['sections']=generated['sections']"""
assert render_loop.count(old)==1;render_loop=render_loop.replace(old,new)
old="  assert step['result']==dict(Ok=value),('result',c['id'],j,step['result'],value)"
new="""  value.update(preprocessing=prepared,actor_abs_max=absolute,projection=projected,instance=render_instance_output if gate=='enabled' else None)
  if gate=='enabled':
   q['preprocessing']['state']['stream']=render_instance_output['animation']['frame']['bytes'].copy()
   q['preprocessing']['state']['section_bytes']=q['geometry']['mesh']['sections']['bytes'].copy()
  assert step['result']==dict(Ok=value),('result',c['id'],j,difference(step['result'],dict(Ok=value)))"""
assert render_loop.count(old)==1;render_loop=render_loop.replace(old,new)
exec(compile(render_loop,str(__file__)+'::joined-native-loop','exec'))
tests=sum(map(int,re.findall(r'test result: ok\. (\d+) passed',(root/'analysis/reports/vertmesh-render-tests.log').read_text())));assert tests==920,tests
assert counts['gate_no_sections']>0 and counts['gate_no_actor_state']>0 and counts['actual_stream_callbacks']==counts['gate_enabled']
sources=set(json.loads((root/'analysis/reports/vertmesh-instance-validation.json').read_text())['source_sha256'])
sources.update(['crates/rc-package/src/vertmesh_render.rs','crates/rc-inspect/src/bin/rc-vertmesh-render-check.rs','scripts/Generate-VertMeshRender.py','scripts/Record-VertMeshRender.py','docs/VERTMESH_RENDER.md','analysis/finalize-vertmesh-render.py'])
validation=dict(date='2026-10-10',counts=dict(counts),rust_tests=tests,debug_release_identical=True,original_core_sha256=sha(core),original_engine_sha256=sha(draw['engine']),original_d3ddrv_sha256=sha(g['dll']),input_sha256=sha(root/'analysis/reports/vertmesh-render.input.json'),report_sha256=sha(path),source_sha256={a:sha(root/a) for a in sorted(sources)},scope=report['scope'],replay_boundary=render_boundary,android='deferred until end per user')
(root/'analysis/reports/vertmesh-render-validation.json').write_text(json.dumps(validation,indent=2)+'\n');print(json.dumps(dict(counts=dict(counts),rust_tests=tests)))
