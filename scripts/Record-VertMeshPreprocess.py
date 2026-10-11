"""Original preprocessing/constructors/GetAbsMax joined to scene/LOD/draw replay.

Allocator count/pointer changes and allocated bytes are supplied boundaries.
__allmul returns the exact low 64-bit product; its body is not interpreted.
SEH registration, logging and final unwind are excluded. Native copy/store,
grouping, constructor and carry instructions execute. x87 FLD/FSTP transfer of
finite float32 GetAbsMax result is exact; general x87/FPTAN not implemented.
Animation/GetFrame filling, material/COM/GPU and matrix construction remain open.
"""
from pathlib import Path
import json,copy,re,struct,math,ast
preprocess_boundary=__doc__
scene_shared_path=Path(__file__).resolve().parent/'Record-SceneProjection.py'
scene_shared_src=scene_shared_path.read_text()
scene_prefix=scene_shared_src.split("\nfor c,actual in zip(fixtures['math']",1)[0]
old_abs="put(vm,EF-0x18,c['actor_abs_max']);vm.steps+=2"
assert scene_prefix.count('prefix.replace(old,new)')==1
scene_prefix=scene_prefix.replace('prefix.replace(old,new)',f'prefix.replace(old,new).replace({old_abs!r},"abs_call(vm,ACTOR+0x1b4,EF-0x18)")')
exec(compile(scene_prefix,str(scene_shared_path),'exec'))
__doc__=preprocess_boundary
for name in ['vertmesh-preprocess.asm','raw-index-ctor.asm','vector-absmax.asm']:
 assemblies.append(name);asm=(root/'analysis/decompiled'/name).read_text()
 g['code'].update({int(a,16):(op,args.split(',') if args else []) for a,op,args in re.findall(r'^([0-9a-f]{8}) ([A-Z0-9.]+)(?: (.*))?$',asm,re.M)})
addresses=sorted(g['code']);g['following']={a:addresses[i+1] for i,a in enumerate(addresses[:-1])}
fixtures=json.loads((root/'analysis/reports/vertmesh-preprocess.input.json').read_text());path=root/'analysis/reports/vertmesh-preprocess.json';report=json.loads(path.read_text())
assert path.read_bytes()==(root/'analysis/reports/vertmesh-preprocess-release.json').read_bytes()
assert fixtures['source_sha256']==report['source_sha256']==sha(root/'analysis/reports/scene-projection.input.json')
assert fixtures['passes_sha256']==report['passes_sha256']==sha(root/'analysis/reports/d3d-draw.input.json')

# Resolve the DLL export and caller import by name, not a guessed module base.
core=Path(r'D:\SteamLibrary\steamapps\common\Star Wars Republic Commando\GameData\System\core.dll')
def pe_view(raw):
 pe=struct.unpack_from('<I',raw,60)[0];opt=pe+24;size=struct.unpack_from('<H',raw,pe+20)[0]
 sections=[struct.unpack_from('<IIII',raw,opt+size+40*i+8) for i in range(struct.unpack_from('<H',raw,pe+6)[0])]
 def offset(rva):
  for size,va,n,off in sections:
   if va<=rva<va+max(size,n):return off+rva-va
  raise AssertionError(hex(rva))
 def string(rva):
  off=offset(rva);return raw[off:raw.index(0,off)].decode('ascii')
 return opt,offset,string
raw_core=core.read_bytes();opt,offset,string=pe_view(raw_core);base=struct.unpack_from('<I',raw_core,opt+28)[0]
export=offset(struct.unpack_from('<I',raw_core,opt+96)[0]);num=struct.unpack_from('<I',raw_core,export+24)[0]
func,names,ords=[offset(v) for v in struct.unpack_from('<III',raw_core,export+28)]
found=[]
for i in range(num):
 name=string(struct.unpack_from('<I',raw_core,names+4*i)[0])
 if name=='?GetAbsMax@FVector@@QBEMXZ':
  ordinal=struct.unpack_from('<H',raw_core,ords+2*i)[0];found.append(base+struct.unpack_from('<I',raw_core,func+4*ordinal)[0])
