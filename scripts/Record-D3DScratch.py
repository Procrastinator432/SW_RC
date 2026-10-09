"""Replay original scratch branches inside resize/ring/upload/bind sequences.

Array-helper allocation/reallocation pointers remain external answers. Its original
packed-count commit instructions are executed. Empty element-destruction loops,
SEH, timing, logging and returns have no modeled effects; native error paths are
excluded. CPU callback payloads and COM answers are explicit synthetic inputs.
"""
from pathlib import Path
import json,re,copy,collections
root=Path(__file__).resolve().parents[1]
p=root/'scripts/Record-D3DResize.py';src=p.read_text();r={'__file__':str(p)}
exec(compile(src.split('\nfixtures=json.loads')[0],str(p),'exec'),r)
exec(compile(src[src.index('def old_world('):src.index('\nfor c,q in zip(fixtures[\'invalidations\']')],str(p),'exec'),r)
n=r['n'];g=r['g'];put=r['put'];get=r['get'];sha=r['sha'];D=r['D'];FRAME=n['FRAME'];TARGET=n['TARGET'];counts=collections.Counter();AF=0x27020000
asm=(root/'analysis/decompiled/d3d-scratch-array.asm').read_text();g['code'].update({int(a,16):(op,args.split(',') if args else []) for a,op,args in re.findall(r'^([0-9a-f]{8}) ([A-Z.]+)(?: (.*))?$',asm,re.M)})
addresses=sorted(g['code']);g['following']={a:addresses[i+1] for i,a in enumerate(addresses[:-1])}
fixtures=json.loads((root/'analysis/reports/d3d-scratch.input.json').read_text());path=root/'analysis/reports/d3d-scratch.json';report=json.loads(path.read_text())
assert path.read_bytes()==(root/'analysis/reports/d3d-scratch-release.json').read_bytes()
assert fixtures['source_sha256']==report['source_sha256']==sha(root/'analysis/reports/d3d-resize.input.json')==json.loads((root/'analysis/reports/d3d-resize-validation.json').read_text())['input_sha256']

def array(vm,pc,answer,second=False):
 global extent
 assert g['code'][pc]==('CALL',['0x10001db0']) and vm.r['ECX']==D+0xe7e8
 count=vm.pop();extra=vm.pop();assert extra==0;vm.steps+=1;counts['array_helper_calls']+=1
 saved=vm.r.copy();flags=(vm.z,vm.s,vm.o,vm.c)
 put(vm,D+0xe7e8,answer);vm.r.update(EDI=D+0xe7e8,EBX=count,EBP=AF)
 vm.run(0x10001ed6,0x10001ee8)
 vm.r.update(saved);vm.z,vm.s,vm.o,vm.c=flags
 if second:extent=count
 return dict(count=count,extra=extra,pointer=answer)

def prepare(vm,s,kind):
 global current_scratch
 calls=[];a=s['array'];start=vm.steps
 if kind=='vertex':
  vm.run(0x1002d7fb,0x1002d80f)
  if not(vm.z or vm.s!=vm.o):
   vm.run(0x1002d815,0x1002d836);calls.append(array(vm,0x1002d836,a['reset_pointer']))
   vm.run(0x1002d83b,0x1002d86b);calls.append(array(vm,0x1002d86b,a['growth_pointer'],True))
   vm.run(0x1002d870,0x1002d892)
  vm.run(0x1002d892,0x1002d8b1)
 else:
  vm.run(0x1002da75,0x1002da86)
  if not(vm.z or vm.s!=vm.o):
   vm.run(0x1002da88,0x1002daa6);calls.append(array(vm,0x1002daa6,a['reset_pointer']))
   vm.run(0x1002daab,0x1002dad6);calls.append(array(vm,0x1002dad6,a['growth_pointer'],True))
   vm.run(0x1002dadb,0x1002db01)
  vm.run(0x1002db01,0x1002db1d)
 size=get(vm,FRAME-0x18)
 current_scratch=dict(calls=calls,guard_offset=size+1,callback_pointer=get(vm,D+0xe7e8))
 counts['scratch_preparations']+=1;counts['scratch_growths']+=int(bool(calls));counts['scratch_reuses']+=int(not calls);counts['scratch_prepare_instructions']+=vm.steps-start

def payload(vm,args,s):
 pointer=get(vm,D+0xe7e8) if get(vm,D+0x4114) else TARGET
 assert args==[pointer]
 for k,v in enumerate(s['source']['payload']):vm.m[pointer+k]=v
 counts['callback_bytes']+=len(s['source']['payload'])
 if get(vm,D+0x4114):counts['scratch_copy_bytes']+=get(vm,FRAME-0x18)

