"""Raw vertex-mesh sections with distinct supplied material/pass answers."""
from pathlib import Path
import json,copy,hashlib,struct
root=Path(__file__).resolve().parents[1]
def sha(p):return hashlib.sha256(p.read_bytes()).hexdigest()
prior=root/'analysis/reports/d3d-particle-draw.input.json';passes=root/'analysis/reports/d3d-draw.input.json'
for path,name in [(prior,'d3d-particle-draw'),(passes,'d3d-draw')]:assert sha(path)==json.loads((root/('analysis/reports/'+name+'-validation.json')).read_text())['input_sha256']
old=json.loads(prior.read_text());draws=json.loads(passes.read_text());rich=[c for c in draws['cases'] if c['passes']];cases=[]
for i,c in enumerate(old['cases']):
 requests=[]
 for mode in range(3):
  base=copy.deepcopy(c['requests'][4]);n=6;raw=bytearray(n*80);primitive=[]
  for j in range(n):
   struct.pack_into('<4H',raw,j*80,j%3,(j*13+i)%65536,[0,7,65535][j%3],[19,3,0][j%3])
   struct.pack_into('<H',raw,j*80+16,0 if (i+j)%7==0 else [1,0x8000,65535][j%3])
   primitive.append(0 if (i+j)%5==0 else [3,7,0xffffffff][j%3])
  packed=[0,0xe0000000,0x1fffffff,0x10000000][i%4] if i%12<2 else [6,0xe0000006][i%2]
  resolved=[0x61000000+(j%3)*0x100 for j in range(n)]
  if i%8==0:resolved[0]=0
  materials=dict(renderer_address=0x21000000,mesh_address=0x39000000,actor_address=0x3e000000,actor_object=0x3e001000,resolver=0x3e005000,slots=[0x62000000+j*0x100 for j in range(3)],resolved=resolved,wire=mode==1,debug=0x80000000 if mode==2 else 0,override_packed_count=[0,1,2,3,0x1fffffff,0xe0000003][i%6],overrides=[0x63000000+j*0x100 for j in range(3)],wire_shader=0x64000000)
  bank=[]
  for k,token in enumerate(sorted(set(resolved+materials['overrides']+[materials['wire_shader']]))):
   pp=copy.deepcopy(rich[(i*7+k)%len(rich)]['passes']);mapping={}
   for p in pp:
    oldaddress=p['pass']['address']
    if oldaddress not in mapping:mapping[oldaddress]=0x50000000+k*0x10000+len(mapping)*0x1000
    p['pass']['address']=mapping[oldaddress]
   if k==6 and i%7==0:pp=[]
   bank.append(dict(material=token,passes=pp))
  # Same native pass identity may be shared by distinct material outputs.
  if bank[0]['passes'] and bank[1]['passes']:bank[1]['passes'][0]=copy.deepcopy(bank[0]['passes'][0])
  requests.append(dict(vertex=base['vertex'],index=base['index'],sections=dict(packed_count=packed,bytes=list(raw),primitive_counts=primitive),materials=materials,bank=bank,context=base['context']))
 cases.append(dict(id=i,runtime=copy.deepcopy(c['runtime']),requests=requests))
errors=[]
def error(mutate):
 c=copy.deepcopy(cases[3]);c['id']=len(errors);c['requests']=c['requests'][:1];mutate(c);errors.append(c)
error(lambda c:c['requests'][0]['sections'].update(packed_count=257))
error(lambda c:c['requests'][0]['sections']['bytes'].clear())
error(lambda c:c['requests'][0]['sections']['primitive_counts'].clear())
error(lambda c:c['requests'][0]['materials']['slots'].clear())
error(lambda c:c['requests'][0]['materials']['resolved'].clear())
error(lambda c:c['requests'][0]['materials']['overrides'].clear())
error(lambda c:c['requests'][0]['materials'].update(wire=True,wire_shader=0))
error(lambda c:c['requests'][0]['bank'].clear())
error(lambda c:c['requests'][0]['bank'].append(copy.deepcopy(c['requests'][0]['bank'][0])))
error(lambda c:c['requests'][0]['bank'][1]['passes'][0].update(fog_color=123))
error(lambda c:c['requests'][0]['index']['upload']['lock'].update(unlock_hresult=0x80004005))
error(lambda c:c['runtime']['index_target'].clear())
# Fail only at a later active section, after earlier material passes executed.
def late(c):
 q=c['requests'][0];q['materials']['override_packed_count']=0
 q['sections']['primitive_counts']=[3]*6
 for j in range(6):q['sections']['bytes'][j*80+16]=1
 token=q['materials']['resolved'][2];bank=next(m for m in q['bank'] if m['material']==token)
 bank['passes'][0]['pass']['address']=0
 c['runtime']['last_pass']=0xdeadbeef
error(late)
error(lambda c:c['requests'][0]['materials'].update(resolver=0))
error(lambda c:c['requests'][0]['vertex']['source'].update(stride=0))
error(lambda c:c['requests'][0]['bank'][0].update(passes=c['requests'][0]['bank'][0]['passes']*9))
# The exact late-error request succeeds through its first two sections. This
# prefix remains in the native replay and demonstrates work before the failure.
prefix=copy.deepcopy(errors[12]);prefix['id']=len(cases)
prefix['requests'][0]['sections']['packed_count']=2;cases.append(prefix)
(root/'analysis/reports/d3d-mesh-draw.input.json').write_text(json.dumps(dict(source_sha256=sha(prior),passes_sha256=sha(passes),cases=cases,errors=errors)),newline='\n')
print(json.dumps(dict(cases=len(cases),requests=sum(len(c['requests']) for c in cases),safe_errors=len(errors))))
