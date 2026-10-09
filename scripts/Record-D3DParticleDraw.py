"""Original ribbon, spark, sprite, trail and resolved mesh-section replay.

Scope begins after particle acceptance/geometry/whole-mesh visibility decisions.
Original interface arguments, upload offsets, wrapping counts, spark sentinels,
mesh zero-count/word gates and ushort draw ranges execute. Mesh sections share
supplied prepared passes; outer mesh traversal and per-section material lookup
are excluded. Existing D3D bodies replay supplied allocator/COM/getter/fill/shader
answers through complete pass dispatch. SEH/timing/logging/destructors/returns,
error paths and actual GPU/allocator execution are excluded.
"""
from pathlib import Path
import json,re,copy,collections
root=Path(__file__).resolve().parents[1];file=root/'scripts/Record-D3DPools.py';src=file.read_text();p={'__file__':str(file)}
exec(compile(src.split('\nfixtures=json.loads')[0],str(file),'exec'),p)
exec(compile(src[src.index('def allocation('):src.index('\nfor c,q in zip(fixtures[\'cases\']')],str(file),'exec'),p)
s=p['s'];r=p['r'];n=p['n'];g=p['g'];put=p['put'];get=p['get'];sha=p['sha'];D=p['D'];R=p['R'];FRAME=n['FRAME'];EF=0x28000000;BASE=r['BASE'];b=n['b'];draw=b['env'];complete=b['previous'];STATE=b['STATE'];counts=collections.Counter()
file=root/'scripts/Record-D3DBuffers.py';src=file.read_text();exec(compile(src[src.index('def draw_sequence('):src.index('\nfor c,q in zip(fixtures[\'cases\']',src.index('def draw_sequence('))],str(file),'exec'),b);b['counts']=collections.Counter()
assemblies=['engine-ribbon-upload.asm','engine-spark-upload.asm','engine-sprite-upload.asm','engine-trail-upload.asm','engine-vertmesh-upload.asm','engine-vertmesh-draw.asm']
for name in assemblies:
 asm=(root/'analysis/decompiled'/name).read_text();g['code'].update({int(a,16):(op,args.split(',') if args else []) for a,op,args in re.findall(r'^([0-9a-f]{8}) ([A-Z.]+)(?: (.*))?$',asm,re.M)})
addresses=sorted(g['code']);g['following']={a:addresses[i+1] for i,a in enumerate(addresses[:-1])}
assert [g['pe_word'](0x1006f8d8+off) for off in [0x88,0x90,0x94,0x8c]]==[0x10020540,0x10020a90,0x10020cb0,0x10020840]
fixtures=json.loads((root/'analysis/reports/d3d-particle-draw.input.json').read_text());path=root/'analysis/reports/d3d-particle-draw.json';report=json.loads(path.read_text())
assert path.read_bytes()==(root/'analysis/reports/d3d-particle-draw-release.json').read_bytes()
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
 # Geometry, particle acceptance and whole-mesh visibility gates precede this scope.
 return True

def handoff(vm,q):
 kind=q['caller']['kind'];address=q['vertex']['source']['address'];put(vm,R,0x1006f8d8)
 vm.r.update(EBP=EF,ESP=0x70060000,EAX=0,EDI=0,EBX=0x39000000)
 if kind in ['ribbon','vert_mesh']:vm.r['ESI']=R
 elif kind in ['spark','trail']:vm.r['EBX']=R
 else:vm.r['EDI']=R
 a,pc={'ribbon':(0x104e3d5a,0x104e3d67),'spark':(0x1051bb12,0x1051bb1c),'sprite':(0x1051ef66,0x1051ef70),'trail':(0x1055c2cf,0x1055c2d8),'vert_mesh':(0x105606d3,0x105606e0)}[kind]
 vm.run(a,pc);assert interface(vm,pc,0x88,2)==[0,address]
 saved=vm.r.copy();vertex=upload(vm,q['vertex'],'vertex');vm.r.update(saved);vm.r['EAX']=vertex['offset']
 index=None
 if q['index'] is not None:
  a,pc={'ribbon':(0x104e3d6d,0x104e3d79),'sprite':(0x1051ef76,0x1051ef7f),'trail':(0x1055c2de,0x1055c2e7),'vert_mesh':(0x105606e6,0x105606ef)}[kind]
  vm.run(a,pc);args=interface(vm,pc,0x90,2);assert args==[q['index']['source']['address'],vertex['offset']]
  q['index']['base']=args[1];saved=vm.r.copy();index=upload(vm,q['index'],'index');vm.r.update(saved);vm.r['EAX']=index['offset']
 else:
  vm.run(0x1051bb22,0x1051bb2c);assert interface(vm,0x1051bb2c,0x8c,2)==[0,0]
  b['index'](vm,dict(index=dict(source=0,size=0),base_vertex=0,frame=0));counts['explicit_unbinds']+=1
 return vertex,index

