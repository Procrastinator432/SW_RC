//! Offline x86_64 reproduction of Core FGlobalMath table initializer.
//! Android/runtime code only reads the resulting portable little-endian words.
use std::{env, fs};
fn main() -> Result<(), Box<dyn std::error::Error>> {
    let args: Vec<_> = env::args().collect();
    if args.len() != 3 {
        return Err("usage: generator CORE_DLL OUTPUT".into());
    }
    let pe = fs::read(&args[1])?;
    let u16at = |i| u16::from_le_bytes(pe[i..i + 2].try_into().unwrap());
    let u32at = |i| u32::from_le_bytes(pe[i..i + 4].try_into().unwrap());
    let header = u32at(60) as usize;
    let opt = u16at(header + 20) as usize;
    let base = u32at(header + 24 + 28);
    let rva = 0x101885d4 - base;
    let mut step = None;
    for i in 0..u16at(header + 6) as usize {
        let s = header + 24 + opt + 40 * i;
        let size = u32at(s + 8).max(u32at(s + 16));
        let va = u32at(s + 12);
        if (va..va + size).contains(&rva) {
            step = Some(f32::from_bits(u32at((u32at(s + 20) + rva - va) as usize)));
        }
    }
    let step = step.ok_or("native sine-table step not mapped")?;
    let mut output = Vec::with_capacity(65536);
    let mut old_control = 0u16;
    let control = 0x037fu16;
    unsafe {
        core::arch::asm!("fnstcw word ptr [{old}]", "fldcw word ptr [{new}]", old=in(reg)&mut old_control,new=in(reg)&control,options(nostack));
    }
    for index in 0i32..16384 {
        let mut value = 0f32;
        unsafe {
            core::arch::asm!("fild dword ptr [{index}]","fmul dword ptr [{step}]","fsin","fstp dword ptr [{out}]",index=in(reg)&index,step=in(reg)&step,out=in(reg)&mut value,options(nostack));
        }
        output.extend_from_slice(&value.to_bits().to_le_bytes());
    }
    unsafe {
        core::arch::asm!("fldcw word ptr [{old}]",old=in(reg)&old_control,options(nostack));
    }
    fs::write(&args[2], output)?;
    println!("16384 entries; original step bits {:08x}", step.to_bits());
    Ok(())
}
