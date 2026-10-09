"""Connect checked pool and material-pass fixtures with original Engine caller fields."""
from pathlib import Path
import json,hashlib,copy
root=Path(__file__).resolve().parents[1]
def sha(p):return hashlib.sha256(p.read_bytes()).hexdigest()
prior=root/'analysis/reports/d3d-pools.input.json';passes=root/'analysis/reports/d3d-draw.input.json'
assert sha(prior)==json.loads((root/'analysis/reports/d3d-pools-validation.json').read_text())['input_sha256']
assert sha(passes)==json.loads((root/'analysis/reports/d3d-draw-validation.json').read_text())['input_sha256']
old=json.loads(prior.read_text());draws=json.loads(passes.read_text());cases=[];EF=0x28000000
light=json.loads((root/'analysis/reports/d3d-complete.input.json').read_text())['cases'][0]['complete']['lights']
for i,c in enumerate(old['cases']):
 w=c['world'];sample=draws['cases'][i%len(draws['cases'])]
 rt=dict(buffers=copy.deepcopy(w['runtime']),state=copy.deepcopy(w['state']),lights=light,last_pass=sample['last_pass'],counters=sample['counters'],vertex_target=copy.deepcopy(w['target']),index_target=copy.deepcopy(w['target']))
 requests=[]
 callers=[dict(kind='grid',columns=[0,1,3,5][i%4],rows=[0,2,3,7][i%4]),dict(kind='fluid',columns=4,rows=5,quad=i%2==0),dict(kind='beam',segments=[0,1,2,3,4,0xffffffff,5,7][i%8],copies=i%3+1,vertices=7,indices=10,primitives=5),dict(kind='canvas',primitive=[1,2,5,6,7,99][i%6],primitive_count=0 if i%12==0 else i%5+1,packed_vertices=0xe0000000|(i%11+1),alternate_vertices=0xa0000000|(i%7+1),alternate=i%2==0),dict(kind='lines',packed_vertices=0xe0000000 if i%12==0 else [6,7,0x1ffffffb,0xe0000009][i%4])]
 for j,caller in enumerate(callers):
  v=copy.deepcopy(c['steps'][0 if j==0 else 2]['request']);idx=copy.deepcopy(c['steps'][1 if j==0 else 3]['request']) if j<3 else None
  va=[EF-0xbc,EF-0x60,EF-0x38,0x38000000,0x38001000][j];ia=[EF-0xa8,EF-0x44,EF-0x50,0,0][j]
  v['source']['address']=v['binding']['source']=va
  if idx:idx['source']['address']=ia
  p=draws['cases'][(i*5+j)%len(draws['cases'])]
  context=copy.deepcopy(p['context']);context['stream_capacity']=16
  requests.append(dict(caller=caller,vertex=v,index=idx,passes=copy.deepcopy(p['passes']),context=context))
 cases.append(dict(id=i,runtime=rt,requests=requests))
errors=[]
def error(i,mutate):
 c=copy.deepcopy(cases[i]);c['id']=len(errors);c['requests']=c['requests'][:1];mutate(c);errors.append(c)
error(1,lambda c:c['requests'][0].update(index=None))
error(1,lambda c:c['requests'][0]['vertex']['binding']['shader'].update(address=0))
error(1,lambda c:c['requests'][0]['index']['upload']['lock'].update(unlock_hresult=0x80004005))
error(1,lambda c:c['runtime']['index_target'].clear())
error(1,lambda c:c['requests'][0]['index']['pool']['allocation'].update(address=0))
error(1,lambda c:c['requests'][0]['vertex']['source'].update(stride=0))
error(1,lambda c:c['requests'][0]['context']['device'].update(capacity=9))
error(1,lambda c:c['requests'][0].update(caller=dict(kind='beam',segments=257,copies=1,vertices=3,indices=3,primitives=1)))
def late_pass(c):
 c['runtime']['last_pass']=0xdeadbeef;c['requests'][0]['passes']=copy.deepcopy(next(p['passes'] for p in draws['cases'] if p['passes']));c['requests'][0]['passes'][0]['pass']['address']=0
error(1,late_pass)
def too_many(c):
 first=next(p['passes'][0] for p in draws['cases'] if p['passes']);c['requests'][0]['passes']=[copy.deepcopy(first) for _ in range(9)]
error(1,too_many)
error(1,lambda c:c['requests'][0]['index']['source'].update(width=0))
error(1,lambda c:c['requests'][0]['index']['pool']['creates']['creates'][0].update(handle=0))
(root/'analysis/reports/d3d-dynamic-draw.input.json').write_text(json.dumps(dict(source_sha256=sha(prior),passes_sha256=sha(passes),cases=cases,errors=errors)),newline='\n')
print(json.dumps(dict(cases=len(cases),requests=sum(len(c['requests']) for c in cases),errors=len(errors))))
