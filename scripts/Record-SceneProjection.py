"""Replay both original scene math bodies and the Project call within mesh LOD.

Matrices are synthetic supplied snapshots. Their construction/inversion, x87
tangent/GetAbsMax, vertex animation, material/allocator/COM bodies remain external.
Default finite IEEE/SSE contract; safe error behavior is Rust-only, not native.
No live game or GPU/Android execution. Existing checked downstream replay reused.
"""
from pathlib import Path
import json,re,copy
boundary=__doc__
shared=Path(__file__).resolve().parent/'Record-VertMeshLod.py'
src=shared.read_text()
prefix,loop=src.split('\nfor c,actual in zip',1)
# Replace only the old Project answer boundary; preserve the surrounding camera code.
old="put(vm,EF-0x18,c['tan_fov']);assert vm.pop()==EF-0x48 and vm.pop()==ACTOR+0x138\n vm.steps+=1;vm.r['EAX']=PROJ"
new="put(vm,EF-0x18,c['tan_fov']);assert get(vm,vm.r['ESP'])==EF-0x48 and get(vm,vm.r['ESP']+4)==ACTOR+0x138\n project_call(vm,SCENE,EF-0x48,ACTOR+0x138)"
assert prefix.count(old)==1
exec(compile(prefix.replace(old,new),str(shared),'exec'))
__doc__=boundary
for name in ['scene-project.asm','scene-deproject.asm']:
 assemblies.append(name)
 asm=(root/'analysis/decompiled'/name).read_text()
 g['code'].update({int(a,16):(op,args.split(',') if args else []) for a,op,args in re.findall(r'^([0-9a-f]{8}) ([A-Z0-9.]+)(?: (.*))?$',asm,re.M)})
addresses=sorted(g['code']);g['following']={a:addresses[i+1] for i,a in enumerate(addresses[:-1])}
fixtures=json.loads((root/'analysis/reports/scene-projection.input.json').read_text())
path=root/'analysis/reports/scene-projection.json';report=json.loads(path.read_text())
assert path.read_bytes()==(root/'analysis/reports/scene-projection-release.json').read_bytes()
assert fixtures['source_sha256']==report['source_sha256']==sha(root/'analysis/reports/vertmesh-lod.input.json')
assert fixtures['passes_sha256']==report['passes_sha256']==sha(root/'analysis/reports/d3d-draw.input.json')

def matrix(vm,scene,offset,m):
 for r,row in enumerate(m):
  for c,v in enumerate(row):put(vm,scene+offset+r*16+c*4,v)

def project_call(vm,scene,out,position):
 assert vm.r['ECX']==scene
 assert get(vm,vm.r['ESP'])==out and get(vm,vm.r['ESP']+4)==position
 vm.push(0x105601fa);vm.steps+=1 # original direct CALL
 vm.run(0x104a3860,0x104a398d)
 assert vm.r['EAX']==out and vm.pop()==0x105601fa
 vm.r['ESP']+=8;vm.steps+=1 # original RET 8
 counts['project_calls']+=1

for c,actual in zip(fixtures['math'],report['math'],strict=True):
 vm=g['Native']();SCENE=0x6d000000;POS=0x6e000000;OUT=0x6f000000
 matrix(vm,SCENE,0x114,c['matrix']);matrix(vm,SCENE,0x154,c['inverse'])
 put(vm,0x10653578,engine_word(0x10653578))
 if c['project_ok']:
  for i,v in enumerate(c['position']):put(vm,POS+4*i,v)
  vm.r.update(ECX=SCENE,ESP=0x70080000);vm.push(POS);vm.push(OUT)
  project_call(vm,SCENE,OUT,POS)
  assert vm.r['ESP']==0x70080000
  assert actual['projection']==dict(Ok=[get(vm,OUT+4*i) for i in range(4)])
 else:assert 'Err' in actual['projection'];counts['math_safe_errors']+=1
 if c['deproject_ok']:
  for i,v in enumerate(c['plane']):put(vm,POS+4*i,v)
  vm.r.update(ECX=SCENE,ESP=0x70080000);vm.push(POS);vm.push(OUT);vm.push(0x12345678)
  vm.run(0x104a3990,0x104a3a9a)
  assert vm.r['EAX']==OUT and vm.pop()==0x12345678
  vm.r['ESP']+=8;vm.steps+=1
  assert vm.r['ESP']==0x70080000
  assert actual['deprojection']==dict(Ok=[get(vm,OUT+4*i) for i in range(3)])
  counts['deproject_calls']+=1
 else:assert 'Err' in actual['deprojection'];counts['math_safe_errors']+=1
 counts['math_instructions']+=vm.steps;counts['math_probes']+=1

# Reuse the exact downstream comparison loop without running its historic writer.
loop='\nfor c,actual in zip'+loop.split('\ntests=sum(',1)[0]
old="q=copy.deepcopy(q);start=vm.steps;lod,gate,vertex_call=camera(vm,q)"
new="""q=copy.deepcopy(q);whole=q;q=q['mesh'];start=vm.steps
  matrix(vm,q['vertex']['scene'],0x114,whole['world_to_screen'])
  actor=q['geometry']['mesh']['materials']['actor_address']
  for k,v in enumerate(whole['actor_position']):put(vm,actor+0x138+4*k,v)
  lod,gate,vertex_call=camera(vm,q)
  projected=[get(vm,EF-0x48+4*k) for k in range(4)]
  q['camera']['projected_w']=projected[3]"""
assert loop.count(old)==1;loop=loop.replace(old,new)
old="  assert step['result']==dict(Ok=value)"
assert loop.count(old)==1
loop=loop.replace(old,"  value=dict(projection=projected,mesh=value);whole['mesh']=q;q=whole\n"+old)
loop=loop.replace("counts['safe_errors']==12","counts['safe_errors']==16")
exec(compile(loop,str(shared)+'::projected-loop','exec'))
tests=sum(map(int,re.findall(r'test result: ok\. (\d+) passed',(root/'analysis/reports/scene-projection-tests.log').read_text())));assert tests==865,tests
sources=set(json.loads((root/'analysis/reports/vertmesh-lod-validation.json').read_text())['source_sha256'])
sources.update(['crates/rc-package/src/lib.rs','crates/rc-package/src/scene_projection.rs','crates/rc-inspect/src/bin/rc-scene-projection-check.rs','scripts/Generate-SceneProjection.py','scripts/Record-SceneProjection.py','analysis/decompiled/scene-project.asm','analysis/decompiled/scene-deproject.asm','analysis/decompiled/scene-project.c','analysis/decompiled/scene-deproject.c','docs/SCENE_PROJECTION.md','analysis/finalize-scene-projection.py'])
validation=dict(date='2026-10-10',counts=dict(counts),rust_tests=tests,debug_release_identical=True,original_engine_sha256=sha(draw['engine']),original_d3ddrv_sha256=sha(g['dll']),input_sha256=sha(root/'analysis/reports/scene-projection.input.json'),report_sha256=sha(path),source_sha256={a:sha(root/a) for a in sorted(sources)},scope=report['scope'],replay_boundary=boundary,android='deferred until end per user')
(root/'analysis/reports/scene-projection-validation.json').write_text(json.dumps(validation,indent=2)+'\n')
print(json.dumps(dict(counts=dict(counts),rust_tests=tests)))
