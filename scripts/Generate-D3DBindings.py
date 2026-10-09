from pathlib import Path
import json,hashlib,copy
root=Path(__file__).resolve().parents[1]
sha=lambda p:hashlib.sha256(p.read_bytes()).hexdigest()
source=root/'analysis/reports/d3d-transforms.json';inputs=root/'analysis/reports/d3d-transforms.input.json'
v=json.loads((root/'analysis/reports/d3d-transforms-validation.json').read_text())
assert sha(source)==v['report_sha256'] and sha(inputs)==v['input_sha256']
prior=json.loads(source.read_text())['probes'];old=json.loads(inputs.read_text())['cases']
caps=[-2147483648,-1,0]+list(range(1,17))+[17,32,2147483647]
def bindings(i):
 applied=dict(vertex_shader=2,pixel_shader=0x12345678,streams=[[0x30000000+s*64,12+s*4] for s in range(16)],indices=[0x40000000,23])
 desired=copy.deepcopy(applied)
 if i&1:desired['vertex_shader']=[0,0x11223344,0xdeadbeef][(i//8)%3]
 if i&2:desired['pixel_shader']=[0,0x87654321][(i//8)%2]
 for s in range(16):
  mode=(i+s)%4
  if mode&1:desired['streams'][s][0]=0 if s%2 else 0x50000000+s*128
  if mode&2:desired['streams'][s][1]=0 if s%3 else 0xffffffff
 if i&4:desired['indices'][0]=0
 if i&1:desired['indices'][1]=0xffffffff
 return dict(desired=desired,applied=applied)
tails=[]
for cap in caps:
 for gate in [0,4,8,12]:
  for pattern in range(8):
   i=len(tails);tails.append(dict(id=i,dirty=gate,stream_capacity=cap,bindings=bindings(i)))
rows=[]
for i in range(len(prior)+128):
 q=prior[i%len(prior)];c=old[i%len(old)];states=copy.deepcopy(q['translated'])
 states['dirty']=(states['dirty']|[0,4,8,12][i%4]) if i<len(prior) else i-len(prior)
 rows.append(dict(id=i,material_id=q['material_id'] if i<len(prior) else None,deferred=dict(states=states,transforms=q['translated_matrices'],bindings=bindings(i)),texture_capacity=c['device']['capacity'],stream_capacity=caps[i%len(caps)],stencil_gate=c['stencil_gate']))
(root/'analysis/reports/d3d-bindings.input.json').write_text(json.dumps(dict(source_sha256=sha(source),cases=rows,tails=tails),separators=(',',':'))+'\n')
print(json.dumps(dict(tails=len(tails),combined=len(rows))))
