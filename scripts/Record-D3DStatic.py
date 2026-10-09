"""Compositional original-instruction proof of captured source-to-draw transactions.

Reuse verified original bodies with explicit callback answers. Upload bodies run
in their ABI fixture frames; their complete 64-byte wrapper images are carried
back into the original cache/list machine. Binding/draw runs in a renderer frame
with the resolved results. This is not an uninterrupted original binary callstack.
"""
from pathlib import Path
import json,re,copy,collections
root=Path(__file__).resolve().parents[1]
def prefix(name):
 p=root/'scripts'/name;d={'__file__':str(p)};src=p.read_text()
 exec(compile(src.split('\nfixtures=json.loads')[0],str(p),'exec'),d)
 return d,src,p
def execute(d,src,p,start,stop):exec(compile(src[src.index(start):src.index(stop)],str(p),'exec'),d)
b,src,p=prefix('Record-D3DBuffers.py')
execute(b,src,p,'def initialize(','\nfor c,q in zip(fixtures[\'streams\']')
execute(b,src,p,'def draw_sequence(','\nfor c,q in zip(fixtures[\'cases\']')
r,src,p=prefix('Record-D3DResource.py')
execute(r,src,p,'def put(','\nhashes=[')
execute(r,src,p,'def initialize(','\nfor c,q in zip(fixtures[\'cases\']')
u,src,p=prefix('Record-D3DUpload.py')
execute(u,src,p,'def put(','\nfor kind,key in')
s,src,p=prefix('Record-D3DSource.py')
s.update(DEST=0x40000000,COMP=0x41000000,OUT=0x42000000)
execute(s,src,p,'def initialize(','\nfor c,q in zip(fixtures[\'indices\']')
sha=r['sha'];counts=collections.Counter();r['g']['code'].update(b['g']['code'])
b['counts']=collections.Counter();b['previous']['counts']=b['counts']
addresses=sorted(r['g']['code']);r['g']['following']={a:addresses[i+1] for i,a in enumerate(addresses[:-1])}
fixtures=json.loads((root/'analysis/reports/d3d-static.input.json').read_text())
path=root/'analysis/reports/d3d-static.json';report=json.loads(path.read_text())
assert path.read_bytes()==(root/'analysis/reports/d3d-static-release.json').read_bytes()
assert report['source_sha256']==fixtures['source_sha256']==sha(root/'analysis/reports/d3d-resource.input.json')
assert fixtures['source_sha256']==json.loads((root/'analysis/reports/d3d-resource-validation.json').read_text())['input_sha256']
original_initialize=u['initialize'];original_result=u['result'];active=None
def upload_initialize(resource,source,device,kind):
 vm=original_initialize(resource,source,device,kind);a=resource['address']
 for k in range(64):vm.m[a+k]=active.m[a+k]
 return vm
def upload_result(vm,original,kind):
 a=original['address']
 for k in range(64):active.m[a+k]=vm.m[a+k]
 return original_result(vm,original,kind)
u['initialize']=upload_initialize;u['result']=upload_result
def resource(vm,a,kind):
 w=r['word'];return dict(address=a,handle=w(vm,a+0x30),capacity=w(vm,a+(0x38 if kind=='Vertex' else 0x34)),revision=w(vm,a+0x10),source=w(vm,a+0x3c))
def source_vm(step,a,width=None):
 c=dict(address=a,regions=step['regions'],width=width);return s['initialize'](c),c
def resolve(vm,cache,step,entry,kind):
 svm,c=source_vm(step,entry['address'],entry.get('width'));a=entry['address'];w=s['get']
 key=[w(svm,a+4),w(svm,a+8)]
 wrapper=r['action'](vm,cache,dict(kind='lookup',key=key));created=wrapper is None
 if created:
  wrapper=entry['allocation'];r['action'](vm,cache,dict(kind='insert',address=wrapper,key=key,resource_kind=kind))
 value=resource(vm,wrapper,kind);counts['cache_misses' if created else 'cache_hits']+=1
 assert r['word'](vm,wrapper)==(0x10072c10 if kind=='Vertex' else 0x10072d28)
 return value,created,svm,c
def outer_gate(vm,value,revision,kind):
 vm.r['EAX']=revision
 if kind=='Vertex':vm.r['EBX']=value['address'];vm.run(0x100203bd,0x100203c0)
 else:vm.r['EDI']=value['address'];vm.run(0x1002092e,0x10020931)
 return not vm.z
def frame_write(vm,value,frame,kind):
 R=b['R'];D=b['D'];FRAME=b['FRAME'];r['put'](vm,R+4,D);r['put'](vm,D+0x46a8,frame);r['put'](vm,FRAME+12,0x37000000)
 vm.r.update(EBP=FRAME)
 if kind=='Vertex':vm.r.update(EDI=R,EBX=value['address']);vm.run(0x100203e3,0x100203f2)
 else:vm.r.update(ESI=R,EDI=value['address']);vm.run(0x10020945,0x10020951)
