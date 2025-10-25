use super::context::ProcContext;
use crate::mem::addr::{PhysAddr, VirtAddr};
use crate::mem::pgtbl::PageTable;
use crate::mem::pmem::pmem_alloc;
use crate::mem::pte::{PTE_A, PTE_D, PTE_R, PTE_U, PTE_W, PTE_X};
use crate::mem::vm::{vm_map_kernel_pages, vm_mappages};
use crate::mem::{PGSIZE, VA_MAX};
use crate::printk;
use crate::trap::TrapFrame;

unsafe extern "C" {
    fn enter_proc(entry: usize, user_sp: usize) -> !;
    fn switch_context(old: *mut TrapFrame, new: *const TrapFrame);
    static __trampoline: u8;
}

pub struct Process {
    pid: usize,                // 进程ID
    page_table: PageTable,     // 进程页表
    heap_top: VirtAddr,        // 进程堆顶地址
    stack_size: usize,         // 进程栈大小
    trapframe: *mut TrapFrame, // 内核态上下文
    context: ProcContext,      // 用户态上下文
    kernel_stack: PhysAddr,    // 内核栈地址
}

pub fn create(payload: &[u8]) -> Process {
    /*
    用户地址空间布局：
    trapoline   (1 page) 映射在最高地址
    trapframe   (1 page)
    ustack      (1 page)
    .......
                        <--heap_top
    code + data (1 page)
    empty space (1 page) 最低的4096字节 不分配物理页，同时不可访问
    */
    let mut page_table = PageTable::new();
    // Map trampoline
    let tramp_addr = &unsafe { __trampoline } as *const u8 as PhysAddr;
    vm_mappages(&mut page_table, tramp_addr, tramp_addr, PGSIZE, PTE_R | PTE_X);
    // Map trapframe
    let trapframe_pa = pmem_alloc(false) as PhysAddr;
    let trapframe_va = tramp_addr + PGSIZE;
    vm_mappages(&mut page_table, trapframe_va, trapframe_pa, PGSIZE, PTE_R | PTE_W | PTE_X);
    // Map user stack
    let stack_pa = pmem_alloc(false) as PhysAddr;
    let stack_va = trapframe_va + PGSIZE;
    vm_mappages(&mut page_table, stack_va, stack_pa, PGSIZE, PTE_U | PTE_R | PTE_W | PTE_A | PTE_D);
    // Map empty page
    let empty_va = VA_MAX - PGSIZE;
    vm_mappages(&mut page_table, empty_va, 0, PGSIZE, PTE_U | PTE_R | PTE_W | PTE_A | PTE_D);
    // Load user code
    let code_va = empty_va - PGSIZE;
    let code_pa = pmem_alloc(false) as PhysAddr;
    let (src_ptr, src_len) = (payload.as_ptr(), payload.len());
    let copy_len = core::cmp::min(src_len, PGSIZE);
    unsafe { core::ptr::copy_nonoverlapping(src_ptr, code_pa as *mut u8, copy_len) };
    // Map user code
    vm_mappages(&mut page_table, code_va, code_pa, PGSIZE, PTE_U | PTE_R | PTE_X | PTE_A);
    Process {
        pid: 0,
        page_table,
        heap_top: code_va,
        stack_size: 0,
        trapframe: trapframe_pa as *mut TrapFrame,
        context: ProcContext::new(),
        kernel_stack: 0,
    }
}

pub fn launch(proc: &mut Process) {
    printk!("PROC: Launching process with pid {}", proc.pid);
}

// 直接在当前核上运行用户态 Payload
pub fn launch_payload(payload: &[u8]) -> ! {
    let code_pa = pmem_alloc(false) as PhysAddr;
    let stack_pa = pmem_alloc(false) as PhysAddr;
    let (src_ptr, src_len) = (payload.as_ptr(), payload.len());
    let copy_len = core::cmp::min(src_len, PGSIZE);
    unsafe { core::ptr::copy_nonoverlapping(src_ptr, code_pa as *mut u8, copy_len) };

    // Code: U|R|X
    vm_map_kernel_pages(code_pa, code_pa, PGSIZE, PTE_U | PTE_R | PTE_X | PTE_A);
    // Stack: U|R|W
    vm_map_kernel_pages(stack_pa, stack_pa, PGSIZE, PTE_U | PTE_R | PTE_W | PTE_A | PTE_D);

    let entry = code_pa;
    let proc_sp = stack_pa + PGSIZE;
    printk!("PROC: Launching proc at {:p}, sp={:p}", entry as *const u8, proc_sp as *const u8);
    unsafe { enter_proc(entry, proc_sp) }
}
