"""Original Engine caller offsets composed with D3D pools/uploads/pass/draw replay.

Caller scopes begin after geometry generation and material decisions. Their
original interface arguments, offset forwarding, packed counts, wrapping math,
signed divisions and draw arguments execute. Existing D3D original bodies replay
the supplied allocator/COM/getter/fill/shader answers and complete pass dispatch.
Unrelated Engine setup, geometry, transforms/material construction and batch
cleanup are excluded, as are SEH/timing/logging/returns and native error paths.
"""
from pathlib import Path
import json,re,copy,collections
root=Path(__file__).resolve().parents[1];file=root/'scripts/Record-D3DPools.py';src=file.read_text();p={'__file__':str(file)}
exec(compile(src.split('\nfixtures=json.loads')[0],str(file),'exec'),p)
exec(compile(src[src.index('def allocation('):src.index('\nfor c,q in zip(fixtures[\'cases\']')],str(file),'exec'),p)
s=p['s'];r=p['r'];n=p['n'];g=p['g'];put=p['put'];get=p['get'];sha=p['sha'];D=p['D'];R=p['R'];FRAME=n['FRAME'];EF=0x28000000;BASE=r['BASE'];b=n['b'];draw=b['env'];complete=b['previous'];STATE=b['STATE'];counts=collections.Counter()
file=root/'scripts/Record-D3DBuffers.py';src=file.read_text();exec(compile(src[src.index('def draw_sequence('):src.index('\nfor c,q in zip(fixtures[\'cases\']',src.index('def draw_sequence('))],str(file),'exec'),b);b['counts']=collections.Counter()
assemblies=['engine-dynamic-grid.asm','engine-dynamic-beam.asm','engine-dynamic-fluid.asm','engine-dynamic-canvas.asm','engine-dynamic-canvas-gate.asm','engine-dynamic-lines.asm','engine-dynamic-line-upload.asm','engine-dynamic-line-gate.asm']
for name in assemblies:
 asm=(root/'analysis/decompiled'/name).read_text();g['code'].update({int(a,16):(op,args.split(',') if args else []) for a,op,args in re.findall(r'^([0-9a-f]{8}) ([A-Z.]+)(?: (.*))?$',asm,re.M)})
addresses=sorted(g['code']);g['following']={a:addresses[i+1] for i,a in enumerate(addresses[:-1])}
assert [g['pe_word'](0x1006f8d8+off) for off in [0x88,0x90,0x94,0x8c]]==[0x10020540,0x10020a90,0x10020cb0,0x10020840]
fixtures=json.loads((root/'analysis/reports/d3d-dynamic-draw.input.json').read_text());path=root/'analysis/reports/d3d-dynamic-draw.json';report=json.loads(path.read_text())
assert path.read_bytes()==(root/'analysis/reports/d3d-dynamic-draw-release.json').read_bytes()
for key,prior in [('source_sha256','d3d-pools'),('passes_sha256','d3d-draw')]:
 assert fixtures[key]==report[key]==sha(root/('analysis/reports/'+prior+'.input.json'))==json.loads((root/('analysis/reports/'+prior+'-validation.json')).read_text())['input_sha256']

def interface(vm,pc,offset,count):
 calls=[];args=n['call'](vm,pc,offset,count,calls,True);assert calls[0]['receiver']==R;counts['interface_'+hex(offset)]+=1;return args

def upload(vm,q,kind):
 global vertex_bytes,index_bytes
 target=vertex_bytes if kind=='vertex' else index_bytes
 for j,v in enumerate(target):vm.m[n['TARGET']+j]=v
 q=copy.deepcopy(q);q['answers']=q['upload'];q['responses']=q['upload']['lock']
 put(vm,q['source']['address'],0x24000000)
 if kind=='index':n['I']=r['I']=0x34000200 if q['source']['width']==4 else 0x34000100
 r['setup'](vm,q['context']);initialized=p['ensure'](vm,q,kind);r['current_plan']=None;s['current_scratch']=None
 value=n[kind](vm,q)
 if kind=='vertex':
  saved=vm.r.copy();vm.r.update(ESI=R,EDI=q['source']['address'],EBP=p['PF'],EAX=value['allocation']['element_offset']);vm.run(0x10020630,0x10020637);vm.r.update(saved)
  n['bind_vertex'](vm,q,value);vm.r['EBP']=p['PF'];vm.run(0x100207e7,0x100207ea)
 else:n['bind_index'](vm,q,value);vm.r['EBP']=FRAME;vm.run(0x10020c63,0x10020c66)
 offset=vm.r['EAX'];assert offset==value['allocation']['element_offset']
 target=[vm.m[n['TARGET']+j] for j in range(len(target))]
 if kind=='vertex':vertex_bytes=target
 else:index_bytes=target
 return dict(initialization=initialized,transfer=dict(resize=r['current_plan'],scratch=s['current_scratch'],upload=value),offset=offset)

