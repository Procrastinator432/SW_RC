//! Offline original f32 product and x87 two-register rotation output.
use std::{convert::TryInto,env,fs};
fn main()->Result<(),Box<dyn std::error::Error>> {
    let args:Vec<_>=env::args_os().collect();if args.len()!=3 {return Err("Expected input and output".into());}
    let input=fs::read(&args[1])?;if input.len()%8!=0 {return Err("Invalid input".into());}
    let modulus=120f64;let control=0x037fu16;let mut saved=0u16;let mut mxcsr=0u32;let nearest=0x1f80u32;
    unsafe {core::arch::asm!("fnstcw word ptr [{saved}]","fldcw word ptr [{control}]","stmxcsr dword ptr [{mxcsr}]","ldmxcsr dword ptr [{nearest}]",saved=in(reg)&mut saved,control=in(reg)&control,mxcsr=in(reg)&mut mxcsr,nearest=in(reg)&nearest,options(nostack));}
    let mut output=Vec::new();
    for pair in input.chunks_exact(8) {
        let time=f32::from_le_bytes(pair[..4].try_into()?);let rate=f32::from_le_bytes(pair[4..].try_into()?);
        let mut wrapped=0f32;let mut argument=0f32;let mut cosine=0f32;let mut sine=0f32;let mut negative=0f32;
        unsafe {core::arch::asm!(
            "fld qword ptr [{modulus}]","fld dword ptr [{time}]","2:","fprem","fnstsw ax","test ax,0x400","jnz 2b","fstp dword ptr [{wrapped}]","fstp st(0)",
            "movss xmm0,dword ptr [{rate}]","mulss xmm0,dword ptr [{wrapped}]","movss dword ptr [{argument}],xmm0",
            "fld dword ptr [{argument}]","fcos","fld dword ptr [{argument}]","fsin","fld st(1)","fstp dword ptr [{cosine}]","fld st(0)","fchs","fstp dword ptr [{negative}]","fstp dword ptr [{sine}]","fstp st(0)",
            modulus=in(reg)&modulus,time=in(reg)&time,rate=in(reg)&rate,wrapped=in(reg)&mut wrapped,argument=in(reg)&mut argument,cosine=in(reg)&mut cosine,sine=in(reg)&mut sine,negative=in(reg)&mut negative,out("ax") _,out("xmm0") _,options(nostack));}
        for word in [argument.to_bits(),cosine.to_bits(),negative.to_bits(),0,0,sine.to_bits(),cosine.to_bits(),0,0].iter() {output.extend_from_slice(&word.to_le_bytes());}
    }
    unsafe {core::arch::asm!("fldcw word ptr [{saved}]","ldmxcsr dword ptr [{mxcsr}]",saved=in(reg)&saved,mxcsr=in(reg)&mxcsr,options(nostack));}
    fs::write(&args[2],output)?;Ok(())
}
