"""Scalar inverse original instructions under explicit nearest x87 precision53.
Independent exact rational x87 stack, rounded after each arithmetic instruction,
f32 memory spills and reciprocal finite gate. Optimized dispatch variants and
other precision modes are excluded. Existing joined CPU render proof reused.
"""
from pathlib import Path
import json,copy,re,struct,math
from fractions import Fraction
inverse_boundary=__doc__
inverse_renderer_path=Path(__file__).resolve().parent/'Record-RendererTransform.py'
inverse_renderer_source=inverse_renderer_path.read_text()
exec(compile(inverse_renderer_source.split("\nfixtures=json.loads((root/'analysis/reports/renderer-transform.input.json')",1)[0],str(inverse_renderer_path),'exec'))
__doc__=inverse_boundary
asm=(root/'analysis/decompiled/renderer-inverse-scalar.asm').read_text()
g['code'].update({int(a,16):(op,args.split(',') if args else []) for a,op,args in re.findall(r'^([0-9a-f]{8}) ([A-Z0-9.]+)(?: (.*))?$',asm,re.M)})
addresses=sorted(g['code']);g['following']={a:addresses[i+1] for i,a in enumerate(addresses[:-1])}

class Excluded(Exception):pass
class Number:
 def __init__(self,value,negative=False):
  self.value=value;self.negative=value<0 if value else negative
 def neg(self):return Number(-self.value if self.value is not None else None,not self.negative)
 def mul(self,other):
  assert self.value is not None and other.value is not None
  return Number(rounded_binary(self.value*other.value,53,-16445),self.negative!=other.negative)
 def add(self,other):
  assert self.value is not None and other.value is not None
  return Number(rounded_binary(self.value+other.value,53,-16445),self.negative and other.negative)
 def div(self,other):
  assert self.value is not None and other.value is not None
  if not other.value:return Number(None,self.negative!=other.negative)
  return Number(rounded_binary(self.value/other.value,53,-16445),self.negative!=other.negative)
 def word(self):
  if self.value is None:return 0xff800000 if self.negative else 0x7f800000
  rounded=rounded_binary(self.value,24,-149)
  if abs(rounded)>=2**128:return 0xff800000 if self.negative else 0x7f800000
  if not rounded:return 0x80000000 if self.negative else 0
  return g['float_word'](float(rounded))
 def double(self):
  if self.value is None:return float('-inf') if self.negative else float('inf')
  return float(self.value) if self.value else -0.0 if self.negative else 0.0
def from_word(word):
 value=g['float_value'](word)
 if math.isnan(value):raise Excluded('NaN spill')
 if math.isinf(value):return Number(None,bool(word&0x80000000))
 return Number(Fraction(value),bool(word&0x80000000))

# Extend only this proof's VM. Integer memory/branches use the existing checked
# interpreter; x87 arithmetic uses rational numbers, not Python double math.
inverse_native_source=(root/'scripts/Record-D3DState.py').read_text()
inverse_run_source=' def run(self,start,stop):'+inverse_native_source.split(' def run(self,start,stop):',1)[1].split('\nBASE=',1)[0]
new='''if op=='FLD':
    self.fpu.insert(0,self.fpu[int(a[0][2:])] if a[0].startswith('ST') else from_word(self.get(a[0])))
   elif op=='FLD1':self.fpu.insert(0,Number(Fraction(1)))
   elif op in ['FMUL','FDIV']:
    other=self.fpu[int(a[-1][2:])] if a[-1].startswith('ST') else from_word(self.get(a[-1]))
    self.fpu[0]=self.fpu[0].mul(other) if op=='FMUL' else self.fpu[0].div(other)
   elif op in ['FADDP','FSUBP']:
    other=self.fpu[0] if op=='FADDP' else self.fpu[0].neg()
    self.fpu[1]=self.fpu[1].add(other);self.fpu.pop(0)
   elif op in ['FST','FSTP']:
    value=self.fpu[0]
    if a[0].startswith('ST'):self.fpu[int(a[0][2:])]=value
    elif a[0].startswith('double ptr'):
     at=self.address(a[0]);self.m.update({at+i:b for i,b in enumerate(struct.pack('<d',value.double()))})
    else:
     assert a[0].startswith('float ptr');self.put(a[0],value.word())
    if op=='FSTP':self.fpu.pop(0)
   elif op=='LEAVE':self.r['ESP']=self.r['EBP'];self.r['EBP']=self.pop()
   elif op=='CALL' and a==['0x1006c464']:
    value=struct.unpack('<d',bytes(self.m[self.r['ESP']+i] for i in range(8)))[0]
    self.r['EAX']=int(math.isfinite(value));counts['inverse_finite_boundaries']+=1
   elif op=='FILD':'''
