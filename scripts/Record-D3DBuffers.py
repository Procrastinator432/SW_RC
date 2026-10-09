"""Original x86 static-buffer handoff replay with explicit external getter/cache results."""
from pathlib import Path
import json,re,collections,copy
root=Path(__file__).resolve().parents[1]
s=root/'scripts/Record-D3DDraw.py';env={'__file__':str(s)};src=s.read_text()
exec(compile(src.split('\nfixtures=json.loads')[0],str(s),'exec'),env)
exec(compile(src[src.index('sample=json.loads'):src.index('\nfor c,q in zip(fixtures[\'draws\']')],str(s),'exec'),env)
g=env['g'];previous=env['env'];sha=env['sha'];BASE=env['BASE'];D=env['D'];R=env['R'];FRAME=env['FRAME'];STATE=env['STATE'];put=env['put'];get=env['get'];write=env['write']
asm=(root/'analysis/decompiled/d3d-buffer-setup.asm').read_text();g['code'].update({int(a,16):(op,args.split(',') if args else []) for a,op,args in re.findall(r'^([0-9a-f]{8}) ([A-Z.]+)(?: (.*))?$',asm,re.M)});addresses=sorted(g['code']);g['following']={a:addresses[i+1] for i,a in enumerate(addresses[:-1])}
fixtures=json.loads((root/'analysis/reports/d3d-buffers.input.json').read_text());path=root/'analysis/reports/d3d-buffers.json';report=json.loads(path.read_text());assert path.read_bytes()==(root/'analysis/reports/d3d-buffers-release.json').read_bytes()
assert fixtures['source_sha256']==report['source_sha256']==sha(root/'analysis/reports/d3d-draw.input.json')==json.loads((root/'analysis/reports/d3d-draw-validation.json').read_text())['input_sha256']
counts=collections.Counter();previous['counts']=counts
def initialize(s,d):
 vm=previous['ns']['initialize'](d['states'],d['bindings'],d['transforms']);put(vm,R+0x9c0c,STATE)
 for key,off in [('declaration',0xec),('hardware_vertex',0x480),('stream_count',0x47c),('index_wrapper',0x484),('base_vertex',0x488)]:put(vm,STATE+off,s[key])
 for i,w in enumerate(s['declarations']):write(vm,STATE+0x33c+i*20,w)
 write(vm,STATE+0x48c,s['wrappers'])
 for i,b in enumerate(s['strides']):vm.m[STATE+0x4cc+i]=b
 vm.m[STATE+0x4dc]=s['previous_stream_count'];return vm
def state(vm):
 s={key:get(vm,STATE+off) for key,off in [('declaration',0xec),('hardware_vertex',0x480),('stream_count',0x47c),('index_wrapper',0x484),('base_vertex',0x488)]};n=s['stream_count'];s['stream_count']=n if n<0x80000000 else n-0x100000000
 s.update(declarations=[[get(vm,STATE+0x33c+i*20+k*4) for k in range(5)] for i in range(16)],wrappers=[get(vm,STATE+0x48c+i*4) for i in range(16)],strides=[vm.m[STATE+0x4cc+i] for i in range(16)],previous_stream_count=vm.m[STATE+0x4dc]);return s
def deferred(vm):return previous['ns']['deferred'](vm)
def shader_memory(vm,shader):
 assert shader and shader['address'];put(vm,shader['address']+0x150,shader['handle']);vm.r['EAX']=shader['address']
def restore(vm,shader):
 vm.r.update(ECX=R);vm.run(0x100201c1,0x100201d1);positive=not vm.z and vm.s==vm.o
 if positive:shader_memory(vm,shader);vm.run(0x100201e3,0x1002021b)
 return positive
