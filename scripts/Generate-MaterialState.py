from pathlib import Path
import json,struct
root=Path('.')
def regions(mem):
 result=[]
 for a,b in sorted(mem.items()):
  if result and result[-1]['address']+len(result[-1]['bytes'])==a:result[-1]['bytes'].append(b)
  else:result.append({'address':a,'bytes':[b]})
 return result
def builder():
 m={}
 def put(a,raw):
  for k,b in enumerate(raw):
   assert a+k not in m;m[a+k]=b
 def word(a,v):put(a,struct.pack('<I',v&0xffffffff))
 return m,put,word
materials=[]
for i in range(512):
 b=0x100000+i*0x20000;r=b;d=b+0xa000;s=b+0x10000;p=b+0x11000;h=b+0x12000
 m,put,word=builder();word(r+4,d);word(r+0x9c0c,s);word(r+0x9fd0,((i*1009)^0xa5a5a5a5)&0xffffffff)
 caps=[0 if (i//32)%4 in (0,2) else (0xffffffff if i%2 else 1),0 if (i//32)%4 in (0,1) else 1]
 word(d+0x4218,caps[0]);word(d+0x4220,caps[1]);word(s+0x304,p);put(s+0x324,bytes([(i*13)&255]));put(p,bytes((i*29+k*17)&255 for k in range(28)))
 word(h+0x28,b+0x17000);word(h+0x92c,((i*0x9e3779b9)&0xffffffe0)|(i%32));put(h+0x930,bytes([(i*17)&255,i&255,(i^255)&255]))
 materials.append({'id':i,'renderer':r,'shader':h,'regions':regions(m),'setup_return':[-1,0,3,-2][i//128],'mutate':i%3==0,'diagnostic_present':i%4!=0,'fallback_present':i%4<2})
samplers=[]
for i in range(3088):
 b=0x5000000+i*0x20000;r=b;d=b+0xa000;t=b+0x14000;res=b+0x15000
 m,put,word=builder();null=i>=3072;mode=i%3;u=(i//12)%256;v=[0,1,2,255][(i//3)%4];filter=(i*1009)^0xa5a5a5a5
 if not null:
  word(r+4,d);word(d+0x466c,filter);put(t+0x65,bytes([u,v]));put(t+0x7c,bytes([(i*17)&255]))
  if mode:word(res+0x3c,0 if mode==1 else 0x80000000)
 initial=[((i*1009+k*97)*0x9e3779b9)&0xffffffff for k in range(28)]
 samplers.append({'id':i,'renderer':r if not null else 0xffffffff,'texture':t if not null else 0,'resource':res if mode else 0,'regions':regions(m),'stage':initial,'renderer_flags':((i*13)^0xa5a5a5a5)&0xffffffff})
import copy
bad=[]
for i in range(6):
 q=copy.deepcopy(materials[511]);q['id']=512+i
 if i==0:q['renderer']=0xffffffff
 elif i==1:q['shader']=0
 elif i in (2,3):
  target=q['renderer']+0x9c0c if i==2 else q['renderer']+0x10000+0x304
  for reg in q['regions']:
   if reg['address']<=target and target+4<=reg['address']+len(reg['bytes']):reg['bytes'][target-reg['address']:target-reg['address']+4]=[0]*4;break
 elif i==4:
  q['regions']=[reg for reg in q['regions'] if reg['address']!=q['shader']+0x92c]
 else:
  for reg in q['regions']:
   if reg['address']==q['renderer']+0x11000:reg['bytes'].pop();break
 bad.append(q)
materials+=bad
for i in range(2):
 q=copy.deepcopy(samplers[2]);q['id']=3088+i
 if i==0:
  for reg in q['regions']:
   if reg['address']==q['resource']+0x3c:reg['bytes'].pop();break
 else:q['texture']=0xffffffff
 samplers.append(q)
(root/'analysis/reports/material-state.input.json').write_text(json.dumps({'materials':materials,'samplers':samplers},separators=(',',':'))+'\n')
print(len(materials),len(samplers))
