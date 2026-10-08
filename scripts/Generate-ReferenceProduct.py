"""Generate native reference-pose parent product, engine 1050a070..1050a408."""
from pathlib import Path
import re
root=Path(__file__).resolve().parents[1];registers={};output={}
def offset(op):
    words=re.findall(r'-?0x[0-9a-f]+',op);n=int(words[-1],16) if words else 0
    return n-0x100000000 if n>=0x80000000 else n
def operand(op):
    if op.startswith('XMM'):return registers[op]
    assert 'EBX' in op or 'EAX' in op,op
    return ('m' if 'EBX' in op else 'p')+f'[{offset(op)//4}]'
for line in (root/'analysis/decompiled/skeletal-root-frame.asm').read_text().splitlines():
    if not line or line.startswith('#'):continue
    address,instruction=line.split(' ',1)
    if not 0x1050a070<=int(address,16)<=0x1050a408:continue
    opcode,_,args=instruction.partition(' ')
    if opcode not in ('MOVSS','MOVAPS','ADDSS','MULSS'):continue
    dst,src=args.split(',',1)
    if dst.startswith('XMM'):
        expr=operand(src);registers[dst]=expr if opcode in ('MOVSS','MOVAPS') else f'({registers[dst]} '+('+' if opcode=='ADDSS' else '*')+f' {expr})'
    else:assert opcode=='MOVSS' and 'EBP' in dst;output[(offset(dst)+0x84)//4]=operand(src)
assert set(output)==set(range(16))
lines=['//! Generated from original reference-parent scalar SSE expressions.','pub fn compose_reference_matrix(local:[u32;16],parent:[u32;16])->[u32;16] {','    let m=local.map(f32::from_bits);','    let p=parent.map(f32::from_bits);','    [']+[f'        {output[i]},' for i in range(16)]+['    ].map(f32::to_bits)','}']
(root/'crates/rc-package/src/skeletal_reference_product.rs').write_text('\n'.join(lines)+'\n',encoding='utf-8');print('16 native reference-parent outputs translated')
