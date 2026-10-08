//! Offline SSE reproduction of the original light-position arithmetic order.
use std::{env,fs,convert::TryInto};
fn main()->Result<(),Box<dyn std::error::Error>> {
 let a:Vec<_>=env::args_os().collect();if a.len()!=3{return Err("usage: probe INPUT OUTPUT".into());}
 let data=fs::read(&a[1])?;if data.len()%104!=0{return Err("Invalid input".into());}
 let mut saved=0u32;let control=0x1f80u32;unsafe{core::arch::asm!("stmxcsr [{old}]","ldmxcsr [{new}]",old=in(reg)&mut saved,new=in(reg)&control,options(nostack));}
 let distance=65365f32;let mut output=vec![];
 for record in data.chunks_exact(104) {
  let input:[u32;26]=std::array::from_fn(|i|u32::from_le_bytes(record[i*4..i*4+4].try_into().unwrap()));let mut point:[u32;3]=input[1..4].try_into().unwrap();let mut result=[0u32;4];
  if input[0]==0x13 {for k in 0..3 {unsafe{core::arch::asm!("movss xmm0,dword ptr [{dir}]","mulss xmm0,dword ptr [{distance}]","movss xmm1,dword ptr [{translation}]","subss xmm1,xmm0","movss dword ptr [{out}],xmm1",dir=in(reg)&input[4+k],distance=in(reg)&distance,translation=in(reg)&input[7+k],out=in(reg)&mut point[k],out("xmm0") _,out("xmm1") _,options(nostack));}}}
  for col in 0..3 {
   let order=if col==0 && input[0]!=0x13 {[2,1,0]}else{[0,2,1]};let m=std::array::from_fn::<_,3,_>(|i|input[10+order[i]*4+col]);let p=order.map(|i|point[i]);
   unsafe{core::arch::asm!("movss xmm0,dword ptr [{p}]","mulss xmm0,dword ptr [{m}]","movss xmm1,dword ptr [{p}+4]","mulss xmm1,dword ptr [{m}+4]","addss xmm0,xmm1","movss xmm1,dword ptr [{p}+8]","mulss xmm1,dword ptr [{m}+8]","addss xmm0,xmm1","addss xmm0,dword ptr [{translation}]","movss dword ptr [{out}],xmm0",p=in(reg)p.as_ptr(),m=in(reg)m.as_ptr(),translation=in(reg)&input[22+col],out=in(reg)&mut result[col],out("xmm0") _,out("xmm1") _,options(nostack));}
  }
  result[3]=1f32.to_bits();for word in point.iter().chain(&result){output.extend_from_slice(&word.to_le_bytes());}
 }
 unsafe{core::arch::asm!("ldmxcsr [{old}]",old=in(reg)&saved,options(nostack));}fs::write(&a[2],output)?;Ok(())
}
