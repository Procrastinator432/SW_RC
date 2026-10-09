from pathlib import Path
import json,hashlib,copy,struct
root=Path(__file__).resolve().parents[1];sha=lambda p:hashlib.sha256(p.read_bytes()).hexdigest();source=root/'analysis/reports/d3d-upload.input.json';assert sha(source)==json.loads((root/'analysis/reports/d3d-upload-validation.json').read_text())['input_sha256'];prior=json.loads(source.read_text())
def regions(i,skin,count,mode=0,owner=0):
 address=0x35000000+i*0x100;data=0x38000000+i*0x1000;header=bytearray(36)
 struct.pack_into('<I',header,0x10,owner if skin else data)
 if skin:struct.pack_into('<III',header,0x18,mode,data,count)
 else:struct.pack_into('<I',header,0x14,count)
 payload=bytes(((i*13+k*17)^0x87)&255 for k in range(256))
 return address,[dict(address=address,bytes=list(header[:19])),dict(address=address+19,bytes=list(header[19:])),dict(address=data,bytes=list(payload))]
indices=[]
for width in ['U16','U32']:
 for flags in range(8):
  for n in [0,1,2,3,7,31,0x08000000,0x10000000,0x1fffffff]:
   i=len(indices);a,r=regions(i,False,(flags<<29)|n);indices.append(dict(id=i,address=a,regions=r,width=width))
skins=[]
for flags in range(8):
 for n in [0,1,2,3,7,0x08000000,0x10000000,0x1fffffff]:
  for mode in [0,255]:
   for owner in [0,0x39000000]:
    i=len(skins);a,r=regions(i,True,(flags<<29)|n,mode,owner);skins.append(dict(id=i,address=a,regions=r,owner_count=(i%8),delegate=[(i+k*7)&255 for k in range((i%8)*32)],initial=[(i*11+k*3)&255 for k in range(16)],vertex=[0,1,-1,-0x80000000,0x7fffffff][i%5]))
errors=[]
for i in range(6):
 a,r=regions(i,True,1)
 if i==0:a=0
 if i==1:r=r[2:]
 if i==2:r[1]['address']+=1
 if i==3:r[1]['address']-=1
 if i==4:a=0xfffffff0
 if i==5:r[0]['bytes']=[]
 errors.append(dict(id=i,address=a,regions=r))
transfers=[]
for kind in ['vertex','index']:
 for i in range(32):
  j=i if kind=='vertex' else i+32;skin=kind=='vertex';a,r=regions(j,skin,i%8,0 if i%3 else 1,0 if i%3 else 0x39000000)
  old=copy.deepcopy(prior['vertices' if skin else 'indices'][0]);old['responses']['creates']=[dict(hresult=0,handle=0x33100000)];old['responses']['evictions']=[0]*3;old['responses']['lock_slot']=0x27000000-(0x2c if skin else 0x14)
  transfers.append(dict(id=len(transfers),kind=kind,address=a,regions=r,width='U16' if i%2 else 'U32',owner_count=i%8,delegate=[(i+k*7)&255 for k in range((i%8)*32)],dynamic=i%2,special=i%3,revision_before=7 if i%3 else 8,revision_after=i+101,resource=old['resource'],device=old['device'],responses=old['responses'],destination=[0xcd]*272))
payload=dict(source_sha256=sha(source),indices=indices,skins=skins,errors=errors,transfers=transfers)
(root/'analysis/reports/d3d-source.input.json').write_text(json.dumps(payload,separators=(',',':'))+'\n');print(json.dumps({k:len(v) for k,v in payload.items() if isinstance(v,list)}))
