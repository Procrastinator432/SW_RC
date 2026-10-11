"""Original SSE camera LOD, gates and vertex callback handoff into index/draw replay.

Scope starts after transform setup and optional preprocessing. Default MXCSR
round-to-nearest-even is assumed. x87 FPTAN stored result, Project W and GetAbsMax
stored result are supplied answers; their implementation bodies are excluded.
GetFrame arguments/count-pointer input and supplied writeback execute at an
explicit callback boundary; vertex animation/filling is not implemented here.
Existing geometry, array allocator, material and GPU boundaries still apply.
Nonfinite inputs and zero/nonfinite denominator/candidate are safe port errors.
"""
from pathlib import Path
import json,re,copy,struct,math
camera_boundary=__doc__
index_shared_path=Path(__file__).resolve().parent/'Record-VertMeshIndices.py'
index_shared_src=index_shared_path.read_text()
exec(compile(index_shared_src.split('\nfor c,actual in zip')[0],str(index_shared_path),'exec'))
__doc__=camera_boundary
assemblies.append('engine-vertmesh-lod.asm')
asm=(root/'analysis/decompiled/engine-vertmesh-lod.asm').read_text()
g['code'].update({int(a,16):(op,args.split(',') if args else []) for a,op,args in re.findall(r'^([0-9a-f]{8}) ([A-Z0-9.]+)(?: (.*))?$',asm,re.M)})
addresses=sorted(g['code']);g['following']={a:addresses[i+1] for i,a in enumerate(addresses[:-1])}
fixtures=json.loads((root/'analysis/reports/vertmesh-lod.input.json').read_text());path=root/'analysis/reports/vertmesh-lod.json';report=json.loads(path.read_text())
assert path.read_bytes()==(root/'analysis/reports/vertmesh-lod-release.json').read_bytes()
for key,prior in [('source_sha256','vertmesh-indices'),('passes_sha256','d3d-draw')]:
 assert fixtures[key]==report[key]==sha(root/('analysis/reports/'+prior+'.input.json'))==json.loads((root/('analysis/reports/'+prior+'-validation.json')).read_text())['input_sha256']
engine_raw=Path(draw['engine']).read_bytes();pe=struct.unpack_from('<I',engine_raw,60)[0];image_base=struct.unpack_from('<I',engine_raw,pe+52)[0];optional=struct.unpack_from('<H',engine_raw,pe+20)[0]
engine_sections=[struct.unpack_from('<IIII',engine_raw,pe+24+optional+40*i+8) for i in range(struct.unpack_from('<H',engine_raw,pe+6)[0])]
def engine_word(address):
 for size,va,rawsize,off in engine_sections:
  if va<=address-image_base<va+max(size,rawsize):return struct.unpack_from('<I',engine_raw,off+address-image_base-va)[0]
 raise AssertionError(hex(address))
constants={0x10676768:0x3b449ba6,0x10664fb0:0x3e800000,0x1067bff4:0x3a8f5c29,0x10668db0:0x3e99999a,0x10653578:0x3f800000,0x1067bff0:0x44048000}
assert all(engine_word(a)==v for a,v in constants.items())
probe=g['Native']();probe.r['EBP']=EF
for value,expected in [(2.5,2),(3.5,4),(-2.5,-2),(-3.5,-4),(2147483648.0,-2147483648),(-2147483648.0,-2147483648),(float('nan'),-2147483648)]:
 put(probe,EF-0x14,struct.unpack('<I',struct.pack('<f',value))[0]);probe.run(0x105602ab,0x105602b0);assert probe.r['EAX']==expected&0xffffffff
counts['conversion_regressions']=7

