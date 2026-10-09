from pathlib import Path
import json,hashlib,copy
root=Path(__file__).resolve().parents[1];sha=lambda p:hashlib.sha256(p.read_bytes()).hexdigest()
source=root/'analysis/reports/d3d-static.input.json';assert sha(source)==json.loads((root/'analysis/reports/d3d-static-validation.json').read_text())['input_sha256']
prior=json.loads((root/'analysis/reports/d3d-buffers.input.json').read_text())['cases']
cases=[]
for i in range(48):
 old=copy.deepcopy(prior[i]['submission']);world=dict(vertex=dict(address=0x34000000,handles=[0x33000000,0x33001000],capacity=[0,32,64,128][i%4],cursor=[0,1,31,64,127][i%5],active=i%2,source=0x12345678),index=dict(address=0x34000100,handle=0x33002000,capacity=[0,16,32,64][i%4],cursor=[0,1,15,32,63][i%5],width=4 if i%2 else 2),device=dict(vertex_limit=[0,1,3,65535][i%4],scratch=0,discards=(0xffffffff-i),vertex_bytes=0xffffffff,index_bytes=0xfffffffe),state=old['state'],deferred=old['complete']['deferred'],target=[(i*13+k)&255 for k in range(128)])
 steps=[]
 for j in range(4):
  vertex=j%2==0;size=[0,1,16,32,64,96][(i+j)%6] if vertex else [0,1,2,3,6,16,31,48][(i+j)%8]
  o=dict(resized=[0x33100000+j*0x1000,0x33200000+j*0x1000],lock_hresult=0,lock_pointer=0x40000000,lock_slot=0x27000000-0x14,unlock_hresult=0)
  payload=[(i*17+j*7+k)&255 for k in range(size-(i%3 if size>=3 else 0))]
  if vertex:
   stride=[1,3,16,32,0x123][i%5];source_data=dict(address=0x35000000,size_before=(-size)&0xffffffff if i%3==0 else size,size_again=0xffffffff if i%4==0 else size,stride=stride,payload=payload)
   binding=dict(source=0x35000000,stride=stride,declaration=[(i*23+j*11+k)^0xabcdef01 for k in range(5)],shader=dict(address=0x38000000,handle=[0,0x11223344,0xffffffff][i%3]),reported_size=(-size)&0xffffffff if i%3==0 else size)
   steps.append(dict(kind='vertex',source=source_data,responses=o,binding=binding))
  else:
   source_data=dict(address=0x35000100,size=size,width=[1,2,4,0xffffffff][i%4],payload=payload)
   steps.append(dict(kind='index',source=source_data,responses=o,base=(i*7+j)^0xffffffff,reported_size=size))
 cases.append(dict(id=i,world=world,steps=steps))
errors=[]
for i in range(10):
 c=copy.deepcopy(cases[3]);c['id']=i;c['steps']=c['steps'][:1];s=c['steps'][0]
 if i==0:s['source']['stride']=0
 if i==1:c['world']['vertex']['active']=2
 if i==2:s['responses']['unlock_hresult']=0x80004005
 if i==3:c['world']['device']['scratch']=1
 if i==4:c['world']['vertex']['capacity']=0;s['responses']['resized']=None
 if i==5:s['binding']['shader']['address']=0
 if i==6:c['world']['target']=[]
 if i==7:s['source']['payload']*=100
 if i==8:c['steps']=[copy.deepcopy(cases[3]['steps'][1])];c['steps'][0]['source']['width']=0
 if i==9:c['world']['state']['previous_stream_count']=17
 errors.append(c)
payload=dict(source_sha256=sha(source),cases=cases,errors=errors)
(root/'analysis/reports/d3d-dynamic.input.json').write_text(json.dumps(payload,separators=(',',':'))+'\n');print(json.dumps(dict(cases=48,steps=192,errors=10)))
