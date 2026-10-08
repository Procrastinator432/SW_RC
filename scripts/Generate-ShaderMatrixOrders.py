"""Derive and check each native dot-product order; also port arithmetic to an offline SSE probe."""
from pathlib import Path
import re
root=Path(__file__).resolve().parents[1]
asm=(root/'analysis/decompiled/shader-constants.asm').read_text().splitlines()
# start, end (inclusive), left memory base/register, right base, output stack base
stages=[('WORLD_SCREEN',0x1001c2bc,0x1001c7b8,('EAX',0x6c),0xac,-0x380),
        ('OBJECT_SCREEN_VIEW',0x1001c8fa,0x1001cd3b,('EAX',0x2c),0x6c,-0x300),
        ('OBJECT_SCREEN_PROJECTION',0x1001cd56,0x1001d0f2,('EBP',-0x500),0,-0x480),
        ('OBJECT_CAMERA',0x1001d721,0x1001db5d,('EAX',0x2c),0x6c,-0x5c0)]
def mem(op):
    m=re.fullmatch(r'dword ptr \[(EAX|EBP)(?: \+ (0x[0-9a-f]+))?\]',op);assert m,op
    off=int(m[2] or '0',16);off=off-2**32 if off>=2**31 else off
    return m[1],off
def flatten(tree):
    if tree[0]=='ADDSS':
        assert tree[2][0]=='MULSS',tree
        return flatten(tree[1])+[tree[2]]
    assert tree[0]=='MULSS',tree
    return [tree]
rust=['//! Generated native scalar-SSE dot-product orders. Do not replace with generic matrix multiplication.',
      '//! See scripts/Generate-ShaderMatrixOrders.py and docs/SHADER_MATRIX_COMPOSITION.md.']
probe=['//! Offline relocated scalar SSE arithmetic; no original DLL execution or live scene.',
       'use std::{env, fs, convert::TryInto};']
for name,start,end,left,right,out in stages:
    registers={};outputs={};code=[]
    def source(op):
        if op.startswith('XMM'):return registers[op]
        reg,offset=mem(op)
        if reg==left[0] and left[1]<=offset<left[1]+64:return ('a',(offset-left[1])//4)
        assert reg=='EAX' and right<=offset<right+64,op
        return ('b',(offset-right)//4)
    def relocate(op):
        if op.startswith('XMM'):return op.lower()
        reg,offset=mem(op)
        if reg=='EAX':return f'dword ptr [r8 + {offset}]'
        return f'dword ptr [r9 + {offset+0x600}]'
    for line in asm:
        if not line or line.startswith('#'):continue
        address,instruction=line.split(' ',1);address=int(address,16)
        if not start<=address<=end:continue
        opcode,_,args=instruction.partition(' ')
        if opcode not in ('MOVSS','MOVAPS','MULSS','ADDSS'):
            assert opcode in ('ADD','MOV','LEA'),instruction
            continue
        dst,src=args.split(',',1)
        code.append(f'"{opcode.lower()} {relocate(dst)}, {relocate(src)}",')
        if dst.startswith('XMM'):
            value=source(src)
            registers[dst]=(opcode,registers[dst],value) if opcode in ('MULSS','ADDSS') else value
        else:
            reg,offset=mem(dst);assert reg=='EBP';outputs[(offset-out)//4]=registers[src]
    assert sorted(outputs)==list(range(16)),name
    orders=[]
    for index in range(16):
        row,col=divmod(index,4);products=flatten(outputs[index]);order=[]
        for _,x,y in products:
            if x[0]=='b':x,y=y,x
            assert x[0]=='a' and y[0]=='b' and x[1]//4==row and y[1]%4==col
            k=x[1]%4;assert y[1]//4==k;order.append(k)
        assert sorted(order)==[0,1,2,3];orders.append(order)
    rust.append(f'pub(super) const {name}: [[usize; 4]; 16] = [')
    rust.extend('    ['+', '.join(map(str,order))+'],' for order in orders);rust.append('];')
    probe.extend([f'unsafe fn {name.lower()}(state: &[u32; 60], scratch: &mut [u32; 384]) {{',
                  '    unsafe { core::arch::asm!(',*('        '+line for line in code),
                  '        in("r8") state.as_ptr(), in("r9") scratch.as_mut_ptr(),',
                  *(f'        out("xmm{i}") _,' for i in range(8)),
                  '        options(nostack)', '    ); }','}'])
probe.extend(['fn main() -> Result<(), Box<dyn std::error::Error>> {',
 '    let args: Vec<_> = env::args_os().collect(); if args.len()!=3 {return Err("usage: probe INPUT OUTPUT".into());}',
 '    let input=fs::read(&args[1])?; if input.len()%192!=0 {return Err("Invalid matrices".into());}',
 '    let mut saved=0u32; let control=0x1f80u32;',
 '    unsafe {core::arch::asm!("stmxcsr [{old}]", "ldmxcsr [{new}]", old=in(reg)&mut saved, new=in(reg)&control, options(nostack));}',
 '    let mut output=vec![];',
 '    for record in input.chunks_exact(192) {',
 '        let mut state=[0u32;60]; for (i,word) in record.chunks_exact(4).enumerate() {state[11+i]=u32::from_le_bytes(word.try_into().unwrap());}',
 '        let mut scratch=[0u32;384];',
 '        unsafe { world_screen(&state,&mut scratch); }',
 '        let world: [u32;16]=scratch[160..176].try_into().unwrap();',
 '        unsafe { object_screen_view(&state,&mut scratch); }',
 '        let view: [u32;16]=scratch[192..208].try_into().unwrap(); scratch[64..80].copy_from_slice(&view);',
 '        let mut projection=[0u32;60]; projection[..16].copy_from_slice(&state[43..59]);',
 '        unsafe { object_screen_projection(&projection,&mut scratch); }',
 '        let screen: [u32;16]=scratch[96..112].try_into().unwrap();',
 '        unsafe { object_camera(&state,&mut scratch); }',
 '        let camera: [u32;16]=scratch[16..32].try_into().unwrap();',
 '        for matrix in [world,screen,camera] { for col in 0..4 { for row in 0..4 {output.extend_from_slice(&matrix[row*4+col].to_le_bytes());} } }',
 '    }',
 '    unsafe {core::arch::asm!("ldmxcsr [{old}]", old=in(reg)&saved, options(nostack));}',
 '    fs::write(&args[2],output)?; Ok(())','}'])
for path,lines in [('crates/rc-package/src/shader_matrix_orders.rs',rust),('scripts/Probe-ShaderMatrices.rs',probe)]:
    with (root/path).open('w',encoding='utf-8',newline='\n') as file:file.write('\n'.join(lines)+'\n')
print('Verified 64 dot products across four native stages; generated orders and offline SSE arithmetic.')
