"""Original GetFrame scheduler and packed bank decode feeding whole runtime replay.
GetSequence return and cache storage supplied. Original time/product/MXCSR/floor/
remainder/address instructions execute, followed by original frame decode/output,
geometry, upload and draw bodies. Guard failures checked only in safe Rust port.
No sequence lookup, cache allocator, live clip, full camera chain or GPU/Android.
"""
from pathlib import Path
import json,copy,re
animation_boundary=__doc__
animation_frame_path=Path(__file__).resolve().parent/'Record-VertMeshFrame.py'
animation_frame_source=animation_frame_path.read_text()
exec(compile(animation_frame_source.split('\nfor i,(q,value)',1)[0],str(animation_frame_path),'exec'))
__doc__=animation_boundary
fixtures=json.loads((root/'analysis/reports/vertmesh-animation.input.json').read_text())
path=root/'analysis/reports/vertmesh-animation.json';report=json.loads(path.read_text())
assert path.read_bytes()==(root/'analysis/reports/vertmesh-animation-release.json').read_bytes()
assert fixtures['source_sha256']==report['source_sha256']==sha(root/'analysis/reports/vertmesh-frame.json')==json.loads((root/'analysis/reports/vertmesh-frame-validation.json').read_text())['report_sha256']

def schedule(vm,q):
 start=vm.steps;INSTANCE=0x76000000;MESH=0x77000000;SEQ=0x7b000000;CHANNEL=0x7c000000;FB=0x28010000
 vm.r.update(EBX=INSTANCE,EBP=FB,ESI=MESH,EDI=CHANNEL,EDX=SEQ if q['sequence'] is not None else 0)
 put(vm,MESH+0x5c,q['vertex_count']);put(vm,CHANNEL+0x1c,q['time'])
 if q['sequence'] is not None:
  put(vm,SEQ+0xc,q['sequence']['start']);put(vm,SEQ+0x10,q['sequence']['count'])
  vm.run(0x1055f217,0x1055f28f)
  assert vm.mxcsr==0x1f80
  result=dict(first_offset=vm.r['ECX'],next_offset=vm.r['EDX'],fraction=vm.xmm['XMM0'],interpolated=vm.c)
  counts['mxcsr_floor_restores']+=1
 else:
  vm.run(0x1055f217,0x1055f295)
  result=dict(first_offset=vm.r['ECX'],next_offset=0,fraction=0,interpolated=False)
 counts['scheduler_calls']+=1;counts['scheduler_instructions']+=vm.steps-start
 return result

# Execute the same proven original decode/output body with original computed offsets
# into full packed banks, rather than copying selected frames to artificial base zero.
animation_decode_source=animation_frame_source.split('def frame(vm,q):',1)[1].split('\ndef difference(',1)[0]
animation_decode_source='def animated_frame(vm,animation_input):\n q=dict(animation_input,vertex_count=animation_input["clock"]["vertex_count"])\n'+animation_decode_source
lo=animation_decode_source.index(" for source,delta in")
hi=animation_decode_source.index(" for i,v in enumerate(q['wedges'])",lo)
animation_decode_source=animation_decode_source[:lo]+" for key,address in [('positions',PACKED),('normals',NORMAL)]:\n  for i,v in enumerate(animation_input[key]):put(vm,address+4*i,v)\n"+animation_decode_source[hi:]
lo=animation_decode_source.index(" vm.r['EAX']=0x7b000000")
hi=animation_decode_source.index(' if interpolated:',lo)
animation_decode_source=animation_decode_source[:lo]+" vm.r['EAX']=0x7b000000 if animation_input['clock']['sequence'] is not None else 0\n vm.run(0x1055f20d,0x1055f217)\n selection=schedule(vm,animation_input['clock']);interpolated=selection['interpolated'];vm.steps+=1\n"+animation_decode_source[hi:]
animation_decode_source=animation_decode_source.replace(' return result',' return dict(selection=selection,frame=result)')
exec(compile(animation_decode_source,str(__file__)+'::native-bank-decode','exec'))