def streams(vm,r):
 size=len(r['streams']);put(vm,FRAME+0x10,size);put(vm,FRAME+8,r['shader_kind']);put(vm,FRAME+12,0x37000000);put(vm,D+0x46a8,r['frame'])
 vm.r.update(EBP=FRAME,EDI=R,EBX=0,ECX=size,EDX=size,ESP=0x70000000);vm.run(0x100202b0,0x10020300)
 for i,s in enumerate(r['streams']):
  vm.r.update(ESI=i,EAX=s['declaration'][4],EDI=R,EBP=FRAME);write(vm,FRAME-0x34,s['declaration'][:4]);vm.run(0x10020327,0x10020357)
 put(vm,FRAME-0x14,0);uploads=[];usage=[]
 for i,s in enumerate(r['streams']):
  a=s['wrapper'];put(vm,a+0x10,s['cached_revision']);put(vm,a+0x30,s['handle']);put(vm,0x37000000+i*4,0x38000000+i*0x100);put(vm,0x38000000+i*0x100,0)
  vm.r.update(EBX=a,EAX=s['source_revision'],ESI=i,EDI=R,EBP=FRAME);vm.run(0x100203bd,0x100203c0)
  if not vm.z:
   uploads.append(i);vm.r['EAX']=s['upload_size'];vm.run(0x100203cd,0x100203d8)
  vm.run(0x100203e3,0x100203f7);vm.r['EAX']=s['stride'];vm.run(0x100203fa,0x10020435)
  usage.append(dict(wrapper=a,frame=get(vm,a+0x14)))
 vm.r.update(EDX=size,EDI=R,EBX=0);vm.run(0x10020440,0x1002044c);vm.run(0x1002044c,0x1002044f);fixed=False
 if vm.z:
  vm.run(0x10020451,0x1002045d)
  if not vm.z and vm.s==vm.o:
   shader_memory(vm,r['shader']);vm.run(0x1002046e,0x100204a2);fixed=True
 return dict(upload_slots=uploads,upload_size=get(vm,FRAME-0x14),usage=usage,fixed_shader=fixed)
def index(vm,r):
 s=r['index'];put(vm,FRAME+8,s['source']);put(vm,FRAME+12,r['base_vertex']);put(vm,FRAME-0x14,0);put(vm,D+0x46a8,r['frame']);vm.r.update(ESI=R,EBP=FRAME,ESP=0x70000000);vm.run(0x100208c2,0x100208cd)
 present=not vm.z
 if present:vm.r['EAX']=s['size'];vm.run(0x100208da,0x100208dc);present=not vm.z
 if not present:
  vm.r.update(EDI=0,ESI=R);vm.run(0x100209a6,0x10020a21);return dict(upload=False,upload_size=0,usage=None)
 a=s['wrapper'];put(vm,a+0x10,s['cached_revision']);put(vm,a+0x30,s['handle']);vm.r.update(EDI=a,ESI=R,EAX=s['source_revision']);vm.run(0x1002092e,0x10020931);upload=not vm.z
 vm.r['ECX']=int(upload);vm.run(0x10020945,0x10020995)
 return dict(upload=upload,upload_size=s['size'] if upload else 0,usage=dict(wrapper=a,frame=get(vm,a+0x14)))
for c,q in zip(fixtures['streams'],report['streams'],strict=True):
 r=c['request'];invalid=len(r['streams'])>16 or c['state']['previous_stream_count']>16 or any(s['wrapper']==0 for s in r['streams']) or (r['shader_kind']==0 and r['streams'] and (r['shader'] is None or r['shader']['address']==0))
 if invalid:
  assert 'Err' in q['result'] and q['state']==c['state'] and q['deferred']==c['deferred'];counts['expected_stream_errors']+=1;continue
 vm=initialize(c['state'],c['deferred']);plan=streams(vm,r)
 assert q==dict(id=c['id'],result=dict(Ok=plan),state=state(vm),deferred=deferred(vm)),('streams',c['id'])
 counts['stream_cases']+=1;counts['stream_instructions']+=vm.steps;counts['stream_uploads']+=len(plan['upload_slots']);counts['stream_usage_writes']+=len(plan['usage']);counts['stream_fixed_restores']+=int(plan['fixed_shader'])
