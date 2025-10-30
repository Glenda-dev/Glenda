use super::ProcContext;
use super::set_current_user_satp;
use crate::mem::addr::align_down;
use crate::mem::pmem::pmem_alloc;
use crate::mem::pte::{PTE_A, PTE_D, PTE_R, PTE_U, PTE_W, PTE_X};
use crate::mem::vm::vm_mappages;
use crate::mem::{PGSIZE, PageTable, PhysAddr, VA_MAX, VirtAddr};
use crate::printk;
use crate::trap::TrapFrame;
use crate::trap::vector;
use riscv::register::{satp, sscratch};
unsafe extern "C" {
    fn trap_user_return(ctx: &mut TrapFrame) -> !;
}

pub struct Process {
    pub pid: usize,                // 进程ID
    pub root_pt_pa: PhysAddr,      // 根页表物理地址
    pub heap_top: VirtAddr,        // 进程堆顶地址
    pub stack_size: usize,         // 进程栈大小
    pub trapframe: *mut TrapFrame, // TrapFrame 指针（物理页）
    pub trapframe_va: VirtAddr,    // TrapFrame 的用户可见虚拟地址
    pub context: ProcContext,      // 用户态上下文
    pub kernel_stack: PhysAddr,    // 内核栈地址
    pub entry_va: VirtAddr,        // 用户入口地址
    pub user_sp_va: VirtAddr,      // 用户栈顶 VA
}

pub fn create(payload: &[u8]) -> Process {
    // 分配一页作为根页表（物理内存）
    let root_pt_pa = pmem_alloc(true) as PhysAddr;
    let page_table = unsafe { &mut *(root_pt_pa as *mut PageTable) };
    unsafe { core::ptr::write_bytes(page_table as *mut PageTable as *mut u8, 0, PGSIZE) };
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
    let tramp_pa = align_down(vector::trampoline as usize) as PhysAddr; // trampoline 物理地址
    let tramp_va = VA_MAX - PGSIZE; // trampoline 虚拟地址（最高页）
    let trapframe_pa = pmem_alloc(false) as PhysAddr; // trapframe 物理地址
    let trapframe_va = tramp_va - PGSIZE; // trapframe 虚拟地址
    let stack_pa = pmem_alloc(false) as PhysAddr; // 用户栈物理地址
    let stack_va = trapframe_va - PGSIZE; // 用户栈虚拟地址
    let empty_va = 0usize; // 最低的空闲页虚拟地址
    let code_va = empty_va + PGSIZE; // 用户代码虚拟地址
    let code_pa = pmem_alloc(false) as PhysAddr; // 用户代码物理地址
    let (src_ptr, src_len) = (payload.as_ptr(), payload.len()); // 用户代码源指针和长度
    let copy_len = core::cmp::min(src_len, PGSIZE); // 复制长度不超过一页
    let kstack = pmem_alloc(true) as PhysAddr; // 内核栈物理地址
    unsafe { core::ptr::copy_nonoverlapping(src_ptr, code_pa as *mut u8, copy_len) }; // 复制用户代码

    // Map trampoline:
    vm_mappages(page_table, tramp_va, tramp_pa, PGSIZE, PTE_R | PTE_X | PTE_A);
    // Map trapframe: place it right below trampoline
    vm_mappages(page_table, trapframe_va, trapframe_pa, PGSIZE, PTE_R | PTE_W | PTE_A | PTE_D);
    // Map user stack: place user stack below trapframe
    vm_mappages(page_table, stack_va, stack_pa, PGSIZE, PTE_U | PTE_R | PTE_W | PTE_A | PTE_D);
    // Lowest page is kept unmapped as a guard (page 0).
    // Ensure I-cache observes freshly written user code
    riscv::asm::fence_i();
    // Map user code
    vm_mappages(page_table, code_va, code_pa, PGSIZE, PTE_U | PTE_R | PTE_X | PTE_A);
    Process {
        pid: 0,
        root_pt_pa,
        // heap_top 应指向可分配 heap 的起始地址（紧跟 code page 之后）
        heap_top: code_va + PGSIZE,
        stack_size: PGSIZE,
        trapframe: trapframe_pa as *mut TrapFrame,
        trapframe_va: trapframe_va,
        context: ProcContext::new(),
        kernel_stack: kstack,
        entry_va: code_va,
        user_sp_va: stack_va + PGSIZE,
    }
}

pub fn launch(proc: &mut Process) {
    // 初始化 trapframe 的返回地址和用户栈（通过物理地址访问）
    let tf = unsafe { &mut *proc.trapframe };
    tf.sp = proc.user_sp_va;
    tf.kernel_epc = proc.entry_va;
    // 记录当前用户页表 SATP
    let satp_bits = proc.root_satp();
    set_current_user_satp(satp_bits);
    // 可选：在 tests 特性下打印页表用于调试
    // 调试用的 vm_print 已移除，避免测试日志噪音
    // 为 trampoline 设置正确的 TrapFrame 用户虚拟地址：
    // - sscratch 指向 TrapFrame 的用户虚拟地址
    // - 在 TrapFrame 中的 a0 字段也写入该虚拟地址，供 user_return 首次恢复使用
    let tf_user_va = proc.trapframe_va as *mut TrapFrame;
    unsafe { sscratch::write(tf_user_va as usize) };
    tf.a0 = tf_user_va as usize;

    // 直接使用内核可见的 TrapFrame 指针进入 trap_user_return（不返回）
    printk!(
        "PROC: Launching proc at {:p}, sp={:p} with pid={}",
        tf.kernel_epc as *const u8,
        tf.sp as *const u8,
        proc.pid
    );
    unsafe { trap_user_return(tf) }
}

impl Process {
    pub fn root_satp(&self) -> usize {
        // 根页表物理页号
        let ppn = (self.root_pt_pa >> 12) & ((1usize << (usize::BITS as usize - 12)) - 1);
        // Compose SATP value for Sv39: MODE in bits [63:60], ASID=0, PPN in [43:0]
        ((satp::Mode::Sv39 as usize) << 60) | ppn
    }
}