assert found==[0x1011e5b0]
opt,offset,string=pe_view(engine_raw);base=struct.unpack_from('<I',engine_raw,opt+28)[0]
imp=offset(struct.unpack_from('<I',engine_raw,opt+104)[0]);found=[]
while True:
 original,stamp,chain,name,iat=struct.unpack_from('<5I',engine_raw,imp)
 if not name:break
 dll_name=string(name);thunk=offset(original or iat);i=0
 while True:
  entry=struct.unpack_from('<I',engine_raw,thunk+4*i)[0]
  if not entry:break
  if base+iat+4*i==0x106509f8:found.append((dll_name.lower(),string(entry+2)))
  i+=1
 imp+=20
assert found==[('core.dll','?GetAbsMax@FVector@@QBEMXZ')]
counts['import_export_regressions']=2

# Original cache counter ADD/ADC sequence, including low/high wrap and signed overflow.
for low,high in [(0xffffffff,7),(0xffffffff,0xffffffff),(0xffffffff,0x7fffffff),(0,9)]:
 vm=g['Native']();CACHE=0x75000000;put(vm,0x106502a0,CACHE)
 vm.r.update(ECX=CACHE,EDI=low,EBX=high)
 vm.run(0x1055fde0,0x1055fdf1)
 carry=low==0xffffffff;expected_high=(high+carry)&0xffffffff
 assert get(vm,CACHE)==(low+1)&0xffffffff and get(vm,CACHE+4)==expected_high
 assert vm.c==(carry and high==0xffffffff)
 assert vm.o==(carry and high==0x7fffffff)
counts['adc_regressions']=4

def abs_call(vm,position,out):
 assert vm.r['ECX']==position
 before=vm.r['ESP'];vm.push(0x1056025e);vm.steps+=1
 vm.run(0x1011e5b0,0x1011e609)
 value=get(vm,vm.r['ESP']);vm.steps+=1 # FLD finite float32 -> exact extended value
 vm.run(0x1011e60c,0x1011e60d)
 assert vm.pop()==0x1056025e and vm.r['ESP']==before
 vm.steps+=1;put(vm,out,value);vm.steps+=1 # RET and original FSTP float32
 counts['abs_max_calls']+=1
 return value

def multiply(vm):
 low=vm.pop();high=vm.pop();assert vm.pop()==256 and vm.pop()==0
 product=(((high<<32)|low)*256)&0xffffffffffffffff
 vm.r['EAX']=product&0xffffffff;vm.r['EDX']=product>>32;vm.steps+=1
 counts['multiply_boundaries']+=1

def resize(vm,obj,offset,count,address):
 assert vm.r['ECX']==obj+offset and vm.pop()==count and vm.pop()==0
 put(vm,obj+offset,address);put(vm,obj+offset+4,(get(vm,obj+offset+4)&0xe0000000)|count)
 vm.steps+=1;counts['array_boundaries']+=1