for i,(q,value) in enumerate(zip(fixtures['clocks'],report['clocks'],strict=True)):
 observed=schedule(g['Native'](),q);assert value==dict(Ok=observed),(i,difference(value,dict(Ok=observed)))
 counts['clock_probes']+=1
for i,(q,value) in enumerate(zip(fixtures['frames'],report['frames'],strict=True)):
 observed=animated_frame(g['Native'](),q);assert value==dict(Ok=observed),(i,difference(value,dict(Ok=observed)))
 counts['animation_probes']+=1
for key in ['clock_errors','frame_errors']:
 assert len(fixtures[key])==len(report[key]) and all('Err' in v for v in report[key]);counts[key]=len(report[key])

animation_loop='\nfor c,actual in zip'+frame_indices_source.split('\nfor c,actual in zip',1)[1].split('\ntests=sum(',1)[0]
old="q=copy.deepcopy(q);start=vm.steps;generated=geometry(vm,q['geometry']);qm=q['mesh'];qm['sections']=generated['sections']"
new="""q=copy.deepcopy(q);outer=q;start=vm.steps;animated=animated_frame(vm,outer['animation']);decoded=animated['frame']
  outer['animation']['cache']=copy.deepcopy(decoded['cache']);outer['animation']['bytes']=decoded['bytes'].copy();q=outer['geometry']
  source=q['mesh']['vertex']['source'];source.update(stride=32,size_before=len(decoded['bytes']),size_again=len(decoded['bytes']),payload=decoded['bytes'].copy())
  generated=geometry(vm,q['geometry']);qm=q['mesh'];qm['sections']=generated['sections']"""
assert animation_loop.count(old)==1;animation_loop=animation_loop.replace(old,new)
old="  assert step['result']==dict(Ok=value)"
assert animation_loop.count(old)==1;animation_loop=animation_loop.replace(old,"  value=dict(animation=animated,draw=value);outer['geometry']=q;q=outer\n"+old)
animation_loop=animation_loop.replace("counts['safe_errors']==12","counts['safe_errors']==8")
exec(compile(animation_loop,str(__file__)+'::composed-loop','exec'))
tests=sum(map(int,re.findall(r'test result: ok\. (\d+) passed',(root/'analysis/reports/vertmesh-animation-tests.log').read_text())));assert tests==908,tests
previous=json.loads((root/'analysis/reports/vertmesh-animation-previous-replay.json').read_text())
assert previous['previous_frame_replay_passed'] and previous['counts']['standalone_frames']==265 and previous['counts']['requests']==146
sources=set(json.loads((root/'analysis/reports/vertmesh-frame-validation.json').read_text())['source_sha256'])
sources.update(['crates/rc-package/src/vertmesh_animation.rs','crates/rc-package/src/lib.rs','crates/rc-inspect/src/bin/rc-vertmesh-animation-check.rs','scripts/Generate-VertMeshAnimation.py','scripts/Record-VertMeshAnimation.py','scripts/Replay-VertMeshFrameRegression.py','docs/VERTMESH_ANIMATION.md','analysis/finalize-vertmesh-animation.py'])
validation=dict(date='2026-10-10',counts=dict(counts),rust_tests=tests,debug_release_identical=True,previous_frame_replay=previous,previous_frame_log_sha256=sha(root/'analysis/reports/vertmesh-animation-previous-replay.json'),original_engine_sha256=sha(draw['engine']),original_d3ddrv_sha256=sha(g['dll']),input_sha256=sha(root/'analysis/reports/vertmesh-animation.input.json'),report_sha256=sha(path),source_sha256={a:sha(root/a) for a in sorted(sources)},scope=report['scope'],replay_boundary=animation_boundary,android='deferred until end per user')
(root/'analysis/reports/vertmesh-animation-validation.json').write_text(json.dumps(validation,indent=2)+'\n');print(json.dumps(dict(counts=dict(counts),rust_tests=tests)))
