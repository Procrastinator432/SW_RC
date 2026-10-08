//! Offline x87 FSIN/FPTAN/FCOS with native multiplication/store order.
//! Never loads or executes the original game DLLs.
use std::{convert::TryInto, env, fs};
fn main() -> Result<(), Box<dyn std::error::Error>> {
    let args: Vec<_> = env::args_os().collect();
    if args.len() != 3 { return Err("Expected binary input and output paths".into()); }
    let input = fs::read(&args[1])?;
    if input.len()%8 != 0 { return Err("Invalid input length".into()); }
    let mut saved = 0u16;
    let control = 0x037fu16;
    let modulus = 120f64;
    let radius = 500f32;
    let mut output = Vec::new();
    unsafe { core::arch::asm!("fnstcw word ptr [{old}]", "fldcw word ptr [{new}]", old=in(reg)&mut saved, new=in(reg)&control, options(nostack)); }
    for pair in input.chunks_exact(8) {
        let time = f32::from_le_bytes(pair[..4].try_into()?);
        let frequency = f32::from_le_bytes(pair[4..].try_into()?);
        let frequency = if frequency == 0. { 1. } else { frequency };
        let mut wrapped = 0f32;
        let mut sine = 0f32;
        let mut tangent = 0f32;
        let mut x = 0f32;
        let mut y = 0f32;
        unsafe { core::arch::asm!(
            "fld qword ptr [{modulus}]", "fld dword ptr [{time}]",
            "2:", "fprem", "fnstsw ax", "test ax,0x400", "jnz 2b",
            "fstp dword ptr [{wrapped}]", "fstp st(0)",
            "fld dword ptr [{frequency}]", "fmul dword ptr [{wrapped}]", "fsin", "fstp dword ptr [{sine}]",
            "fld dword ptr [{frequency}]", "fmul dword ptr [{wrapped}]", "fptan", "fstp st(0)", "fstp dword ptr [{tangent}]",
            "fld dword ptr [{wrapped}]", "fcos", "fmul dword ptr [{radius}]", "fstp dword ptr [{x}]",
            "fld dword ptr [{wrapped}]", "fsin", "fmul dword ptr [{radius}]", "fstp dword ptr [{y}]",
            modulus=in(reg)&modulus, time=in(reg)&time, wrapped=in(reg)&mut wrapped,
            frequency=in(reg)&frequency, sine=in(reg)&mut sine, tangent=in(reg)&mut tangent,
            radius=in(reg)&radius, x=in(reg)&mut x, y=in(reg)&mut y,
            out("ax") _, options(nostack)
        ); }
        for v in [wrapped,sine,tangent,x,y].iter() { output.extend_from_slice(&v.to_le_bytes()); }
    }
    unsafe { core::arch::asm!("fldcw word ptr [{old}]",old=in(reg)&saved,options(nostack)); }
    fs::write(&args[2],output)?;
    Ok(())
}
