"""Camera LOD/gate/GetFrame fixtures connected to checked index geometry."""
from pathlib import Path
import json,copy,hashlib,struct
root=Path(__file__).resolve().parents[1]
def sha(p):return hashlib.sha256(p.read_bytes()).hexdigest()
def f(v):return struct.unpack('<I',struct.pack('<f',v))[0]
prior=root/'analysis/reports/vertmesh-indices.input.json';passes=root/'analysis/reports/d3d-draw.input.json'
for p,name in [(prior,'vertmesh-indices'),(passes,'d3d-draw')]:assert sha(p)==json.loads((root/('analysis/reports/'+name+'-validation.json')).read_text())['input_sha256']
old=json.loads(prior.read_text());cases=[]
for i,c in enumerate(old['cases']):
 requests=[]
 for j,q in enumerate(c['requests']):
  q=copy.deepcopy(q)
  camera=dict(vertex_count=20,viewport_width=[320,640,800,1024][i%4],minimum=[1,2,3,4,0,-2][i%6],mesh_lod_scale=f([0.5,1,2][i%3]),mesh_distance_scale=f(1),distance_bias=f([0,2,-1][i%3]),projected_w=f([0,1,2,4,16][i%5]),tan_fov=f([0.25,1,2][i%3]),actor_lod_bias=f([0.00001,0.0001,0.001,0.01,0.1,-0.001][i%6]),actor_draw_scale=f([1,-2,-0.0,3][i%4]),actor_abs_max=f([0.5,1,2][i%3]))
  mode=(i+j)%4
  gates=dict(packed_sections=0xe0000000 if i%10==0 else q['geometry']['packed_count'],actor_state=1 if mode==0 else 0,owner_state=1 if mode==1 else (0 if mode==2 else None),fallback_state=0x80000000 if mode==2 else 0)
  vertex=dict(instance=0x3f040000,scene=0x3e003000,source=0x3f050000,count_pointer=0x28000000-0x34,count_after=i*17+j+99)
  requests.append(dict(camera=camera,gates=gates,vertex=vertex,geometry=q))
 cases.append(dict(id=i,runtime=copy.deepcopy(c['runtime']),requests=requests))
errors=[]
def error(mutate):
 c=copy.deepcopy(cases[2]);c['id']=len(errors);c['requests']=c['requests'][:1];mutate(c);errors.append(c)
error(lambda c:c['requests'][0]['camera'].update(tan_fov=0))
error(lambda c:c['requests'][0]['camera'].update(projected_w=0x7fc00000))
error(lambda c:c['requests'][0]['camera'].update(mesh_distance_scale=f(3.4e38),projected_w=f(3.4e38)))
error(lambda c:c['requests'][0]['camera'].update(actor_lod_bias=f(3.4e38),actor_draw_scale=f(3.4e38)))
error(lambda c:c['requests'][0]['vertex'].update(instance=0))
error(lambda c:c['requests'][0]['vertex'].update(count_pointer=0xfffffffe))
error(lambda c:c['requests'][0]['gates'].update(packed_sections=7))
error(lambda c:c['requests'][0]['geometry']['mesh']['index']['upload']['lock'].update(unlock_hresult=0x80004005))
error(lambda c:c['requests'][0]['geometry']['mesh']['bank'].clear())
error(lambda c:c['requests'][0]['geometry']['mesh']['vertex']['source'].update(stride=0))
error(lambda c:c['requests'][0]['geometry']['geometry'].update(face_thresholds=[0]*12,collapse=list(range(8))))
error(lambda c:c['requests'][0]['geometry']['geometry']['section_bytes'].clear())
(root/'analysis/reports/vertmesh-lod.input.json').write_text(json.dumps(dict(source_sha256=sha(prior),passes_sha256=sha(passes),cases=cases,errors=errors)),newline='\n')
print(json.dumps(dict(cases=len(cases),requests=sum(len(c['requests']) for c in cases),errors=len(errors))))
