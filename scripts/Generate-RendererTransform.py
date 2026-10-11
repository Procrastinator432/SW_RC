"""Raw setter, dirty/flush and persistent joined renderer-state fixtures."""
from pathlib import Path
import json,copy,hashlib,random
root=Path(__file__).resolve().parents[1]
def sha(p):return hashlib.sha256(p.read_bytes()).hexdigest()
prior=root/'analysis/reports/mesh-transform.json'
assert sha(prior)==json.loads((root/'analysis/reports/mesh-transform-validation.json').read_text())['report_sha256']
old=json.loads((root/'analysis/reports/mesh-transform.input.json').read_text());rng=random.Random(1001)
def words():return [rng.getrandbits(32) for _ in range(16)]
def scene():return dict(matrices=[words() for _ in range(3)],inverse=words())
def inverse(i):return dict(kind='unchanged') if i%3==0 else dict(kind='written',words=words())
setters=[]
for i in range(256):
 d=copy.deepcopy(old['cases'][i%50]['runtime']['buffers']['deferred']);d['states']['dirty']=[0,1,2,0x10,0x73][i%5];d['transforms']['mask']=rng.getrandbits(32)
 m=words();m[:4]=[0x7fc12345,0x7f800000,0xff800000,0x80000000]
 requests=[dict(slot=slot,matrix=words() if slot else m.copy(),inverse=inverse(i) if slot==0 else dict(kind='unresolved')) for slot in [0,1,2]]
 requests += [dict(slot=[3,10,0xffffffff,0x80000000][i%4],matrix=None,inverse=dict(kind='unresolved')),dict(slot=0,matrix=m.copy(),inverse=inverse(i+1)),dict(slot=i%3,matrix=None,inverse=dict(kind='unresolved'))]
 if i%4==0:requests.append(dict(slot=0,matrix=m.copy(),inverse=dict(kind='unresolved')))
 setters.append(dict(id=i,scene=scene(),deferred=d,requests=requests,capacity=i%9,stencil_gate=bool(i%2)))
def wrapped(c):
 c=copy.deepcopy(c);c['runtime']=dict(scene=scene(),runtime=c['runtime'])
 c['requests']=[dict(mesh=q,inverse=inverse(c['id']*3+j)) for j,q in enumerate(c['requests'])]
 return c
cases=[wrapped(c) for c in old['cases']];errors=[wrapped(c) for c in old['errors']]
q=copy.deepcopy(cases[2]['requests'][2]);q['inverse']=dict(kind='unresolved')
errors.append(dict(id=len(errors),runtime=copy.deepcopy(cases[2]['runtime']),requests=[q]))
out=dict(source_sha256=sha(prior),passes_sha256=old['passes_sha256'],setters=setters,cases=cases,errors=errors)
(root/'analysis/reports/renderer-transform.input.json').write_text(json.dumps(out,separators=(',',':'))+'\n')
print(json.dumps(dict(setter_cases=len(setters),setter_requests=sum(len(c['requests']) for c in setters),cases=len(cases),requests=sum(len(c['requests']) for c in cases),errors=len(errors))))
