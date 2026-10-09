from pathlib import Path
import json,hashlib,copy
root=Path(__file__).resolve().parents[1];sha=lambda p:hashlib.sha256(p.read_bytes()).hexdigest()
source=root/'analysis/reports/d3d-buffers.input.json';assert sha(source)==json.loads((root/'analysis/reports/d3d-buffers-validation.json').read_text())['input_sha256']
def base(i):
 return dict(id=i,resource=dict(address=0x34000000,handle=0x33000000,capacity=16,revision=7,source=0x12345678),device=dict(address=0x23000000,hardware_vertices=1,special_vertices=1,skip_eviction=0),responses=dict(creates=[dict(hresult=0,handle=0x33100000)],evictions=[0]*3,lock_hresult=0,lock_pointer=0x32000000,unlock_hresult=0,lock_slot=0x27000000-0x2c),expected_error=False)
vertices=[]
for flags in range(32):
 for reuse in range(3):
  for retry in range(3):
   i=len(vertices);c=base(i);d=c['device'];on=[1,128,0xffffffff][i%3]
   d.update(hardware_vertices=on if flags&1 else 0,special_vertices=on if flags&2 else 0,skip_eviction=on if flags&4 else 0)
   size=[0,1,16,0x7fffffff,0x80000000,0xffffffff][i%6]
   c['source']=dict(address=0x35000000,size=size,dynamic=on if flags&8 else 0,special=on if flags&16 else 0,revision_after=(i*17)^0xabcdef01)
   c['resource']['handle']=0 if reuse==0 else 0x33000000;c['resource']['capacity']=size if reuse==1 else size^1
   failures=min(retry,1) if d['skip_eviction'] else retry
   c['responses']['creates']=[dict(hresult=0x80004005,handle=0) for _ in range(failures)]+[dict(hresult=[0,1,0x7fffffff][i%3],handle=0x33100000)]
   vertices.append(c)
for error in range(8):
 c=base(len(vertices));c['source']=dict(address=0x35000000,size=17,dynamic=0,special=0,revision_after=8);c['expected_error']=True
 if error==0:c['responses']['lock_hresult']=0x80004005
 if error==1:c['responses']['unlock_hresult']=0xffffffff
 if error==2:c['responses']['creates'][0]['handle']=0
 if error==3:c['responses']['creates']=[dict(hresult=0x80004005,handle=0)];c['responses']['evictions'][0]=0x80004005
 if error==4:c['responses']['creates']=[dict(hresult=0x80004005,handle=0)]*3
 if error==5:c['source']['address']=0
 if error==6:c['resource']['address']=0
 if error==7:c['device']['address']=0
 vertices.append(c)
indices=[]
for size in [0,1,2,3,16,0x7fffffff,0x80000000,0xffffffff]:
 for width in [0,2,3,4,0xffffffff]:
  for hardware in [0,255]:
   for reuse in range(3):
    for equal in [False,True]:
     i=len(indices);c=base(i);c['device']['hardware_vertices']=hardware;c['responses']['lock_slot']=0x27000000-0x14
     clamped=size if 2<size<0x80000000 else 2;c['resource']['handle']=0 if reuse==0 else 0x33000000;c['resource']['capacity']=clamped if reuse==1 else clamped^1
     c['source']=dict(address=0x35000000,size=size,width=width,revision_before=7 if equal else 8,revision_after=(i*23)^0xabcdef01);indices.append(c)
for error in range(6):
 c=copy.deepcopy(indices[0]);c['id']=len(indices);c['expected_error']=True
 if error==0:c['responses']['creates'][0]['hresult']=0x80004005
 if error==1:c['responses']['lock_hresult']=0x80004005
 if error==2:c['responses']['unlock_hresult']=0xffffffff
 if error==3:c['source']['address']=0
 if error==4:c['responses']['creates'][0]['handle']=0
 if error==5:c['responses']['lock_pointer']=0
 indices.append(c)
sequences=[]
for kind in ['vertex','index']:
 for i in range(16):
  pool=vertices if kind=='vertex' else indices;initial=base(i)['resource'];actions=[]
  for j in range(4):
   c=copy.deepcopy(pool[(i*13+j*7)%(288 if kind=='vertex' else 480)])
   actions.append(dict(source=c['source'],device=c['device'],responses=c['responses']))
  sequences.append(dict(id=len(sequences),kind=kind,resource=initial,actions=actions))
payload=dict(source_sha256=sha(source),vertices=vertices,indices=indices,sequences=sequences)
(root/'analysis/reports/d3d-upload.input.json').write_text(json.dumps(payload,separators=(',',':'))+'\n')
print(json.dumps({k:len(v) for k,v in payload.items() if isinstance(v,list)}))
