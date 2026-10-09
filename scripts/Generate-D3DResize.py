from pathlib import Path
import json,hashlib,copy
root=Path(__file__).resolve().parents[1];sha=lambda p:hashlib.sha256(p.read_bytes()).hexdigest()
source=root/'analysis/reports/d3d-dynamic.input.json';assert sha(source)==json.loads((root/'analysis/reports/d3d-dynamic-validation.json').read_text())['input_sha256']
prior=json.loads(source.read_text())['cases']
def answers(seed,kind,skip):
 fail=dict(hresult=0x80004005,handle=0);creates=[];failures=min(seed%3,1) if skip else seed%3
 for i in range(failures+1):
  if kind=='vertex':creates += [dict(hresult=0,handle=0x33100000+i*0x10000),copy.deepcopy(fail) if i<failures else dict(hresult=0,handle=0x33200000+i*0x10000)]
  else:creates += [copy.deepcopy(fail) if i<failures else dict(hresult=0,handle=0x33300000+i*0x10000)]
 return dict(creates=creates,evictions=[0]*3)
cases=[];resizes=[];invalidations=[]
for i,c in enumerate(prior):
 w=copy.deepcopy(c['world']);d=w['deferred'];d['bindings']['desired']['streams'][0]=[0x33000000,17];d['bindings']['applied']['streams'][1]=[0x33000000,19];d['bindings']['desired']['streams'][2]=[0x33001000,23];d['bindings']['applied']['streams'][3]=[0x33001000,29]
 runtime={k:w[k] for k in ['vertex','index','device','deferred']};world=dict(runtime=runtime,state=w['state'],target=w['target'])
 context=dict(device=dict(address=0x23000000,hardware_vertices=i%2,special_vertices=1,skip_eviction=(i//2)%2),stream_capacity=[-1,0,1,8,16,99][i%6])
 steps=[]
 for j,old in enumerate(c['steps']):
  step=copy.deepcopy(old);o=step.pop('responses');o['resized']=None;step.update(context=context,answers=dict(resize=answers(i+j,step['kind'],context['device']['skip_eviction']),lock=o));steps.append(step)
 cases.append(dict(id=i,world=world,steps=steps))
 for kind in ['vertex','index']:
  rt=copy.deepcopy(runtime)
  if kind=='vertex':rt['vertex']['handles']=[[0,0],[0x33000000,0x33000000],[0x33000000,0],[0x33000000,0x33001000]][i%4]
  elif i%4==0:rt['index']['handle']=0
  resizes.append(dict(id=len(resizes),kind=kind,runtime=rt,size=[0,1,31,64,0xffffffff][i%5],context=copy.deepcopy(context),answers=answers(i,kind,context['device']['skip_eviction'])))
for capacity in [-0x80000000,-1,0,1,8,16,17,0x7fffffff]:
 for handle in [0,0x33000000,0x33001000]:
  invalidations.append(dict(id=len(invalidations),deferred=copy.deepcopy(cases[4]['world']['runtime']['deferred']),capacity=capacity,handle=handle,device=0x23000000))
errors=[]
for i in range(12):
 c=copy.deepcopy(cases[3]);c['id']=i;c['steps']=c['steps'][:1];s=c['steps'][0];r=c['world']['runtime'];r['vertex']['capacity']=0
 s['answers']['resize']=answers(0,'vertex',False)
 if i==0:s['answers']['resize']['creates'][0]['hresult']=0x80004005
 if i==1:s['answers']['resize']['creates']=[]
 if i==2:s['answers']['resize']['creates'][1]['hresult']=0x80004005;s['answers']['resize']['evictions'][0]=0x80004005;s['context']['device']['skip_eviction']=0
 if i==3:s['answers']['lock']['unlock_hresult']=0x80004005
 if i==4:s['binding']['shader']['address']=0
 if i==5:r['device']['scratch']=1
 if i==6:s['answers']['resize']['creates'][0]['handle']=0
 if i==7:s['context']['device']['address']=0
 if i==8:s['source']['stride']=0
 if i==9:c['world']['target']=[]
 if i in [10,11]:
  c['steps']=[copy.deepcopy(cases[3]['steps'][1])];s=c['steps'][0];r['index']['capacity']=0;s['context']['device']['skip_eviction']=0
  s['answers']['resize']=dict(creates=[dict(hresult=0x80004005,handle=0)]*3,evictions=[0]*3)
  if i==11:s['answers']['resize']['creates']=[dict(hresult=0,handle=0)]
 errors.append(c)
payload=dict(source_sha256=sha(source),cases=cases,errors=errors,resizes=resizes,invalidations=invalidations)
(root/'analysis/reports/d3d-resize.input.json').write_text(json.dumps(payload,separators=(',',':'))+'\n');print(json.dumps(dict(cases=48,steps=192,resizes=96,invalidations=24,errors=12)))
