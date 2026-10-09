"""Original dynamic ring/direct upload and resolved handoff instruction replay.

SEH, timing instrumentation, logging and returns are excluded. Resize helper
calls receive explicit successful handle results; their COM/allocation internals
are not replayed. Source fill and shader/getter calls use supplied answers.
Only Device+4114==0 is executed. Safe errors are checked for port rollback only.
"""
from pathlib import Path
import json,re,collections,copy
root=Path(__file__).resolve().parents[1];p=root/'scripts/Record-D3DBuffers.py';b={'__file__':str(p)};src=p.read_text()
exec(compile(src.split('\nfixtures=json.loads')[0],str(p),'exec'),b)
exec(compile(src[src.index('def initialize('):src.index('\nfor c,q in zip(fixtures[\'streams\']')],str(p),'exec'),b)
g=b['g'];sha=b['sha'];put=b['put'];get=b['get'];FRAME=b['FRAME'];D=b['D'];R=b['R'];counts=collections.Counter()
assemblies=['d3d-dynamic-ring.asm','d3d-dynamic-setup.asm']
for name in assemblies:
 asm=(root/'analysis/decompiled'/name).read_text();g['code'].update({int(a,16):(op,args.split(',') if args else []) for a,op,args in re.findall(r'^([0-9a-f]{8}) ([A-Z.]+)(?: (.*))?$',asm,re.M)})
addresses=sorted(g['code']);g['following']={a:addresses[i+1] for i,a in enumerate(addresses[:-1])}
fixtures=json.loads((root/'analysis/reports/d3d-dynamic.input.json').read_text());path=root/'analysis/reports/d3d-dynamic.json';report=json.loads(path.read_text())
assert path.read_bytes()==(root/'analysis/reports/d3d-dynamic-release.json').read_bytes()
assert fixtures['source_sha256']==report['source_sha256']==sha(root/'analysis/reports/d3d-static.input.json')==json.loads((root/'analysis/reports/d3d-static-validation.json').read_text())['input_sha256']
V=0x34000000;I=0x34000100;OUT=0x27001000;TARGET=0x40000000
def initialize(w):
 vm=b['initialize'](w['state'],w['deferred']);put(vm,R+4,D)
 for obj,fields in [(w['vertex'],[('capacity',0x38),('cursor',0x40),('active',0x44),('source',0x3c)]),(w['index'],[('handle',0x30),('capacity',0x34),('cursor',0x38),('width',0x3c)])]:
  put(vm,obj['address']+4,D)
  for k,off in fields:put(vm,obj['address']+off,obj[k])
 for j,h in enumerate(w['vertex']['handles']):put(vm,V+0x30+j*4,h)
 for k,off in [('vertex_limit',0x420c),('scratch',0x4114),('discards',0x10078),('vertex_bytes',0x100a0),('index_bytes',0x100c8)]:put(vm,D+off,w['device'][k])
 for a in [0x35000000,0x35000100,*w['vertex']['handles'],w['index']['handle']]:put(vm,a,0x24000000)
 for k,v in enumerate(w['target']):vm.m[TARGET+k]=v
 put(vm,FRAME-12,0);return vm
def world(vm,w):
 v=dict(address=V,handles=[get(vm,V+0x30+j*4) for j in range(2)],**{k:get(vm,V+off) for k,off in [('capacity',0x38),('cursor',0x40),('active',0x44),('source',0x3c)]})
 i=dict(address=I,**{k:get(vm,I+off) for k,off in [('handle',0x30),('capacity',0x34),('cursor',0x38),('width',0x3c)]})
 d={k:get(vm,D+off) for k,off in [('vertex_limit',0x420c),('scratch',0x4114),('discards',0x10078),('vertex_bytes',0x100a0),('index_bytes',0x100c8)]}
 return dict(vertex=v,index=i,device=d,state=b['state'](vm),deferred=b['deferred'](vm),target=[vm.m[TARGET+k] for k in range(len(w['target']))])
def call(vm,pc,offset,n,commands,source=False):
 op,args=g['code'][pc];assert op=='CALL' and args[0].endswith(f'+ {hex(offset)}]')
 receiver=vm.r['ECX'] if source else vm.pop();arguments=[vm.pop() for _ in range(n)];vm.steps+=1
 commands.append(dict(receiver=receiver,vtable_offset=offset,arguments=arguments));return arguments
def resize(vm,s,kind):
 size=vm.pop();assert vm.r['ECX']==(V if kind=='vertex' else I);h=s['responses']['resized'];assert h
 for v in h:put(vm,v,0x24000000)
 if kind=='vertex':
  for j,v in enumerate(h):put(vm,V+0x30+j*4,v)
  put(vm,V+0x38,size);put(vm,V+0x40,0)
 else:put(vm,I+0x30,h[0]);put(vm,I+0x34,size);put(vm,I+0x38,0)
 vm.steps+=1;counts[kind+'_resize_calls']+=1
