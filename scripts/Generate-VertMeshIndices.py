"""Generate indexed geometry fixtures from the checked mesh draw requests."""
from pathlib import Path
import json,copy,hashlib,struct
root=Path(__file__).resolve().parents[1]
def sha(p):return hashlib.sha256(p.read_bytes()).hexdigest()
prior=root/'analysis/reports/d3d-mesh-draw.input.json';passes=root/'analysis/reports/d3d-draw.input.json'
for p,name in [(prior,'d3d-mesh-draw'),(passes,'d3d-draw')]:assert sha(p)==json.loads((root/('analysis/reports/'+name+'-validation.json')).read_text())['input_sha256']
old=json.loads(prior.read_text());cases=[]
for i,c in enumerate(old['cases']):
 requests=[]
 for j,q in enumerate(c['requests']):
  q=copy.deepcopy(q);raw=bytearray(q['sections']['bytes']);n=len(raw)//80
  for k in range(n):struct.pack_into('<2H',raw,k*80+14,k*2,0 if (i+k)%7==0 else 2)
  lod=[1,3,5,9,0,-1][i%6]
  geometry=dict(packed_count=q['sections']['packed_count'],section_bytes=list(raw),lod=lod,face_thresholds=[(k+i)%5 if lod>0 else 1 for k in range(n*2)],faces=[[(k)%8,(k+1)%8,(k+2)%8,0xabcd] for k in range(n*2)],collapse=[0]+list(range(7)),vertex_thresholds=list(range(8)))
  q['index']['pool']['allocation']['address']=0x34000100
  # Actual raw index source is always u16; its constructor answer is for slot 0.
  requests.append(dict(geometry=geometry,mesh=q))
 rt=copy.deepcopy(c['runtime']);rt['index_target']+=( [0xa7]*max(0,256-len(rt['index_target'])) )
 cases.append(dict(id=i,runtime=rt,requests=requests))
# Keep a separate original first-index WORD truncation probe as an integrated case.
c=copy.deepcopy(cases[3]);c['id']=len(cases);c['requests']=c['requests'][:1];q=c['requests'][0];g=q['geometry'];g['packed_count']=2;g['section_bytes']=g['section_bytes'][:160]
struct.pack_into('<2H',raw:=bytearray(g['section_bytes']),14,0,21846);struct.pack_into('<2H',raw,94,0,0);g['section_bytes']=list(raw)
g.update(lod=9,face_thresholds=[0]*21846,faces=[[0,1,2,65535]]*21846)
c['runtime']['index_target']=[0xa7]*131080;cases.append(c)
errors=[]
def error(mutate):
 c=copy.deepcopy(cases[3]);c['id']=len(errors);c['requests']=c['requests'][:1];mutate(c);errors.append(c)
error(lambda c:c['requests'][0]['geometry'].update(packed_count=257))
error(lambda c:c['requests'][0]['geometry']['section_bytes'].clear())
error(lambda c:c['requests'][0]['geometry']['face_thresholds'].clear())
error(lambda c:c['requests'][0]['geometry']['faces'].clear())
error(lambda c:c['requests'][0]['geometry']['vertex_thresholds'].clear())
def cycle(c):
 g=c['requests'][0]['geometry'];g['lod']=1;g['face_thresholds']=[0]*12;g['collapse']=list(range(8))
error(cycle)
def missing_link(c):
 cycle(c);c['requests'][0]['geometry']['collapse']=[]
error(missing_link)
error(lambda c:c['requests'][0]['mesh']['index']['upload']['lock'].update(unlock_hresult=0x80004005))
error(lambda c:c['runtime']['index_target'].clear())
error(lambda c:c['requests'][0]['mesh']['bank'].clear())
error(lambda c:c['requests'][0]['mesh']['materials'].update(resolver=0))
error(lambda c:c['requests'][0]['mesh']['vertex']['source'].update(stride=0))
(root/'analysis/reports/vertmesh-indices.input.json').write_text(json.dumps(dict(source_sha256=sha(prior),passes_sha256=sha(passes),cases=cases,errors=errors)),newline='\n')
print(json.dumps(dict(cases=len(cases),requests=sum(len(c['requests']) for c in cases),errors=len(errors))))