def gate(vm,q):
 kind=q['caller']['kind'];c=q['caller'];address=q['vertex']['source']['address'];vm.r.update(ESI=address,EBP=EF,ESP=0x70060000,EBX=0)
 if kind=='canvas':
  vm.r['EAX']=c['primitive_count'];vm.run(0x104c77c2,0x104c77cd);return not(vm.z or vm.s!=vm.o)
 if kind=='lines':
  put(vm,address+0x14,c['packed_vertices']);vm.run(0x104d280f,0x104d2816);return not vm.z
 return True

def handoff(vm,q):
 kind=q['caller']['kind'];address=q['vertex']['source']['address'];put(vm,R,0x1006f8d8)
 vm.r.update(EBP=EF,ESP=0x70060000,EAX=0)
 if kind in ['grid','beam']:vm.r['EBX']=R
 elif kind=='fluid':vm.r['ESI']=R
 else:vm.r.update(ESI=address,EBX=0);put(vm,address+0x1c,R)
 a,pc={'grid':(0x103aa0c1,0x103aa0ce),'fluid':(0x1040bfc7,0x1040bfd1),'beam':(0x103dd609,0x103dd616),'canvas':(0x104c7920,0x104c7927),'lines':(0x104d2827,0x104d282f)}[kind]
 vm.run(a,pc);assert interface(vm,pc,0x88,2)==[0,address]
 saved=vm.r.copy();vertex=upload(vm,q['vertex'],'vertex');vm.r.update(saved);vm.r['EAX']=vertex['offset']
 index=None
 if q['index'] is not None:
  a,pc={'grid':(0x103aa0d4,0x103aa0e0),'fluid':(0x1040bfd7,0x1040bfe0),'beam':(0x103dd61c,0x103dd625)}[kind]
  vm.run(a,pc);args=interface(vm,pc,0x90,2);assert args==[q['index']['source']['address'],vertex['offset']]
  q['index']['base']=args[1];saved=vm.r.copy();index=upload(vm,q['index'],'index');vm.r.update(saved);vm.r['EAX']=index['offset']
 else:
  vm.r.update(ESI=address,EBX=0,EBP=EF)
  a,pc=(0x104c7958,0x104c795f) if kind=='canvas' else (0x104d2951,0x104d295a)
  vm.run(a,pc);assert interface(vm,pc,0x8c,2)==[0,0]
  b['index'](vm,dict(index=dict(source=0,size=0),base_vertex=0,frame=0));counts['explicit_unbinds']+=1
 return vertex,index

def caller_draws(vm,q,vertex,index):
 c=q['caller'];kind=c['kind'];out=[];OBJ=0x39000000;vo=vertex['offset'];io=index['offset'] if index else None
 def capture(pc):
  args=interface(vm,pc,0x94,5);out.append(dict(zip(['primitive','start','primitive_count','min_vertex','max_vertex'],args),indexed=index is not None))
 vm.r.update(EBP=EF,ESP=0x70060000)
 if kind=='grid':
  vm.r.update(EBX=R,ESI=io);put(vm,EF-0x18,OBJ);put(vm,OBJ+0xc,c['columns']);put(vm,OBJ+0x10,c['rows']);vm.run(0x103aa0f9,0x103aa11b);capture(0x103aa11b)
 elif kind=='fluid':
  vm.r.update(ESI=R,EDI=OBJ);put(vm,EF+12,io);put(vm,OBJ+0x28c,c['columns']);put(vm,OBJ+0x290,c['rows'])
  if c['quad']:vm.run(0x1040c095,0x1040c0a5);capture(0x1040c0a5)
  else:vm.run(0x1040c0ba,0x1040c0e2);capture(0x1040c0e2)
 elif kind=='beam':
  vm.r.update(ESI=OBJ,EDI=c['copies'],EBX=R);put(vm,EF+0x14,c['copies']);put(vm,EF+0x10,io);put(vm,EF+12,0)
  for k,off in [('segments',0x448),('vertices',0x44c),('indices',0x450),('primitives',0x454)]:put(vm,OBJ+off,c[k])
  while True:
   vm.run(0x103dd635,0x103dd640)
   if vm.z or vm.s==vm.o:break
   vm.run(0x103dd642,0x103dd681);capture(0x103dd681);vm.run(0x103dd687,0x103dd691)
   assert len(out)<=256
 elif kind=='canvas':
  address=q['vertex']['source']['address'];vm.r.update(ESI=address,EDI=vo,EBX=0)
  for k,off in [('primitive',0x10),('primitive_count',0x18),('packed_vertices',0xa4),('alternate_vertices',0xac)]:put(vm,address+off,c[k])
  vm.m[address+0xb0]=int(c['alternate']);vm.run(0x104c796f,0x104c797c)
  if vm.z:vm.run(0x104c79bd,0x104c79d5);capture(0x104c79d5)
  else:vm.run(0x104c797e,0x104c7996);capture(0x104c7996)
 else:
  address=q['vertex']['source']['address'];vm.r.update(ESI=address,EDI=vo);put(vm,address+0x14,c['packed_vertices']);vm.run(0x104d2960,0x104d297d);capture(0x104d297d)
 counts[kind+'_draws']+=len(out);return out

