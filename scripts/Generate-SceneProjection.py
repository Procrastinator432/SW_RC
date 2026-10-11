"""Synthetic scene matrix probes and checked LOD fixtures; no live capture."""
from pathlib import Path
import json, hashlib, struct, copy, random
root=Path(__file__).resolve().parents[1]
def sha(p):return hashlib.sha256(p.read_bytes()).hexdigest()
def f(x):return struct.unpack('<I',struct.pack('<f',x))[0]
def identity():return [[f(int(r==c)) for c in range(4)] for r in range(4)]
prior=root/'analysis/reports/vertmesh-lod.input.json'
assert sha(prior)==json.loads((root/'analysis/reports/vertmesh-lod-validation.json').read_text())['input_sha256']
old=json.loads(prior.read_text());rng=random.Random(1043860)
def wrap(q,i):
 m=[[f(rng.randint(-16,16)/8) for c in range(4)] for r in range(4)]
 for r in range(3):m[r][3]=0
 m[3][3]=f([0.5,1,2,4,16,-2][i%6])
 q['camera']['projected_w']=0x7fc00000 # must be replaced before camera validation
 return dict(world_to_screen=m,actor_position=[f(rng.randint(-32,32)/4) for _ in range(3)],mesh=q)
cases=copy.deepcopy(old['cases'])
for i,c in enumerate(cases):c['requests']=[wrap(q,i+j) for j,q in enumerate(c['requests'])]
errors=[]
def error(mutate):
 c=copy.deepcopy(cases[2]);c['id']=len(errors);c['requests']=c['requests'][:1];mutate(c['requests'][0]);errors.append(c)
error(lambda q:q['world_to_screen'][3].__setitem__(3,0))
error(lambda q:q['world_to_screen'][3].__setitem__(3,0x80000000))
error(lambda q:q['actor_position'].__setitem__(0,0x7fc00000))
error(lambda q:q['world_to_screen'][0].__setitem__(0,0x7f800000))
error(lambda q:q['world_to_screen'][3].__setitem__(3,1))
error(lambda q:(q['world_to_screen'][0].__setitem__(0,f(3.4e38)),q['actor_position'].__setitem__(0,f(4))))
# Keep later rollback failures from the checked LOD fixture (skip overridden W errors).
for i in [0,3,4,5,6,7,8,9,10,11]:
 c=copy.deepcopy(old['errors'][i]);c['id']=len(errors);c['requests']=[wrap(q,2) for q in c['requests']];errors.append(c)
math=[]
for i in range(256):
 m=[[f(rng.randint(-64,64)/8) for _ in range(4)] for _ in range(4)]
 p=[f(rng.randint(-64,64)/8) for _ in range(3)]
 # Guarantee nonzero clip W without making the other matrix columns trivial.
 for r in range(3):m[r][3]=0
 m[3][3]=f([0.125,0.5,1,3,-2,16][i%6])
 inv=[[f(rng.randint(-64,64)/8) for _ in range(4)] for _ in range(4)]
 math.append(dict(matrix=m,inverse=inv,position=p,plane=[f(rng.randint(-64,64)/8) for _ in range(4)],project_ok=True,deproject_ok=True))
# Full perspective W depends on all three coordinates, with exact nonzero dyadic result.
for i in range(64):
 q=copy.deepcopy(math[i]);q['matrix'][0][3]=f(0.25);q['matrix'][1][3]=f(-0.5);q['matrix'][2][3]=f(0.125);q['matrix'][3][3]=f(128);math.append(q)
# Cancellation exposes native component-specific operation order.
q=dict(matrix=identity(),inverse=[[0]*4 for _ in range(4)],position=[f(1)]*3,plane=[f(1)]*4,project_ok=True,deproject_ok=True)
q['matrix'][1][0]=f(1e20);q['matrix'][2][0]=f(-1e20)
for c in range(3):q['inverse'][2][c]=f(1e20);q['inverse'][1][c]=f(-1e20);q['inverse'][0][c]=f(1)
math.append(q)
for word in [0,0x80000000,1,0x7fc00000,0x7f800000]:
 q=dict(matrix=identity(),inverse=identity(),position=[0]*3,plane=[f(1),0,0,f(1)],project_ok=False,deproject_ok=True)
 q['matrix'][3][3]=word;math.append(q)
q=copy.deepcopy(math[0]);q['plane'][3]=0x7fc00000;q['deproject_ok']=False;math.append(q)
out=dict(source_sha256=sha(prior),passes_sha256=old['passes_sha256'],cases=cases,errors=errors,math=math)
(root/'analysis/reports/scene-projection.input.json').write_text(json.dumps(out,separators=(',',':'))+'\n')
print(json.dumps(dict(cases=len(cases),requests=sum(len(c['requests']) for c in cases),errors=len(errors),math=len(math))))
