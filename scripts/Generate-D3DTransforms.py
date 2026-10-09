from pathlib import Path
import json,hashlib,copy
root=Path(__file__).resolve().parents[1]
sha=lambda p:hashlib.sha256(p.read_bytes()).hexdigest()
source=root/'analysis/reports/d3d-pass.input.json'
validation=json.loads((root/'analysis/reports/d3d-pass-validation.json').read_text())
assert sha(source)==validation['input_sha256']
prior=json.loads(source.read_text())['cases'];rows=[]
def matrices(i,mask):
 return dict(words=[[(i*101+s*1009+k*2137)&0xffffffff for k in range(16)] for s in range(11)],mask=mask)
for i in range(256+len(prior)):
 c=copy.deepcopy(prior[(i-256) if i>=256 else i%len(prior)])
 c['id']=i;c['transforms']=matrices(i,(i*131)&0xffffffff)
 if i<256:c['pass']['header'][9]=8;c['last_pass']=0;c['material_id']=None
 pattern=i&255
 for s,stage in enumerate(c['pass']['stages']):
  stage[4]=(stage[4]&~0x40)|(0x40 if pattern&(1<<s) else 0)
  stage[5]=[0,0x1ff,0x200,0xfffff9ab][(i+s)%4]
  stage[6:22]=[([0x7fc12345,0xff800000,0x80000000,0x3f800000][(i+s+k)%4] if k%3==0 else (i*103+s*31+k*19)&0xffffffff) for k in range(16)]
 rows.append(c)
v=dict(render=[0]*32,stages=[[0]*21 for _ in range(8)],textures=[0]*8)
flush=[]
for mask,gate in [(m,0x60) for m in range(2048)]+[(1<<s,g) for s in range(11) for g in [0,0x20,0x40,0x60]]:
 i=len(flush);cache=dict(desired=copy.deepcopy(v),applied=copy.deepcopy(v),dirty=gate|0x11)
 cache['desired']['render'][29]=0xdeadbeef;cache['desired']['textures'][0]=0x12340000+i
 flush.append(dict(id=i,cache=cache,transforms=matrices(i,mask|0x80000000),capacity=i%9,stencil_gate=False))
(root/'analysis/reports/d3d-transforms.input.json').write_text(json.dumps(dict(source_sha256=sha(source),cases=rows,flush_cases=flush),separators=(',',':'))+'\n')
print(json.dumps(dict(passes=len(rows),flushes=len(flush))))
