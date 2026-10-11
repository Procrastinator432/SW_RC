"""Cache lifecycle, shared channel lookup and vertexmesh render integration fixtures."""
from pathlib import Path
import json,hashlib,copy,random
root=Path(__file__).resolve().parents[1]
def sha(p):return hashlib.sha256(p.read_bytes()).hexdigest()
prior=root/'analysis/reports/vertmesh-animation.json'
assert sha(prior)==json.loads((root/'analysis/reports/vertmesh-animation-validation.json').read_text())['report_sha256']
old=json.loads((root/'analysis/reports/vertmesh-animation.input.json').read_text());rng=random.Random(10550)
def array(n,flags):return dict(packed_count=flags|n,values=[[rng.getrandbits(32) for _ in range(3)] for _ in range(n)])
def cache(i,vc):
 n=0 if i%3==0 else i%5 if i%3==1 else vc+2
 nn=i%4 if i%3!=2 else vc+3
 return dict(cached_mesh=0x77001000 if i%3==1 else 0x77000000,positions=array(n,0xa0000000),normals=array(nn,0x60000000),word_9c=rng.getrandbits(32),word_90=rng.getrandbits(32),word_8c=rng.getrandbits(32),last_count=i,flags=[i%256,(255-i)%256])
def instance(a,i):
 a=copy.deepcopy(a);vc=a['clock']['vertex_count'];seq=a['clock']['sequence'];time=a['clock']['time']
 identity=0 if seq is None else 0x7b000100;editor=i%4==0;answer=0 if i%8==0 else 0x7b000200
 channels=[dict(words=[rng.getrandbits(32) for _ in range(18)]) for _ in range(3)]
 channels[0]['words'][0]=100+i;channels[0]['words'][7]=time;channels[0]['words'][17]=identity
 sequences=[dict(identity=0x7b000100,sequence=seq or dict(start=1,count=4)),dict(identity=0x7b000200,sequence=dict(start=1,count=4))]
 a['clock'].update(sequence=dict(start=-1,count=0),time=0x7fc00000)
 a['cache']=dict(positions=[],normals=[],last_count=-1,flags=[0,0])
 return dict(mesh=0x77000000,cache=cache(i,vc),channels=channels,editor=editor,bindings=[[100+i,answer],[100+i,0xdeadbeef]],sequences=sequences,animation=a)
cases=copy.deepcopy(old['cases'])
for c in cases:
 for j,q in enumerate(c['requests']):q['instance']=instance(q.pop('animation'),c['id']*3+j)
frames=[instance(a,i) for i,a in enumerate(old['frames'])]
caches=[dict(cache=cache(i,i%33),mesh=0x77000000,vertex_count=i%33) for i in range(256)]
for i in range(3):caches.append(dict(cache=cache(0,0),mesh=0x77002000 if i==1 else 0x77000000,vertex_count=0))
cache_errors=[]
for change in [lambda q:q.update(mesh=0),lambda q:q.update(vertex_count=65537),lambda q:q['cache']['positions'].update(packed_count=0x10000000),lambda q:q['cache']['normals']['values'].append([0,0,0])]:
 q=copy.deepcopy(caches[1]);change(q);cache_errors.append(q)
frame_errors=[]
for change in [lambda q:q.update(channels=[]),lambda q:q.update(sequences=[]),lambda q:q['animation'].update(normals=[]),lambda q:q['cache']['positions'].update(packed_count=0x10000000),lambda q:q['channels'][0]['words'].__setitem__(7,0x7fc00000)]:
 q=copy.deepcopy(frames[5]);q['animation']['requested_count']=1;change(q);frame_errors.append(q)
errors=[]
for change in [lambda q:q['instance'].update(channels=[]),lambda q:q['instance'].update(sequences=[]),lambda q:q['instance']['animation'].update(normals=[]),lambda q:q['instance']['animation'].update(mode=1),lambda q:q['instance']['animation'].update(requested_count=1),lambda q:q['geometry']['mesh']['index']['upload']['lock'].update(unlock_hresult=0x80004005),lambda q:q['geometry']['mesh'].update(bank=[]),lambda q:q['geometry']['geometry'].update(face_thresholds=[])]:
 c=copy.deepcopy(cases[2]);c['id']=len(errors);c['requests']=c['requests'][:1];change(c['requests'][0]);errors.append(c)
out=dict(source_sha256=sha(prior),passes_sha256=old['passes_sha256'],cases=cases,errors=errors,frames=frames,frame_errors=frame_errors,caches=caches,cache_errors=cache_errors)
(root/'analysis/reports/vertmesh-instance.input.json').write_text(json.dumps(out,separators=(',',':'))+'\n')
print(json.dumps({k:len(out[k]) for k in ['cases','errors','frames','frame_errors','caches','cache_errors']}))
