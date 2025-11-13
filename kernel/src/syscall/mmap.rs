use crate::irq::TrapContext;
use crate::mem::mmap;
use crate::mem::uvm;
use crate::mem::{MMAP_BEGIN, MMAP_END, PageTable};
use crate::proc::current_proc;

pub fn sys_mmap(ctx: &mut TrapContext) -> usize {
    let begin = ctx.a0;
    let len = ctx.a1;
    let flags = 0;
    let p = current_proc();
    let pt = unsafe { &mut *(p.root_pt_pa as *mut PageTable) };
    match uvm::mmap(pt, &mut p.mmap_head, begin, len, flags, MMAP_BEGIN, MMAP_END) {
        Ok(va) => {
            #[cfg(feature = "tests")]
            {
                mmap::print_mmaplist(p.mmap_head);
                crate::mem::vm::print(pt);
            }
            va
        }
        Err(_) => usize::MAX,
    }
}

pub fn sys_munmap(ctx: &mut TrapContext) -> usize {
    let begin = ctx.a0;
    let len = ctx.a1;
    let p = current_proc();
    let pt = unsafe { &mut *(p.root_pt_pa as *mut PageTable) };
    match uvm::munmap(pt, &mut p.mmap_head, begin, len) {
        Ok(()) => {
            #[cfg(feature = "tests")]
            {
                mmap::print_mmaplist(p.mmap_head);
                crate::mem::vm::print(pt);
            }
            0
        }
        Err(_) => usize::MAX,
    }
}