def signed(value):return value-(1<<32) if value&0x80000000 else value
def camera(vm,q):
 c=q['camera'];gate=q['gates'];answer=q['vertex'];qm=q['geometry']['mesh'];OBJ=0x39000000;ACTOR=qm['materials']['actor_address'];SCENE=answer['scene'];VIEW=0x3e004000;CAM=0x3f000000;PROJ=0x3f010000;OWNER=0x3f020000;FALLBACK=0x3f030000
 for a,v in constants.items():put(vm,a,engine_word(a))
 for off,v in [(0x5c,c['vertex_count']),(0xbc,c['mesh_lod_scale']),(0xc0,c['mesh_distance_scale']),(0xc4,c['minimum']),(0xcc,c['distance_bias']),(0x1a4,gate['packed_sections']),(0x170,answer['source'])]:put(vm,OBJ+off,v&0xffffffff)
 for off,v in [(0xd8,c['actor_lod_bias']),(0x1b0,c['actor_draw_scale']),(0x1a4,gate['actor_state']),(0xa0,OWNER if gate['owner_state'] is not None else 0),(0x94,FALLBACK)]:put(vm,ACTOR+off,v)
 put(vm,OWNER+0x2f8,gate['owner_state'] or 0);put(vm,FALLBACK+0x2f8,gate['fallback_state'])
 put(vm,SCENE+4,VIEW);put(vm,VIEW+0x30,CAM);put(vm,VIEW+0x80,c['viewport_width']&0xffffffff);put(vm,PROJ+0xc,c['projected_w'])
 put(vm,EF+12,SCENE);put(vm,EF-0x30,ACTOR);put(vm,EF-0x38,answer['instance']);put(vm,answer['instance'],0x24000000)
 vm.r.update(EBX=OBJ,EBP=EF,ESP=0x70060000)
 vm.run(0x10560194,0x105601a2);vm.run(0x105601a8,0x105601b0);vm.run(0x105601b6,0x105601c6)
 vertex_factor=get(vm,EF-0x14)
 vm.run(0x105601c8,0x105601f0);viewport_factor=get(vm,EF-0x20)
 # Supplied x87 stored tangent and Project answers, with actual call arguments.
 put(vm,EF-0x18,c['tan_fov']);assert vm.pop()==EF-0x48 and vm.pop()==ACTOR+0x138
 vm.steps+=1;vm.r['EAX']=PROJ
 vm.run(0x105601fa,0x10560217);distance=vm.xmm['XMM1']
 vm.run(0x10560217,0x10560258);assert vm.r['ECX']==ACTOR+0x1b4
 denominator=get(vm,EF-0x14);absolute_scale=get(vm,EF-0x1c)
 put(vm,EF-0x18,c['actor_abs_max']);vm.steps+=2
 vm.run(0x10560261,0x10560296);candidate=vm.xmm['XMM1']
 vm.run(0x10560296,0x105602ab);selected_float=get(vm,EF-0x14)
 vm.run(0x105602ab,0x105602bb);rounded=signed(vm.r['EAX']);selected=signed(vm.r['EDI'])
 lod=dict(vertex_factor=vertex_factor,viewport_factor=viewport_factor,distance=distance,denominator=denominator,absolute_scale=absolute_scale,candidate=candidate,selected_float=selected_float,rounded=rounded,selected=selected)
 vm.run(0x105602bb,0x105602c8)
 if vm.z:return lod,'no_sections',None
 vm.run(0x105602ce,0x105602d6);enabled=not vm.z
 if not enabled:
  vm.run(0x105602d8,0x105602e0)
  if not vm.z:vm.run(0x105602e2,0x105602ea);enabled=not vm.z
  if not enabled:vm.run(0x105602ec,0x105602fa);enabled=not vm.z
 if not enabled:return lod,'no_actor_state',None
 vm.run(0x10560300,0x10560306);assert vm.r['ECX']==VIEW;vm.r['EAX']=int(qm['materials']['wire']);vm.steps+=1
 vm.run(0x1056030b,0x1056032a);calls=[];n['call'](vm,0x1056032a,0xdc,6,calls,True)
 assert answer['count_pointer']==EF-0x34 and get(vm,EF-0x34)==selected&0xffffffff
 put(vm,EF-0x34,answer['count_after']);counts['vertex_callbacks']+=1
 return lod,'enabled',calls[0]

