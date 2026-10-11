"""Translate reviewed scalar x87 stack/dataflow to portable 53-bit arithmetic.
Every arithmetic instruction rounds to binary64; explicit f32 spills are retained.
No claim for x87 precision24/64, non-nearest modes or optimized dispatch variants.
"""
from pathlib import Path
import re
root=Path(__file__).resolve().parents[1]
stack=[];memory={};registers={};serial=0;output={};lines=[]
def bind(expression,address,kind='v'):
 global serial
 name=f'{kind}{serial}';serial+=1;lines.append(f'    let {name} = {expression}; // {address}')
 return name
def mem(arg):
 if 'EAX' in arg:
  assert arg.startswith(('float ptr [EAX','dword ptr [EAX')),arg
  offset=re.search(r'\+ (0x[0-9a-f]+)',arg);return f'input[{int(offset[1],16)//4 if offset else 0}]'
 key=arg[arg.index('['):];assert key in memory,(arg,key);return memory[key]
def operand(arg,address):
 if arg.startswith('ST'):return stack[int(arg[2:])]
 assert arg.startswith('float ptr'),arg
 if address=='100319f6':return f'f64::from(f32::from_bits({mem(arg)}))'
 return bind(f'load({mem(arg)})?',address)
for line in (root/'analysis/decompiled/renderer-inverse-scalar.asm').read_text().splitlines():
 if not line or line.startswith('#'):continue
 address,op,*tail=line.split(' ',2);args=tail[0].split(',') if tail else []
 pc=int(address,16)
 if op=='MOV':
  dst,src=args
  if dst in ['ECX','EAX'] and src.startswith('dword ptr') and ('[EAX' in src or src[src.index('['):] in memory):registers[dst]=mem(src)
  elif dst.startswith('dword ptr [EBP') and src in registers:memory[dst[dst.index('['):]]=registers[src]
 elif op=='FLD':stack.insert(0,operand(args[0],address))
 elif op=='FLD1':stack.insert(0,bind('1.0f64',address))
 elif op in ['FMUL','FDIV']:
  if len(args)==2:
   assert args==['ST0','ST1'];expression=f'{stack[0]} / {stack[1]}'
  else:expression=f'{stack[0]} * {operand(args[0],address)}'
  stack[0]=bind(expression,address)
 elif op in ['FADDP','FSUBP']:
  assert not args
  value=bind(f'{stack[1]} '+('+' if op=='FADDP' else '-')+f' {stack[0]}',address);stack= [value]+stack[2:]
 elif op in ['FST','FSTP']:
  arg=args[0]
  if arg.startswith('ST'):stack[int(arg[2:])]=stack[0]
  elif arg.startswith('double ptr'):
   assert pc==0x100319f9 # exact conversion of the previously stored f32 reciprocal
  elif arg.startswith('float ptr'):
   name=bind(f'({stack[0]} as f32).to_bits()',address,'w')
   if arg.startswith('float ptr [EAX'):
    if pc==0x100319e9:lines.append(f'    let determinant = store_determinant.then_some({name});')
    else:
     off=re.search(r'\+ (0x[0-9a-f]+)',arg);output[int(off[1],16)//4 if off else 0]=name
   else:memory[arg[arg.index('['):]]=name
   if pc==0x100319f1:lines.append(f'    let reciprocal = {name};')
  else:raise AssertionError(arg)
  if op=='FSTP':stack.pop(0)
 elif pc==0x10031a05:
  lines+=['    if !f32::from_bits(reciprocal).is_finite() {', '        return Ok(Output {determinant, reciprocal, matrix:None});','    }']
assert set(output)==set(range(16)) and not stack
header='''//! Scalar D3D inverse 10031709 under nearest x87 53-bit precision.
//! Generated from the reviewed original stack/dataflow; f32 spills remain explicit.
use serde::{Deserialize, Serialize};
#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct Input { pub matrix:[u32;16], pub store_determinant:bool }
#[derive(Clone, Debug, Serialize)]
pub struct Output { pub determinant:Option<u32>, pub reciprocal:u32, pub matrix:Option<[u32;16]> }
fn load(word:u32)->Result<f64,String> {
    let value=f32::from_bits(word);
    if value.is_finite() { Ok(f64::from(value)) } else {Err("Nonfinite scalar inverse input/spill".into())}
}
/// This models the scalar fallback with precision-control=53, rounding=nearest.
/// It does not select a native CPU variant. Finite f32 inputs/spills keep every
/// extended intermediate within binary64's exponent range; arithmetic therefore
/// has the same significand rounding. The rounded reciprocal alone gates writes.
pub fn scalar53(input:[u32;16],store_determinant:bool)->Result<Output,String> {
    for word in input {load(word)?;}
'''
tail='    Ok(Output {determinant, reciprocal, matrix:Some(['+', '.join(output[i] for i in range(16))+'])})\n}\n'
(root/'crates/rc-package/src/renderer_inverse.rs').write_text(header+'\n'.join(lines)+'\n'+tail)
print(f'{serial} original arithmetic/load/store assignments translated')
