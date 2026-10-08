"""Lift the rigid-section inverse-reference * pose product in Render."""
from pathlib import Path
import re
root=Path(__file__).resolve().parents[1]
registers={}; output={}
def offset(op):
    values=re.findall(r'-?0x[0-9a-f]+',op)
    n=int(values[-1],16) if values else 0
    return n-0x100000000 if n>=0x80000000 else n
def operand(op):
    if op.startswith('XMM'):return registers[op]
    assert 'EAX' in op or 'ECX' in op,op
    return ('pose' if 'EAX' in op else 'inverse')+f'[{offset(op)//4}]'
for line in (root/'analysis/decompiled/skeletal-render.asm').read_text().splitlines():
    if not line or line.startswith('#'):continue
    addr,ins=line.split(' ',1)
    if not 0x10510647<=int(addr,16)<=0x10510a12:continue
    opcode,_,args=ins.partition(' ')
    if opcode not in ('MOVSS','MOVAPS','ADDSS','MULSS'):continue
    dst,src=args.split(',',1)
    # Registers can load actor-transform locals before this block ends; they
    # do not contribute to the inverse/pose stores and are not part of the product.
    if 'EBP' in src:continue
    expression=operand(src)
    if dst.startswith('XMM'):
        registers[dst]=expression if opcode in ('MOVSS','MOVAPS') else f'({registers[dst]} '+('+' if opcode=='ADDSS' else '*')+f' {expression})'
    else:
        assert 'EBP' in dst and opcode=='MOVSS'
        output[(offset(dst)+0x1d8)//4]=expression
assert set(output)==set(range(16))
lines=['//! Native rigid Render product, 10510647..10510a12; generated, before actor transform.',
       'pub fn rigid_render_product(inverse: [u32;16], pose: [u32;16]) -> [u32;16] {',
       'let inverse=inverse.map(f32::from_bits);','let pose=pose.map(f32::from_bits);','[']
lines += [output[i]+',' for i in range(16)]+['].map(f32::to_bits)','}']
(root/'crates/rc-package/src/skeletal_render_product.rs').write_text('\n'.join(lines)+'\n',encoding='utf-8')
print('16 rigid render product expressions generated')
