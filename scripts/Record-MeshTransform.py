"""Original Core matrix/IsZero and complete MeshToWorld CPU replay plus caller copies.
Stable actor +9c returns are address boundaries. The three-instruction x87 height
addition uses a bounded finite f32 + stored-double -> f32 semantic replay; no
general x87 or FPTAN claimed. Renderer +48 is captured, its body/GPU excluded.
The prior render replay executes downstream; safe errors are Rust rollback only.
"""
from pathlib import Path
import json,copy,re,struct,math
from fractions import Fraction
transform_boundary=__doc__
transform_render_path=Path(__file__).resolve().parent/'Record-VertMeshRender.py'
transform_render_source=transform_render_path.read_text()
exec(compile(transform_render_source.split("exec(compile(render_loop",1)[0],str(transform_render_path),'exec'))
__doc__=transform_boundary
for name in ['mesh-transform-full.asm','rotator-matrix.asm']:
 asm=(root/'analysis/decompiled'/name).read_text()
 g['code'].update({int(a,16):(op,args.split(',') if args else []) for a,op,args in re.findall(r'^([0-9a-f]{8}) ([A-Z0-9.]+)(?: (.*))?$',asm,re.M)})
addresses=sorted(g['code']);g['following']={a:addresses[i+1] for i,a in enumerate(addresses[:-1])}
TRANS_ACTOR=0x3e000000
# The body is outside this proof: every +9c accessor returns one stable snapshot.
accessors=[]
for pc,(op,args) in list(g['code'].items()):
 if 0x1044e760<=pc<=0x1044f1af and op=='CALL' and '+ 0x9c]' in args[0]:
  accessors.append(pc);g['code'][pc]=('MOV',['EAX',hex(TRANS_ACTOR)])
assert len(accessors)==8
# Native TEST CH,8 is equivalent to TEST ECX,0x800 for the sole following JZ.
assert g['code'][0x1044ecc8]==('TEST',['CH','0x8'])
g['code'][0x1044ecc8]=('TEST',['ECX','0x800'])

# Verify PE constants/imports/exports rather than relying on assumed DLL bases.
core_bytes=core.read_bytes();core_opt,core_off,core_str=pe_view(core_bytes)
assert struct.unpack_from('<I',core_bytes,core_opt+28)[0]==0x10100000
assert struct.unpack_from('<I',core_bytes,core_off(0x86c18))[0]==0x3f800000
height=struct.unpack('<d',struct.pack('<II',engine_word(0x106739f8),engine_word(0x106739fc)))[0]
assert height==5.850000090897083
table=(root/'crates/rc-package/src/material_sine_table.bin').read_bytes()
step=struct.unpack_from('<f',core_bytes,core_off(0x885d4))[0]
assert struct.unpack('<16384I',table)==tuple(g['float_word'](math.sin(i*step)) for i in range(16384))
export=core_off(struct.unpack_from('<I',core_bytes,core_opt+96)[0]);nf=struct.unpack_from('<I',core_bytes,export+24)[0]
func,names,ords=[core_off(v) for v in struct.unpack_from('<III',core_bytes,export+28)]
exports={}
for i in range(nf):
 name=core_str(struct.unpack_from('<I',core_bytes,names+4*i)[0]);ordinal=struct.unpack_from('<H',core_bytes,ords+2*i)[0]
 exports[name]=0x10100000+struct.unpack_from('<I',core_bytes,func+ordinal*4)[0]
expected={0x10650248:'??0FMatrix@@QAE@ABVFRotator@@ABVFVector@@@Z',0x1065096c:'?IsZero@FRotator@@QBEHXZ'}
assert [exports[n] for n in expected.values()]==[0x101444a0,0x10115130]
eo,ef,es=pe_view(engine_raw);imp=ef(struct.unpack_from('<I',engine_raw,eo+104)[0]);found={}
while True:
 original,stamp,chain,name,iat=struct.unpack_from('<5I',engine_raw,imp)
 if not name:break
 thunk=ef(original or iat);i=0
 while True:
  entry=struct.unpack_from('<I',engine_raw,thunk+4*i)[0]
  if not entry:break
  address=0x10300000+iat+4*i
  if address in expected:found[address]=(es(name).lower(),es(entry+2))
  i+=1
 imp+=20
assert found=={a:('core.dll',n) for a,n in expected.items()}

def core_ctor(vm,pc,constructors):
 sp=vm.r['ESP'];out=vm.r['ECX'];rot=get(vm,sp);pos=get(vm,sp+4)
 rotation=[signed(get(vm,rot+i*4)) for i in range(3)];translation=[get(vm,pos+i*4) for i in range(3)]
 vm.push(g['following'][pc]);vm.steps+=1
 vm.run(0x101444a0,0x1014461c)
 assert vm.r['EAX']==out and vm.pop()==g['following'][pc]
 vm.r['ESP']+=8;vm.steps+=1;assert vm.r['ESP']==sp+8
 answer=[[get(vm,out+16*r+4*c) for c in range(4)] for r in range(4)]
 constructors.append(dict(rotation=rotation,translation=translation,matrix=answer));counts['matrix_constructors']+=1

