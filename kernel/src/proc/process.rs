use super::ProcContext;
use super::set_current_user_satp;
use crate::hart;
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
    fn switch_context(old_ctx: &mut ProcContext, new_ctx: &mut ProcContext) -> !;
}

pub struct Process {
    pub pid: usize,                // 进程ID
    pub root_pt_pa: PhysAddr,      // 根页表物理地址
    pub heap_top: VirtAddr,        // 进程堆顶地址
    pub heap_base: VirtAddr,       // 进程堆底（最小堆顶），用于 brk 下限约束
    pub stack_pages: usize,        // 进程栈大小
    pub trapframe: *mut TrapFrame, // TrapFrame 指针（物理页）
    pub trapframe_va: VirtAddr,    // TrapFrame 的用户可见虚拟地址
    pub context: ProcContext,      // 用户态上下文
    pub kernel_stack: PhysAddr,    // 内核栈地址
    pub entry_va: VirtAddr,        // 用户入口地址
    pub user_sp_va: VirtAddr,      // 用户栈顶 VA
}

impl Process {
    pub fn new() -> Self {
        Self {
            pid: 0,
            root_pt_pa: 0,
            heap_top: 0,
            heap_base: 0,
            stack_pages: 0,
            trapframe: core::ptr::null_mut(),
            trapframe_va: 0,
            context: ProcContext::new(),
            kernel_stack: 0,
            entry_va: 0,
            user_sp_va: 0,
        }
    }
    pub fn root_satp(&self) -> usize {
        // 根页表物理页号
        let ppn = (self.root_pt_pa >> 12) & ((1usize << (usize::BITS as usize - 12)) - 1);
        // Compose SATP value for Sv39: MODE in bits [63:60], ASID=pid, PPN in [43:0]
        ((satp::Mode::Sv39 as usize) << 60) | (self.pid << 44) | ppn
    }
    #[cfg(feature = "tests")]
    pub fn print(&self) {
        printk!(
            "Process:\n  pid: {}\n  root_pt_pa: 0x{:x}\n  heap_top: 0x{:x}\n  heap_base: 0x{:x}\n  stack_pages: {}\n  trapframe: 0x{:x}\n  trapframe_va: 0x{:x}\n  kernel_stack: 0x{:x}\n  entry_va: 0x{:x}\n  user_sp_va: 0x{:x}",
            self.pid,
            self.root_pt_pa,
            self.heap_top,
            self.heap_base,
            self.stack_pages,
            self.trapframe as usize,
            self.trapframe_va,
            self.kernel_stack,
            self.entry_va,
            self.user_sp_va,
        );
        let page_table = unsafe { &*(self.root_pt_pa as *const PageTable) };
        page_table.print();
        let tf = unsafe { &*(self.trapframe) };
        tf.print();
        self.context.print();
    }
}