def locked(vm,args,s):
 o=s['responses'];assert args[2]==o['lock_slot'];put(vm,args[2],o['lock_pointer']);vm.r['EAX']=o['lock_hresult'];assert vm.r['EAX']<0x80000000
def payload(vm,args,s):
 assert args==[s['responses']['lock_pointer']]==[TARGET]
 for k,v in enumerate(s['source']['payload']):vm.m[TARGET+k]=v
 counts['payload_bytes']+=len(s['source']['payload'])
def vertex(vm,s):
 start=vm.steps;src=s['source'];o=s['responses'];commands=[];before=get(vm,D+0x10078)
 vm.r.update(EBX=V,ESI=src['address'],EBP=FRAME,ESP=0x70000000,EAX=src['size_before']);put(vm,FRAME+8,src['address']);put(vm,FRAME+12,OUT)
 vm.run(0x1002d644,0x1002d651);vm.r['EAX']=src['stride'];vm.run(0x1002d654,0x1002d668);grow=not (vm.z or vm.s!=vm.o)
 if grow:vm.run(0x1002d66a,0x1002d66d);resize(vm,s,'vertex');vm.run(0x1002d672,0x1002d68b)
 vm.r['EAX']=src['size_again'];vm.run(0x1002d692,0x1002d694);discard=vm.s!=vm.o
 if not discard:
  vm.run(0x1002d696,0x1002d6a4);discard=not vm.z and vm.s==vm.o
  if not discard:vm.run(0x1002d6a6,0x1002d6b3);discard=not (vm.c or vm.z)
 if discard:vm.run(0x1002d6b5,0x1002d6d2)
 vm.run(0x1002d6d2,0x1002d6ee);vm.run(0x1002d6f3,0x1002d6f6);vm.run(0x1002d6f8,0x1002d6fb)
 offset=vm.r['ESI'];size=get(vm,FRAME-0x18);flags=get(vm,FRAME-0x1c)
 vm.run(0x1002d73a,0x1002d755);args=call(vm,0x1002d755,0x2c,4,commands);locked(vm,args,s);vm.run(0x1002d758,0x1002d75a)
 vm.run(0x1002d7e7,0x1002d7f5);assert vm.z
 vm.run(0x1002d8b1,0x1002d8b7);args=call(vm,0x1002d8b7,0x24,1,commands,True);payload(vm,args,s)
 vm.run(0x1002d8ba,0x1002d8c5);assert vm.z
 vm.run(0x1002d8e0,0x1002d8ea);call(vm,0x1002d8ea,0x30,0,commands);vm.r['EAX']=o['unlock_hresult'];vm.run(0x1002d8ed,0x1002d8ef);assert not vm.s
 vm.run(0x1002d92c,0x1002d942);vm.run(0x1002d943,0x1002d947)
 a=dict(size=size,byte_offset=offset,element_offset=vm.r['EAX'],flags=flags,slot=get(vm,OUT),resized=grow,discard_events=(get(vm,D+0x10078)-before)&0xffffffff)
 counts['vertex_instructions']+=vm.steps-start;counts['vertex_uploads']+=1;counts['vertex_discards']+=a['discard_events'];counts['vertex_nooverwrite']+=int(flags==0x1000)
 return dict(allocation=a,copied=len(src['payload']),commands=commands)
def index(vm,s):
 start=vm.steps;src=s['source'];o=s['responses'];commands=[]
 vm.r.update(EBX=I,ESI=src['address'],EBP=FRAME,ESP=0x70000000,EAX=src['size']);put(vm,FRAME+8,src['address'])
 vm.run(0x1002d9c8,0x1002d9d1);vm.r['EAX']=src['width'];vm.run(0x1002d9d4,0x1002d9e6);grow=not(vm.z or vm.s!=vm.o)
 if grow:vm.run(0x1002d9e8,0x1002d9eb);resize(vm,s,'index');vm.run(0x1002d9f0,0x1002d9f5)
 vm.run(0x1002d9f5,0x1002d9ff)
 if not vm.z and vm.s==vm.o:vm.run(0x1002da01,0x1002da09)
 vm.run(0x1002da09,0x1002da1f);args=call(vm,0x1002da1f,0x2c,4,commands);locked(vm,args,s);offset=args[0];flags=args[3];vm.run(0x1002da22,0x1002da24)
 vm.run(0x1002da61,0x1002da6f);assert vm.z
 vm.run(0x1002db1d,0x1002db23);args=call(vm,0x1002db23,0x14,1,commands,True);payload(vm,args,s)
 vm.run(0x1002db26,0x1002db31);assert vm.z
 vm.run(0x1002db49,0x1002db4f);call(vm,0x1002db4f,0x30,0,commands);vm.r['EAX']=o['unlock_hresult'];vm.run(0x1002db52,0x1002db54);assert not vm.s
 vm.run(0x1002db91,0x1002db9d);vm.run(0x1002db9e,0x1002dba2)
 a=dict(size=src['size'],byte_offset=offset,element_offset=vm.r['EAX'],flags=flags,slot=0,resized=grow,discard_events=0)
 counts['index_instructions']+=vm.steps-start;counts['index_uploads']+=1;counts['index_nooverwrite']+=int(flags==0x1000);counts['index_discards']+=int(flags==0x2000)
 return dict(allocation=a,copied=len(src['payload']),commands=commands)
