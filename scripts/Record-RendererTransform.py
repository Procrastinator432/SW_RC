"""Original renderer +48 copies/branches/masks joined to transform flush and mesh draw.
Inverse dispatch output is an explicit raw write/no-write boundary; neither its
CPU-selected x87/SIMD implementation nor live GPU/Android execution is claimed.
Existing MeshToWorld, camera, instance, geometry and render proofs are reused.
"""
from pathlib import Path
import json,copy,re
renderer_boundary=__doc__
renderer_mesh_path=Path(__file__).resolve().parent/'Record-MeshTransform.py'
renderer_mesh_source=renderer_mesh_path.read_text()
exec(compile(renderer_mesh_source.split("\nfixtures=json.loads((root/'analysis/reports/mesh-transform.input.json')",1)[0],str(renderer_mesh_path),'exec'))
__doc__=renderer_boundary
for name in ['renderer-set-transform.asm','renderer-transform-helper.asm']:
 asm=(root/'analysis/decompiled'/name).read_text()
 g['code'].update({int(a,16):(op,args.split(',') if args else []) for a,op,args in re.findall(r'^([0-9a-f]{8}) ([A-Z0-9.]+)(?: (.*))?$',asm,re.M)})
addresses=sorted(g['code']);g['following']={a:addresses[i+1] for i,a in enumerate(addresses[:-1])}
assert g['pe_word'](0x1006f8d8+0x48)==0x1001fc70
assert g['pe_word'](0x100802a8)==0x100316f6 and g['pe_word'](0x10080348+0x48)==0x10031709
counts['renderer_vtable_dispatch_regressions']=3

def initialize_scene(vm,scene):
 put(vm,R+0x9c0c,STATE)
 for slot,words in enumerate(scene['matrices']):
  for k,w in enumerate(words):put(vm,STATE+0x2c+slot*64+k*4,w)
 for k,w in enumerate(scene['inverse']):put(vm,BASE+0x5e4+k*4,w)

def renderer_scene(vm):
 return dict(matrices=[[get(vm,STATE+0x2c+s*64+k*4) for k in range(16)] for s in range(3)],inverse=[get(vm,BASE+0x5e4+k*4) for k in range(16)])

def renderer_set(vm,slot,words,inverse):
 start=vm.steps;source=0x78000000 if words is not None else 0
 if words is not None:
  for k,w in enumerate(words):put(vm,source+k*4,w)
 vm.r.update(ECX=R,ESP=0x70068000)
 vm.push(source);vm.push(slot);vm.push(0x6f000000)
 if slot==0:
  vm.run(0x1001fc70,0x1001fcd7)
  assert vm.pop()==BASE+0x5e4 and vm.pop()==0 and vm.pop()==source
  assert [get(vm,source+k*4) for k in range(16)]==words
  assert inverse['kind']!='unresolved';vm.steps+=2 # original CALL and thunk JMP
  if inverse['kind']=='written':
   for k,w in enumerate(inverse['words']):put(vm,BASE+0x5e4+k*4,w)
  vm.r['EAX']=BASE+0x5e4 if inverse['kind']=='written' else 0
  vm.run(0x1001fcdc,0x1001fcdf);counts['inverse_boundaries']+=1
 else:vm.run(0x1001fc70,0x1001fd7c)
 assert vm.pop()==0x6f000000;vm.r['ESP']+=8;vm.steps+=1;assert vm.r['ESP']==0x70068000
 counts['renderer_set_calls']+=1;counts['renderer_set_instructions']+=vm.steps-start
 return dict(slot=slot,copied=slot<=2,inverse_call=dict(matrix=words,determinant_pointer=0) if slot==0 else None)

renderer_flush_path=root/'scripts/Record-D3DTransforms.py'
renderer_flush_source=renderer_flush_path.read_text()
exec(compile('def flush(vm,capacity,gate):'+renderer_flush_source.split('def flush(vm,capacity,gate):',1)[1].split('\ncounts=',1)[0],str(renderer_flush_path)+'::transform-flush','exec'))

fixtures=json.loads((root/'analysis/reports/renderer-transform.input.json').read_text());path=root/'analysis/reports/renderer-transform.json';report=json.loads(path.read_text())
assert path.read_bytes()==(root/'analysis/reports/renderer-transform-release.json').read_bytes()
assert fixtures['source_sha256']==report['source_sha256']==sha(root/'analysis/reports/mesh-transform.json')==json.loads((root/'analysis/reports/mesh-transform-validation.json').read_text())['report_sha256']
for c,actual in zip(fixtures['setters'],report['setters'],strict=True):
 d=c['deferred'];vm=g['initialize'](dict(cache=d['states'],**{'pass':[0]*28},hardware=True,lod_bias=0));initialize_scene(vm,c['scene'])
 for setter_matrix_slot,words in enumerate(d['transforms']['words']):
  for k,w in enumerate(words):put(vm,BASE+0x324+setter_matrix_slot*64+k*4,w)
 put(vm,BASE+0x624,d['transforms']['mask'])
 def deferred():return dict(states=g['state'](vm),transforms=dict(words=[[get(vm,BASE+0x324+s*64+k*4) for k in range(16)] for s in range(11)],mask=get(vm,BASE+0x624)),bindings=d['bindings'])
 for q,step in zip(c['requests'],actual['steps'],strict=True):
  if 'Err' in step['result']:
   assert q['slot']<=2 and (q['matrix'] is None or (q['slot']==0 and q['inverse']['kind']=='unresolved'));counts['setter_safe_errors']+=1
  else:
   value=renderer_set(vm,q['slot'],q['matrix'],q['inverse']);assert step['result']==dict(Ok=value)
  assert step['scene']==renderer_scene(vm) and step['deferred']==deferred(),(c['id'],q['slot'],'setter state')
 plan=flush(vm,c['capacity'],c['stencil_gate']);repeated=flush(vm,c['capacity'],c['stencil_gate'])
 assert actual['plan']==dict(Ok=plan) and actual['repeated']==dict(Ok=repeated)
 assert actual['scene']==renderer_scene(vm) and actual['deferred']==deferred()
 counts['setter_cases']+=1;counts['setter_flush_calls']+=len(plan['transforms'])