assert inverse_run_source.count("if op=='FILD':")==1
inverse_run_source=inverse_run_source.replace("if op=='FILD':",new)
inverse_vm_env=dict(g,Number=Number,from_word=from_word,counts=counts,Fraction=Fraction)
inverse_vm_env['NativeBase']=g['Native']
exec(compile('class InverseNative(NativeBase):\n'+inverse_run_source,str(__file__)+'::rational-x87','exec'),inverse_vm_env)
InverseNative=inverse_vm_env['InverseNative']

def inverse_call(vm,store_determinant,return_pc=0x1001fcdc):
 sp=vm.r['ESP'];out=get(vm,sp);det=get(vm,sp+4);source=get(vm,sp+8);assert bool(det)==store_determinant
 before=[get(vm,out+k*4) for k in range(16)];start=vm.steps;old_ebp=vm.r['EBP'];assert not vm.fpu
 # Share the actual caller's byte memory/registers with the specialized runner.
 oracle=InverseNative();oracle.__dict__=vm.__dict__
 oracle.push(return_pc);oracle.steps+=2 # CALL and selected scalar thunk JMP
 frame=oracle.r['ESP']-4
 oracle.run(0x10031709,0x10031a93)
 reciprocal=get(oracle,frame-8);written=math.isfinite(g['float_value'](reciprocal))
 matrix=[get(oracle,out+k*4) for k in range(16)] if written else None
 if not written:assert [get(oracle,out+k*4) for k in range(16)]==before
 assert oracle.pop()==return_pc;oracle.r['ESP']+=12;oracle.steps+=1
 assert oracle.r['ESP']==sp+12 and oracle.r['EBP']==old_ebp and not oracle.fpu
 assert oracle.r['EAX']==out if written else oracle.r['EAX']==0
 counts['scalar_inverse_calls']+=1;counts['scalar_inverse_instructions']+=oracle.steps-start
 counts['inverse_written' if written else 'inverse_unchanged']+=1
 return dict(determinant=get(oracle,det) if det else None,reciprocal=reciprocal,matrix=matrix)

def inverse_math(q):
 vm=g['Native']();source=0x79000000;out=source if q['alias'] else 0x7a000000;det=0x7b000000 if q['store_determinant'] else 0
 for k,w in enumerate(q['matrix']):put(vm,source+k*4,w)
 if out!=source:
  for k in range(16):put(vm,out+k*4,0xa5a50000+k)
 vm.r['ESP']=0x70069000;vm.push(source);vm.push(det);vm.push(out)
 return inverse_call(vm,q['store_determinant'])

fixtures=json.loads((root/'analysis/reports/renderer-inverse.input.json').read_text());path=root/'analysis/reports/renderer-inverse.json';report=json.loads(path.read_text())
assert path.read_bytes()==(root/'analysis/reports/renderer-inverse-release.json').read_bytes()
assert fixtures['source_sha256']==report['source_sha256']==sha(root/'analysis/reports/renderer-transform.json')==json.loads((root/'analysis/reports/renderer-transform-validation.json').read_text())['report_sha256']
for q,expected in zip(fixtures['math'],report['math'],strict=True):
 if 'Err' in expected:
  assert 'Nonfinite scalar inverse' in expected['Err'];counts['math_safe_errors']+=1;continue
 value=inverse_math(q);assert expected==dict(Ok=value),difference(expected,dict(Ok=value));counts['math_cases']+=1
 counts['aliased_math_cases']+=int(q['alias'])
 if value['matrix'] is not None:
  counts['zero_reciprocal_writes']+=int(value['reciprocal']&0x7fffffff==0)
  counts['nonfinite_output_words']+=sum(w&0x7f800000==0x7f800000 for w in value['matrix'])
assert counts['math_cases']==544 and counts['math_safe_errors']==24
assert counts['zero_reciprocal_writes']>=2 and counts['nonfinite_output_words']>=1

