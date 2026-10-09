from pathlib import Path
import json,struct,hashlib
root=Path(__file__).resolve().parents[1]
source=root/'analysis/reports/material-state.json';native=json.loads(source.read_text());validation=json.loads((root/'analysis/reports/material-state-validation.json').read_text())
assert hashlib.sha256(source.read_bytes()).hexdigest()==validation['report_sha256']
rows=[]
for i in range(256):
 p=list(struct.pack('<7I',0,i%64,((i*13)&255),i//64,(i*31)&0xffffffff,5,2))
 rows.append({'pass':p,'material_id':None})
for p in native['materials']:
 if p.get('status'):rows.append({'pass':p['host']['pass'],'material_id':p['id']})
for i,row in enumerate(rows):
 sample=native['samplers'][(i*11)%3072];stage=sample['stage'][:];stage[4]&=~0x40
 stage[1]=[0xc2ca0000,0xc2c80000,0x7fc12345,0x80000000,0x7f800000,0xff800000,0x3f000000][i%7]
 values={'render':[(i*101+k*13)&0xffffffff for k in range(32)],'stages':[[(i*103+s*17+k*19)&0xffffffff for k in range(21)] for s in range(8)],'textures':[(i*107+s*31)&0xffffffff for s in range(8)]}
 values['render'][11]=i%2
 applied=json.loads(json.dumps(values))
 for k in range(32):
  if k%3==i%3:applied['render'][k]^=0x12345678
 for s in range(8):
  for k in range(21):
   if (s+k)%3==i%3:applied['stages'][s][k]^=0x87654321
  if s%3==i%3:applied['textures'][s]^=0xfedcba98
 row.update(id=i,cache={'desired':values,'applied':applied,'dirty':[0,1,2,0x10,0x13][i%5]},stage=stage,index=i%8,resource=None if stage[0]==0 else [0x30000000+i*64,sample['resource']['word_3c']],hardware=bool(i%2),lod_bias=0xbf800000,cull_mode=(i*73)^0xabcdef,capacity=i%9,stencil_gate=bool(i%3))
(root/'analysis/reports/d3d-state.input.json').write_text(json.dumps({'cases':rows,'source_material_report_sha256':validation['report_sha256']},separators=(',',':'))+'\n')
print(len(rows))
