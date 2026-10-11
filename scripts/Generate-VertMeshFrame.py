"""Synthetic packed frame pairs and checked prepared section/material snapshots."""
from pathlib import Path
import json,copy,struct,hashlib,random
root=Path(__file__).resolve().parents[1]
def sha(p):return hashlib.sha256(p.read_bytes()).hexdigest()
def f(v):return struct.unpack('<I',struct.pack('<f',v))[0]
prior=root/'analysis/reports/vertmesh-preprocess.json'
assert sha(prior)==json.loads((root/'analysis/reports/vertmesh-preprocess-validation.json').read_text())['report_sha256']
old=json.loads(prior.read_text());inputs=json.loads((root/'analysis/reports/vertmesh-preprocess.input.json').read_text());rng=random.Random(1055_2080)
state=old['probes'][2]['steps'][0]['result']['Ok']['preprocessing']['state'];assert state['packed_sections']&0x1fffffff==3
template_mesh=old['probes'][2]['steps'][0]['request']['mesh']['mesh']['geometry']['mesh']
template_token=old['probes'][2]['steps'][0]['result']['Ok']['mesh']['mesh']['draw']['draw']['sections'][0]['selection']['material']
template_passes=next(v['passes'] for v in template_mesh['bank'] if v['material']==template_token)
def frame(n,requested,mode=0,stride=32):
 count=min(n,requested);wedges=[0,max(count-1,0),count,65535,0] if n else [65535]
 records=count if mode==1 else len(wedges)
 return dict(vertex_count=n,requested_count=requested,first=dict(positions=[rng.getrandbits(32) for _ in range(n)],normals=[rng.getrandbits(32) for _ in range(n)]),next=dict(positions=[rng.getrandbits(32) for _ in range(n)],normals=[rng.getrandbits(32) for _ in range(n)]),fraction=f(0.5),cache=dict(positions=[[rng.getrandbits(32) for _ in range(3)] for _ in range(n)],normals=[[rng.getrandbits(32) for _ in range(3)] for _ in range(n)],last_count=99,flags=[5,6]),wedges=wedges,mode=mode,stride=stride,bytes=list(rng.randbytes(records*stride+8)))
cases=[]
for i,c in enumerate(inputs['cases']):
 requests=[]
 for j,oldq in enumerate(c['requests']):
  q=frame(4,[3,5][(i+j)%2]);q['wedges']=[0,1,2,3,65535];q['bytes']=list(rng.randbytes(160));q['bytes'][:96]=state['stream']
  q['fraction']=f([0,0.25,0.5,1,-0.0,-1,1.5][(i+j)%7])
  if (i+j)%4==0:q['next']=None
  mesh=copy.deepcopy(oldq['mesh']['mesh']['geometry']['mesh'])
  for bank in mesh['bank']:bank['passes']=copy.deepcopy(template_passes)
  geom=dict(packed_count=state['packed_sections'],section_bytes=state['section_bytes'].copy(),lod=q['requested_count'],face_thresholds=[0,2,3,0,2,1],faces=[[0,1,2,m] for m in [0,0,1,1,0,0]],collapse=[0]*5,vertex_thresholds=q['wedges'].copy())
  requests.append(dict(frame=q,geometry=dict(geometry=geom,mesh=mesh)))
 runtime=copy.deepcopy(c['runtime'])
 runtime['vertex_target'] += [0x5a]*max(0,256-len(runtime['vertex_target']))
 runtime['buffers']['scratch']['bytes'] += [0xa5]*max(0,4096-len(runtime['buffers']['scratch']['bytes']))
 cases.append(dict(id=i,runtime=runtime,requests=requests))
frames=[]
for i in range(256):
 n=i%33;mode=[0,1,7][i%3];width=12 if mode==1 else 24;stride=[width,width+4,32,64][i%4]
 q=frame(n,rng.randrange(n+4),mode,stride);q['fraction']=f([-1,-0.0,0,0.125,0.5,1,1.5][i%7])
 if i%3==0:q['next']=None
 frames.append(q)
# Signed bit edges / all packed normal lanes including ignored top bits.
for word in [0,0xffffffff,0x80000400,0x001ff800,0x7fc00000,0x40000000,0xc0000000]:
 q=frame(1,1);q['first']['positions']=[word];q['first']['normals']=[word];q['next']=None;frames.append(q)
q=frame(3,0);q['next']=dict(positions=[],normals=[]);frames.append(q)
q=frame(1,1);q['fraction']=f(-1);q['next']=dict(positions=[],normals=[]);frames.append(q)
frame_errors=[]
def frame_error(change):
 q=frame(4,3);change(q);frame_errors.append(q)
frame_error(lambda q:q.update(requested_count=-1))
frame_error(lambda q:q.update(vertex_count=65537))
frame_error(lambda q:q['cache']['positions'].pop())
frame_error(lambda q:q['first']['positions'].clear())
frame_error(lambda q:q['next']['normals'].clear())
frame_error(lambda q:q.update(fraction=0x7fc00000))
frame_error(lambda q:q.update(stride=8))
frame_error(lambda q:q.update(bytes=[0]*24))
frame_error(lambda q:(q.update(fraction=f(3.4e38)),q['first']['positions'].__setitem__(0,0),q['next']['positions'].__setitem__(0,1023)))
errors=[]
def error(change):
 c=copy.deepcopy(cases[2]);c['id']=len(errors);c['requests']=c['requests'][:1];change(c['requests'][0]);errors.append(c)
error(lambda q:q['frame'].update(mode=1))
error(lambda q:q['frame'].update(stride=24))
error(lambda q:q['frame'].update(requested_count=1))
error(lambda q:q['frame']['wedges'].__setitem__(0,1))
error(lambda q:q['frame']['first']['normals'].clear())
error(lambda q:q['geometry']['mesh']['index']['upload']['lock'].update(unlock_hresult=0x80004005))
error(lambda q:q['geometry']['mesh'].update(bank=[]))
error(lambda q:q['geometry']['geometry'].update(face_thresholds=[]))
out=dict(source_sha256=sha(prior),passes_sha256=inputs['passes_sha256'],cases=cases,errors=errors,frames=frames,frame_errors=frame_errors)
(root/'analysis/reports/vertmesh-frame.input.json').write_text(json.dumps(out,separators=(',',':'))+'\n')
print(json.dumps(dict(cases=len(cases),requests=sum(len(c['requests']) for c in cases),errors=len(errors),frames=len(frames),frame_errors=len(frame_errors))))
