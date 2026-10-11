"""Bounded synthetic RenderPreProcess/GetAbsMax probes and checked scene cases."""
from pathlib import Path
import json,copy,struct,hashlib,random
root=Path(__file__).resolve().parents[1]
def sha(p):return hashlib.sha256(p.read_bytes()).hexdigest()
def f(v):return struct.unpack('<I',struct.pack('<f',v))[0]
prior=root/'analysis/reports/scene-projection.input.json'
assert sha(prior)==json.loads((root/'analysis/reports/scene-projection-validation.json').read_text())['input_sha256']
old=json.loads(prior.read_text());rng=random.Random(1055)
def preparation(i=0):
 faces=[[0,1,2,m] for m in [0,0,1,1,0,0]]
 return dict(state=dict(packed_sections=0xe0000000,section_bytes=[19]*80,packed_stream_count=0xa0000003,stream=[67]*96,revision=0xffffffff if i%3==0 else i,cache_index=[0xffffffff if i%2==0 else i,0xffffffff if i%5==0 else 7]),packed_wedges=0x60000003,wedges=[[0x12340000,f(i+0.25),0x80000000],[0,f(-2),0x7fc01234],[0,f(3),f(4)]],packed_faces=0xa0000006,faces=faces,section_allocation=[rng.randrange(256) for _ in range(240)])
cases=copy.deepcopy(old['cases'])
for i,c in enumerate(cases):
 out=[]
 for j,q in enumerate(c['requests']):
  p=preparation(i+j);g=q['mesh']['geometry']['geometry'];q['mesh']['camera']['actor_abs_max']=0x7fc00000;q['mesh']['camera']['minimum']=3
  if i%3==0 and g['packed_count']&0x1fffffff:
   p['state']['packed_sections']=g['packed_count'];p['state']['section_bytes']=g['section_bytes'].copy()
   # Skipped native preprocessing must ignore invalid, unused array inputs.
   p['packed_wedges']=0x1fffffff;p['wedges']=[];p['section_allocation']=[]
  else:
   g['face_thresholds']=[0,3,4,1,2,0];g['collapse']=[0,0,1]
   if i%11==0:p['packed_faces']=0;p['section_allocation']=[]
  out.append(dict(preprocessing=p,actor_draw_scale=[[f(-2),f(1),f(0.5)],[0x80000000,0,0],[f(0.5),f(-3),f(3)]][(i+j)%3],mesh=q))
 c['requests']=out
errors=[]
def error(change):
 c=copy.deepcopy(cases[2]);c['id']=len(errors);c['requests']=c['requests'][:1];change(c['requests'][0]);errors.append(c)
error(lambda q:q['preprocessing'].update(packed_wedges=0x1fffffff))
error(lambda q:q['preprocessing'].update(packed_faces=65537))
error(lambda q:q['preprocessing']['wedges'].pop())
error(lambda q:q['preprocessing']['section_allocation'].pop())
error(lambda q:q['preprocessing']['state'].update(packed_stream_count=0x1fffffff))
error(lambda q:q['actor_draw_scale'].__setitem__(0,0x7fc00000))
error(lambda q:q['mesh']['world_to_screen'][3].__setitem__(3,0))
error(lambda q:q['mesh']['mesh']['camera'].update(tan_fov=0))
error(lambda q:q['mesh']['mesh']['vertex'].update(instance=0))
error(lambda q:q['mesh']['mesh']['geometry']['mesh']['index']['upload']['lock'].update(unlock_hresult=0x80004005))
error(lambda q:q['mesh']['mesh']['geometry']['mesh'].update(bank=[]))
error(lambda q:q['mesh']['mesh']['geometry']['geometry'].update(collapse=[0],vertex_thresholds=[65535]))
# Last error must reach a missing collapse target: source wedge threshold overwritten by preprocessing.
errors[-1]['requests'][0]['preprocessing']['wedges'][1][0]=65535
preparations=[preparation(i) for i in range(40)]
q=preparation();q['packed_faces']=0;q['section_allocation']=[];preparations.append(q)
q=preparation();q['packed_wedges']=0;preparations.append(q)
q=preparation();q['packed_wedges']=0;q['packed_faces']=0;q['section_allocation']=[];preparations.append(q)
q=preparation();q['packed_faces']=65536;q['faces']=[[0,0,0,2]]*65536;q['section_allocation']=[0xa5]*80;preparations.append(q)
q=preparation();q['packed_wedges']=65536;q['wedges']=[[0,0x80000000,f(0.5)]]*65536;preparations.append(q)
q=preparation();q['packed_faces']=257;q['faces']=[[0,0,0,i%2] for i in range(257)];q['section_allocation']=[0]*257*80;preparations.append(q)
q=preparation();q['state']['packed_sections']=0x10000000;q['packed_wedges']=0x1fffffff;q['wedges']=[];preparations.append(q)
for change in [lambda q:q.update(packed_wedges=0x1fffffff),lambda q:q['section_allocation'].pop(),lambda q:q.update(packed_faces=65537)]:
 q=preparation();change(q);preparations.append(q)
abs_probes=[[f(rng.randint(-1024,1024)/8) for _ in range(3)] for i in range(256)]
abs_probes += [[0x80000000 if mask&(1<<k) else 0 for k in range(3)] for mask in range(8)]
abs_probes += [[0x7f800000,0,0],[0,0xff800000,0],[0,0,0x7fc01234]]
out=dict(source_sha256=sha(prior),passes_sha256=old['passes_sha256'],cases=cases,errors=errors,preparations=preparations,abs_probes=abs_probes)
(root/'analysis/reports/vertmesh-preprocess.input.json').write_text(json.dumps(out,separators=(',',':'))+'\n')
print(json.dumps(dict(cases=len(cases),requests=sum(len(c['requests']) for c in cases),errors=len(errors),preparations=len(preparations),abs_probes=len(abs_probes))))