# Replace the former inverse-response boundary with the actual scalar body at
# the original CALL: its three stack arguments and output address stay intact.
inverse_set_source='def renderer_set(vm,slot,words,inverse):'+inverse_renderer_source.split('def renderer_set(vm,slot,words,inverse):',1)[1].split('\nrenderer_flush_path=',1)[0]
old="""assert vm.pop()==BASE+0x5e4 and vm.pop()==0 and vm.pop()==source
  assert [get(vm,source+k*4) for k in range(16)]==words
  assert inverse['kind']!='unresolved';vm.steps+=2 # original CALL and thunk JMP
  if inverse['kind']=='written':
   for k,w in enumerate(inverse['words']):put(vm,BASE+0x5e4+k*4,w)
  vm.r['EAX']=BASE+0x5e4 if inverse['kind']=='written' else 0"""
new="""assert get(vm,vm.r['ESP'])==BASE+0x5e4 and get(vm,vm.r['ESP']+4)==0 and get(vm,vm.r['ESP']+8)==source
  global computed_inverse_output
  computed_inverse_output=inverse_call(vm,False)"""
assert inverse_set_source.count(old)==1;inverse_set_source=inverse_set_source.replace(old,new)
exec(compile(inverse_set_source,str(__file__)+'::computed-inverse-setter','exec'))

# Reuse the existing persistent scene/renderer loop and its original instructions.
inverse_loop_definition='renderer_original_model_handoff=model_handoff'+inverse_renderer_source.split('\nrenderer_original_model_handoff=model_handoff',1)[1].split('exec(compile(renderer_loop,str(__file__)',1)[0]
exec(compile(inverse_loop_definition,str(inverse_renderer_path)+'::loop-definition','exec'))
inverse_loop=renderer_loop
inverse_loop=inverse_loop.replace("renderer_request=copy.deepcopy(q);outer=renderer_request['mesh']", "inverse_request=copy.deepcopy(q);renderer_request=inverse_request['renderer'];outer=renderer_request['mesh']")
old="value=dict(setter=renderer_set_output,mesh=dict(transform=transformed,handoff=model,render=value))"
new="""renderer_request['inverse']=dict(kind='unchanged') if computed_inverse_output['matrix'] is None else dict(kind='written',words=computed_inverse_output['matrix'])
  value=dict(inverse=computed_inverse_output,renderer=dict(setter=renderer_set_output,mesh=dict(transform=transformed,handoff=model,render=value)))"""
assert inverse_loop.count(old)==1;inverse_loop=inverse_loop.replace(old,new)
inverse_loop=inverse_loop.replace("assert step['request']==renderer_request,", "assert step['request']==inverse_request,")
exec(compile(inverse_loop,str(__file__)+'::scalar53-render-native-loop','exec'))
tests=sum(map(int,re.findall(r'test result: ok\. (\d+) passed',(root/'analysis/reports/renderer-inverse-tests.log').read_text())));assert tests==920,tests
sources=set(json.loads((root/'analysis/reports/renderer-transform-validation.json').read_text())['source_sha256'])
sources.update(['crates/rc-package/src/lib.rs','crates/rc-package/src/renderer_inverse.rs','crates/rc-package/src/renderer_inverse_draw.rs','crates/rc-inspect/src/bin/rc-renderer-inverse-check.rs','scripts/Generate-RendererInverseCode.py','scripts/Generate-RendererInverse.py','scripts/Record-RendererInverse.py','docs/RENDERER_INVERSE.md','analysis/finalize-renderer-inverse.py'])
validation=dict(date='2026-10-10',counts=dict(counts),rust_tests=tests,debug_release_identical=True,original_core_sha256=sha(core),original_engine_sha256=sha(draw['engine']),original_d3ddrv_sha256=sha(g['dll']),input_sha256=sha(root/'analysis/reports/renderer-inverse.input.json'),report_sha256=sha(path),source_sha256={a:sha(root/a) for a in sorted(sources)},scope=report['scope'],replay_boundary=inverse_boundary,android='deferred until end per user')
(root/'analysis/reports/renderer-inverse-validation.json').write_text(json.dumps(validation,indent=2)+'\n');print(json.dumps(dict(counts=dict(counts),rust_tests=tests)))
