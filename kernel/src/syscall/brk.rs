use crate::mem::MMAP_BEGIN;
use crate::mem::PageTable;
use crate::mem::addr::align_up;
use crate::mem::uvm;
use crate::printk;
use crate::proc::current_proc;
use crate::trap::TrapContext;

pub fn sys_brk(ctx: &mut TrapContext) -> usize {
    let new_top = ctx.a0;
    let p = current_proc();
    let old_top = p.heap_top;
    if new_top == 0 {
        return old_top;
    }
    if new_top > MMAP_BEGIN {
        printk!("brk: new_top=0x{:x} > MMAP_BEGIN=0x{:x}", new_top, MMAP_BEGIN);
        return usize::MAX;
    }
    if new_top < p.heap_base {
        printk!("brk: new_top=0x{:x} < heap_base=0x{:x}", new_top, p.heap_base);
        return usize::MAX;
    }

    let table = unsafe { &mut *(p.root_pt_pa as *mut PageTable) };
    let new_heap_top = align_up(new_top);
    let res = if new_heap_top > old_top {
        uvm::heap_grow(table, old_top, new_heap_top)
    } else if new_heap_top < old_top {
        uvm::heap_ungrow(table, old_top, new_heap_top)
    } else {
        Ok(())
    };
    match res {
        Ok(()) => {
            let proc = current_proc();
            proc.heap_top = new_heap_top;
            printk!("brk: old=0x{:x} -> new=0x{:x}", old_top, proc.heap_top);
            proc.heap_top
        }
        Err(e) => {
            printk!("brk: failed: {:?}", e);
            usize::MAX
        }
    }
}
