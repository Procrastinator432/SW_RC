from pathlib import Path
import json,hashlib,copy
root=Path(__file__).resolve().parents[1];sha=lambda p:hashlib.sha256(p.read_bytes()).hexdigest()
bindings_path=root/'analysis/reports/d3d-bindings.input.json';bv=json.loads((root/'analysis/reports/d3d-bindings-validation.json').read_text());assert sha(bindings_path)==bv['input_sha256']
passes_path=root/'analysis/reports/d3d-transforms.input.json';pv=json.loads((root/'analysis/reports/d3d-transforms-validation.json').read_text());assert sha(passes_path)==pv['input_sha256']
b=json.loads(bindings_path.read_text())['cases'];p=json.loads(passes_path.read_text())['cases']
def lights(i,mask):
 enabled=[([1,0xffffffff,0x80000000,7][(s+i)%4] if mask&(1<<s) else 0) for s in range(8)]
 applied=[enabled[s] if i%3==0 else (0 if i%3==1 else enabled[s]^0x12345678) for s in range(8)]
 words=[[([0x7fc12345,0x80000000,0xff800000,0x3f800000][(i+s+k)%4] if k%3==0 else (i*101+s*109+k*19)&0xffffffff) for k in range(26)] for s in range(8)]
 return dict(words=words,enabled=enabled,applied_enabled=applied)
tails=[dict(id=i,lights=lights(i,i),dirty=0x80,capacity=8) for i in range(256)]
for cap in [-2147483648,-1]+list(range(9))+[9,2147483647]:
 for j in range(4):
  i=len(tails);tails.append(dict(id=i,lights=lights(i,(i*37)&255),dirty=0x80 if j%2 else 0,capacity=cap))
selectors=[]
for hardware in [False,True]:
 for kind in [0,1,2,0xffffffff]:
  for handle in [None,0,1,0xdeadbeef]:
   for initialized in [False,True]:
    i=len(selectors);selectors.append(dict(id=i,bindings=copy.deepcopy(b[i]['deferred']['bindings']),dirty=i%16,kind=kind,choice=dict(hardware=hardware,resolved=handle),initialized=initialized))
rows=[]
for i,c in enumerate(p):
 c=copy.deepcopy(c);choice=dict(hardware=bool(i%2),resolved=(0x41000000+i*64)&0xffffffff)
 if i<256:c['pass']['header'][:4]=list(([0,1,2,0xffffffff][(i//2)%4]).to_bytes(4,'little'))
 d=dict(states=c['cache'],transforms=c['transforms'],bindings=copy.deepcopy(b[i]['deferred']['bindings']))
 d['states']['dirty']|=0x80 if i%3 else 0
 rows.append(dict(id=i,material_id=c['material_id'],complete=dict(deferred=d,lights=lights(i,i&255)),pass_=c['pass'],resources=c['resources'],device=c['device'],last_pass=c['last_pass'],stencil_gate=c['stencil_gate'],choice=choice,initialized=bool(i%3),stream_capacity=b[i]['stream_capacity'],light_capacity=[-1,0,1,3,8,9][i%6]))
for c in rows:c['pass']=c.pop('pass_')
flushes=[]
for i in range(256):
 c=copy.deepcopy(b[i]);c['deferred']['states']['dirty']=i
 if i==0:c['deferred']['transforms']['mask']=0xdeadbeef # entry skip must preserve this
 flushes.append(dict(id=i,complete=dict(deferred=c['deferred'],lights=lights(i,i)),texture_capacity=c['texture_capacity'],stream_capacity=c['stream_capacity'],light_capacity=[-1,0,1,3,8,9][i%6],stencil_gate=c['stencil_gate']))
(root/'analysis/reports/d3d-complete.input.json').write_text(json.dumps(dict(source_bindings_sha256=sha(bindings_path),source_passes_sha256=sha(passes_path),lights=tails,selectors=selectors,cases=rows,flushes=flushes),separators=(',',':'))+'\n')
print(json.dumps(dict(lights=len(tails),selectors=len(selectors),passes=len(rows),flushes=len(flushes))))