renderer_original_model_handoff=model_handoff
def model_handoff(vm,q,instance=0x76000000,renderer=0x21000000):
 global renderer_set_output
 value,model=renderer_original_model_handoff(vm,q,instance,renderer)
 assert renderer==R
 renderer_set_output=renderer_set(vm,0,[w for row in model['matrix'] for w in row],renderer_active_inverse)
 return value,model

# Recreate the previously verified joined loop without executing its standalone
# math probes. Only runtime/scene envelope and the real +48 body are inserted.
renderer_loop_source='transform_loop=render_loop'+renderer_mesh_source.split('\ntransform_loop=render_loop',1)[1].split("exec(compile(transform_loop",1)[0]
exec(compile(renderer_loop_source,str(renderer_mesh_path)+'::loop-definition','exec'))
renderer_loop=transform_loop
renderer_loop=renderer_loop.replace("rt=c['runtime'];oldw=", "renderer_initial=c['runtime'];rt=renderer_initial['runtime'];oldw=")
renderer_loop=renderer_loop.replace("vm,old=p['initial'](oldw);", "vm,old=p['initial'](oldw);initialize_scene(vm,renderer_initial['scene']);")
renderer_loop=renderer_loop.replace("outer=copy.deepcopy(q);start=vm.steps", "renderer_request=copy.deepcopy(q);outer=renderer_request['mesh'];renderer_active_inverse=renderer_request['inverse'];start=vm.steps")
renderer_loop=renderer_loop.replace("value=dict(transform=transformed,handoff=model,render=value)", "value=dict(setter=renderer_set_output,mesh=dict(transform=transformed,handoff=model,render=value))")
renderer_loop=renderer_loop.replace("assert step['request']==outer,", "assert step['request']==renderer_request,")
renderer_loop=renderer_loop.replace("observed=runtime(vm,oldw,old)", "observed=dict(scene=renderer_scene(vm),runtime=runtime(vm,oldw,old))")
renderer_loop=renderer_loop.replace("actual['runtime']==runtime(vm,oldw,old)", "actual['runtime']==dict(scene=renderer_scene(vm),runtime=runtime(vm,oldw,old))")
renderer_loop=renderer_loop.replace("counts['safe_errors']==11", "counts['safe_errors']==12")
exec(compile(renderer_loop,str(__file__)+'::persistent-renderer-native-loop','exec'))
tests=sum(map(int,re.findall(r'test result: ok\. (\d+) passed',(root/'analysis/reports/renderer-transform-tests.log').read_text())));assert tests==920,tests
sources=set(json.loads((root/'analysis/reports/mesh-transform-validation.json').read_text())['source_sha256'])
sources.update(['crates/rc-package/src/lib.rs','crates/rc-package/src/renderer_transform.rs','crates/rc-inspect/src/bin/rc-renderer-transform-check.rs','scripts/Generate-RendererTransform.py','scripts/Record-RendererTransform.py','scripts/Record-D3DTransforms.py','scripts/ghidra/DefineFunctions.java','analysis/decompiled/renderer-set-transform.asm','analysis/decompiled/renderer-set-transform.c','analysis/decompiled/renderer-transform-helper.asm','analysis/decompiled/renderer-transform-helper.c','analysis/decompiled/renderer-transform-resolver.asm','analysis/decompiled/renderer-transform-resolver.c','analysis/decompiled/renderer-inverse-scalar.asm','analysis/decompiled/renderer-inverse-scalar.c','docs/RENDERER_TRANSFORM.md','analysis/finalize-renderer-transform.py'])
validation=dict(date='2026-10-10',counts=dict(counts),rust_tests=tests,debug_release_identical=True,original_core_sha256=sha(core),original_engine_sha256=sha(draw['engine']),original_d3ddrv_sha256=sha(g['dll']),input_sha256=sha(root/'analysis/reports/renderer-transform.input.json'),report_sha256=sha(path),source_sha256={a:sha(root/a) for a in sorted(sources)},scope=report['scope'],replay_boundary=renderer_boundary,android='deferred until end per user')
(root/'analysis/reports/renderer-transform-validation.json').write_text(json.dumps(validation,indent=2)+'\n');print(json.dumps(dict(counts=dict(counts),rust_tests=tests)))