def caller_draws(vm,q,vertex,index):
 c=q['caller'];kind=c['kind'];out=[];OBJ=0x39000000;vo=vertex['offset'];io=index['offset'] if index else None
 def capture(pc):
  args=interface(vm,pc,0x94,5);out.append(dict(zip(['primitive','start','primitive_count','min_vertex','max_vertex'],args),indexed=index is not None))
 vm.r.update(EBP=EF,ESP=0x70060000)
 if kind=='ribbon':
  vm.r.update(ESI=R,EAX=io);put(vm,EF-0x1c,c['pairs']);vm.run(0x104e3d7f,0x104e3d9f);capture(0x104e3d9f)
 elif kind=='spark':
  vm.r.update(EBX=R,EDI=OBJ,ESI=vo);put(vm,OBJ+0x3b8,c['points']);put(vm,EF+0x14,c['sparks']);vm.run(0x1051bb32,0x1051bb48);capture(0x1051bb48)
 elif kind=='sprite':
  vm.r.update(EDI=R,EAX=io);put(vm,EF+0x14,c['sprites']);vm.run(0x1051ef85,0x1051ef9c);capture(0x1051ef9c)
 elif kind=='trail':
  vm.r.update(EBX=R,EDI=0,EAX=io);put(vm,EF+0x10,c['vertices']);put(vm,EF+8,c['primitives']);vm.run(0x1055c2ed,0x1055c2fe);capture(0x1055c2fe)
 else:
  assert len(c['sections'])<=256
  # Execute the original two section gates and ushort argument loads for each
  # resolved section. Outer mesh array traversal/material lookup is excluded.
  put(vm,EF+0x18,R);put(vm,EF-0x24,io);put(vm,EF-0x2c,0x3a000000)
  for j,section in enumerate(c['sections']):
   put(vm,0x3a000000+j*4,section['primitive_count']);put(vm,OBJ+0x1a0,0x3b000000)
   entry=0x3b000000+j*0x50
   for off,key in [(2,'first_index'),(4,'min_vertex'),(6,'max_vertex'),(0x10,'enabled_word')]:
    v=section[key];vm.m[entry+off]=v&255;vm.m[entry+off+1]=v>>8
   vm.r.update(ESI=entry,EDI=j,EDX=0x3a000000,ECX=OBJ,EAX=j*0x50)
   vm.run(0x105607e7,0x105607eb)
   if vm.z:counts['mesh_zero_count']+=1;continue
   vm.run(0x105607f1,0x105607fd)
   if vm.z:counts['mesh_zero_word']+=1;continue
   put(vm,EF+0x10,j);vm.run(0x105608dc,0x105608ff);capture(0x105608ff)
 counts[kind+'_draws']+=len(out);return out

def runtime(vm,oldw,old):
 value=p['world'](vm,oldw,old)
 return dict(buffers=value['runtime'],state=value['state'],lights=complete['lights'](vm),last_pass=get(vm,STATE+0x300),counters=draw['counters'](vm),vertex_target=vertex_bytes,index_target=index_bytes)

# Explicit width regression: an adjacent nonzero halfword must not affect CMP,
# and MOVZX must zero-extend the low unsigned word alone.
probe=g['Native']();probe.r.update(ESI=0x3c000000,ECX=0x3c000000,EAX=0)
put(probe,0x3c000006,0xabcdffff);probe.run(0x105608dc,0x105608e0);assert probe.r['EDX']==65535
put(probe,0x3c000010,0xabcd0000);probe.run(0x105607f7,0x105607fd);assert probe.z
put(probe,0x3c000010,0x00008000);probe.run(0x105607f7,0x105607fd);assert not probe.z
counts['word_access_regressions']=3

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
assert counts['cases']==48 and counts['requests']==240 and counts['safe_errors']==14
tests=sum(map(int,re.findall(r'test result: ok\. (\d+) passed',(root/'analysis/reports/d3d-particle-draw-tests.log').read_text())));assert tests==807,tests
sources=['crates/rc-package/src/d3d_dynamic_draw.rs','crates/rc-inspect/src/bin/rc-d3d-particle-draw-check.rs','scripts/Generate-D3DParticleDraw.py','scripts/Record-D3DParticleDraw.py','analysis/decompiled/engine-particle-dynamic.c','analysis/decompiled/d3d-dynamic-entries.txt','analysis/decompiled/d3d-interface-table.txt']+['analysis/decompiled/'+a for a in assemblies]
sources=sorted(set(sources)|set(json.loads((root/'analysis/reports/d3d-pools-validation.json').read_text())['source_sha256']))
validation=dict(date='2026-10-09',counts=dict(counts),rust_tests=tests,debug_release_identical=True,original_d3ddrv_sha256=sha(g['dll']),original_engine_sha256=sha(draw['engine']),input_sha256=sha(root/'analysis/reports/d3d-particle-draw.input.json'),report_sha256=sha(path),source_sha256={a:sha(root/a) for a in sources},scope=report['scope'],replay_boundary=__doc__,android='deferred until end per user')
(root/'analysis/reports/d3d-particle-draw-validation.json').write_text(json.dumps(validation,indent=2)+'\n');print(json.dumps(dict(counts=dict(counts),rust_tests=tests)))