def preprocess(vm,q):
 start=vm.steps;state=q['state'];OBJ=0x39000000;STREAM=0x72000000;WEDGES=0x71000000;FACES=0x73000000;SEC=0x74000000;CACHE=0x75000000
 for off,value in [(0x1a4,state['packed_sections']),(0x174,state['packed_stream_count']),(0x170,STREAM),(0x16c,state['revision']),(0xac,WEDGES),(0xb0,q['packed_wedges']),(0x9c,FACES),(0xa0,q['packed_faces']),(0x1a0,SEC)]:put(vm,OBJ+off,value)
 vm.r.update(EBX=OBJ,EBP=EF,ESP=0x70060000);put(vm,EF-0x18,OBJ);put(vm,EF-4,0)
 vm.run(0x1055fbfe,0x1055fc08)
 if not vm.z:
  counts['preprocess_skipped']+=1;counts['preprocessing_instructions']+=vm.steps-start
  return dict(processed=False,state=copy.deepcopy(state))
 for i,v in enumerate(state['section_bytes']):vm.m[SEC+i]=v
 for i,w in enumerate(q['wedges']):
  for j,value in enumerate(w):put(vm,WEDGES+i*12+j*4,value)
 for i,w in enumerate(q['faces']):
  for j,value in enumerate(w):vm.put(f'word ptr [{FACES+i*8+j*2}]',value)
 put(vm,0x106502a0,CACHE);put(vm,CACHE,state['cache_index'][0]);put(vm,CACHE+4,state['cache_index'][1])
 vm.run(0x1055fc2c,0x1055fc48);resize(vm,OBJ,0x170,0,0)
 vm.run(0x1055fc4d,0x1055fc72);nw=q['packed_wedges']&0x1fffffff
 resize(vm,OBJ,0x170,nw,STREAM)
 for i in range(nw*32):vm.m[STREAM+i]=0xcc
 vm.run(0x1055fc77,0x1055fcb0)
 while True:
  vm.run(0x1055fcb0,0x1055fcbe);vm.steps+=1
  if vm.s==vm.o:break
  vm.run(0x1055fcc0,0x1055fcb0);counts['uv_copies']+=1
 # Logging excluded, original loop setup retained.
 vm.run(0x1055fd1d,0x1055fd23)
 while True:
  vm.run(0x1055fd23,0x1055fd34);vm.steps+=1
  if vm.s==vm.o:break
  vm.run(0x1055fd3a,0x1055fd54);new=vm.z;vm.steps+=1
  if not new:
   vm.run(0x1055fd56,0x1055fd5b);new=not vm.z;vm.steps+=1
  if new:
   vm.run(0x1055fd61,0x1055fd7f);nsec=(get(vm,OBJ+0x1a4)&0x1fffffff)+1
   resize(vm,OBJ,0x1a0,nsec,SEC)
   for k,v in enumerate(q['section_allocation'][(nsec-1)*80:nsec*80]):vm.m[SEC+(nsec-1)*80+k]=v
   vm.run(0x1055fd84,0x1055fd9e);assert not vm.z;vm.steps+=1
   vm.run(0x1055fda0,0x1055fdcd);multiply(vm)
   vm.run(0x1055fdd2,0x1055fe09)
   vm.push(0x1055fe0e);vm.steps+=1 # actual FRawIndexBuffer constructor CALL
   vm.run(0x104c7c00,0x104c7c35);multiply(vm)
   vm.run(0x104c7c3a,0x104c7c67)
   assert vm.pop()==0x1055fe0e;vm.steps+=1
   vm.run(0x1055fe0e,0x1055fe1b)
   vm.run(0x1055fe1b,0x1055fe46);counts['constructed_sections']+=1
  vm.run(0x1055fe46,0x1055fd23);counts['preprocessed_faces']+=1
 nsec=get(vm,OBJ+0x1a4)&0x1fffffff
 image_length=nsec*80 if nsec else len(state['section_bytes'])
 result=dict(packed_sections=get(vm,OBJ+0x1a4),section_bytes=[vm.m[SEC+i] for i in range(image_length)],packed_stream_count=get(vm,OBJ+0x174),stream=[vm.m[STREAM+i] for i in range(nw*32)],revision=get(vm,OBJ+0x16c),cache_index=[get(vm,CACHE),get(vm,CACHE+4)])
 counts['preprocessing_runs']+=1;counts['preprocessing_instructions']+=vm.steps-start
 return dict(processed=True,state=result)

def first_difference(a,b,path=''):
 if type(a)!=type(b):return path,type(a).__name__,type(b).__name__
 if isinstance(a,dict):
  for k in a:
   if a[k]!=b[k]:return first_difference(a[k],b[k],path+'.'+k)
 elif isinstance(a,list):
  if len(a)!=len(b):return path+'.length',len(a),len(b)
  for k in range(len(a)):
   if a[k]!=b[k]:return first_difference(a[k],b[k],path+'['+str(k)+']')
 return path,a,b

for x,y in zip(fixtures['abs_probes'],report['abs_probes'],strict=True):
 if any(not math.isfinite(struct.unpack('<f',struct.pack('<I',v))[0]) for v in x):assert 'Err' in y;counts['abs_safe_errors']+=1;continue
 vm=g['Native']();POS=0x6f600000;OUT=POS+16
 for i,v in enumerate(x):put(vm,POS+4*i,v)
 vm.r.update(ECX=POS,ESP=0x70080000);value=abs_call(vm,POS,OUT)
 assert y==dict(Ok=value);counts['abs_instructions']+=vm.steps;counts['abs_probes']+=1
for q,y in zip(fixtures['preparations'],report['preparations'],strict=True):
 if 'Err' in y:
  nw=(q['packed_wedges']<<3&0xffffffff);nw=signed(nw)>>3
  nf=(q['packed_faces']<<3&0xffffffff);nf=signed(nf)>>3
  runs=sum(i==0 or q['faces'][i-1][3]!=face[3] for i,face in enumerate(q['faces'][:max(nf,0)]))
  assert nw<0 or nw>65536 or nf<0 or nf>65536 or runs>256 or len(q['section_allocation'])!=runs*80
  counts['preparation_safe_errors']+=1;continue
 vm=g['Native']();value=preprocess(vm,q);assert y==dict(Ok=value),('preparation',first_difference(y,dict(Ok=value)))
 counts['preparation_probes']+=1

