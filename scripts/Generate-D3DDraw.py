from pathlib import Path
import json,hashlib,copy
root=Path(__file__).resolve().parents[1];sha=lambda p:hashlib.sha256(p.read_bytes()).hexdigest()
source=root/'analysis/reports/d3d-complete.input.json';v=json.loads((root/'analysis/reports/d3d-complete-validation.json').read_text());assert sha(source)==v['input_sha256']
prior=json.loads(source.read_text())['cases'];mesh=root/'analysis/reports/skeletal-draw.json';mv=json.loads((root/'analysis/reports/skeletal-draw-validation.json').read_text());assert sha(mesh)==mv['report_sha256']
obj=next(o for o in json.loads(mesh.read_text())['objects'] if o['object']=='CloneCommando');sections=obj['lods'][0]['sections'];assert sum(s['triangle_count'] for s in sections)==3500
draws=[]
for kind in [0,1,2,3,4,5,6,7,8,0xffffffff]:
 for indexed in [False,True]:
  for count in [0,1,2,7,0xffffffff,0x80000000]:
   for lo,hi in [(0,0),(3,11),(9,2),(0xffffffff,0xffffffff)]:
    i=len(draws);draws.append(dict(id=i,draw=dict(primitive=kind,start=(i*73)&0xffffffff,primitive_count=count,min_vertex=lo,max_vertex=hi,indexed=indexed),counters=dict(draw_passes=(0xfffffff0+i)&0xffffffff,submitted_primitives=(0xfffffff0+i*3)&0xffffffff,submitted_vertices=(0xfffffff0+i*7)&0xffffffff)))
fog=[]
for flags,enabled in [(f,e) for f in range(256) for e in [0,255]]+[(f,e) for f in [0,0x40,0xc0] for e in [0,1,128,255]]:
 i=len(fog);p=copy.deepcopy(prior[i%len(prior)]);p['pass']['header'][4]=flags
 fog.append(dict(id=i,cache=p['complete']['deferred']['states'],pass_=dict(pass_=p['pass'],resources=p['resources'],shader=p['choice'],fog_color=(i*0x10203)^0x7f123456),enabled=enabled,restore=(i*0x20304)^0xfedcba98))
for c in fog:c['pass']=c.pop('pass_');c['pass']['pass']=c['pass'].pop('pass_')
rows=[]
for i in range(195):
 sample=copy.deepcopy(prior[(i*7)%len(prior)]);passes=[]
 for j in range((i%9) if i<192 else 2):
  if j%3==1:passes.append(copy.deepcopy(passes[-1]));continue
  q=copy.deepcopy(prior[(i*7+j*13)%len(prior)]);q['pass']['address']=0x26000000+j*0x1000
  q['pass']['header'][4]=(q['pass']['header'][4]&~0x40)|(0x40 if (i+j)%3 else 0)
  passes.append(dict(pass_=q['pass'],resources=q['resources'],shader=q['choice'],fog_color=((i*31+j*17)*0x10203)^0xab123456))
 for p in passes:
  if 'pass_' in p:p['pass']=p.pop('pass_')
 d=copy.deepcopy(draws[(i*11)%len(draws)]['draw']);section=None
 if i>=192:
  section=i-192;s=sections[section];d=dict(primitive=5,start=s['first_index'],primitive_count=s['triangle_count'],min_vertex=s['min_vertex'],max_vertex=s['max_vertex'],indexed=True)
 context=dict(device=sample['device'],stream_capacity=sample['stream_capacity'],light_capacity=sample['light_capacity'],stencil_gate=sample['stencil_gate'],fog_enabled=[0,1,128,255][i%4],restore_fog=(i*103)^0x9abcdef0)
 rows.append(dict(id=i,complete=sample['complete'],last_pass=(passes[0]['pass']['address'] if passes and i%7==0 else 0),counters=draws[i]['counters'],passes=passes,context=context,draw=d,section=section))
(root/'analysis/reports/d3d-draw.input.json').write_text(json.dumps(dict(source_sha256=sha(source),mesh_sha256=sha(mesh),mesh_sections=sections,draws=draws,fog=fog,cases=rows),separators=(',',':'))+'\n')
print(json.dumps(dict(draws=len(draws),fog=len(fog),sequences=len(rows),sections=len(sections))))
