from pathlib import Path
import json,hashlib,copy,struct
root=Path(__file__).resolve().parents[1];sha=lambda p:hashlib.sha256(p.read_bytes()).hexdigest();source=root/'analysis/reports/d3d-source.input.json';assert sha(source)==json.loads((root/'analysis/reports/d3d-source-validation.json').read_text())['input_sha256']
def bucket(low):return (((low>>4)&0xff0)+((low>>16)&15))^(low&0xfff)
def put(n,o,v):n['bytes'][o:o+4]=list(struct.pack('<I',v&0xffffffff))
def initial(seed):
 c=dict(device=0x22000000,head=0,buckets=[0]*4096,nodes=[dict(address=0x34000000+i*0x100,bytes=[(seed*17+i*23+k)&255 for k in range(64)]) for i in range(8)])
 low=[0,1,0xfff,0x12345678,0xffffffff,0x10000000][seed%6];high=(seed*113)^0xabcdef01;keys=[[low,high],[low,high if seed%2 else high^1],[low^0x10000000,high],[low^15,high^5]]
 for i,key in enumerate(keys):
  n=c['nodes'][i];h=bucket(key[0]);kind=0x10072d28 if i==1 else 0x10072c10
  for o,v in [(0,kind),(4,c['device']),(8,key[0]),(12,key[1]),(0x10,seed+i),(0x14,seed*37),(0x18,h),(0x28,c['head']),(0x2c,c['buckets'][h])]:put(n,o,v)
  for o,v in [(0x30,0 if seed%4==0 else 0x33000000),(0x34,128 if i==1 else (0 if seed%4==1 else (0x33000000 if seed%4==2 else 0x33001000)))]:put(n,o,v)
  c['head']=n['address'];c['buckets'][h]=n['address']
 return c,keys
cases=[]
for i in range(64):
 c,k=initial(i);a=lambda j:c['nodes'][j]['address'];new=[k[0][0],k[0][1]^0xff]
 actions=[dict(kind='lookup',key=key) for key in [*k,[k[0][0],k[0][1]^0xffff],new]]
 actions += [dict(kind='reset_vertex',address=a(0)),dict(kind='destroy_index',address=a(1)),dict(kind='unlink',address=a(2)),dict(kind='unlink',address=a(3)),dict(kind='unlink',address=a(0)),dict(kind='unlink',address=a(0)),dict(kind='insert',address=a(4),key=new,resource_kind=['Base','Vertex','Index'][i%3]),dict(kind='insert',address=a(5),key=new,resource_kind='Index'),dict(kind='lookup',key=new),dict(kind='unlink',address=a(4)),dict(kind='lookup',key=new),dict(kind='destroy_index',address=a(5)),dict(kind='lookup',key=new),dict(kind='insert',address=a(6),key=k[2],resource_kind='Base'),dict(kind='insert',address=a(7),key=k[3],resource_kind='Vertex'),dict(kind='lookup',key=k[2])]
 cases.append(dict(id=i,cache=c,actions=actions))
errors=[]
for i in range(12):
 c,k=initial(i);action=dict(kind='lookup',key=k[0])
 if i==0:c['buckets'].pop()
 if i==1:c['nodes'][0]['bytes'].pop()
 if i==2:c['nodes'][1]['address']=c['nodes'][0]['address']
 if i==3:c['nodes'][0]['address']=0
 if i==4:c['device']=0xfffffff0
 if i==5:put(c['nodes'][3],0x28,c['nodes'][3]['address']);action=dict(kind='unlink',address=c['nodes'][3]['address'])
 if i==6:put(c['nodes'][0],0x2c,c['nodes'][0]['address']);action=dict(kind='lookup',key=[k[0][0],0x11223344])
 if i==7:c['buckets'][bucket(k[0][0])]=0x1234
 if i==8:put(c['nodes'][0],0x18,4096);action=dict(kind='unlink',address=c['nodes'][0]['address'])
 if i==9:put(c['nodes'][0],4,0x1234);action=dict(kind='destroy_index',address=c['nodes'][0]['address'])
 if i==10:action=dict(kind='insert',address=c['nodes'][0]['address'],key=k[0],resource_kind='Base')
 if i==11:action=dict(kind='insert',address=0x1234,key=k[0],resource_kind='Base')
 errors.append(dict(id=i,cache=c,action=action))
hashes=list(range(4096))+[((n<<16)|(n<<4)) for n in range(4096)]+[0xffffffff,0x80000000,0x7fffffff,0x12345678]
payload=dict(source_sha256=sha(source),hashes=hashes,cases=cases,errors=errors)
(root/'analysis/reports/d3d-resource.input.json').write_text(json.dumps(payload,separators=(',',':'))+'\n');print(json.dumps(dict(hashes=len(hashes),cases=len(cases),steps=sum(len(c['actions']) for c in cases),errors=len(errors))))