/*
用户地址空间布局：
trampoline  (1 page) 映射在最高地址
trapframe   (1 page)
ustack      (N pages)
-------------------  MMAP_END
mmap region [MMAP_BEGIN, MMAP_END)
-------------------  MMAP_BEGIN
heap        (手动管理)
code + data (1 page)
empty space (1 page) 最低的4096字节 不分配物理页，同时不可访问
*/
pub fn create(payload: &[u8]) -> Process {
    let mut proc = Process::new();
    // Setup pid
    proc.pid = 0;
    // 分配一页作为根页表（物理内存）
    proc.root_pt_pa = pmem_alloc(true) as PhysAddr;
    let page_table = unsafe { &mut *(proc.root_pt_pa as *mut PageTable) };
    unsafe { core::ptr::write_bytes(page_table as *mut PageTable as *mut u8, 0, PGSIZE) };
    // Setup Trampoline
    let tramp_pa = align_down(vector::trampoline as usize) as PhysAddr; // trampoline 物理地址
    let tramp_va = VA_MAX - PGSIZE; // trampoline 虚拟地址（最高页）
    vm_mappages(page_table, tramp_va, tramp_pa, PGSIZE, PTE_R | PTE_X | PTE_A);
    // Setup TrapFrame
    let trapframe_pa = pmem_alloc(false) as PhysAddr; // trapframe 物理地址
    let trapframe_va = tramp_va - PGSIZE; // trapframe 虚拟地址
    proc.trapframe_va = trapframe_va;
    proc.trapframe = trapframe_pa as *mut TrapFrame;
    vm_mappages(page_table, trapframe_va, trapframe_pa, PGSIZE, PTE_R | PTE_W | PTE_A | PTE_D);
    // Setup User Stack
    let stack_pa = pmem_alloc(false) as PhysAddr; // 用户栈物理地址
    let stack_va = trapframe_va - PGSIZE; // 用户栈虚拟地址
    proc.stack_pages = 1;
    proc.user_sp_va = stack_va + PGSIZE; // 用户栈顶虚拟地址
    vm_mappages(page_table, stack_va, stack_pa, PGSIZE, PTE_U | PTE_R | PTE_W | PTE_A | PTE_D);
    // Setup Protected Page
    let empty_va = 0usize; // 最低的空闲页虚拟地址
    let code_va = empty_va + PGSIZE;
    let (src_ptr, src_len) = (payload.as_ptr(), payload.len());
    let mut mapped_len = 0usize;
    if src_len == 0 {
        let code_pa = pmem_alloc(false) as PhysAddr;
        unsafe { core::ptr::write_bytes(code_pa as *mut u8, 0, PGSIZE) };
        vm_mappages(page_table, code_va, code_pa, PGSIZE, PTE_U | PTE_R | PTE_X | PTE_A);
        mapped_len = PGSIZE;
    } else {
        let total = src_len;
        while mapped_len < total {
            let pa = pmem_alloc(false) as PhysAddr;
            let this_len = core::cmp::min(PGSIZE, total - mapped_len);
            unsafe {
                core::ptr::write_bytes(pa as *mut u8, 0, PGSIZE);
                core::ptr::copy_nonoverlapping(src_ptr.add(mapped_len), pa as *mut u8, this_len);
            }
            let va = code_va + mapped_len;
            vm_mappages(page_table, va, pa, PGSIZE, PTE_U | PTE_R | PTE_X | PTE_A);
            mapped_len += this_len;
        }
    }
    proc.entry_va = code_va;
    // Setup Kernel Stack
    let kstack_pa = pmem_alloc(true) as PhysAddr;
    unsafe {
        core::ptr::write_bytes(kstack_pa as *mut u8, 0, PGSIZE);
    }
    proc.kernel_stack = kstack_pa + PGSIZE;
    // Setup Heap
    proc.heap_top = align_down(code_va + ((mapped_len + PGSIZE - 1) & !(PGSIZE - 1)));
    proc.heap_base = proc.heap_top;
    // Ensure I-cache observes freshly written user code
    riscv::asm::fence_i();
    proc
}

pub fn launch(proc: &mut Process) {
    // 初始化 trapframe 的返回地址和用户栈（通过物理地址访问）
    let tf = unsafe { &mut *proc.trapframe };
    tf.sp = proc.user_sp_va;
    tf.kernel_epc = proc.entry_va;
    tf.kernel_satp = satp::read().bits();
    tf.kernel_hartid = hart::getid();
    tf.kernel_sp = proc.kernel_stack;
    // 记录当前用户页表 SATP
    let satp_bits = proc.root_satp();
    set_current_user_satp(satp_bits);
    // 为 trampoline 设置正确的 TrapFrame 用户虚拟地址：
    // - sscratch 指向 TrapFrame 的用户虚拟地址
    // - 在 TrapFrame 中的 a0 字段也写入该虚拟地址，供 user_return 首次恢复使用
    let tf_user_va = proc.trapframe_va as *mut TrapFrame;
    unsafe { sscratch::write(tf_user_va as usize) };
    tf.a0 = tf_user_va as usize;
    // 设置内核态上下文
    proc.context.ra = trap_user_return as usize;
    let kstack_va = proc.kernel_stack as *mut u8;
    proc.context.sp = kstack_va as usize;
    // 直接使用内核可见的 TrapFrame 指针进入 trap_user_return（不返回）
    printk!("PROC: Launching proc with pid={}", proc.pid);
    let hart = hart::get();
    hart.proc = proc as *mut Process;
    unsafe {
        switch_context(&mut hart.context, &mut proc.context);
    }
}
