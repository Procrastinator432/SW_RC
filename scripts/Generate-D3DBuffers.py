from pathlib import Path
import json,hashlib,copy
root=Path(__file__).resolve().parents[1];sha=lambda p:hashlib.sha256(p.read_bytes()).hexdigest()
source=root/'analysis/reports/d3d-draw.input.json';validation=json.loads((root/'analysis/reports/d3d-draw-validation.json').read_text());assert sha(source)==validation['input_sha256']
prior=json.loads(source.read_text())['cases']
def state(seed,old):
 return dict(declaration=0x12345678,hardware_vertex=0x87654321,declarations=[[(seed*17+j*23+k)&0xffffffff for k in range(5)] for j in range(16)],stream_count=old,wrappers=[0x34000000+j*0x1000 if (seed+j)%3 else 0 for j in range(16)],strides=[(seed+j*37)&255 for j in range(16)],previous_stream_count=old,index_wrapper=0x34100000 if seed%2 else 0,base_vertex=(seed*157)^0xffffffff)
def request(seed,count):
 streams=[dict(declaration=[(seed*103+j*17+k*3)^0xa1234567 for k in range(5)],source_revision=(seed+j)%7,cached_revision=((seed+j)%7 if (seed+j)%3 else 0xffffffff),upload_size=[0,1,0xffffffff,0x80000000][(seed+j)%4],wrapper=0x34000000+j*0x1000,handle=[0,0x12345678,0xffffffff][(seed+j)%3],stride=[0,1,16,0x123,0xffffffff][(seed+j)%5]) for j in range(count)]
 return dict(streams=streams,shader_kind=0 if seed%2 else 0xffffffff,shader=dict(address=0x36000000,handle=[0,0x11223344,0xffffffff][seed%3]),index=dict(source=0 if seed%4==0 else 0x35000000,size=0 if seed%4==1 else (seed*73)^0xffffffff,source_revision=seed%7,cached_revision=seed%7 if seed%3 else 0xffffffff,wrapper=0x34100000,handle=[0,0xabcdef01,0xffffffff][seed%3]),base_vertex=(seed*97)^0xffffffff,frame=(seed*101)^0xabcdef01)
streams=[]
for old in range(17):
 for count in range(17):
  for variant in range(2):
   i=len(streams);r=request(i,count);r['shader_kind']=variant;s=state(i,old)
   streams.append(dict(id=i,state=s,deferred=copy.deepcopy(prior[i%len(prior)]['complete']['deferred']),request=r))
for kind in range(4):
 c=copy.deepcopy(streams[37]);c['id']=len(streams)
 if kind==0:c['request']['streams']*=17
 if kind==1:c['state']['previous_stream_count']=17
 if kind==2:c['request']['streams'][0]['wrapper']=0
 if kind==3:c['request']['shader_kind']=0;c['request']['shader']=None
 streams.append(c)
indices=[]
for i in range(128):
 r=request(i,0)
 if i%17==0:r['index']['wrapper']=0
 indices.append(dict(id=i,state=state(i,i%17),deferred=copy.deepcopy(prior[i%len(prior)]['complete']['deferred']),request=r))
restores=[]
for count in [-0x80000000,-1,0,1,16,0x7fffffff]:
 for shader in [None,dict(address=0,handle=1),dict(address=0x36000000,handle=0),dict(address=0x36000000,handle=0xffffffff)]:
  i=len(restores);s=state(i,i%17);s['stream_count']=count
  restores.append(dict(id=i,state=s,deferred=copy.deepcopy(prior[i%len(prior)]['complete']['deferred']),shader=shader))
cases=[]
for i in range(96):
 c=copy.deepcopy(prior[(i*11)%len(prior)]);r=request(i,i%17)
 cases.append(dict(id=i,submission=dict(state=state(i,(i*7)%17),complete=c['complete'],last_pass=c['last_pass'],counters=c['counters'],passes=c['passes']),request=r,context=c['context'],draw=c['draw']))
payload=dict(source_sha256=sha(source),streams=streams,indices=indices,restores=restores,cases=cases)
(root/'analysis/reports/d3d-buffers.input.json').write_text(json.dumps(payload,separators=(',',':'))+'\n')
print(json.dumps({k:len(v) for k,v in payload.items() if isinstance(v,list)}))
