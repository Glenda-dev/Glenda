use crate::mem::mmap::mmap_show_mmaplist;
use crate::mem::uvm::{uvm_mmap, uvm_munmap};
use crate::mem::{MMAP_BEGIN, MMAP_END, PageTable};
use crate::printk;
use crate::proc::current_proc;
use crate::trap::TrapContext;

pub fn sys_mmap(ctx: &mut TrapContext) -> usize {
    let begin = ctx.a0;
    let len = ctx.a1;
    let flags = 0;
    let p = current_proc();
    let pt = unsafe { &mut *(p.root_pt_pa as *mut PageTable) };
    match uvm_mmap(pt, &mut p.mmap_head, begin, len, flags, MMAP_BEGIN, MMAP_END) {
        Ok(va) => {
            mmap_show_mmaplist(p.mmap_head);
            #[cfg(feature = "tests")]
            {
                crate::mem::vm::vm_print(pt);
                printk!("\n");
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
    match uvm_munmap(pt, &mut p.mmap_head, begin, len) {
        Ok(()) => {
            mmap_show_mmaplist(p.mmap_head);
            #[cfg(feature = "tests")]
            {
                crate::mem::vm::vm_print(pt);
            }
            0
        }
        Err(_) => usize::MAX,
    }
}