def runtime(vm,oldw,old):
 value=p['world'](vm,oldw,old)
 return dict(buffers=value['runtime'],state=value['state'],lights=complete['lights'](vm),last_pass=get(vm,STATE+0x300),counters=draw['counters'](vm),vertex_target=vertex_bytes,index_target=index_bytes)

for c,q in zip(fixtures['cases'],report['probes'],strict=True):
 assert c['id']==q['id'];rt=c['runtime'];oldw=dict(runtime=rt['buffers'],state=rt['state'],target=rt['vertex_target']);n['I']=r['I']=0x34000100
 vm,old=p['initial'](oldw);p['nodes']=[dict(address=a['address'],size=len(a['bytes'])) for a in rt['buffers']['cache']['nodes']];s['extent']=len(rt['buffers']['scratch']['bytes'])
 # The pool-only replay's allocator stub at 26000000 aliases material pass P.
 # Give the combined replay a distinct synthetic allocator object.
 put(vm,0x1006f014,0x2f000000);put(vm,0x2f000000,0x2f000100);put(vm,0x2f000100,0x2f000200)
 vertex_bytes=copy.deepcopy(rt['vertex_target']);index_bytes=copy.deepcopy(rt['index_target']);put(vm,STATE+0x300,rt['last_pass'])
 for i,w in enumerate(rt['lights']['words']):b['write'](vm,BASE+0x6d8+i*0x68,w)
 b['write'](vm,BASE+0xa18,rt['lights']['enabled']);b['write'](vm,BASE+0x144c,rt['lights']['applied_enabled'])
 for k,off in [('draw_passes',0xfec0),('submitted_primitives',0xfc90),('submitted_vertices',0xfcb8)]:put(vm,D+off,rt['counters'][k])
 for j,(request,actual) in enumerate(zip(c['requests'],q['steps'],strict=True)):
  request=copy.deepcopy(request);start=vm.steps
  if gate(vm,request):
   vertex,index=handoff(vm,request);draws=caller_draws(vm,request,vertex,index);rendered=[]
   for d in draws:
    assert d['indexed']==(get(vm,STATE+0x484)!=0)
    submission=dict(last_pass=get(vm,STATE+0x300),counters=draw['counters'](vm),passes=request['passes'])
    result,mutated=b['draw_sequence'](vm,submission,request['context'],d);request['passes']=mutated;rendered.append(result);counts['material_passes']+=len(result)
   value=dict(vertex=vertex,index=index,draws=draws,rendered=rendered)
  else:value=dict(vertex=None,index=None,draws=[],rendered=[]);counts['disabled_callers']+=1
  assert actual['result']==dict(Ok=value),('result',c['id'],j,actual['result'],value)
  assert actual['request']==request,('request',c['id'],j)
  assert actual['runtime']==runtime(vm,oldw,old),('runtime',c['id'],j)
  counts['total_instructions']+=vm.steps-start;counts['requests']+=1
 assert q['runtime']==runtime(vm,oldw,old);counts['cases']+=1
for c,q in zip(fixtures['errors'],report['errors'],strict=True):
 assert c['id']==q['id'] and q['runtime']==c['runtime']
 for original,step in zip(c['requests'],q['steps'],strict=True):assert 'Err' in step['result'] and step['runtime']==c['runtime'] and step['request']==original
 counts['safe_errors']+=1
assert counts['cases']==48 and counts['requests']==240 and counts['safe_errors']==12
tests=sum(map(int,re.findall(r'test result: ok\. (\d+) passed',(root/'analysis/reports/d3d-dynamic-draw-tests.log').read_text())));assert tests==795,tests
sources=['crates/rc-package/src/d3d_dynamic_draw.rs','crates/rc-inspect/src/bin/rc-d3d-dynamic-draw-check.rs','scripts/Generate-D3DDynamicDraw.py','scripts/Record-D3DDynamicDraw.py','analysis/decompiled/engine-dynamic-bridge.c','analysis/decompiled/engine-dynamic-simple.c','analysis/decompiled/d3d-dynamic-entries.txt','analysis/decompiled/d3d-interface-table.txt']+['analysis/decompiled/'+a for a in assemblies]
sources=sorted(set(sources)|set(json.loads((root/'analysis/reports/d3d-pools-validation.json').read_text())['source_sha256']))
validation=dict(date='2026-10-09',counts=dict(counts),rust_tests=tests,debug_release_identical=True,original_d3ddrv_sha256=sha(g['dll']),original_engine_sha256=sha(draw['engine']),input_sha256=sha(root/'analysis/reports/d3d-dynamic-draw.input.json'),report_sha256=sha(path),source_sha256={a:sha(root/a) for a in sources},scope=report['scope'],replay_boundary=__doc__,android='deferred until end per user')
(root/'analysis/reports/d3d-dynamic-draw-validation.json').write_text(json.dumps(validation,indent=2)+'\n');print(json.dumps(dict(counts=dict(counts),rust_tests=tests)))
