//! Offline scalar SSE light arithmetic; original DLL is not executed.
use std::{env,fs,convert::TryInto};
fn main()->Result<(),Box<dyn std::error::Error>> {
    let args:Vec<_>=env::args_os().collect();if args.len()!=3 {return Err("usage: probe INPUT OUTPUT".into());}
    let data=fs::read(&args[1])?;if data.len()%24!=0 {return Err("Invalid input".into());}
    let mut saved=0u32;let control=0x1f80u32;
    unsafe {core::arch::asm!("stmxcsr [{old}]","ldmxcsr [{new}]",old=in(reg)&mut saved,new=in(reg)&control,options(nostack));}
    let two=2f32;let factor=f32::from_bits(0x3c008081);let scale=f32::from_bits(0x3a800142);let gain=f32::from_bits(0x3f89999a);let tenth=f32::from_bits(0x3dcccccd);let one=1f32;
    let mut output=vec![];
    for record in data.chunks_exact(24) {
        let input:[u32;6]=std::array::from_fn(|i|u32::from_le_bytes(record[i*4..i*4+4].try_into().unwrap()));
        let mut result=[0u32;11];
        for channel in 0..3 {
            unsafe {core::arch::asm!("movss xmm0,dword ptr [{component}]","mulss xmm0,dword ptr [{two}]","mulss xmm0,dword ptr [{brightness}]","movss dword ptr [{out}],xmm0",component=in(reg)&input[channel],two=in(reg)&two,brightness=in(reg)&input[3],out=in(reg)&mut result[channel],out("xmm0") _,options(nostack));}
            let byte=((input[4]>>((2-channel)*8))&255) as i32;
            unsafe {core::arch::asm!("cvtsi2ss xmm0,{byte:e}","mulss xmm0,dword ptr [{factor}]","movss dword ptr [{out}],xmm0",byte=in(reg)byte,factor=in(reg)&factor,out=in(reg)&mut result[3+channel],out("xmm0") _,options(nostack));}
        }
        result[6]=two.to_bits();result[10]=one.to_bits();
        if input[5]!=0 {
            unsafe {core::arch::asm!("cvtsi2ss xmm2,{cone:e}","mulss xmm2,dword ptr [{scale}]","movss xmm1,dword ptr [{one}]","movaps xmm0,xmm1","subss xmm0,xmm2","mulss xmm0,xmm0","mulss xmm0,dword ptr [{gain}]","movss dword ptr [{out}],xmm0","mulss xmm0,dword ptr [{tenth}]","movaps xmm2,xmm1","divss xmm2,xmm0","movss dword ptr [{out}+4],xmm0","movss dword ptr [{out}+8],xmm2",cone=in(reg)input[5] as i32,scale=in(reg)&scale,one=in(reg)&one,gain=in(reg)&gain,tenth=in(reg)&tenth,out=in(reg)&mut result[7],out("xmm0") _,out("xmm1") _,out("xmm2") _,options(nostack));}
        }
        for word in result {output.extend_from_slice(&word.to_le_bytes());}
    }
    unsafe {core::arch::asm!("ldmxcsr [{old}]",old=in(reg)&saved,options(nostack));}
    fs::write(&args[2],output)?;Ok(())
}
