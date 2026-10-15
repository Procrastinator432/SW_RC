"""Synthetic rotator/MeshToWorld and joined renderer-handoff fixtures."""
from pathlib import Path
import json,copy,random,struct,hashlib
root=Path(__file__).resolve().parents[1]
def sha(p):return hashlib.sha256(p.read_bytes()).hexdigest()
def f(v):return struct.unpack('<I',struct.pack('<f',v))[0]
prior=root/'analysis/reports/vertmesh-render.json'
assert sha(prior)==json.loads((root/'analysis/reports/vertmesh-render-validation.json').read_text())['report_sha256']
old=json.loads((root/'analysis/reports/vertmesh-render.input.json').read_text());rng=random.Random(1044760)
angles=[0,1,3,4,16384,32768,49152,65536,-65536,2147483647,-2147483648,-16384]
def make(i):
 def vec():return [f(rng.choice([0.,-0.,.1,-.25,1.,-3.,123.125,-65536.,1048576.])) for _ in range(3)]
 return dict(actor_location=vec(),actor_rotation=[rng.choice(angles) if i%2 else rng.randint(-2**31,2**31-1) for _ in range(3)],actor_draw_scale=f(rng.choice([0.,-0.,1.,-1.5,1.000001])),actor_draw_scale_3d=vec(),actor_prepivot=vec(),actor_flags=[0,0x800,0xffffffff,0x400][i%4],actor_height=f(rng.choice([0.,-5.85,1.,-1.,1048576.])),mesh_scale=vec(),mesh_origin=vec(),mesh_rotation=[0,65536,-65536] if i%4==0 else [rng.choice(angles) for _ in range(3)],suppress_origin=[0,1,0xffffffff][i%3],bypass_mesh_adjustments=[0,0,1,255][i%4])
math=[make(i) for i in range(512)]
# Independent flags: cover both origin gates, both actor correction paths and bypass.
for i,q in enumerate(math):
 q['bypass_mesh_adjustments']=[0,0,0,0,1,255][i%6]
 if q['bypass_mesh_adjustments']:
  q.update(mesh_origin=[0x7fc00000]*3,actor_prepivot=[0x7fc00000]*3,actor_height=0x7fc00000)
 elif q['suppress_origin']:q['mesh_origin']=[0x7fc00000]*3
 if q['actor_flags']&0x800:q['actor_prepivot']=[0x7fc00000]*3
 else:q['actor_height']=0x7fc00000
# Exact low16 zero test versus nonzero but quantized-to-zero rotation.
math[0]['mesh_rotation']=[0,65536,-65536];math[1]['mesh_rotation']=[1,3,1]
# Exercise the extended-add/f32-store boundary across exponents and cancellation.
height_words=[f(v) for e in [-126,-100,-53,-24,-1,0,1,23,24,25,26,27,53,100,127] for v in [2.**e,-2.**e]]+[0x7f7fffff,0xff7fffff,f(-5.85),f(5.85),1,0x80000001]
height_index=0
for q in math:
 if not q['bypass_mesh_adjustments'] and q['actor_flags']&0x800:
  word=height_words[height_index] if height_index<len(height_words) else rng.getrandbits(32)&0xff7fffff
  q['actor_height']=word;height_index+=1
cases=copy.deepcopy(old['cases'])
for c in cases:
 out=[]
 for j,q in enumerate(c['requests']):
  t=make(c['id']*3+j)
  t.update(actor_location=q['actor_position'].copy(),actor_draw_scale=q['camera']['actor_draw_scale'],actor_draw_scale_3d=q['actor_draw_scale'].copy())
  # Stale legacy snapshot must be replaced even behind a disabled draw gate.
  q['actor_position']=[0x7fc00000]*3;q['actor_draw_scale']=[0x7fc00000]*3;q['camera']['actor_draw_scale']=0x7fc00000
  out.append(dict(transform=t,render=q))
 c['requests']=out
errors=[]
base=copy.deepcopy(cases[2]);enabled=copy.deepcopy(base['requests'][2]);enabled['transform'].update(bypass_mesh_adjustments=0,suppress_origin=0,actor_flags=0,actor_height=f(1))
for change in [lambda q:q['transform'].update(actor_location=[0x7fc00000]*3),lambda q:q['transform'].update(mesh_scale=[0x7f7fffff]*3,actor_draw_scale=f(100)),lambda q:q['transform'].update(mesh_origin=[0x7fc00000]*3),lambda q:q['transform'].update(actor_prepivot=[0x7fc00000]*3),lambda q:q['transform'].update(actor_flags=0x800,actor_height=0x7fc00000),lambda q:q['render']['geometry']['mesh']['materials'].update(renderer_address=0),lambda q:(q['render']['vertex'].update(instance=0),q['render']['gates'].update(actor_state=0,owner_state=None,fallback_state=0)),lambda q:q['render']['instance'].update(channels=[]),lambda q:q['render']['geometry']['mesh']['index']['upload']['lock'].update(unlock_hresult=0x80004005),lambda q:q['render']['geometry']['mesh'].update(bank=[]),lambda q:q['render']['world_to_screen'][0].__setitem__(0,0x7fc00000)]:
 q=copy.deepcopy(enabled);change(q);errors.append(dict(id=len(errors),runtime=copy.deepcopy(base['runtime']),requests=[q]))
out=dict(source_sha256=sha(prior),passes_sha256=old['passes_sha256'],math=math,cases=cases,errors=errors)
(root/'analysis/reports/mesh-transform.input.json').write_text(json.dumps(out,separators=(',',':'))+'\n')
print(json.dumps(dict(math=len(math),cases=len(cases),requests=sum(len(c['requests']) for c in cases),errors=len(errors))))