def rounded_binary(value, precision, minimum_exponent):
 if not value:return value
 sign=-1 if value<0 else 1;value=abs(value)
 exponent=value.numerator.bit_length()-value.denominator.bit_length()
 power=lambda e:Fraction(2**e) if e>=0 else Fraction(1,2**(-e))
 if value<power(exponent):exponent-=1
 unit=power(max(exponent-precision+1,minimum_exponent))
 return sign*round(value/unit)*unit

def native_transform(vm,q,instance,out):
 start=vm.steps;mesh=0x39000000;actor=TRANS_ACTOR;ctors=[]
 put(vm,instance,0x24000000);put(vm,instance+0x54,mesh);put(vm,instance+0x74,q['suppress_origin']);vm.put(f'byte ptr [{instance+0x78}]',q['bypass_mesh_adjustments'])
 for address,values in [(actor+0x138,q['actor_location']),(actor+0x144,q['actor_rotation']),(actor+0x1b4,q['actor_draw_scale_3d']),(actor+0x1c0,q['actor_prepivot']),(mesh+0x70,q['mesh_scale']),(mesh+0x7c,q['mesh_origin']),(mesh+0x88,q['mesh_rotation'])]:
  for i,v in enumerate(values):put(vm,address+4*i,v&0xffffffff)
 put(vm,actor+0x6c,q['actor_flags']);put(vm,actor+0x1b0,q['actor_draw_scale']);put(vm,actor+0x1fc,q['actor_height'])
 put(vm,0x10653578,0x3f800000);put(vm,0x10186c18,0x3f800000);put(vm,0x10650248,0x101444a0)
 vm.m.update({0x101c37ec+i:b for i,b in enumerate(table)})
 savedsp=vm.r['ESP'];vm.push(0x10560174);vm.steps+=1;vm.r['ECX']=instance
 vm.run(0x1044e760,0x1044e8a8);frame=vm.r['ESP']
 scale=[get(vm,frame+0x30+4*i) for i in range(3)]
 if q['bypass_mesh_adjustments']:
  vm.run(0x1044e8a8,0x1044edca);counts['bypass']+=1
 else:
  vm.run(0x1044e8a8,0x1044e919)
  vm.push(0x1044e91f);vm.steps+=1
  zero=all((v&65535)==0 for v in q['mesh_rotation'])
  vm.run(0x10115130,0x1011514a if zero else 0x1011514d);assert vm.pop()==0x1044e91f;vm.steps+=1
  assert vm.r['EAX']==int(zero);counts['is_zero_calls']+=1
  if zero:vm.run(0x1044e91f,0x1044ecbb);counts['zero_mesh_rotation']+=1
  else:
   vm.run(0x1044e91f,0x1044e951);core_ctor(vm,0x1044e951,ctors);vm.run(0x1044e953,0x1044ecbb);counts['mesh_rotations']+=1
  if q['actor_flags']&0x800:
   vm.run(0x1044ecbb,0x1044ed02);core_ctor(vm,0x1044ed02,ctors);vm.run(0x1044ed04,0x1044ed25)
   # Exact restricted FLD f32 / FADD double / FSTP f32 semantics. Both inputs
   # are binary floats; the final finite f32 agrees with nearest extended sum.
   # Independent rational oracle: nearest 64-significand-bit extended addition,
   # then nearest 24-bit/subnormal float32 store. No host double-sum oracle.
   h=Fraction(g['float_value'](get(vm,vm.r['EAX']+0x1fc)))+Fraction(height);vm.steps+=2
   h=rounded_binary(rounded_binary(h,64,-16445),24,-149)
   vm.run(0x1044ed31,0x1044ed43);put(vm,vm.r['ESP']+0x40,g['float_word'](float(h)));vm.steps+=1
   vm.run(0x1044ed47,0x1044edca);counts['height_corrections']+=1
  else:vm.run(0x1044ecbb,0x1044edca);counts['prepivot_corrections']+=1
 base=[[get(vm,frame+0x4c+16*r+4*c) for c in range(4)] for r in range(4)]
 location=[get(vm,frame+0x20+4*i) for i in range(3)]
 vm.run(0x1044edca,0x1044ede6);core_ctor(vm,0x1044ede6,ctors);vm.run(0x1044ede8,0x1044f1af)
 assert vm.r['EAX']==out and vm.pop()==0x10560174;vm.r['ESP']+=4;vm.steps+=1
 assert vm.r['ESP']==savedsp+4
 answer=[[get(vm,out+16*r+4*c) for c in range(4)] for r in range(4)]
 counts['transform_instructions']+=vm.steps-start;counts['transform_calls']+=1
 return dict(effective_scale=scale,base_matrix=base,adjusted_location=location,constructors=ctors,matrix=answer)