# Execute the existing complete downstream loop with prepared snapshots/abs input.
lod_src=(root/'scripts/Record-VertMeshLod.py').read_text()
loop='\nfor c,actual in zip'+lod_src.split('\nfor c,actual in zip',1)[1].split('\ntests=sum(',1)[0]
old="q=copy.deepcopy(q);start=vm.steps;lod,gate,vertex_call=camera(vm,q)"
new="""q=copy.deepcopy(q);outer=q;start=vm.steps
  prepared=preprocess(vm,outer['preprocessing']);outer['preprocessing']['state']=copy.deepcopy(prepared['state'])
  whole=outer['mesh'];q=whole['mesh'];state=prepared['state'];q['gates']['packed_sections']=state['packed_sections']
  geom=q['geometry']['geometry'];geom['packed_count']=state['packed_sections'];geom['section_bytes']=state['section_bytes'].copy()
  if prepared['processed']:
   inp=outer['preprocessing'];geom['faces']=copy.deepcopy(inp['faces'][:inp['packed_faces']&0x1fffffff]);geom['vertex_thresholds']=[w[0]&65535 for w in inp['wedges'][:inp['packed_wedges']&0x1fffffff]]
  matrix(vm,q['vertex']['scene'],0x114,whole['world_to_screen']);actor=q['geometry']['mesh']['materials']['actor_address']
  for k,v in enumerate(whole['actor_position']):put(vm,actor+0x138+4*k,v)
  for k,v in enumerate(outer['actor_draw_scale']):put(vm,actor+0x1b4+4*k,v)
  lod,gate,vertex_call=camera(vm,q);projected=[get(vm,EF-0x48+4*k) for k in range(4)]
  absolute=get(vm,EF-0x18);q['camera']['projected_w']=projected[3];q['camera']['actor_abs_max']=absolute"""
assert loop.count(old)==1;loop=loop.replace(old,new)
old="  assert step['result']==dict(Ok=value)"
new="  value=dict(preprocessing=prepared,actor_abs_max=absolute,mesh=dict(projection=projected,mesh=value));whole['mesh']=q;outer['mesh']=whole;q=outer\n"+old
assert loop.count(old)==1;loop=loop.replace(old,new)
exec(compile(loop,str(__file__)+'::composed-loop','exec'))
tests=sum(map(int,re.findall(r'test result: ok\. (\d+) passed',(root/'analysis/reports/vertmesh-preprocess-tests.log').read_text())));assert tests==879,tests
previous_log=root/'analysis/reports/vertmesh-preprocess-previous-replay.log'
previous=ast.literal_eval(previous_log.read_text())
assert previous['previous_scene_replay_passed'] and previous['counts']['math_probes']==327 and previous['counts']['requests']==146
sources=set(json.loads((root/'analysis/reports/scene-projection-validation.json').read_text())['source_sha256'])
sources.update(['crates/rc-package/src/vertmesh_preprocess.rs','crates/rc-inspect/src/bin/rc-vertmesh-preprocess-check.rs','scripts/Generate-VertMeshPreprocess.py','scripts/Record-VertMeshPreprocess.py','analysis/decompiled/vertmesh-preprocess.asm','analysis/decompiled/vertmesh-preprocess.c','analysis/decompiled/vertmesh-preprocess-helpers.c','analysis/decompiled/raw-index-ctor.asm','analysis/decompiled/vector-absmax.asm','analysis/decompiled/vector-absmax.c','docs/VERTMESH_PREPROCESS.md','analysis/finalize-vertmesh-preprocess.py'])
validation=dict(date='2026-10-10',counts=dict(counts),rust_tests=tests,debug_release_identical=True,previous_scene_replay=previous,previous_scene_log_sha256=sha(previous_log),original_core_sha256=sha(core),original_engine_sha256=sha(draw['engine']),original_d3ddrv_sha256=sha(g['dll']),input_sha256=sha(root/'analysis/reports/vertmesh-preprocess.input.json'),report_sha256=sha(path),source_sha256={a:sha(root/a) for a in sorted(sources)},scope=report['scope'],replay_boundary=preprocess_boundary,android='deferred until end per user')
(root/'analysis/reports/vertmesh-preprocess-validation.json').write_text(json.dumps(validation,indent=2)+'\n');print(json.dumps(dict(counts=dict(counts),rust_tests=tests)))
