use crate::mem::pmem::pmem_alloc;
use crate::mem::pte::{PTE_A, PTE_D, PTE_R, PTE_U, PTE_W, PTE_X};
use crate::mem::vm::vm_map_kernel_pages;
use crate::printk;

const PAGE_SIZE: usize = crate::mem::PGSIZE;

unsafe extern "C" {
    fn enter_proc(entry: usize, user_sp: usize) -> !;
}

pub fn launch(payload: &[u8]) -> ! {
    let code_pa = pmem_alloc(false) as usize;
    let stack_pa = pmem_alloc(false) as usize;
    let (src_ptr, src_len) = (payload.as_ptr(), payload.len());
    let copy_len = core::cmp::min(src_len, PAGE_SIZE);
    unsafe { core::ptr::copy_nonoverlapping(src_ptr, code_pa as *mut u8, copy_len) };

    // Code: U|R|X
    vm_map_kernel_pages(code_pa, PAGE_SIZE, code_pa, PTE_U | PTE_R | PTE_X | PTE_A);
    // Stack: U|R|W
    vm_map_kernel_pages(stack_pa, PAGE_SIZE, stack_pa, PTE_U | PTE_R | PTE_W | PTE_A | PTE_D);

    let entry = code_pa;
    let proc_sp = stack_pa + PAGE_SIZE;
    printk!("PROC: Launching proc at {:p}, sp={:p}", entry as *const u8, proc_sp as *const u8);
    unsafe { enter_proc(entry, proc_sp) }
}
