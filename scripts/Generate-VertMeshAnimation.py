"""Deterministic scheduler, frame bank and transactional draw fixtures."""
from pathlib import Path
import json,copy,hashlib,struct,random
root=Path(__file__).resolve().parents[1]
def sha(p):return hashlib.sha256(p.read_bytes()).hexdigest()
def f(v):return struct.unpack('<I',struct.pack('<f',v))[0]
prior=root/'analysis/reports/vertmesh-frame.json'
assert sha(prior)==json.loads((root/'analysis/reports/vertmesh-frame-validation.json').read_text())['report_sha256']
old=json.loads((root/'analysis/reports/vertmesh-frame.input.json').read_text())
rng=random.Random(1055)
def animation(q,i):
 q=copy.deepcopy(q);vc=q.pop('vertex_count');q.pop('first');q.pop('next');q.pop('fraction')
 q['clock']=dict(vertex_count=vc,sequence=None if i%9==0 else dict(start=1,count=4),time=f([-0.25,-0.0,0,0.125,0.2,0.875,1,2.2,9.999][i%9]))
 q['positions']=[rng.getrandbits(32) for _ in range(vc*5)]
 q['normals']=[rng.getrandbits(32) for _ in range(vc*5)]
 return q
cases=copy.deepcopy(old['cases'])
for c in cases:
 for j,q in enumerate(c['requests']):q['animation']=animation(q.pop('frame'),c['id']*3+j)
frames=[animation(q,i) for i,q in enumerate(old['frames'])]
# Exact-frame reads must not require the following frame; count zero reads neither bank.
q=copy.deepcopy(frames[2]);q['clock']['time']=0;q['positions']=q['positions'][:q['clock']['vertex_count']+q['requested_count']];q['normals']=q['normals'][:len(q['positions'])];frames.append(q)
q=copy.deepcopy(frames[3]);q['requested_count']=0;q['positions']=[];q['normals']=[];frames.append(q)
clocks=[]
for i in range(512):
 count=[1,2,3,4,7,31,1024,16777217][i%8];vc=0 if count>1024 else i%257
 time=[-100,-0.0,0,0.125,0.2,0.5,0.875,1,1.000001,9.999,100][i%11]
 clocks.append(dict(vertex_count=vc,sequence=None if i%17==0 else dict(start=i%7,count=count),time=(0x7fc00000 if i%34==0 else 0x7f800000) if i%17==0 else f(time)))
clock_errors=[]
for seq,time,vc in [(dict(start=0,count=0),0,4),(dict(start=-1,count=4),0,4),(dict(start=0,count=-1),0,4),(dict(start=0,count=4),0x7fc00000,4),(dict(start=0,count=4),0x7f800000,4),(dict(start=0,count=4),f(536870912),4),(dict(start=65536,count=4),0,65536),(None,0,65537),(dict(start=2147483647,count=4),f(0.5),4)]:clock_errors.append(dict(vertex_count=vc,sequence=seq,time=time))
frame_errors=[]
for change in [lambda q:q.update(requested_count=-1),lambda q:q.update(positions=[]),lambda q:q.update(normals=[]),lambda q:q['clock'].update(time=0x7fc00000),lambda q:q.update(stride=8),lambda q:q['cache']['positions'].pop()]:
 q=copy.deepcopy(frames[5]);q['requested_count']=1;change(q);frame_errors.append(q)
errors=[]
for change in [lambda q:q['animation']['clock'].update(time=0x7fc00000),lambda q:q['animation'].update(normals=[]),lambda q:q['animation'].update(mode=1),lambda q:q['animation'].update(requested_count=1),lambda q:q['animation']['wedges'].__setitem__(0,1),lambda q:q['geometry']['mesh']['index']['upload']['lock'].update(unlock_hresult=0x80004005),lambda q:q['geometry']['mesh'].update(bank=[]),lambda q:q['geometry']['geometry'].update(face_thresholds=[])]:
 c=copy.deepcopy(cases[2]);c['id']=len(errors);c['requests']=c['requests'][:1];change(c['requests'][0]);errors.append(c)
out=dict(source_sha256=sha(prior),passes_sha256=old['passes_sha256'],cases=cases,errors=errors,frames=frames,frame_errors=frame_errors,clocks=clocks,clock_errors=clock_errors)
(root/'analysis/reports/vertmesh-animation.input.json').write_text(json.dumps(out,separators=(',',':'))+'\n')
print(json.dumps({k:len(out[k]) for k in ['cases','errors','frames','frame_errors','clocks','clock_errors']}))
