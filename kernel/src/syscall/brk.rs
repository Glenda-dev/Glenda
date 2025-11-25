use crate::irq::TrapContext;
use crate::mem::MMAP_BEGIN;
use crate::mem::PageTable;
use crate::mem::addr::align_up;
use crate::mem::uvm;
use crate::printk;
use crate::printk::{ANSI_RESET, ANSI_YELLOW};
use crate::proc::current_proc;

pub fn sys_brk(ctx: &mut TrapContext) -> usize {
    let new_top = ctx.a0;
    let p = current_proc();
    let old_top = p.heap_top;
    if new_top == 0 {
        return old_top;
    }
    if new_top > MMAP_BEGIN {
        printk!(
            "{}[WARN] brk: new_top=0x{:x} > MMAP_BEGIN=0x{:x}{}\n",
            ANSI_YELLOW,
            new_top,
            MMAP_BEGIN,
            ANSI_RESET
        );
        return usize::MAX;
    }
    if new_top < p.heap_base {
        printk!(
            "{}[WARN] brk: new_top=0x{:x} < heap_base=0x{:x}{}\n",
            ANSI_YELLOW,
            new_top,
            p.heap_base,
            ANSI_RESET
        );
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
            printk!("brk: old=0x{:x} -> new=0x{:x}\n", old_top, proc.heap_top);
            proc.heap_top
        }
        Err(e) => {
            printk!("{}[WARN] brk: failed: {:?}{}\n", ANSI_YELLOW, e, ANSI_RESET);
            usize::MAX
        }
    }
}
