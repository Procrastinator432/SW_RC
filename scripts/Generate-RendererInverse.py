"""Scalar53 dense/singular/range matrices and computed-inverse render fixtures."""
from pathlib import Path
import json,copy,hashlib,random,struct
root=Path(__file__).resolve().parents[1]
def sha(p):return hashlib.sha256(p.read_bytes()).hexdigest()
def f(v):return struct.unpack('<I',struct.pack('<f',v))[0]
prior=root/'analysis/reports/renderer-transform.json'
assert sha(prior)==json.loads((root/'analysis/reports/renderer-transform-validation.json').read_text())['report_sha256']
old=json.loads((root/'analysis/reports/renderer-transform.input.json').read_text());rng=random.Random(10031709)
math=[]
for i in range(512):
 m=[f(rng.uniform(-4,4)) for _ in range(16)]
 if i%5==0:m[4:8]=m[:4]
 if i%5==1:m[4:8]=m[:4];m[7]=(m[7]+1)&0xffffffff
 if i%7==0:m[12:]=[f(-2),f(3),f(.25),f(1)]
 math.append(dict(matrix=m,store_determinant=bool(i%2),alias=bool(i%3==0)))
for x in [0.,-0.,1.,-1.,1e-20,1e-10,1e10,1e20,1e30]:
 for flags in range(4):
  m=[f(x) if k//4==k%4 else 0x80000000 if flags&1 else 0 for k in range(16)]
  m[15]=f(1);math.append(dict(matrix=m,store_determinant=bool(flags&2),alias=bool(flags&1)))
for k in range(16):
 m=[f(int(i//4==i%4)) for i in range(16)];m[k]=0x7fc00000 if k%2 else 0x7f800000
 math.append(dict(matrix=m,store_determinant=True,alias=False))
for diagonal in [[1e12,1e12,1e12,1e12],[-1e12,1e12,1e12,1e12],[1e-40,1e10,1.,1.],[1.401298464324817e-45,1.,1.,1.]]:
 m=[f(diagonal[k//4]) if k//4==k%4 else 0 for k in range(16)]
 math.append(dict(matrix=m,store_determinant=True,alias=True))
cases=copy.deepcopy(old['cases']);errors=copy.deepcopy(old['errors'][:-1])
def wrap(c):
 c['requests']=[dict(renderer=q) for q in c['requests']]
 for q in c['requests']:q['renderer']['inverse']=dict(kind='unresolved')
 return c
cases=[wrap(c) for c in cases];errors=[wrap(c) for c in errors]
q=copy.deepcopy(cases[2]['requests'][2]);t=q['renderer']['mesh']['transform'];t.update(actor_draw_scale=f(1),actor_draw_scale_3d=[f(1)]*3,actor_rotation=[0]*3,mesh_scale=[f(1e30)]*3,mesh_rotation=[0]*3,bypass_mesh_adjustments=1)
errors.append(dict(id=len(errors),runtime=copy.deepcopy(cases[2]['runtime']),requests=[q]))
out=dict(source_sha256=sha(prior),passes_sha256=old['passes_sha256'],math=math,cases=cases,errors=errors)
(root/'analysis/reports/renderer-inverse.input.json').write_text(json.dumps(out,separators=(',',':'))+'\n')
print(json.dumps(dict(math=len(math),cases=len(cases),requests=sum(len(c['requests']) for c in cases),errors=len(errors))))