def bind_vertex(vm,s,transfer):
 start=vm.steps;bnd=s['binding'];put(vm,D+0x40bc,V);put(vm,FRAME-0x14,transfer['allocation']['slot']);vm.r.update(ESI=R,EBP=FRAME,EAX=bnd['stride'])
 vm.run(0x1002063a,0x100206ef)
 b['write'](vm,FRAME-0x38,bnd['declaration'][:4]);vm.r.update(EAX=bnd['declaration'][4],EDI=bnd['source']);vm.run(0x10020704,0x10020730)
 shader=bnd['shader'];put(vm,shader['address']+0x150,shader['handle']);vm.r['EAX']=shader['address'];vm.run(0x10020749,0x10020795)
 vm.r['EAX']=bnd['reported_size'];vm.run(0x10020798,0x1002079b);counts['vertex_binding_instructions']+=vm.steps-start
def bind_index(vm,s,transfer):
 start=vm.steps;vm.r.update(ESI=R,EAX=s['source']['width']);vm.run(0x10020b20,0x10020b26);pool=int(vm.z)
 vm.r.update(ESI=R,EBX=I,EDI=s['source']['address'],EBP=FRAME,EAX=transfer['allocation']['element_offset']);put(vm,FRAME+12,s['base']);vm.run(0x10020bc8,0x10020c11)
 vm.r['EAX']=s['reported_size'];vm.run(0x10020c14,0x10020c17);counts['index_binding_instructions']+=vm.steps-start;counts['pool_'+str(pool)]+=1
 return pool
for c,q in zip(fixtures['cases'],report['probes'],strict=True):
 assert c['id']==q['id'];vm=initialize(c['world'])
 for j,(step,actual) in enumerate(zip(c['steps'],q['steps'],strict=True)):
  transfer=globals()[step['kind']](vm,step)
  if step['kind']=='vertex':bind_vertex(vm,step,transfer);pool=None
  else:pool=bind_index(vm,step,transfer)
  expected=dict(result=dict(Ok=dict(transfer=transfer,pool=pool)),world=world(vm,c['world']))
  assert actual==expected,('sequence',c['id'],j,actual['result'],expected['result'])
  counts['steps']+=1
 assert q['world']==world(vm,c['world']);counts['cases']+=1
for c,q in zip(fixtures['errors'],report['errors'],strict=True):
 assert c['id']==q['id'] and q['world']==c['world']
 for step in q['steps']:assert 'Err' in step['result'] and step['world']==c['world']
 counts['safe_errors']+=1
assert counts['cases']==48 and counts['steps']==192 and counts['safe_errors']==10
assert counts['vertex_nooverwrite']>0 and counts['index_nooverwrite']>0 and counts['vertex_resize_calls']>0 and counts['index_resize_calls']>0 and counts['pool_0']>0 and counts['pool_1']>0
counts['total_instructions']=sum(v for k,v in counts.items() if k.endswith('_instructions'))
tests=sum(map(int,re.findall(r'test result: ok\. (\d+) passed',(root/'analysis/reports/d3d-dynamic-tests.log').read_text())));assert tests==731,tests
sources=['crates/rc-package/src/d3d_dynamic.rs','crates/rc-inspect/src/bin/rc-d3d-dynamic-check.rs','scripts/Generate-D3DDynamic.py','scripts/Record-D3DDynamic.py','scripts/Record-D3DBuffers.py','scripts/Record-D3DState.py','scripts/Record-MaterialState.py','analysis/decompiled/d3d-dynamic-ring.c','analysis/decompiled/d3d-dynamic-buffers.c','analysis/decompiled/d3d-dynamic-research.c']+['analysis/decompiled/'+name for name in assemblies]
sources += ['crates/rc-package/src/'+name+'.rs' for name in ['d3d_buffers','d3d_bindings','d3d_state','d3d_transforms','d3d_source','d3d_upload']]
sources += ['scripts/Record-'+name+'.py' for name in ['D3DDraw','D3DComplete','D3DBindings','D3DTransforms']]
validation=dict(date='2026-10-09',counts=dict(counts),rust_tests=tests,debug_release_identical=True,original_d3ddrv_sha256=sha(g['dll']),input_sha256=sha(root/'analysis/reports/d3d-dynamic.input.json'),report_sha256=sha(path),source_sha256={p:sha(root/p) for p in sources},scope=report['scope'],replay_boundary=__doc__,android='deferred until end per user')
(root/'analysis/reports/d3d-dynamic-validation.json').write_text(json.dumps(validation,indent=2)+'\n');print(json.dumps(dict(counts=dict(counts),rust_tests=tests)))
