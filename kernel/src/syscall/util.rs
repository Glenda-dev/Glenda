use crate::irq::TrapContext;
use crate::irq::timer;
use crate::mem::PageTable;
use crate::mem::uvm;
use crate::printk;
use crate::proc::current_proc;

pub fn sys_print_str(ctx: &mut TrapContext) -> usize {
    let u_src = ctx.a0;
    let mut buf: [u8; 256] = [0; 256];
    let p = current_proc();
    let pt = unsafe { &*(p.root_pt_pa as *const PageTable) };
    match uvm::copyin_str(pt, &mut buf, u_src) {
        Ok(len) => {
            let s = &buf[..len.saturating_sub(1)];
            if let Ok(text) = core::str::from_utf8(s) {
                crate::print!("{}", text);
            }
            0
        }
        Err(_) => usize::MAX,
    }
}

pub fn sys_print_int(ctx: &mut TrapContext) -> usize {
    let val = ctx.a0 as i32;
    printk!("{}", val);
    0
}