def fill(svm,entry,c,targets,kind,size):
 target=next(t for t in targets if t['address']==entry['responses']['lock_pointer'])
 for k,v in enumerate(target['bytes']):svm.m[s['DEST']+k]=v
 if kind=='Vertex':payload=s['skin_payload'](svm,c,s['raw_skin'](svm,c))
 else:payload=s['index_payload'](svm,c,size)
 assert payload is not None and len(payload)<=len(target['bytes'])
 target['bytes']=[svm.m[s['DEST']+k] for k in range(len(target['bytes']))]
 counts['copied_bytes']+=len(payload);return len(payload)
def resolved(vm,cache,targets,step):
 global active
 active=vm;req=step['request'];streams=[];bindings=[]
 for entry in req['streams']:
  value,created,svm,c=resolve(vm,cache,step,entry,'Vertex');cached=value['revision'];transfer=None;size=0
  # GetComponents writes only six bytes into the caller's initialized storage.
  for k,v in enumerate(entry['initial_components']):svm.m[s['COMP']+k]=v
  s['put'](svm,svm.r['ESP']+4,s['COMP']);svm.run(0x103b0800,0x103b081f)
  declaration=[s['get'](svm,s['COMP']+k*4) for k in range(4)]+[svm.r['EAX']]
  svm.run(0x104fde10,0x104fde15);stride=svm.r['EAX']
  if outer_gate(vm,value,entry['revision'],'Vertex'):
   c.update(owner_count=entry['owner_count'],delegate=entry['delegate']);skin=s['raw_skin'](svm,c);size=s['skin_size'](svm,c,skin)
   src=dict(address=entry['address'],size=size,dynamic=entry['dynamic'],special=entry['special'],revision_after=entry['revision_after'])
   plan,value=u['vertex'](value,src,req['device'],entry['responses']);copied=fill(svm,entry,c,targets,'Vertex',size)
   transfer=dict(plan=plan,copied=copied);counts['vertex_outer_uploads']+=1
  else:counts['vertex_outer_skips']+=1
  frame_write(vm,value,req['frame'],'Vertex')
  bindings.append(dict(declaration=declaration,source_revision=entry['revision'],cached_revision=cached,upload_size=size,wrapper=value['address'],handle=value['handle'],stride=stride))
  streams.append(dict(wrapper=value['address'],created=created,cached_revision=cached,transfer=transfer));counts['source_instructions']+=svm.steps
 index=None;ib=dict(source=0,size=0,source_revision=0,cached_revision=0,wrapper=0,handle=0);entry=req['index']
 if entry and entry['address']:
  svm,c=source_vm(step,entry['address'],entry['width']);size,width=s['index_size'](svm,c);ib.update(source=entry['address'],size=size)
  counts['source_instructions']+=svm.steps
  if size:
   value,created,svm,c=resolve(vm,cache,step,entry,'Index');cached=value['revision'];transfer=None
   if outer_gate(vm,value,entry['revision'],'Index'):
    src=dict(address=entry['address'],size=size,width=width,revision_before=entry['revision_before'],revision_after=entry['revision_after'])
    plan,value=u['index'](value,src,req['device'],entry['responses'])
    copied=fill(svm,entry,c,targets,'Index',size) if plan['uploaded'] else 0
    transfer=dict(plan=plan,copied=copied);counts['index_outer_uploads']+=1;counts['index_inner_skips']+=int(not plan['uploaded'])
   else:counts['index_outer_skips']+=1
   frame_write(vm,value,req['frame'],'Index')
   ib=dict(source=entry['address'],size=size,source_revision=entry['revision'],cached_revision=cached,wrapper=value['address'],handle=value['handle'])
   index=dict(wrapper=value['address'],created=created,cached_revision=cached,transfer=transfer)
  else:counts['zero_index_skips']+=1
  if size:counts['source_instructions']+=svm.steps
 request=dict(streams=bindings,index=ib,shader_kind=req['shader_kind'],shader=req['shader'],base_vertex=req['base_vertex'],frame=req['frame'])
 return dict(request=request,streams=streams,index=index)
def bind_draw(sub,step,request):
 vm=b['initialize'](sub['state'],sub['complete']['deferred']);BASE=b['BASE'];lights=sub['complete']['lights']
 for i,w in enumerate(lights['words']):b['write'](vm,BASE+0x6d8+i*0x68,w)
 b['write'](vm,BASE+0xa18,lights['enabled']);b['write'](vm,BASE+0x144c,lights['applied_enabled'])
 preparation=dict(streams=b['streams'](vm,request),index=b['index'](vm,request));draw=copy.deepcopy(step['draw']);draw['indexed']=b['get'](vm,b['STATE']+0x484)!=0
 passes,mutated=b['draw_sequence'](vm,sub,step['context'],draw)
 plan=dict(preparation=preparation,draw=draw,passes=passes)
 sub=dict(state=b['state'](vm),complete=b['previous']['complete'](vm),last_pass=b['get'](vm,b['STATE']+0x300),counters=b['env']['counters'](vm),passes=mutated)
 counts['binding_draw_instructions']+=vm.steps;counts['draw_passes']+=len(passes)
 return plan,sub
