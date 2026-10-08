"""Generate untransposed skin product from original scalar SSE expressions."""
from pathlib import Path
import re
root = Path(__file__).resolve().parents[1]
registers, output = {}, {}
def offset(op):
    values = re.findall(r'-?0x[0-9a-f]+', op)
    n = int(values[-1], 16) if values else 0
    return n - 0x100000000 if n >= 0x80000000 else n
def operand(op):
    if op.startswith('XMM'): return registers[op]
    assert 'EAX' in op or 'ECX' in op, op
    return ('pose' if 'EAX' in op else 'inverse') + f'[{offset(op)//4}]'
for line in (root/'analysis/decompiled/skeletal-skin-vertices.asm').read_text().splitlines():
    if not line or line.startswith('#'): continue
    address, instruction = line.split(' ',1)
    if not 0x1050bd96 <= int(address,16) <= 0x1050c14f: continue
    opcode, _, args = instruction.partition(' ')
    if opcode not in ('MOVSS','MOVAPS','ADDSS','MULSS'): continue
    dst, src = args.split(',',1)
    if dst.startswith('XMM'):
        expr = operand(src)
        registers[dst] = expr if opcode in ('MOVSS','MOVAPS') else f'({registers[dst]} '+('+' if opcode=='ADDSS' else '*')+f' {expr})'
    else:
        assert opcode=='MOVSS' and 'EBP' in dst
        output[(offset(dst)+0xd8)//4] = operand(src)
assert set(output)==set(range(16))
lines = ['//! Generated from ComputeSkinVerts 1050bd96..1050c14f, before transpose.',
         'pub fn skin_product(inverse: [u32;16], pose: [u32;16]) -> [u32;16] {',
         '    let inverse = inverse.map(f32::from_bits);', '    let pose = pose.map(f32::from_bits);', '    [']
lines += [f'        {output[i]},' for i in range(16)] + ['    ].map(f32::to_bits)', '}']
(root/'crates/rc-package/src/skeletal_skin_product.rs').write_text('\n'.join(lines)+'\n',encoding='utf-8')
print('16 native skin product expressions generated')
