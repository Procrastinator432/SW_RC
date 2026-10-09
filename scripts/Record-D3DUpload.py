"""Replay successful original static-buffer lifecycle with external COM/getter responses."""
from pathlib import Path
import json,re,collections,copy
root=Path(__file__).resolve().parents[1];s=root/'scripts/Record-D3DState.py';g={'__file__':str(s)}
exec(compile(s.read_text().split('\nfixtures=json.loads')[0],str(s),'exec'),g)
sha=g['sha'];FRAME=g['FRAME'];D=g['D'];counts=collections.Counter()
assemblies=['d3d-static-upload.asm','d3d-static-index-upload.asm']
for name in assemblies:
 asm=(root/'analysis/decompiled'/name).read_text();g['code'].update({int(a,16):(op,args.split(',') if args else []) for a,op,args in re.findall(r'^([0-9a-f]{8}) ([A-Z.]+)(?: (.*))?$',asm,re.M)})
addresses=sorted(g['code']);g['following']={a:addresses[i+1] for i,a in enumerate(addresses[:-1])}
fixtures=json.loads((root/'analysis/reports/d3d-upload.input.json').read_text());path=root/'analysis/reports/d3d-upload.json';report=json.loads(path.read_text());assert path.read_bytes()==(root/'analysis/reports/d3d-upload-release.json').read_bytes()
assert fixtures['source_sha256']==report['source_sha256']==sha(root/'analysis/reports/d3d-buffers.input.json')==json.loads((root/'analysis/reports/d3d-buffers-validation.json').read_text())['input_sha256']
def put(vm,a,v):vm.put(f'dword ptr [{a}]',v)
def get(vm,a):return vm.get(f'dword ptr [{a}]')
def initialize(resource,source,device,kind):
 vm=g['Native']();vm.r.update(EBP=FRAME,ESP=0x70000000,ECX=resource['address']);a=resource['address'];put(vm,a+4,D);put(vm,a+0x30,resource['handle']);put(vm,a+(0x38 if kind=='vertex' else 0x34),resource['capacity']);put(vm,a+0x10,resource['revision']);put(vm,a+0x3c,resource['source']);put(vm,source['address'],0x24000000);put(vm,FRAME+8,source['address']);put(vm,FRAME-12,0)
 put(vm,D+0x40dc,device['hardware_vertices']);put(vm,D+0x4118,device['special_vertices']);put(vm,D+0x4120,device['skip_eviction']);put(vm,D+0x46b4,device['address']);put(vm,device['address'],0x24000000)
 for handle in [resource['handle'],0x33100000]:
  if handle:put(vm,handle,0x24000000)
 return vm
def call(vm,pc,offset,n,commands,source=False):
 op,args=g['code'][pc];assert op=='CALL' and args[0].endswith(f'+ {hex(offset)}]'),(hex(pc),args,offset)
 receiver=vm.r['ECX'] if source else vm.pop();arguments=[vm.pop() for _ in range(n)];vm.steps+=1
 commands.append(dict(receiver=receiver,vtable_offset=offset,arguments=arguments));return arguments
def positive(vm):return not vm.s
def result(vm,original,kind):
 a=original['address'];return dict(address=a,handle=get(vm,a+0x30),capacity=get(vm,a+(0x38 if kind=='vertex' else 0x34)),revision=get(vm,a+0x10),source=get(vm,a+0x3c))
