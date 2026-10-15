"""Derive GetMoveCoords(relative=true) scalar SSE product orders from original ASM."""
from pathlib import Path
import json
import re

ROOT = Path(__file__).resolve().parents[1]
ASM = ROOT / 'analysis/mission13-rappel/native-root-motion.asm'
registers = {}
outputs = {}
code = []


def memory(op):
    m = re.fullmatch(r'dword ptr \[(EBX|EBP|ESP)(?: \+ (0x[0-9a-f]+))?\]', op)
    assert m, op
    return m[1], int(m[2] or '0', 16)


def source(op):
    if op.startswith('XMM'):
        return registers[op]
    reg, offset = memory(op)
    if reg == 'EBX':
        assert 0 <= offset < 64
        return ('a', offset // 4)
    assert reg == 'EBP' and 0x128 <= offset < 0x168, op
    return ('b', (offset - 0x128) // 4)


def terms(tree):
    if tree[0] == 'ADDSS':
        return terms(tree[1]) + terms(tree[2])
    assert tree[0] == 'MULSS', tree
    return [tree]


def relocated(op):
    if op.startswith('XMM'):
        return op.lower()
    reg, offset = memory(op)
    base, offset = {'EBX': ('r8', offset), 'EBP': ('r9', offset - 0x128),
                    'ESP': ('r10', offset - 0x20)}[reg]
    return f'dword ptr [{base} + {offset}]'


for line in ASM.read_text().splitlines():
    if not line or line.startswith('#'):
        continue
    address, instruction = line.split(' ', 1)
    if not 0x10500b0e <= int(address, 16) <= 0x10500f6f:
        continue
    opcode, _, args = instruction.partition(' ')
    if opcode not in ('MOVSS', 'MULSS', 'ADDSS'):
        continue
    dst, src = args.split(',', 1)
    code.append(f'        "{opcode.lower()} {relocated(dst)}, {relocated(src)}",')
    if dst.startswith('XMM'):
        value = source(src)
        registers[dst] = (opcode, registers[dst], value) if opcode != 'MOVSS' else value
    else:
        reg, offset = memory(dst)
        assert reg == 'ESP' and 0x20 <= offset < 0x60
        outputs[(offset - 0x20) // 4] = registers[src]
assert sorted(outputs) == list(range(16))
orders = []
for index in range(16):
    row, col = divmod(index, 4)
    order = []
    for _, left, right in terms(outputs[index]):
        pair = {left[0]: left[1], right[0]: right[1]}
        assert set(pair) == {'a', 'b'}
        k = pair['a'] % 4
        assert pair['a'] // 4 == row and pair['b'] == k * 4 + col
        order.append(k)
    assert sorted(order) == list(range(4))
    orders.append(order)
rust = ['//! Generated from GetMoveCoords 10500b0e..10500f6f; keep scalar SSE order.',
        'pub(super) const RELATIVE_ORDER: [[usize; 4]; 16] = [']
rust += ['    [' + ', '.join(map(str, order)) + '],' for order in orders]
rust.append('];')
(ROOT / 'crates/rc-package/src/root_motion_orders.rs').write_text('\n'.join(rust) + '\n')
(ROOT / 'analysis/mission13-rappel/relative-orders.json').write_text(json.dumps({'function': 'USkeletalMeshInstance.GetMoveCoords', 'relative_order': orders}, indent=2) + '\n')
probe = '''// Offline relocated original scalar SSE; no DLL calls or game execution.
use std::{env,fs};
unsafe fn native(a:&[u32;16],b:&[u32;16])->[u32;16] {
    let mut out=[0u32;16];
    core::arch::asm!(
''' + '\n'.join(code) + '''
        in("r8") a.as_ptr(), in("r9") b.as_ptr(), in("r10") out.as_mut_ptr(),
        out("xmm0") _, out("xmm1") _, options(nostack)
    );
    out
}
fn main() {
    let a:Vec<_>=env::args_os().collect();
    let input=fs::read(&a[1]).unwrap(); assert_eq!(input.len()%128,0);
    let mut result=vec![];
    for row in input.chunks_exact(128) {
        let left=std::array::from_fn(|i|u32::from_le_bytes(row[i*4..i*4+4].try_into().unwrap()));
        let right=std::array::from_fn(|i|u32::from_le_bytes(row[64+i*4..68+i*4].try_into().unwrap()));
        for value in unsafe { native(&left,&right) } { result.extend(value.to_le_bytes()); }
    }
    fs::write(&a[2],result).unwrap();
}
'''
(ROOT / 'analysis/mission13-rappel/root-motion-sse.rs').write_text(probe)
print('16 native scalar SSE product orders derived')