for c,q in zip(fixtures['cases'],report['probes'],strict=True):
 cache=copy.deepcopy(c['resources']['cache']);targets=copy.deepcopy(c['resources']['targets']);sub=copy.deepcopy(c['submission']);vm=r['initialize'](cache)
 assert c['id']==q['id']
 for i,(step,actual) in enumerate(zip(c['steps'],q['steps'],strict=True)):
  value=resolved(vm,cache,targets,step);plan,sub=bind_draw(sub,step,value['request'])
  expected=dict(result=dict(Ok=dict(resolved=value,submission=plan)),resources=dict(cache=r['image'](vm,cache),targets=targets),submission=sub)
  assert actual==expected,('sequence',c['id'],i)
  counts['steps']+=1
 assert q['resources']==expected['resources'] and q['submission']==sub
 counts['cache_instructions']+=vm.steps;counts['cases']+=1
for c,q in zip(fixtures['errors'],report['errors'],strict=True):
 assert q['id']==c['id'] and q['resources']==c['resources'] and q['submission']==c['submission']
 for step in q['steps']:assert 'Err' in step['result'] and step['resources']==c['resources'] and step['submission']==c['submission']
 counts['safe_errors']+=1
assert counts['cases']==24 and counts['steps']==96 and counts['safe_errors']==10
counts.update({'upload_'+k:v for k,v in u['counts'].items()})
counts['hash_instructions']=r['counts']['hash_instructions']
assert counts['upload_vertex_reuses']>0 and counts['upload_index_reuses']>0
assert counts['index_inner_skips']>0 and counts['vertex_outer_skips']>0 and counts['cache_hits']>0
counts['total_instructions']=sum(counts[k] for k in ['source_instructions','binding_draw_instructions','cache_instructions','upload_vertex_instructions','upload_index_instructions','hash_instructions'])
tests=sum(map(int,re.findall(r'test result: ok\. (\d+) passed',(root/'analysis/reports/d3d-static-tests.log').read_text())));assert tests==716,tests
sources=['crates/rc-package/src/d3d_static.rs','crates/rc-package/src/d3d_resource.rs','crates/rc-package/src/d3d_source.rs','crates/rc-package/src/d3d_buffers.rs','crates/rc-package/src/d3d_upload.rs','crates/rc-inspect/src/bin/rc-d3d-static-check.rs','scripts/Generate-D3DStatic.py','scripts/Record-D3DStatic.py','scripts/Record-D3DResource.py','scripts/Record-D3DSource.py','scripts/Record-D3DUpload.py','scripts/Record-D3DBuffers.py','scripts/Record-D3DState.py','scripts/Record-MaterialState.py']
sources += ['scripts/Record-'+name+'.py' for name in ['D3DDraw','D3DComplete','D3DBindings','D3DTransforms']]
sources += ['crates/rc-package/src/'+name+'.rs' for name in ['d3d_draw','d3d_complete','d3d_bindings','d3d_transforms','d3d_pass','d3d_state','shader_snapshot']]
# All instruction exports used by the composed machines, including inherited bodies.
for namespace in [r,u,s,b,b['env'],b['previous']]:
 for name in namespace.get('assemblies',[]):
  path_name='analysis/decompiled/'+name
  if path_name not in sources:sources.append(path_name)
sources += ['analysis/decompiled/'+name for name in ['d3d-buffer-setup.asm','d3d-pass-translation.asm','d3d-state-cache.asm','hardware-material-state.asm'] if 'analysis/decompiled/'+name not in sources]
validation=dict(date='2026-10-09',counts=dict(counts),rust_tests=tests,debug_release_identical=True,original_d3ddrv_sha256=sha(r['g']['dll']),original_engine_sha256=sha(Path('D:/SteamLibrary/steamapps/common/Star Wars Republic Commando/GameData/System/Engine.dll')),input_sha256=sha(root/'analysis/reports/d3d-static.input.json'),report_sha256=sha(path),source_sha256={p:sha(root/p) for p in sources},scope=report['scope'],replay_boundary=__doc__,android='deferred until end per user')
(root/'analysis/reports/d3d-static-validation.json').write_text(json.dumps(validation,indent=2)+'\n')
print(json.dumps(dict(counts=dict(counts),rust_tests=tests)))