# Keep the established original scheduling, lock, unlock and binding replay;
# replace only the previously excluded scratch branches and callback boundary.
p=root/'scripts/Record-D3DDynamic.py';src=p.read_text();defs=src[src.index('def vertex(vm,s):'):src.index('\ndef bind_vertex(')]
defs=defs.replace("vm.run(0x1002d7e7,0x1002d7f5);assert vm.z","vm.run(0x1002d7e7,0x1002d7f5)\n if not vm.z:native_prepare(vm,s,'vertex')")
defs=defs.replace('vm.run(0x1002d8ba,0x1002d8c5);assert vm.z','vm.run(0x1002d8ba,0x1002d8c5)\n if not vm.z:vm.run(0x1002d8c7,0x1002d8e0)')
defs=defs.replace("vm.run(0x1002da61,0x1002da6f);assert vm.z","vm.run(0x1002da61,0x1002da6f)\n if not vm.z:native_prepare(vm,s,'index')")
defs=defs.replace('vm.run(0x1002db26,0x1002db31);assert vm.z','vm.run(0x1002db26,0x1002db31)\n if not vm.z:vm.run(0x1002db33,0x1002db49)')
n['native_prepare']=prepare;n['payload']=payload
exec(compile(defs,str(p),'exec'),n)

def old_world(w):
 rt=w['runtime']['buffers'];return dict(vertex=rt['vertex'],index=rt['index'],device=rt['device'],deferred=rt['deferred'],state=w['state'],target=w['target'])
def world(vm,w):
 value=n['world'](vm,old_world(w));pointer=get(vm,D+0xe7e8)
 return dict(runtime=dict(buffers={k:value[k] for k in ['vertex','index','device','deferred']},scratch=dict(address=pointer,packed_count=get(vm,D+0xe7ec),bytes=[vm.m.get(pointer+j,0) for j in range(extent)])),state=value['state'],target=value['target'])

for c,q in zip(fixtures['cases'],report['probes'],strict=True):
 assert c['id']==q['id'];w=c['world'];vm=n['initialize'](old_world(w));buf=w['runtime']['scratch'];extent=len(buf['bytes']);put(vm,D+0xe7e8,buf['address']);put(vm,D+0xe7ec,buf['packed_count'])
 for j,v in enumerate(buf['bytes']):vm.m[buf['address']+j]=v
 for j,(step,actual) in enumerate(zip(c['steps'],q['steps'],strict=True)):
  r['current_plan']=None;current_scratch=None;r['setup'](vm,step['context']);s=copy.deepcopy(step);s['responses']=s['answers']['lock']
  upload=n[s['kind']](vm,s)
  if s['kind']=='vertex':n['bind_vertex'](vm,s,upload);pool=None
  else:pool=n['bind_index'](vm,s,upload)
  expected=dict(result=dict(Ok=dict(transfer=dict(resize=r['current_plan'],scratch=current_scratch,upload=upload),pool=pool)),world=world(vm,w))
  assert actual['result']==expected['result'],('result',c['id'],j,actual['result'],expected['result'])
  assert actual['world']==expected['world'],('world',c['id'],j)
  counts['steps']+=1
 assert q['world']==world(vm,w);counts['cases']+=1
for c,q in zip(fixtures['errors'],report['errors'],strict=True):
 assert c['id']==q['id'] and q['world']==c['world']
 for step in q['steps']:assert 'Err' in step['result'] and step['world']==c['world']
 counts['safe_errors']+=1
assert counts['cases']==48 and counts['steps']==192 and counts['safe_errors']==12
assert counts['scratch_growths']>0 and counts['scratch_reuses']>0
counts['total_instructions']=sum(v for k,v in n['counts'].items() if k.endswith('_instructions'))
counts['resize_calls']=r['counts']['integrated_resizes'];counts['original_creates']=r['counts']['create_calls']
tests=sum(map(int,re.findall(r'test result: ok\. (\d+) passed',(root/'analysis/reports/d3d-scratch-tests.log').read_text())));assert tests==764,tests
sources=['crates/rc-package/src/d3d_scratch.rs','crates/rc-inspect/src/bin/rc-d3d-scratch-check.rs','scripts/Generate-D3DScratch.py','scripts/Record-D3DScratch.py','scripts/Record-D3DResize.py','scripts/Record-D3DDynamic.py','scripts/Record-D3DState.py','analysis/decompiled/d3d-scratch-array.asm','analysis/decompiled/d3d-scratch-array.c','analysis/decompiled/d3d-dynamic-ring.c']
sources+=['crates/rc-package/src/'+x+'.rs' for x in ['d3d_resize','d3d_dynamic','d3d_bindings','d3d_buffers','d3d_upload']]
sources+=['analysis/decompiled/'+x+'.asm' for x in ['d3d-resize-cache','d3d-resize-index','d3d-dynamic-vertex','d3d-dynamic-ring','d3d-dynamic-setup']]
sources=sorted(set(sources)|set(json.loads((root/'analysis/reports/d3d-dynamic-validation.json').read_text())['source_sha256']))
validation=dict(date='2026-10-09',counts=dict(counts),rust_tests=tests,debug_release_identical=True,original_d3ddrv_sha256=sha(g['dll']),input_sha256=sha(root/'analysis/reports/d3d-scratch.input.json'),report_sha256=sha(path),source_sha256={p:sha(root/p) for p in sources},scope=report['scope'],replay_boundary=__doc__,android='deferred until end per user')
(root/'analysis/reports/d3d-scratch-validation.json').write_text(json.dumps(validation,indent=2)+'\n');print(json.dumps(dict(counts=dict(counts),rust_tests=tests)))
