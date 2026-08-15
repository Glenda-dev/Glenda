use core::slice;

use crate::irq::TrapContext;
use crate::mem::PageTable;
use crate::mem::uvm;
use crate::printk;
use crate::printk::{ANSI_RESET, ANSI_YELLOW};
use crate::proc::current_proc;

pub fn sys_copyout(ctx: &mut TrapContext) -> usize {
    let u_dst = ctx.a0;
    let buf: [u32; 5] = [1, 2, 3, 4, 5];
    let bytes = unsafe {
        slice::from_raw_parts(buf.as_ptr() as *const u8, core::mem::size_of::<u32>() * buf.len())
    };
    let p = current_proc();
    let pt = unsafe { &*(p.root_pt_pa as *const PageTable) };
    match uvm::copyout(pt, u_dst, bytes) {
        Ok(()) => 0,
        Err(e) => {
            printk!("{}[WARN] sys_copyout failed: {:?}{}\n", ANSI_YELLOW, e, ANSI_RESET);
            usize::MAX
        }
    }
}

pub fn sys_copyin(ctx: &mut TrapContext) -> usize {
    let u_src = ctx.a0;
    let n = ctx.a1;
    let mut tmp: [u32; 32] = [0; 32];
    let count = core::cmp::min(n, tmp.len());
    let dst_bytes = unsafe {
        slice::from_raw_parts_mut(tmp.as_mut_ptr() as *mut u8, count * core::mem::size_of::<u32>())
    };
    let p = current_proc();
    let pt = unsafe { &*(p.root_pt_pa as *const PageTable) };
    match uvm::copyin(pt, dst_bytes, u_src) {
        Ok(()) => {
            for i in 0..count {
                printk!("copyin[{}] = {}\n", i, tmp[i]);
            }
            0
        }
        Err(e) => {
            printk!("{}[WARN] sys_copyin failed: {:?}{}\n", ANSI_YELLOW, e, ANSI_RESET);
            usize::MAX
        }
    }
}

pub fn sys_copyinstr(ctx: &mut TrapContext) -> usize {
    let u_src = ctx.a0;
    let mut buf: [u8; 256] = [0; 256];
    let p = current_proc();
    let pt = unsafe { &*(p.root_pt_pa as *const PageTable) };
    match uvm::copyin_str(pt, &mut buf, u_src) {
        Ok(len) => {
            let s = &buf[..len.saturating_sub(1)];
            match core::str::from_utf8(s) {
                Ok(text) => printk!("copyinstr: {}\n", text),
                Err(_) => printk!("copyinstr: <non-utf8> len={} bytes\n", len),
            }
            0
        }
        Err(e) => {
            printk!(
                "{}[WARN] sys_copyinstr failed: {:?}, u_src=0x{:x}{}\n",
                ANSI_YELLOW,
                e,
                u_src,
                ANSI_RESET
            );
            usize::MAX
        }
    }
}
