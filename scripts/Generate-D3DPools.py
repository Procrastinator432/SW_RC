"""Construct synthetic lazy pool sequences linked to the checked scratch fixtures."""
from pathlib import Path
import json,hashlib,copy
root=Path(__file__).resolve().parents[1];prior=root/'analysis/reports/d3d-scratch.input.json';sha=hashlib.sha256(prior.read_bytes()).hexdigest()
assert sha==json.loads((root/'analysis/reports/d3d-scratch-validation.json').read_text())['input_sha256']
old=json.loads(prior.read_text());cases=[]
def put(n,off,v):n['bytes'][off:off+4]=v.to_bytes(4,'little')
def hashkey(low):return (((low>>4)&0xff0)+((low>>16)&15))^(low&0xfff)
for i,c in enumerate(old['cases']):
 c=copy.deepcopy(c);w=c['world'];prev=w.pop('runtime');rt=prev['buffers']
 cache=dict(device=0x22000000,head=0,buckets=[0]*4096,nodes=[]);pools=dict(vertex=0,indices=[0,0])
 # Some sequences start with a populated pool to retain ring-growth coverage.
 def seeded(address,vertex,width=2):
  n=dict(address=address,bytes=[(j*11+i)&255 for j in range(72 if vertex else 64)])
  key=[0x1234,i];bucket=hashkey(key[0]);fields=[(0,0x10072f68 if vertex else 0x10073018),(4,cache['device']),(8,key[0]),(12,key[1]),(0x10,0),(0x14,0),(0x18,bucket),(0x20,0),(0x24,0),(0x28,cache['head']),(0x2c,cache['buckets'][bucket])]
  n['bytes'][0x1c:0x1e]=[0,0]
  obj=rt['vertex'] if vertex else rt['index']
  fields+=([(0x30,obj['handles'][0]),(0x34,obj['handles'][1]),(0x38,obj['capacity']),(0x3c,obj['source']),(0x40,obj['cursor']),(0x44,obj['active'])] if vertex else [(0x30,obj['handle']),(0x34,obj['capacity']),(0x38,obj['cursor']),(0x3c,width)])
  for off,v in fields:put(n,off,v)
  cache['head']=address;cache['buckets'][bucket]=address;cache['nodes'].append(n)
 if i%3==0:seeded(0x34000000,True);pools['vertex']=0x34000000
 if i%4==0:seeded(0x34000100,False);pools['indices'][0]=0x34000100
 w['runtime']=dict(cache=cache,pools=pools,device=rt['device'],deferred=rt['deferred'],scratch=prev['scratch'])
 original=c['steps'];steps=[]
 # Revisit each index pool and the vertex pool with different source keys.
 expanded=original+[copy.deepcopy(original[1]),copy.deepcopy(original[3]),copy.deepcopy(original[0]),copy.deepcopy(original[1])]
 for j,s in enumerate(expanded):
  kind=s.pop('kind');s['upload']=s.pop('answers')
  if kind=='index':s['source']['width']=4 if j in [3,5] else [1,2,3,8][i%4]
  width=s['source'].get('width',0);slot=int(width==4)
  address=0x34000000 if kind=='vertex' else 0x34000100+slot*0x100
  # Distinct high halves deliberately share the same hash bucket.
  s['key']=[0x1234,(i<<8)+j]
  responses=[dict(hresult=0,handle=0x36000000+slot*0x10000+j*0x1000),dict(hresult=0,handle=0x36001000+j*0x1000)]
  if kind=='index':responses=responses[:1]
  s['pool']=dict(allocation=dict(address=address,bytes=[(k*17+i+j)&255 for k in range(72 if kind=='vertex' else 64)]),creates=dict(creates=responses,evictions=[]))
  steps.append(dict(kind=kind,request=s))
 c['steps']=steps;cases.append(c)
errors=[]
def error(index,mutate,step=0):
 c=copy.deepcopy(cases[index]);c['id']=len(errors);c['steps']=[c['steps'][step]];mutate(c);errors.append(c)
error(1,lambda c:c['steps'][0]['request']['pool']['allocation'].update(address=0))
error(1,lambda c:c['steps'][0]['request']['pool']['allocation']['bytes'].pop())
error(1,lambda c:c['steps'][0]['request']['pool']['creates']['creates'][0].update(hresult=0x80004005,handle=0))
error(1,lambda c:c['steps'][0]['request']['pool']['creates']['creates'].clear())
error(1,lambda c:c['steps'][0]['request']['binding']['shader'].update(address=0))
error(1,lambda c:c['steps'][0]['request']['upload']['lock'].update(unlock_hresult=0x80004005))
error(1,lambda c:c['world']['target'].clear())
error(0,lambda c:c['world']['runtime']['cache']['nodes'][0]['bytes'].__setitem__(0,0))
error(1,lambda c:c['steps'][0]['request']['pool']['allocation'].update(address=0xfffffff0))
error(1,lambda c:c['steps'][0]['request']['pool']['creates']['creates'][0].update(handle=0))
error(1,lambda c:c['steps'][0]['request']['pool']['allocation']['bytes'].clear(),step=1)
error(1,lambda c:c['steps'][0]['request']['pool']['creates']['creates'][0].update(hresult=0x80004005,handle=0),step=3)
(root/'analysis/reports/d3d-pools.input.json').write_text(json.dumps(dict(source_sha256=sha,cases=cases,errors=errors)),newline='\n')
print(json.dumps(dict(cases=len(cases),steps=sum(len(c['steps']) for c in cases),errors=len(errors))))
