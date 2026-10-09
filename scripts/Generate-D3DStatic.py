from pathlib import Path
import json,hashlib,copy
root=Path(__file__).resolve().parents[1];sha=lambda p:hashlib.sha256(p.read_bytes()).hexdigest()
source=root/'analysis/reports/d3d-resource.input.json'
assert sha(source)==json.loads((root/'analysis/reports/d3d-resource-validation.json').read_text())['input_sha256']
prior=json.loads((root/'analysis/reports/d3d-buffers.input.json').read_text())['cases']
def region(a,n,fields):
 b=[0]*n
 for off,v in fields.items():b[off:off+4]=v.to_bytes(4,'little')
 return dict(address=a,bytes=b)
def response(i,slot,index=False):
 return dict(creates=([dict(hresult=0x80004005,handle=0)] if not index and i%6==0 else [])+[dict(hresult=0,handle=0x33100000+slot*0x1000)],evictions=[0]*3,lock_hresult=0,lock_pointer=0x40000000+slot*0x10000,unlock_hresult=0,lock_slot=0x27000000-(0x14 if index else 0x2c))
cases=[]
for i in range(24):
 p=copy.deepcopy(prior[i*3]);resources=dict(cache=dict(device=0x22000000,head=0,buckets=[0]*4096,nodes=[dict(address=0x34000000+j*0x100,bytes=[(i*13+j*7+k)&255 for k in range(64)]) for j in range(8)]),targets=[dict(address=0x40000000+j*0x10000,bytes=[0xc0+j]*128) for j in range(3)])
 steps=[]
 for step in range(4):
  count=1 if step==2 and i%2==0 else 2;rev=0 if i==23 and step==0 else (2 if step==2 else 1);regions=[];streams=[]
  for slot in range(2):
   a=0x35000000+slot*0x100;data=0x36000000+slot*0x100;owner=0x39000000 if i%4==0 and slot==0 else 0;mode=int(owner!=0)
   regions += [region(a,36,{4:1,8:0x12340000+slot,0x10:owner,0x18:mode,0x1c:data,0x20:count}),dict(address=data,bytes=[(i*17+step*11+slot*31+k)&255 for k in range(count*32)])]
   streams.append(dict(address=a,allocation=0x34000000+slot*0x100,revision=rev,revision_after=rev,initial_components=[(i*19+slot*7+k)&255 for k in range(16)],owner_count=count if owner else None,delegate=[(i*23+step*13+k)&255 for k in range(count*32)] if owner else None,dynamic=i%2,special=(i//2)%2,responses=response(i,slot)))
  if i%5==0:streams[1]['responses']['lock_pointer']=streams[0]['responses']['lock_pointer']
  if i%2==0:streams.append(copy.deepcopy(streams[0]))
  width='U32' if i%2 else 'U16';icount=0 if step==3 or i%7==0 else (3 if step<2 or i%2 else 5)
  regions.append(region(0x35000200,24,{4:0x10000001,8:0x12340002,0x10:0x36000200,0x14:icount}))
  if icount:regions.append(dict(address=0x36000200,bytes=[(i*29+step*5+k)&255 for k in range(icount*(4 if width=='U32' else 2))]))
  index=dict(address=0x35000200,allocation=0x34000200,width=width,revision=rev,revision_before=(0 if step==0 else 1) if i%4==0 else rev,revision_after=rev,responses=response(i,2,True))
  if step==3:streams=[]
  if step==3 and i%2:index=None
  r=dict(streams=streams,index=index,shader_kind=i%2,shader=dict(address=0x38000000,handle=0x12121212),base_vertex=i*7,frame=step+10,device=dict(address=0x23000000,hardware_vertices=i%2,special_vertices=(i//2)%2,skip_eviction=(i//3)%2))
  steps.append(dict(regions=regions,request=r,context=p['context'],draw=p['draw']))
 cases.append(dict(id=i,resources=resources,submission=p['submission'],steps=steps))
errors=[]
for i in range(10):
 c=copy.deepcopy(cases[1]);c['id']=i;c['steps']=c['steps'][:1];s=c['steps'][0];r=s['request']
 if i==0:r['streams'][0]['allocation']=None
 if i==1:r['index']['responses']['unlock_hresult']=0x80004005
 if i==2:r['shader_kind']=0;r['shader']=None
 if i==3:r['streams'][1]['responses']['lock_pointer']=0x1234
 if i==4:c['submission']['passes']=[c['submission']['passes'][0]]*9
 if i==5:s['regions']=[p for p in s['regions'] if p['address']!=0x36000000]
 if i==6:c['resources']['targets'].append(copy.deepcopy(c['resources']['targets'][0]))
 if i==7:r['streams'][0]['responses']['creates'][0]['hresult']=0x80004005
 if i==8:r['streams']*=17
 if i==9:c['resources']['cache']['buckets'].pop()
 errors.append(c)
payload=dict(source_sha256=sha(source),cases=cases,errors=errors)
(root/'analysis/reports/d3d-static.input.json').write_text(json.dumps(payload,separators=(',',':'))+'\n')
print(json.dumps(dict(cases=len(cases),steps=sum(len(c['steps']) for c in cases),errors=len(errors))))