def vertex(resource,source,device,o):
 vm=initialize(resource,source,device,'vertex');commands=[]
 vm.run(0x1002a39e,0x1002a3b7);vm.r['EAX']=source['size'];vm.run(0x1002a3ba,0x1002a3d1);vm.r['EAX']=source['dynamic'];vm.run(0x1002a3d4,0x1002a3ea);vm.r['EAX']=source['special'];vm.run(0x1002a3ed,0x1002a41b)
 reused=False;usage=get(vm,FRAME-0x14);flags=get(vm,FRAME-0x24)
 if not vm.z:vm.run(0x1002a41d,0x1002a420);reused=vm.z
 if not reused:
  if resource['handle']:
   vm.run(0x1002a426,0x1002a42d);call(vm,0x1002a42d,8,0,commands)
  vm.run(0x1002a430,0x1002a436);i=0;eviction=0
  while True:
   assert vm.r['EBX']<3
   vm.run(0x1002a436,0x1002a460);args=call(vm,0x1002a460,0x5c,5,commands);response=o['creates'][i];i+=1;put(vm,args[4],response['handle']);put(vm,response['handle'],0x24000000);vm.r['EAX']=response['hresult'];vm.run(0x1002a463,0x1002a467)
   if positive(vm):break
   vm.run(0x1002a469,0x1002a477)
   if not vm.z:vm.run(0x1002a479,0x1002a436)
   else:
    vm.run(0x1002a47d,0x1002a488);call(vm,0x1002a488,0x14,1,commands);vm.r['EAX']=o['evictions'][eviction];eviction+=1;vm.run(0x1002a48b,0x1002a48d);assert positive(vm);vm.run(0x1002a4a6,0x1002a436)
  vm.run(0x1002a4e8,0x1002a4f1)
 vm.run(0x1002a4f1,0x1002a509);args=call(vm,0x1002a509,0x2c,4,commands);assert args[2]==o['lock_slot'];put(vm,args[2],o['lock_pointer']);vm.r['EAX']=o['lock_hresult'];vm.run(0x1002a50c,0x1002a50e);assert positive(vm)
 vm.run(0x1002a54a,0x1002a552);call(vm,0x1002a552,0x24,1,commands,True);vm.run(0x1002a555,0x1002a55b);call(vm,0x1002a55b,0x30,0,commands);vm.r['EAX']=o['unlock_hresult'];vm.run(0x1002a55e,0x1002a560);assert positive(vm)
 vm.run(0x1002a59d,0x1002a5a1);vm.r['EAX']=source['revision_after'];vm.run(0x1002a5a4,0x1002a5aa);vm.run(0x1002a5ab,0x1002a5ae)
 plan=dict(reused=reused,usage=usage,lock_flags=flags,size=source['size'],format=None,uploaded=True,commands=commands);counts['vertex_instructions']+=vm.steps;counts['vertex_commands']+=len(commands);counts['vertex_reuses']+=int(reused)
 for c in commands:
  if c['vtable_offset']==0x5c:counts['vertex_creates']+=1;counts['vertex_system_pool']+=int(c['arguments'][3]==2)
  if c['vtable_offset']==0x14:counts['vertex_evictions']+=1
 return plan,result(vm,resource,'vertex')
def index(resource,source,device,o):
 vm=initialize(resource,source,device,'index');commands=[];vm.run(0x1002c28c,0x1002c293);put(vm,FRAME-0x14,resource['address']);vm.r['EAX']=source['size'];vm.run(0x1002c2a5,0x1002c2b9);size=vm.r['EDI'];reused=False;usage=None;format=None
 if not vm.z:vm.run(0x1002c2bb,0x1002c2be);reused=vm.z
 if not reused:
  if resource['handle']:vm.run(0x1002c2c4,0x1002c2cb);call(vm,0x1002c2cb,8,0,commands)
  vm.run(0x1002c2ce,0x1002c2d3);vm.r['EAX']=source['width'];vm.run(0x1002c2d6,0x1002c2e4)
  assert g['code'][0x1002c2e4]==('SETZ',['DL']);vm.put('DL',int(vm.z));vm.steps+=1
  vm.run(0x1002c2e7,0x1002c307);args=call(vm,0x1002c307,0x60,5,commands);usage=args[1];format=args[2];response=o['creates'][0];put(vm,args[4],response['handle']);put(vm,response['handle'],0x24000000);vm.r['EAX']=response['hresult'];vm.run(0x1002c30a,0x1002c30c);assert positive(vm);vm.run(0x1002c348,0x1002c34b)
 vm.run(0x1002c34b,0x1002c350);vm.r['EAX']=source['revision_before'];vm.run(0x1002c353,0x1002c356);uploaded=not vm.z
 if uploaded:
  vm.run(0x1002c35c,0x1002c371);args=call(vm,0x1002c371,0x2c,4,commands);assert args[2]==o['lock_slot'];put(vm,args[2],o['lock_pointer']);vm.r['EAX']=o['lock_hresult'];vm.run(0x1002c374,0x1002c376);assert positive(vm)
  vm.run(0x1002c3b2,0x1002c3bb);call(vm,0x1002c3bb,0x14,1,commands,True);vm.run(0x1002c3be,0x1002c3c3);call(vm,0x1002c3c3,0x30,0,commands);vm.r['EAX']=o['unlock_hresult'];vm.run(0x1002c3c6,0x1002c3c8);assert positive(vm)
 vm.run(0x1002c405,0x1002c40a);vm.r['EAX']=source['revision_after'];vm.run(0x1002c40d,0x1002c413);vm.run(0x1002c414,0x1002c417)
 plan=dict(reused=reused,usage=usage,lock_flags=0,size=size,format=format,uploaded=uploaded,commands=commands);counts['index_instructions']+=vm.steps;counts['index_commands']+=len(commands);counts['index_reuses']+=int(reused);counts['index_uploads']+=int(uploaded)
 return plan,result(vm,resource,'index')
