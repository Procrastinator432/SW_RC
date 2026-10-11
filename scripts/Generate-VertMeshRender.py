"""Synthetic joined preprocessing, camera and real instance GetFrame fixtures."""
from pathlib import Path
import json,copy,struct,hashlib,random
root=Path(__file__).resolve().parents[1]
def sha(p):return hashlib.sha256(p.read_bytes()).hexdigest()
def f(v):return struct.unpack('<I',struct.pack('<f',v))[0]
prior=root/'analysis/reports/vertmesh-instance.json'
assert sha(prior)==json.loads((root/'analysis/reports/vertmesh-instance-validation.json').read_text())['report_sha256']
old=json.loads((root/'analysis/reports/vertmesh-instance.input.json').read_text());rng=random.Random(105601)
def matrix(i):
 m=[[f(int(r==c)) for c in range(4)] for r in range(4)]
 if i%3==1:m[2][2]=m[3][3]=0;m[2][3]=m[3][2]=f(1)
 if i%3==2:m[0][3]=f(.25);m[1][3]=f(.125);m[2][3]=f(.5);m[3][3]=f(2)
 return m
cases=copy.deepcopy(old['cases'])
for c in cases:
 out=[]
 for j,q in enumerate(c['requests']):
  i=c['id']*3+j;inst=q['instance'];geometry=q['geometry'];g=geometry['geometry'];vc=inst['animation']['clock']['vertex_count']
  inst['cache']['cached_mesh']=0x39000000 if inst['cache']['cached_mesh']==inst['mesh'] else 0x39001000
  inst['mesh']=0x39000000
  stream=inst['animation']['bytes'].copy();wedges=g['vertex_thresholds'].copy();faces=copy.deepcopy(g['faces'])
  p=dict(state=dict(packed_sections=0xe0000000,section_bytes=[19]*80,packed_stream_count=0xa0000005,stream=stream,revision=0xffffffff if i%2 else i,cache_index=[0xffffffff if i%2 else i,0xffffffff if i%5==0 else 7]),packed_wedges=0x60000005,wedges=[[w,f(k+.25),0x80000000 if k%2 else f(-k)] for k,w in enumerate(wedges)],packed_faces=0xa0000006,faces=faces,section_allocation=[rng.randrange(256) for _ in range(240)])
  if i%3==0:
   p['state']['packed_sections']=g['packed_count'];p['state']['section_bytes']=g['section_bytes'].copy()
   p.update(packed_wedges=0x1fffffff,wedges=[],packed_faces=65537,faces=[],section_allocation=[])
  if i%11==0:
   p['state']['packed_sections']=0xe0000000;p.update(packed_wedges=0x60000005,wedges=[[w,f(k+.25),f(-k)] for k,w in enumerate(wedges)],packed_faces=0,faces=[],section_allocation=[])
  g['face_thresholds']=[1,2,3,4,2,1];g['collapse']=[0,0,1,2,0]
  gate=dict(packed_sections=0xdeadbeef,actor_state=1 if i%4==0 else 0,owner_state=2 if i%4==1 else None,fallback_state=3 if i%4>=2 else 0)
  if i%7==0:gate.update(actor_state=0,owner_state=0 if i%2 else None,fallback_state=0)
  if i%7==0 or i%11==0:inst['channels']=[] # ignored by both early draw gates
  camera=dict(vertex_count=vc,viewport_width=640+i*7,minimum=[0,1,2,3,7][i%5],mesh_lod_scale=f(1),mesh_distance_scale=f(1.25),distance_bias=f(.5),projected_w=0x7fc00000,tan_fov=f(.75),actor_lod_bias=f([0,.00001,.0001,.001][i%4]),actor_draw_scale=f(-1.5 if i%2 else 1),actor_abs_max=0x7fc00000)
  # Caller answers/layout are stale on purpose and must be derived by the joined path.
  inst['animation'].update(requested_count=-1,mode=7,stride=1,wedges=[],bytes=[])
  out.append(dict(preprocessing=p,actor_draw_scale=[[f(-2),f(1),f(.5)],[0x80000000,0,0],[f(.5),f(-3),f(3)]][i%3],world_to_screen=matrix(i),actor_position=[f(3+i%4),f(-1),f(4+i%7)],camera=camera,gates=gate,vertex=dict(instance=0x76000000,scene=0x3e003000,source=0x72000000,count_pointer=0x28000000-0x34,count_after=0xffffffff),instance=inst,geometry=geometry))
 c['requests']=out
errors=[]
for change in [lambda q:q['preprocessing'].update(packed_wedges=0x1fffffff),lambda q:q['actor_draw_scale'].__setitem__(0,0x7fc00000),lambda q:q['world_to_screen'][0].__setitem__(0,0x7fc00000),lambda q:q['camera'].update(tan_fov=0),lambda q:q['vertex'].update(instance=0),lambda q:q['instance'].update(channels=[]),lambda q:q['instance']['animation']['clock'].update(vertex_count=5),lambda q:q['preprocessing']['state'].update(packed_stream_count=0xa0000004),lambda q:q['geometry']['mesh']['index']['upload']['lock'].update(unlock_hresult=0x80004005),lambda q:q['geometry']['mesh'].update(bank=[]),lambda q:q['geometry']['geometry'].update(face_thresholds=[]),lambda q:q['preprocessing'].update(section_allocation=[])]:
 # i=8: enabled, preprocessing active, editor refresh and cache state both pending.
 base=copy.deepcopy(cases[2]);q=base['requests'][2 if len(base['requests'])>2 else 0]
 # Stream shape error needs the skipped-preprocessing path to retain supplied count.
 if len(errors)==7:
  q=copy.deepcopy(cases[1]['requests'][0]);q['gates'].update(actor_state=1,owner_state=None,fallback_state=0)
 change(q);errors.append(dict(id=len(errors),runtime=copy.deepcopy(base['runtime'] if len(errors)!=7 else cases[1]['runtime']),requests=[q]))
out=dict(source_sha256=sha(prior),passes_sha256=old['passes_sha256'],cases=cases,errors=errors)
(root/'analysis/reports/vertmesh-render.input.json').write_text(json.dumps(out,separators=(',',':'))+'\n')
print(json.dumps(dict(cases=len(cases),requests=sum(len(c['requests']) for c in cases),errors=len(errors))))