def model_handoff(vm,q,instance=0x76000000,renderer=0x21000000):
 start=vm.steps;put(vm,0x106502d8,0x77000000)
 for i in range(16):put(vm,0x77000000+4*i,0x3f800000 if i//4==i%4 else 0)
 put(vm,EF-0x38,instance);put(vm,instance,0x24000000);put(vm,EF+0x18,renderer);put(vm,renderer,0x24000000)
 vm.r.update(EBP=EF,ESP=0x70060000)
 vm.run(0x1056014f,0x1056016e);assert get(vm,vm.r['ESP'])==EF-0xec and vm.r['ECX']==instance
 value=native_transform(vm,q,instance,EF-0xec)
 vm.run(0x10560174,0x10560191);assert vm.r['ECX']==renderer and vm.pop()==0 and vm.pop()==EF-0xac
 copied=[[get(vm,EF-0xac+16*r+4*c) for c in range(4)] for r in range(4)];assert copied==value['matrix'];vm.steps+=1
 counts['transform_handoffs']+=1;counts['handoff_instructions']+=vm.steps-start
 return value,dict(receiver=renderer,vtable_offset=0x48,slot=0,matrix=copied)

fixtures=json.loads((root/'analysis/reports/mesh-transform.input.json').read_text());path=root/'analysis/reports/mesh-transform.json';report=json.loads(path.read_text())
assert path.read_bytes()==(root/'analysis/reports/mesh-transform-release.json').read_bytes()
assert fixtures['source_sha256']==report['source_sha256']==sha(root/'analysis/reports/vertmesh-render.json')==json.loads((root/'analysis/reports/vertmesh-render-validation.json').read_text())['report_sha256']
for q,expected in zip(fixtures['math'],report['math'],strict=True):
 vm=g['Native']();value,h=model_handoff(vm,q);assert expected==dict(Ok=value),difference(expected,dict(Ok=value));counts['math_cases']+=1

# Preserve the previously checked full downstream replay and insert the original
# +e8 call/body/copy/+48 sequence before camera work. Actor snapshot is authoritative.
transform_loop=render_loop
old="q=copy.deepcopy(q);start=vm.steps;prepared=preprocess(vm,q['preprocessing'])"
new="""outer=copy.deepcopy(q);start=vm.steps
  q=outer['render'];prepared=preprocess(vm,q['preprocessing'])
  transformed,model=model_handoff(vm,outer['transform'],outer['render']['vertex']['instance'],outer['render']['geometry']['mesh']['materials']['renderer_address'])
  q['actor_position']=outer['transform']['actor_location'].copy();q['actor_draw_scale']=outer['transform']['actor_draw_scale_3d'].copy();q['camera']['actor_draw_scale']=outer['transform']['actor_draw_scale']"""
assert transform_loop.count(old)==1;transform_loop=transform_loop.replace(old,new)
old="  assert step['result']==dict(Ok=value),('result',c['id'],j,difference(step['result'],dict(Ok=value)))"
new="""  value=dict(transform=transformed,handoff=model,render=value)
  assert step['result']==dict(Ok=value),('result',c['id'],j,difference(step['result'],dict(Ok=value)))"""
assert transform_loop.count(old)==1;transform_loop=transform_loop.replace(old,new)
transform_loop=transform_loop.replace("assert step['request']==q,", "assert step['request']==outer,")
transform_loop=transform_loop.replace("counts['safe_errors']==12", "counts['safe_errors']==11")
exec(compile(transform_loop,str(__file__)+'::joined-transform-render','exec'))
tests=sum(map(int,re.findall(r'test result: ok\. (\d+) passed',(root/'analysis/reports/mesh-transform-tests.log').read_text())));assert tests==920,tests
sources=set(json.loads((root/'analysis/reports/vertmesh-render-validation.json').read_text())['source_sha256'])
sources.update(['crates/rc-package/src/lib.rs','crates/rc-package/src/mesh_transform.rs','crates/rc-package/src/material_panner.rs','crates/rc-package/src/material_sine_table.bin','crates/rc-inspect/src/bin/rc-mesh-transform-check.rs','scripts/Generate-MeshTransform.py','scripts/Record-MeshTransform.py','analysis/decompiled/mesh-transform-full.asm','analysis/decompiled/mesh-transform.c','analysis/decompiled/rotator-matrix.asm','analysis/decompiled/rotator-matrix.c','docs/MESH_TRANSFORM.md','analysis/finalize-mesh-transform.py'])
validation=dict(date='2026-10-10',counts=dict(counts),rust_tests=tests,debug_release_identical=True,original_core_sha256=sha(core),original_engine_sha256=sha(draw['engine']),original_d3ddrv_sha256=sha(g['dll']),input_sha256=sha(root/'analysis/reports/mesh-transform.input.json'),report_sha256=sha(path),source_sha256={a:sha(root/a) for a in sorted(sources)},scope=report['scope'],replay_boundary=transform_boundary,android='deferred until end per user')
(root/'analysis/reports/mesh-transform-validation.json').write_text(json.dumps(validation,indent=2)+'\n');print(json.dumps(dict(counts=dict(counts),rust_tests=tests)))