for kind,key in [('vertex','vertices'),('index','indices')]:
 for c,q in zip(fixtures[key],report[key],strict=True):
  if c['expected_error']:
   assert 'Err' in q['result'] and q['resource']==c['resource'];counts[kind+'_safe_errors']+=1;continue
  plan,resource=globals()[kind](c['resource'],c['source'],c['device'],c['responses']);assert q==dict(id=c['id'],result=dict(Ok=plan),resource=resource),(kind,c['id']);counts[kind+'_cases']+=1
for c,q in zip(fixtures['sequences'],report['sequences'],strict=True):
 resource=copy.deepcopy(c['resource']);steps=[]
 for action in c['actions']:
  plan,resource=globals()[c['kind']](resource,action['source'],action['device'],action['responses']);steps.append(dict(plan=plan,resource=resource));counts['sequence_steps']+=1
 assert q==dict(id=c['id'],kind=c['kind'],steps=steps,resource=resource),('sequence',c['id']);counts['sequences']+=1
assert counts['vertex_cases']==288 and counts['index_cases']==480 and counts['vertex_safe_errors']==8 and counts['index_safe_errors']==6 and counts['sequences']==32 and counts['sequence_steps']==128
tests=sum(map(int,re.findall(r'test result: ok\. (\d+) passed',(root/'analysis/reports/d3d-upload-tests.log').read_text())));assert tests==677,tests
sources=['crates/rc-package/src/d3d_upload.rs','crates/rc-inspect/src/bin/rc-d3d-upload-check.rs','scripts/Generate-D3DUpload.py','scripts/Record-D3DUpload.py','scripts/Record-D3DState.py','scripts/Record-MaterialState.py','analysis/decompiled/d3d-buffer-setup.c']+['analysis/decompiled/'+p for p in assemblies]
validation=dict(date='2026-10-09',counts=dict(counts),rust_tests=tests,debug_release_identical=True,original_d3ddrv_sha256=sha(g['dll']),input_sha256=sha(root/'analysis/reports/d3d-upload.input.json'),report_sha256=sha(path),source_sha256={p:sha(root/p) for p in sources},scope=report['scope'],android='deferred until end per user')
(root/'analysis/reports/d3d-upload-validation.json').write_text(json.dumps(validation,indent=2)+'\n')
ledger=root/'analysis/evidence.json';e=json.loads(ledger.read_text(encoding='utf-8'));e['d3d_upload_validation']=validation;e['rust_tests']=tests;ledger.write_text(json.dumps(e,indent=2,ensure_ascii=False)+'\n',encoding='utf-8')
print(json.dumps(dict(counts=dict(counts),rust_tests=tests)))
