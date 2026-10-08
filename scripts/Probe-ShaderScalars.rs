//! Offline x86_64 x87 oracle for the original FPREM-equivalent remainder + FCOS.
//! Runtime Android code uses portable operations instead of this probe.
use std::{env,fs,convert::TryInto};
fn main()->Result<(),Box<dyn std::error::Error>> {
    let args:Vec<_>=env::args_os().collect();if args.len()!=3 {return Err("usage: probe INPUT OUTPUT".into());}
    let input=fs::read(&args[1])?;if input.len()%8!=0 {return Err("Invalid probe input".into());}
    let modulus=120f64;let control=0x037fu16;let mut saved=0u16;
    unsafe {core::arch::asm!("fnstcw word ptr [{old}]","fldcw word ptr [{new}]",old=in(reg)&mut saved,new=in(reg)&control,options(nostack));}
    let mut output=vec![];
    for pair in input.chunks_exact(8) {
        let time=f32::from_le_bytes(pair[..4].try_into().unwrap());let rate=f32::from_le_bytes(pair[4..].try_into().unwrap());let rate=if rate==0. {1.}else{rate};
        let mut wrapped=0f32;let mut cosine=0f32;
        unsafe {core::arch::asm!("fld qword ptr [{modulus}]","fld dword ptr [{time}]","2:","fprem","fnstsw ax","test ax,0x400","jnz 2b","fstp dword ptr [{wrapped}]","fstp st(0)","fld dword ptr [{rate}]","fmul dword ptr [{wrapped}]","fcos","fstp dword ptr [{cosine}]",modulus=in(reg)&modulus,time=in(reg)&time,wrapped=in(reg)&mut wrapped,rate=in(reg)&rate,cosine=in(reg)&mut cosine,out("ax") _,options(nostack));}
        output.extend_from_slice(&wrapped.to_bits().to_le_bytes());output.extend_from_slice(&cosine.to_bits().to_le_bytes());
    }
    unsafe {core::arch::asm!("fldcw word ptr [{old}]",old=in(reg)&saved,options(nostack));}
    fs::write(&args[2],output)?;Ok(())
}
