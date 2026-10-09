from pathlib import Path
import json,hashlib,copy
root=Path(__file__).resolve().parents[1]
sha=lambda p:hashlib.sha256(p.read_bytes()).hexdigest()
source=root/'analysis/reports/d3d-state.input.json'
validation=json.loads((root/'analysis/reports/d3d-state-validation.json').read_text())
assert sha(source)==validation['input_sha256']
prior=json.loads(source.read_text())['cases'];rows=[]
for active in range(9):
 for capacity in range(9):
  for hardware in [False,True]:
   i=len(rows);sample=copy.deepcopy(prior[i]);stages=[];resources=[]
   for s in range(8):
    q=copy.deepcopy(prior[(i*7+s*13)%len(prior)])
    stages.append(q['stage']);resources.append(q['resource'])
    # Explicit independent resource fixture addresses, no aliasing between stages.
    if stages[-1][0]:stages[-1][0]=0x31000000+s*0x100
   header=sample['pass'];header[9]=active
   p={'address':0x26000000,'header':header,'color_write':[0,1,15,0xdeadbeef][i%4],'stages':stages}
   rows.append(dict(id=i,material_id=None,cache=sample['cache'],pass_=p,resources=resources,device=dict(cull_mode=sample['cull_mode'],lod_bias=sample['lod_bias'],capacity=capacity),hardware=hardware,last_pass=p['address'] if i%11==0 else 0,stencil_gate=sample['stencil_gate']))
for sample in prior:
 if sample['material_id'] is None or sample['pass'][9]>8:continue
 c=copy.deepcopy(rows[len(rows)%162]);c['id']=len(rows);c['material_id']=sample['material_id']
 c['pass_']['header']=sample['pass'];c['cache']=sample['cache'];c['hardware']=True;c['last_pass']=0
 rows.append(c)
assert len(rows)==226
for c in rows:c['pass']=c.pop('pass_')
(root/'analysis/reports/d3d-pass.input.json').write_text(json.dumps({'source_sha256':sha(source),'cases':rows},separators=(',',':'))+'\n')
print(len(rows))