for c,q in zip(fixtures['indices'],report['indices'],strict=True):
 r=c['request'];s=r['index'];invalid=s['source']!=0 and s['size']!=0 and s['wrapper']==0
 if invalid:
  assert 'Err' in q['result'] and q['state']==c['state'] and q['deferred']==c['deferred'];counts['expected_index_errors']+=1;continue
 vm=initialize(c['state'],c['deferred']);plan=index(vm,r)
 assert q==dict(id=c['id'],result=dict(Ok=plan),state=state(vm),deferred=deferred(vm)),('index',c['id'])
 counts['index_cases']+=1;counts['index_instructions']+=vm.steps;counts['index_uploads']+=int(plan['upload']);counts['index_usage_writes']+=int(plan['usage'] is not None)
for c,q in zip(fixtures['restores'],report['restores'],strict=True):
 invalid=c['state']['stream_count']>0 and (c['shader'] is None or c['shader']['address']==0)
 if invalid:
  assert 'Err' in q['result'] and q['state']==c['state'] and q['deferred']==c['deferred'];counts['expected_restore_errors']+=1;continue
 vm=initialize(c['state'],c['deferred']);changed=restore(vm,c['shader'])
 assert q==dict(id=c['id'],result=dict(Ok=changed),state=state(vm),deferred=deferred(vm)),('restore',c['id'])
 counts['restore_cases']+=1;counts['restore_instructions']+=vm.steps
def draw_sequence(vm,submission,context,draw):
 passes=submission['passes'];device=context['device'];put(vm,STATE+0x300,submission['last_pass']);vm.m[STATE+0x324]=len(passes);vm.m[STATE+0x17]=context['fog_enabled'];put(vm,STATE+0xf0,device['cull_mode']);put(vm,STATE+0x2f8,context['restore_fog']);put(vm,D+0x4140,device['lod_bias']);put(vm,D+0x41e8,device['capacity'])
 for key,off in [('draw_passes',0xfec0),('submitted_primitives',0xfc90),('submitted_vertices',0xfcb8)]:put(vm,D+off,submission['counters'][key])
 loaded=set()
 for i,p in enumerate(passes):
  a=p['pass']['address'];put(vm,STATE+0x304+i*4,a)
  if a not in loaded:env['load_pass'](vm,p);loaded.add(a)
 results=[]
 for i,p in enumerate(passes):
  a=p['pass']['address'];previous['P']=a;vm.r.update(ESI=R,ECX=i,EBP=FRAME,ESP=0x70000000);vm.run(0x10020cf0,0x10020cff);assert not (vm.z or vm.s==vm.o)
  vm.run(0x10020d05,0x10020d16);assert get(vm,vm.r['ESP'])==a;vm.r['ESP']+=8
  for j,w in enumerate(p['pass']['stages']):
   if w[0]:write(vm,w[0]+0x38,p['resources'][j])
  skipped=previous['guard'](vm);immediate=None
  if not skipped:
   immediate=previous['select'](vm,int.from_bytes(bytes(p['pass']['header'][:4]),'little'),p['shader'],True)
   vm.r.update(EBX=R,ESI=a,EDI=1,EBP=FRAME);put(vm,FRAME+12,device['cull_mode']);vm.run(0x1001efb7,0x1001f165)
   for j in range(vm.m[a+9]):
    vm.r.update(EDX=j,EBX=R,ESI=a,ECX=2,EBP=FRAME);put(vm,FRAME+8,a);put(vm,FRAME+12,j);vm.xmm['XMM0']=g['pe_word'](0x100726c4);vm.run(0x1001f176,0x1001f58e)
   vm.r.update(EBX=R,ESI=a,ECX=2,EDI=1);vm.run(0x1001f59f,0x1001f6ae)
  overridden=env['fog_begin'](vm);commands=previous['flush'](vm,device['capacity'],context['stream_capacity'],context['light_capacity'],context['stencil_gate']);call=env['dispatch'](vm,draw);env['finish'](vm,draw,i)
  results.append(dict(address=a,selection=dict(changed=not skipped,immediate=immediate),commands=commands,draw=call,fog_override=overridden));counts['integration_passes']+=1;counts['integration_draw_calls']+=int(call is not None)
 vm.r.update(ESI=R,ECX=len(passes));vm.run(0x10020cf0,0x10020cff);assert vm.z or vm.s==vm.o
 mutated=copy.deepcopy(passes)
 for p in mutated:p['pass']=env['raw_pass'](vm,p['pass']['address'])
 return results,mutated
