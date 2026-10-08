"""Lift scalar SSE arithmetic in the five remaining ComputeSkinVerts kernels."""
from pathlib import Path
import re
root = Path(__file__).resolve().parents[1]
asm = (root/'analysis/decompiled/skeletal-skin-vertices.asm').read_text().splitlines()
def offset(op):
    a = re.findall(r'-?0x[0-9a-f]+',op)
    n = int(a[-1],16) if a else 0
    return n-0x100000000 if n>=0x80000000 else n

def lift(name,start,end,matrices,conversions,initial,outputs,acc=False):
    regs = dict(initial)
    stack = {-0x20:'n[0]',-0x1c:'n[1]',-0x18:'n[2]'}
    if acc: stack.update({-0x3c:'previous[4]',-0x38:'previous[5]'})
    result = {}
    def operand(op):
        if op.startswith('XMM'): return regs[op]
        if 'EBP' in op: return stack[offset(op)]
        if 'ECX' in op: return f'p[{offset(op)//4}]'
        for reg,matrix in matrices.items():
            if reg in op: return f'{matrix}[{offset(op)//4}]'
        raise ValueError(op)
    for line in asm:
        if not line or line.startswith('#'): continue
        addr,ins = line.split(' ',1); addr=int(addr,16)
        if not start<=addr<=end: continue
        opcode,_,args = ins.partition(' ')
        if opcode == 'CVTSI2SS':
            dst,_=args.split(',',1); regs[dst]=conversions[addr]; continue
        if opcode not in ('MOVSS','MOVAPS','ADDSS','MULSS'): continue
        dst,src=args.split(',',1); expr=operand(src)
        if dst.startswith('XMM'):
            regs[dst]=expr if opcode in ('MOVSS','MOVAPS') else f'({regs[dst]} '+('+' if opcode=='ADDSS' else '*')+f' {expr})'
        elif 'EBP' in dst: stack[offset(dst)]=expr
        elif 'EBX' in dst: result[offset(dst)//4]=expr
        else: assert 'EDX' in dst # duplicated cache stores; performed by stream caller
    if outputs == 'stack': result={i:stack[k] for i,k in enumerate([-0x54,-0x50,-0x4c,-0x40,-0x3c,-0x38])}
    assert set(result)==set(range(6)),name
    matrix_args = ', '.join(m+': [f32;16]' for m in dict.fromkeys(matrices.values()))
    weight_args = '' if name=='cached_rigid' else ', w: [f32;2]' if 'two' in name else ', w: f32'
    prev_args = ', previous: [f32;6]' if acc else ''
    lines=[f'pub(super) fn {name}(p: [f32;3], n: [f32;3], {matrix_args}{weight_args}{prev_args}) -> [f32;6] {{']
    if name!='cached_rigid': lines.append('    let scale = f32::from_bits(0x31800080);')
    lines+=['    [']+[f'        {result[i]},' for i in range(6)]+['    ]','}']
    return '\n'.join(lines)

functions=[lift('weighted_two',0x1050c518,0x1050c7de,{'ESI':'m0','EAX':'m1'},
    {0x1050c549:'n[0]',0x1050c575:'n[1]',0x1050c581:'n[2]'},
    {'XMM3':'w[0]','XMM4':'w[1]','XMM6':'scale'},'output'),
lift('cached_rigid',0x1050c813,0x1050c99b,{'EAX':'m'},
    {0x1050c829:'n[0]',0x1050c852:'n[1]',0x1050c85b:'n[2]'}, {},'output'),
lift('cached_two',0x1050c9f6,0x1050cce7,{'ESI':'m0','EAX':'m1'},
    {0x1050ca14:'w[1]',0x1050ca1b:'w[0]',0x1050ca41:'n[0]',0x1050ca6a:'n[1]',0x1050ca73:'n[2]'},
    {'XMM6':'scale'},'output'),
lift('weighted_first',0x1050cd59,0x1050cee9,{'EAX':'m'},
    {0x1050cd67:'n[0]',0x1050cd98:'n[1]',0x1050cda4:'n[2]'},
    {'XMM0':'w','XMM6':'scale'},'stack'),
lift('weighted_next',0x1050cf00,0x1050d074,{'EAX':'m'},
    {0x1050cf30:'w'}, {'XMM1':'previous[0]','XMM2':'previous[1]',
    'XMM3':'previous[2]','XMM4':'previous[3]','XMM5':'n[2]','XMM6':'scale'},'stack',True)]
(root/'crates/rc-package/src/skeletal_weighted_kernels.rs').write_text(
    '//! Generated scalar SSE order from ComputeSkinVerts; see Generate-WeightedSkin.py.\n'+ '\n'.join(functions)+'\n',encoding='utf-8')
print('five native weighted/cache kernels generated')
