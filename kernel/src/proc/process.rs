use super::ProcContext;
use super::set_current_user_satp;
use super::table::{GLOBAL_PID, NPROC, PROC_TABLE};
use crate::hart;
use crate::irq::TrapFrame;
use crate::irq::vector;
use crate::mem::addr::align_down;
use crate::mem::frame::PhysFrame;
use crate::mem::mmap::{self, MmapRegion};
use crate::mem::pmem;
use crate::mem::pte::{PTE_A, PTE_D, PTE_R, PTE_U, PTE_W, PTE_X};
use crate::mem::uvm;
use crate::mem::vm;
use crate::mem::{PGSIZE, PageTable, PhysAddr, VA_MAX, VirtAddr};
use crate::printk;
use crate::proc::scheduler::wakeup;
use core::sync::atomic::Ordering;
use riscv::asm::wfi;
use riscv::register::{satp, sscratch};

unsafe extern "C" {
    pub fn switch_context(old_ctx: &mut ProcContext, new_ctx: &mut ProcContext);
    fn trap_user_return(ctx: &mut ProcContext) -> !;
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ProcState {
    Unused,
    Zombie,
    Dying,
    Sleeping,
    Runnable,
    Running,
}

pub struct Process {
    pub name: [u8; 16],                     // 进程名称
    pub state: ProcState,                   // 进程状态
    pub parent: *mut Process,               // 父进程指针
    pub exit_code: i32,                     // 退出码
    pub sleep_chan: usize,                  // 睡眠通道
    pub pid: usize,                         // 进程ID
    pub root_pt_pa: PhysAddr,               // 根页表物理地址
    pub root_pt_frame: Option<PhysFrame>,   // RAII frame
    pub heap_top: VirtAddr,                 // 进程堆顶地址
    pub heap_base: VirtAddr,                // 进程堆底（最小堆顶），用于 brk 下限约束
    pub stack_pages: usize,                 // 进程栈大小
    pub trapframe: *mut TrapFrame,          // TrapFrame 指针（物理页）
    pub trapframe_frame: Option<PhysFrame>, // RAII frame
    pub trapframe_va: VirtAddr,             // TrapFrame 的用户可见虚拟地址
    pub context: ProcContext,               // 用户态上下文
    pub kernel_stack: PhysAddr,             // 内核栈地址
    pub entry_va: VirtAddr,                 // 用户入口地址
    pub user_sp_va: VirtAddr,               // 用户栈顶 VA
    pub mmap_head: *mut MmapRegion,         // mmap 链表头
}

unsafe impl Send for Process {}
unsafe impl Sync for Process {}

impl Process {
    pub const fn new() -> Self {
        Self {
            name: [0; 16],
            state: ProcState::Unused,
            parent: core::ptr::null_mut(),
            exit_code: 0,
            sleep_chan: 0,

            pid: 0,
            root_pt_pa: 0,
            root_pt_frame: None,
            heap_top: 0,
            heap_base: 0,
            stack_pages: 0,
            trapframe: core::ptr::null_mut(),
            trapframe_frame: None,
            trapframe_va: 0,
            context: ProcContext::new(),
            kernel_stack: 0,
            entry_va: 0,
            user_sp_va: 0,
            mmap_head: core::ptr::null_mut(),
        }
    }

    pub fn ustack_grow(&mut self, fault_va: VirtAddr) -> Result<(), ()> {
        let pt = unsafe { &mut *(self.root_pt_pa as *mut PageTable) };
        match uvm::ustack_grow(pt, &mut self.stack_pages, self.trapframe_va, fault_va) {
            Ok(_) => Ok(()),
            Err(_) => Err(()),
        }
    }

    pub fn root_satp(&self) -> usize {
        // 根页表物理页号
        let ppn = (self.root_pt_pa >> 12) & ((1usize << (usize::BITS as usize - 12)) - 1);
        // Compose SATP value for Sv39: MODE in bits [63:60], ASID=pid, PPN in [43:0]
        ((satp::Mode::Sv39 as usize) << 60) | (self.pid << 44) | ppn
    }

    #[cfg(debug_assertions)]
    pub fn print(&self) {
        use crate::printk;

        printk!(
            "Process:\n  pid: {}\n  root_pt_pa: 0x{:x}\n  heap_top: 0x{:x}\n  heap_base: 0x{:x}\n  stack_pages: {}\n  trapframe: 0x{:x}\n  trapframe_va: 0x{:x}\n  kernel_stack: 0x{:x}\n  entry_va: 0x{:x}\n  user_sp_va: 0x{:x}\n",
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

    pub fn free(&mut self) {
        let page_table = unsafe { &mut *(self.root_pt_pa as *mut PageTable) };

        // Destory page table first (frees TrapFrame and children)
        page_table.destroy();
        self.root_pt_frame = None;

        // Free mmap regions
        mmap::region_free(self.mmap_head);

        // Free Kernel Stack
        if self.kernel_stack != 0 {
            let kstack_pages = vm::KSTACK_SIZE / PGSIZE;
            let kstack_base = self.kernel_stack - vm::KSTACK_SIZE;
            for i in 0..kstack_pages {
                let pa = kstack_base + i * PGSIZE;
                pmem::free(pa, true);
            }
            self.kernel_stack = 0;
        }
    }

    pub fn exit(&mut self) {
        // Wake up parent if sleeping in wait()
        if !self.parent.is_null() {
            let parent = self.parent as usize;
            wakeup(parent);
        }
        {
            let mut table = PROC_TABLE.lock();
            let init_ptr: *mut Process = table.as_mut_ptr();
            for i in 0..NPROC {
                let p = &mut table[i];
                if p.parent == (self as *mut Process) {
                    p.parent = init_ptr; // init process
                }
            }
        }
        self.state = ProcState::Dying;
    }

    pub fn launch(&mut self) {
        let hart = hart::get();
        hart.proc = self as *mut Process;
        unsafe {
            switch_context(&mut hart.context, &mut self.context);
        }
    }

    // TODO: Copy-on-write fork
    pub fn fork(&mut self) -> &'static mut Process {
        let child = alloc().expect("Failed to allocate process");
        // quiet fork path in release
        // Copy process state from parent to child
        child.parent = self as *mut Process;
        child.entry_va = self.entry_va;
        child.user_sp_va = self.user_sp_va;
        child.trapframe_va = self.trapframe_va;

        // Copy page table
        let parent_pt = unsafe { &*(self.root_pt_pa as *const PageTable) };
        // Alloc RAII frame for child root pt
        let child_pt_pa_raw = parent_pt.copy().expect("Failed to copy page table");
        // We must wrap the raw PA from `copy` into a PhysFrame.
        // Since `copy` allocated it using `pmem::alloc`, it has ref_count=1.
        // Wrapping it in PhysFrame is correct ownership transfer.
        let child_pt_frame = unsafe { PhysFrame::from_raw(child_pt_pa_raw) };
        child.root_pt_pa = child_pt_pa_raw;
        child.root_pt_frame = Some(child_pt_frame);

        // Copy heap info
        child.heap_base = self.heap_base;
        child.heap_top = self.heap_top;
        // Copy stack size
        child.stack_pages = self.stack_pages;

        // Allocate new TrapFrame page for child
        let child_tf_pa = pmem::alloc(true) as PhysAddr;
        child.trapframe = child_tf_pa as *mut TrapFrame;

        // Allocate new Kernel Stack for child
        let kstack_pages = vm::KSTACK_SIZE / PGSIZE;
        let child_kstack_pa = pmem::alloc_contiguous(kstack_pages, true) as PhysAddr;
        child.kernel_stack = child_kstack_pa + kstack_pages * PGSIZE;

        // Map new TrapFrame in child's page table (overwrite copied mapping)
        let child_pt = unsafe { &mut *(child.root_pt_pa as *mut PageTable) };
        vm::unmappages(child_pt, child.trapframe_va, PGSIZE, false);
        vm::mappages(
            child_pt,
            child.trapframe_va,
            child_tf_pa,
            PGSIZE,
            PTE_R | PTE_W | PTE_A | PTE_D,
        );

        // Copy TrapFrame content
        unsafe {
            core::ptr::copy_nonoverlapping(self.trapframe, child.trapframe, 1);
        }

        // Update child's TrapFrame to return 0 from fork
        let child_tf = unsafe { &mut *child.trapframe };
        child_tf.a0 = 0; // fork 返回值为0
        child_tf.kernel_epc = child_tf.kernel_epc.wrapping_add(4); // skip ecall
        child_tf.kernel_sp = child.kernel_stack; // Use child's own kernel stack

        let parent_tf = unsafe { &mut *self.trapframe };
        // 父进程从 fork 返回子进程的 pid
        parent_tf.a0 = child.pid;
        child.context.sp = (child.kernel_stack) as usize;
        child.context.ra = trap_user_return as usize;
        child.state = ProcState::Runnable;
        child
    }

    pub fn exec(&mut self, payload: &[u8]) {
        let page_table = unsafe { &mut *(self.root_pt_pa as *mut PageTable) };
        let empty_va = 0usize;
        let code_va = empty_va + PGSIZE;
        let (src_ptr, src_len) = (payload.as_ptr(), payload.len());
        let mut mapped_len = 0usize;
        if src_len == 0 {
            let code_pa = pmem::alloc(false) as PhysAddr;
            unsafe { core::ptr::write_bytes(code_pa as *mut u8, 0, PGSIZE) };
            vm::mappages(
                page_table,
                code_va,
                code_pa,
                PGSIZE,
                PTE_U | PTE_R | PTE_W | PTE_X | PTE_A,
            );
            mapped_len = PGSIZE;
        } else {
            let total = src_len;
            while mapped_len < total {
                let pa = pmem::alloc(false) as PhysAddr;
                let this_len = core::cmp::min(PGSIZE, total - mapped_len);
                unsafe {
                    core::ptr::write_bytes(pa as *mut u8, 0, PGSIZE);
                    core::ptr::copy_nonoverlapping(
                        src_ptr.add(mapped_len),
                        pa as *mut u8,
                        this_len,
                    );
                }
                let va = code_va + mapped_len;
                vm::mappages(
                    page_table,
                    va,
                    pa,
                    PGSIZE,
                    PTE_U | PTE_R | PTE_W | PTE_X | PTE_A | PTE_D,
                );
                mapped_len += this_len;
            }
        }
        self.entry_va = code_va;
        self.heap_top = align_down(code_va + ((mapped_len + PGSIZE - 1) & !(PGSIZE - 1)));
        self.heap_base = self.heap_top;
    }
}

#[unsafe(no_mangle)]
extern "C" fn proc_return() -> ! {
    crate::proc::scheduler::sched();
    loop {
        wfi();
    }
}

pub fn init() {
    GLOBAL_PID.store(1, Ordering::SeqCst);
}

pub fn alloc() -> Option<&'static mut Process> {
    let mut table = PROC_TABLE.lock();
    for i in 0..NPROC {
        if table[i].state == ProcState::Unused {
            // Take a raw pointer to the slot inside the static table and
            // convert it to a 'static mutable reference (unsafe).
            let p_ptr: *mut Process = &mut table[i] as *mut Process;
            let p: &'static mut Process = unsafe { &mut *p_ptr };

            p.pid = GLOBAL_PID.fetch_add(1, Ordering::SeqCst);
            p.parent = core::ptr::null_mut();
            p.exit_code = 0;
            p.sleep_chan = 0;
            p.context = ProcContext::new();
            p.context.ra = proc_return as usize;
            p.context.sp = 0;
            p.state = ProcState::Runnable;
            return Some(p);
        }
    }
    None
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
pub fn create(payload: &[u8]) -> &'static mut Process {
    let proc = alloc().expect("Failed to allocate process");
    // Setup pid
    // 分配一页作为根页表（物理内存）
    let root_pt_frame = PhysFrame::alloc().expect("Failed to alloc root pt");
    proc.root_pt_pa = root_pt_frame.addr();
    proc.root_pt_frame = Some(root_pt_frame);
    let page_table = unsafe { &mut *(proc.root_pt_pa as *mut PageTable) };
    unsafe { core::ptr::write_bytes(page_table as *mut PageTable as *mut u8, 0, PGSIZE) };
    // Setup Trampoline
    let tramp_pa = align_down(vector::trampoline as usize) as PhysAddr; // trampoline 物理地址
    let tramp_va = VA_MAX - PGSIZE; // trampoline 虚拟地址（最高页）
    vm::mappages(page_table, tramp_va, tramp_pa, PGSIZE, PTE_R | PTE_X | PTE_A);
    // Setup TrapFrame
    // TrapFrame 放在内核物理页区域，避免占用用户物理页池
    let trapframe_pa = pmem::alloc(true) as PhysAddr; // trapframe 物理地址
    let trapframe_va = tramp_va - PGSIZE; // trapframe 虚拟地址
    proc.trapframe_va = trapframe_va;
    proc.trapframe = trapframe_pa as *mut TrapFrame;
    vm::mappages(page_table, trapframe_va, trapframe_pa, PGSIZE, PTE_R | PTE_W | PTE_A | PTE_D);
    // Load payload
    proc.exec(payload);
    // Setup Kernel Stack
    let kstack_pages = vm::KSTACK_SIZE / PGSIZE;
    let kstack_pa = pmem::alloc_contiguous(kstack_pages, true) as PhysAddr;
    unsafe {
        core::ptr::write_bytes(kstack_pa as *mut u8, 0, vm::KSTACK_SIZE);
    }
    proc.kernel_stack = kstack_pa + vm::KSTACK_SIZE;
    // Setup initial user stack top (matches service/hello/link.ld)
    proc.user_sp_va = 0x20000 + 24576; // STACK_TOP
    // Ensure I-cache observes freshly written user code
    riscv::asm::fence_i();
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
    // 设置进程状态为可运行
    proc.state = ProcState::Runnable;
    proc
}
