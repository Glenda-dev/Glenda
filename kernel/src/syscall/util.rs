use crate::irq::TrapContext;
use crate::mem::PageTable;
use crate::mem::uvm;
use crate::printk;
use crate::proc;

// TODO: Deprecated
pub fn sys_print(ctx: &mut TrapContext) -> usize {
    let u_src = ctx.a0;
    let mut buf: [u8; 256] = [0; 256];
    let p = proc::current();
    let pt = unsafe { &*(p.root_pt_pa as *const PageTable) };
    match uvm::copyin_str(pt, &mut buf, u_src) {
        Ok(len) => {
            let s = &buf[..len.saturating_sub(1)];
            if let Ok(text) = core::str::from_utf8(s) {
                printk!("{}", text);
            }
            0
        }
        Err(_) => usize::MAX,
    }
}