for c,q in zip(fixtures['cases'],report['probes'],strict=True):
 sub=c['submission'];vm=initialize(sub['state'],sub['complete']['deferred']);lights=sub['complete']['lights']
 for i,w in enumerate(lights['words']):write(vm,BASE+0x6d8+i*0x68,w)
 write(vm,BASE+0xa18,lights['enabled']);write(vm,BASE+0x144c,lights['applied_enabled'])
 preparation=dict(streams=streams(vm,c['request']),index=index(vm,c['request']));draw=copy.deepcopy(c['draw']);draw['indexed']=get(vm,STATE+0x484)!=0
 results,passes=draw_sequence(vm,sub,c['context'],draw)
 expected=dict(id=c['id'],result=dict(preparation=preparation,draw=draw,passes=results),submission=dict(state=state(vm),complete=previous['complete'](vm),last_pass=get(vm,STATE+0x300),counters=env['counters'](vm),passes=passes))
 assert q==expected,('integration',c['id'])
 counts['integration_cases']+=1;counts['integration_instructions']+=vm.steps;counts['indexed_integrations']+=int(draw['indexed'])
assert counts['stream_cases']==578 and counts['expected_stream_errors']==4 and counts['index_cases']==124 and counts['expected_index_errors']==4 and counts['restore_cases']==18 and counts['expected_restore_errors']==6 and counts['integration_cases']==96
tests=sum(map(int,re.findall(r'test result: ok\. (\d+) passed',(root/'analysis/reports/d3d-buffers-tests.log').read_text())));assert tests==667,tests
sources=['crates/rc-package/src/d3d_buffers.rs','crates/rc-package/src/d3d_draw.rs','crates/rc-package/src/d3d_bindings.rs','crates/rc-package/src/d3d_complete.rs','crates/rc-package/src/d3d_pass.rs','crates/rc-package/src/d3d_transforms.rs','crates/rc-package/src/d3d_state.rs','crates/rc-inspect/src/bin/rc-d3d-buffers-check.rs','scripts/Generate-D3DBuffers.py','scripts/Record-D3DBuffers.py','scripts/Record-D3DDraw.py','scripts/Record-D3DComplete.py','scripts/Record-D3DBindings.py','scripts/Record-D3DTransforms.py','scripts/Record-D3DState.py','scripts/Record-MaterialState.py','analysis/decompiled/d3d-buffer-setup.asm','analysis/decompiled/d3d-buffer-setup.c']
validation=dict(date='2026-10-09',counts=dict(counts),rust_tests=tests,debug_release_identical=True,original_d3ddrv_sha256=sha(g['dll']),input_sha256=sha(root/'analysis/reports/d3d-buffers.input.json'),report_sha256=sha(path),source_sha256={s:sha(root/s) for s in sources},scope=report['scope'],android='deferred until end per user')
(root/'analysis/reports/d3d-buffers-validation.json').write_text(json.dumps(validation,indent=2)+'\n')
ledger=root/'analysis/evidence.json';e=json.loads(ledger.read_text(encoding='utf-8'));e['d3d_buffers_validation']=validation;e['rust_tests']=tests;ledger.write_text(json.dumps(e,indent=2,ensure_ascii=False)+'\n',encoding='utf-8')
print(json.dumps(dict(counts=dict(counts),rust_tests=tests)))