for c,actual in zip(fixtures['cases'],report['probes'],strict=True):
 assert c['id']==actual['id'];rt=c['runtime'];oldw=dict(runtime=rt['buffers'],state=rt['state'],target=rt['vertex_target']);n['I']=r['I']=0x34000100
 vm,old=p['initial'](oldw);p['nodes']=[dict(address=a['address'],size=len(a['bytes'])) for a in rt['buffers']['cache']['nodes']];s['extent']=len(rt['buffers']['scratch']['bytes'])
 put(vm,0x1006f014,0x2f000000);put(vm,0x2f000000,0x2f000100);put(vm,0x2f000100,0x2f000200)
 vertex_bytes=copy.deepcopy(rt['vertex_target']);index_bytes=copy.deepcopy(rt['index_target']);put(vm,STATE+0x300,rt['last_pass'])
 for i,w in enumerate(rt['lights']['words']):b['write'](vm,BASE+0x6d8+i*0x68,w)
 b['write'](vm,BASE+0xa18,rt['lights']['enabled']);b['write'](vm,BASE+0x144c,rt['lights']['applied_enabled'])
 for k,off in [('draw_passes',0xfec0),('submitted_primitives',0xfc90),('submitted_vertices',0xfcb8)]:put(vm,D+off,rt['counters'][k])
 for j,(q,step) in enumerate(zip(c['requests'],actual['steps'],strict=True)):
  q=copy.deepcopy(q);start=vm.steps;lod,gate,vertex_call=camera(vm,q)
  if gate=='enabled':
   qg=q['geometry'];qg['geometry']['lod']=lod['selected'];generated=geometry(vm,qg['geometry']);qm=qg['mesh'];qm['sections']=generated['sections']
   payload=list(b''.join(struct.pack('<H',i) for i in generated['indices']))
   qm['index']['source'].update(width=2,size=len(payload),payload=payload);qm['index']['reported_size']=len(payload)
   upload_request=dict(caller=dict(kind='vert_mesh'),vertex=qm['vertex'],index=qm['index'])
   vertex,index=handoff(vm,upload_request);qm['index']=upload_request['index']
   upload_plan=dict(vertex=vertex,index=index,draws=[],rendered=[])
   draw_plan=dict(geometry=generated,draw=dict(upload=upload_plan,sections=mesh(vm,qm,index)))
   value=dict(lod=lod,gate=gate,vertex_call=vertex_call,count_before=lod['selected']&0xffffffff,count_after=q['vertex']['count_after'],draw=draw_plan)
  else:
   value=dict(lod=lod,gate=gate,vertex_call=None,count_before=None,count_after=None,draw=None);counts[gate]+=1
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
tests=sum(map(int,re.findall(r'test result: ok\. (\d+) passed',(root/'analysis/reports/vertmesh-lod-tests.log').read_text())));assert tests==853,tests
sources=set(json.loads((root/'analysis/reports/vertmesh-indices-validation.json').read_text())['source_sha256'])
sources.update(['crates/rc-package/src/vertmesh_lod.rs','crates/rc-inspect/src/bin/rc-vertmesh-lod-check.rs','scripts/Generate-VertMeshLod.py','scripts/Record-VertMeshLod.py','analysis/decompiled/engine-vertmesh-lod.asm'])
validation=dict(date='2026-10-10',counts=dict(counts),rust_tests=tests,debug_release_identical=True,original_d3ddrv_sha256=sha(g['dll']),original_engine_sha256=sha(draw['engine']),input_sha256=sha(root/'analysis/reports/vertmesh-lod.input.json'),report_sha256=sha(path),source_sha256={a:sha(root/a) for a in sorted(sources)},scope=report['scope'],replay_boundary=__doc__,android='deferred until end per user')
(root/'analysis/reports/vertmesh-lod-validation.json').write_text(json.dumps(validation,indent=2)+'\n');print(json.dumps(dict(counts=dict(counts),rust_tests=tests)))
