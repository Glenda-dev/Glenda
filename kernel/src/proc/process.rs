use super::ProcContext;
use super::set_current_user_satp;
use super::runnable_queue;
use super::table::{GLOBAL_PID, NPROC, PROC_TABLE};
use crate::fs::inode;
use crate::hart;
use crate::irq::TrapFrame;
use crate::irq::vector;
use crate::mem::addr::align_down;
use crate::mem::frame::PhysFrame;
use crate::mem::mmap::{self, MmapRegion};
use crate::mem::pmem;
use crate::mem::pte::{PTE_A, PTE_D, PTE_R, PTE_U, PTE_W, PTE_X};
use crate::mem::uvm;
use crate::mem::vm::{self, KernelStack};
use crate::mem::{PGSIZE, PageTable, PhysAddr, VA_MAX, VirtAddr};
use crate::printk;
use crate::proc::scheduler::wakeup;
use core::sync::atomic::Ordering;
use riscv::asm::wfi;
use riscv::register::{satp, sscratch, sstatus};

unsafe extern "C" {
    pub fn switch_context(old_ctx: &mut ProcContext, new_ctx: &mut ProcContext);
    fn trap_user_return(ctx: &mut ProcContext) -> !;
}

#[unsafe(no_mangle)]
extern "C" fn fs_init_wrapper() -> ! {
    crate::fs::fs::fs_init();
    // call trap_user_return
    unsafe {
        let f: unsafe extern "C" fn() -> ! = core::mem::transmute(trap_user_return as usize);
        f();
    }
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

pub const NOFILE: usize = 32; // 每进程最大 FD

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
    pub kstack: Option<KernelStack>,        // 内核栈 RAII
    pub entry_va: VirtAddr,                 // 用户入口地址
    pub user_sp_va: VirtAddr,               // 用户栈顶 VA
    pub mmap_head: *mut MmapRegion,         // mmap 链表头
    pub open_files: [Option<usize>; NOFILE], // 打开的文件表索引
    pub cwd: u32,                           // 当前工作目录 inode 号
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
            kstack: None,
            entry_va: 0,
            user_sp_va: 0,
            mmap_head: core::ptr::null_mut(),
            open_files: [None; NOFILE],
            cwd: crate::fs::inode::ROOT_INODE,
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
            "Process:\n  pid: {}\n  root_pt_pa: 0x{:x}\n  heap_top: 0x{:x}\n  heap_base: 0x{:x}\n  stack_pages: {}\n  trapframe: 0x{:x}\n  trapframe_va: 0x{:x}\n  kstack top: 0x{:x}\n  entry_va: 0x{:x}\n  user_sp_va: 0x{:x}\n",
            self.pid,
            self.root_pt_pa,
            self.heap_top,
            self.heap_base,
            self.stack_pages,
            self.trapframe as usize,
            self.trapframe_va,
            self.kstack.as_ref().map(|k| k.top()).unwrap_or(0),
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

        // Kernel Stack is freed by Drop of KernelStack in self.kstack
        self.kstack = None;
    }

    pub fn exit(&mut self) {
        // Disable interrupts to avoid deadlock with ISR
        let sstatus_val = sstatus::read();
        let sie_enabled = sstatus_val.sie();
        unsafe { sstatus::clear_sie(); }

        // Wake up parent if sleeping in wait()
        if !self.parent.is_null() {
            let parent = self.parent as usize;
            wakeup(parent);
        }
        {
            let mut table = PROC_TABLE.lock();
            let init_ptr: *mut Process = table.as_mut_ptr();
            let mut self_idx = None;
            for i in 0..NPROC {
                let p = &mut table[i];
                if p.parent == (self as *mut Process) {
                    p.parent = init_ptr; // init process
                }
                // Find our own index
                if &mut table[i] as *mut Process == self as *mut Process {
                    self_idx = Some(i);
                }
            }
            // Clear runnable bit if we were runnable
            if let Some(idx) = self_idx {
                runnable_queue::mark_not_runnable(idx);
            }
        }
        self.state = ProcState::Dying;

        // Close open files
        for i in 0..NOFILE {
            if let Some(f_idx) = self.open_files[i] {
                crate::fs::file::file_close(f_idx);
                self.open_files[i] = None;
            }
        }

        if sie_enabled { unsafe { sstatus::set_sie(); } }
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

        // Copy FD table and increment refcnts
        child.open_files = self.open_files;
        for i in 0..NOFILE {
            if let Some(f_idx) = child.open_files[i] {
                let mut table = crate::fs::file::FILE_TABLE.lock();
                table.files[f_idx].refcnt += 1;
            }
        }
        child.cwd = self.cwd;
        // Increment refcnt for cwd inode if we track it via file objects? 
        // For now cwd is just an inum. In a full system, we might want to hold an Inode ref.
        // If cwd is just inum, no refcnt to increment here unless we use inode_get/put.
        // The design in STEPS.md says "cwd: u32 (inode_num)".

        // Allocate new TrapFrame page for child
        let child_tf_frame = PhysFrame::alloc().expect("Failed to alloc trapframe");
        let child_tf_pa = child_tf_frame.addr();
        child.trapframe = child_tf_pa as *mut TrapFrame;
        child.trapframe_frame = Some(child_tf_frame);

        // Allocate new Kernel Stack for child
        let kstack = KernelStack::new(child.pid);
        let kstack_top = kstack.top();
        child.kstack = Some(kstack);

        // Map new TrapFrame in child's page table (overwrite copied mapping)
        let child_pt = unsafe { &mut *(child.root_pt_pa as *mut PageTable) };
        // Free the TrapFrame page created by copy() (it was a duplicate of parent's, but we want a fresh one)
        vm::unmappages(child_pt, child.trapframe_va, PGSIZE, true);
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
        child_tf.kernel_sp = kstack_top; // Use child's own kernel stack

        let parent_tf = unsafe { &mut *self.trapframe };
        // 父进程从 fork 返回子进程的 pid
        parent_tf.a0 = child.pid;
        child.context.sp = kstack_top;
        child.context.ra = trap_user_return as usize;
        // Note: child.state is already set to Runnable by alloc(), and bitmap is already updated
        // This line is redundant but kept for clarity
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

    pub fn proc_exec(&mut self, path: &[u8], u_argv: &[usize]) -> Result<(), ()> {
        crate::printk!("proc_exec: path='{}'\n", core::str::from_utf8(path).unwrap_or("?"));
        let fd = crate::syscall::fs::fs_open(self, path, 0).map_err(|_| {
            crate::printk!("proc_exec: failed to open path\n");
        })?;
        let f_idx = self.open_files[fd].ok_or_else(|| {
            crate::printk!("proc_exec: invalid fd\n");
        })?;
        let inum = {
            let table = crate::fs::file::FILE_TABLE.lock();
            table.files[f_idx].inum
        };
        let ip = inode::inode_get(inum);

        let mut elf_header = [0u8; 64];
        if inode::inode_read_data(ip, 0, 64, &mut elf_header) != 64 {
            crate::printk!("proc_exec: failed to read ELF header\n");
            inode::inode_put(ip);
            crate::syscall::fs::fs_close(self, fd)?;
            return Err(());
        }

        // Check magic
        if elf_header[0..4] != [0x7f, b'E', b'L', b'F'] {
            crate::printk!("proc_exec: invalid ELF magic\n");
            inode::inode_put(ip);
            crate::syscall::fs::fs_close(self, fd)?;
            return Err(());
        }

        // Setup NEW page table
        let root_pt_frame = PhysFrame::alloc().ok_or_else(|| {
            crate::printk!("proc_exec: failed to alloc root pt\n");
        })?;
        let root_pt_pa = root_pt_frame.addr();
        let pt = unsafe { &mut *(root_pt_pa as *mut PageTable) };
        unsafe { core::ptr::write_bytes(pt as *mut PageTable as *mut u8, 0, PGSIZE) };

        // Map Trampoline
        let tramp_pa = align_down(vector::trampoline as usize) as PhysAddr;
        let tramp_va = VA_MAX - PGSIZE;
        vm::mappages(pt, tramp_va, tramp_pa, PGSIZE, PTE_R | PTE_X | PTE_A);

        // Setup NEW TrapFrame
        let trapframe_frame = PhysFrame::alloc().ok_or_else(|| {
            crate::printk!("proc_exec: failed to alloc trapframe\n");
        })?;
        let trapframe_pa = trapframe_frame.addr();
        let trapframe_va = tramp_va - PGSIZE;
        vm::mappages(pt, trapframe_va, trapframe_pa, PGSIZE, PTE_R | PTE_W | PTE_A | PTE_D);

        // Parse program headers
        let phoff = u64::from_le_bytes(elf_header[32..40].try_into().unwrap()) as u32;
        let phnum = u16::from_le_bytes(elf_header[56..58].try_into().unwrap()) as usize;
        let phentsize = u16::from_le_bytes(elf_header[54..56].try_into().unwrap()) as usize;

        let mut max_va = 0;

        for i in 0..phnum {
            let mut ph = [0u8; 56]; // Size of Phdr
            if inode::inode_read_data(ip, phoff + (i * phentsize) as u32, 56, &mut ph) != 56 {
                crate::printk!("proc_exec: failed to read phdr {}\n", i);
                break;
            }
            let p_type = u32::from_le_bytes(ph[0..4].try_into().unwrap());
            if p_type == 1 { // PT_LOAD
                let p_offset = u64::from_le_bytes(ph[8..16].try_into().unwrap()) as usize;
                let p_vaddr = u64::from_le_bytes(ph[16..24].try_into().unwrap()) as usize;
                let p_filesz = u64::from_le_bytes(ph[32..40].try_into().unwrap()) as usize;
                let p_memsz = u64::from_le_bytes(ph[40..48].try_into().unwrap()) as usize;
                let p_flags = u32::from_le_bytes(ph[4..8].try_into().unwrap());

                // IMPORTANT: During loading, we must be able to write to the pages.
                // We add PTE_W now, and ideally we should set final permissions later.
                let mut perm = PTE_U | PTE_A | PTE_D | PTE_W; // Always add W for loading
                if p_flags & 1 != 0 { perm |= PTE_X; }
                if p_flags & 4 != 0 { perm |= PTE_R; }

                // Map and Load
                let start_va = align_down(p_vaddr);
                let end_va = (p_vaddr + p_memsz + PGSIZE - 1) & !(PGSIZE - 1);
                
                let mut va = start_va;
                while va < end_va {
                    let pa = pmem::alloc(false) as PhysAddr;
                    unsafe { core::ptr::write_bytes(pa as *mut u8, 0, PGSIZE) };
                    vm::mappages(pt, va, pa, PGSIZE, perm);
                    va += PGSIZE;
                }

                let mut read_off = 0;
                while read_off < p_filesz {
                    let chunk = core::cmp::min(p_filesz - read_off, 512);
                    let mut kbuf = [0u8; 512];
                    if inode::inode_read_data(ip, (p_offset + read_off) as u32, chunk as u32, &mut kbuf[..chunk]) != chunk as u32 {
                        crate::printk!("proc_exec: failed to read segment data\n");
                        break;
                    }
                    if let Err(e) = uvm::copyout(pt, p_vaddr + read_off, &kbuf[..chunk]) {
                        crate::printk!("proc_exec: copyout segment failed: {:?}\n", e);
                        return Err(());
                    }
                    read_off += chunk;
                }
                max_va = core::cmp::max(max_va, end_va);
            }
        }
        inode::inode_put(ip);
        crate::syscall::fs::fs_close(self, fd)?;

        // Setup Stack
        let stack_base = 0x20000;
        let stack_top = stack_base + 24576;
        let mut va = stack_top - PGSIZE;
        while va >= stack_base {
            if pt.lookup(va).is_none() {
                let pa = pmem::alloc(false) as PhysAddr;
                unsafe { core::ptr::write_bytes(pa as *mut u8, 0, PGSIZE) };
                vm::mappages(pt, va, pa, PGSIZE, PTE_U | PTE_R | PTE_W | PTE_A | PTE_D);
            }
            va -= PGSIZE;
        }

        // Copy arguments to stack
        let mut sp = stack_top;
        let mut stack_argv = [0usize; 16];
        let old_pt = unsafe { &*(self.root_pt_pa as *const PageTable) };

        for i in (0..u_argv.len()).rev() {
            let mut arg_buf = [0u8; 128];
            let len = uvm::copyin_str(old_pt, &mut arg_buf, u_argv[i]).map_err(|_| {
                crate::printk!("proc_exec: failed to copyin argv[{}]\n", i);
            })?;
            sp -= len;
            sp &= !7; // align 8
            uvm::copyout(pt, sp, &arg_buf[..len]).map_err(|_| {
                crate::printk!("proc_exec: failed to copyout argv[{}] string\n", i);
            })?;
            stack_argv[i] = sp;
        }
        
        // Push argv pointers
        sp -= u_argv.len() * 8;
        sp &= !7;
        for i in 0..u_argv.len() {
            let bytes = stack_argv[i].to_ne_bytes();
            uvm::copyout(pt, sp + i * 8, &bytes).map_err(|_| {
                crate::printk!("proc_exec: failed to copyout argv[{}] pointer\n", i);
            })?;
        }
        let argv_ptr = sp;

        // Commit NEW state
        let old_pt_frame = self.root_pt_frame.take();
        if let Some(mut frame) = old_pt_frame {
            unsafe { &mut *(frame.addr() as *mut PageTable) }.destroy();
        }
        self.root_pt_pa = root_pt_pa;
        self.root_pt_frame = Some(root_pt_frame);
        
        let _old_tf_frame = self.trapframe_frame.take();
        self.trapframe = trapframe_pa as *mut TrapFrame;
        self.trapframe_frame = Some(trapframe_frame);
        self.trapframe_va = trapframe_va;

        self.heap_base = max_va;
        self.heap_top = max_va;
        self.stack_pages = 24576 / PGSIZE;
        self.entry_va = u64::from_le_bytes(elf_header[24..32].try_into().unwrap()) as usize;
        self.user_sp_va = sp;

        // Init trapframe
        let tf = unsafe { &mut *self.trapframe };
        tf.sp = sp;
        tf.kernel_epc = self.entry_va;
        tf.a0 = u_argv.len();
        tf.a1 = argv_ptr;
        tf.kernel_satp = satp::read().bits();
        tf.kernel_hartid = hart::getid();
        tf.kernel_sp = self.kstack.as_ref().unwrap().top();

        let satp_bits = self.root_satp();
        set_current_user_satp(satp_bits);
        unsafe { sscratch::write(self.trapframe_va) };

        crate::printk!("proc_exec: success, entry=0x{:x}\n", self.entry_va);
        Ok(())
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
    // Disable interrupts
    let sstatus_val = sstatus::read();
    let sie_enabled = sstatus_val.sie();
    unsafe { sstatus::clear_sie(); }

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
            runnable_queue::mark_runnable(i);

            if sie_enabled { unsafe { sstatus::set_sie(); } }
            return Some(p);
        }
    }

    if sie_enabled { unsafe { sstatus::set_sie(); } }
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
    let trapframe_frame = PhysFrame::alloc().expect("Failed to alloc trapframe");
    let trapframe_pa = trapframe_frame.addr();
    let trapframe_va = tramp_va - PGSIZE; // trapframe 虚拟地址
    proc.trapframe_va = trapframe_va;
    proc.trapframe = trapframe_pa as *mut TrapFrame;
    proc.trapframe_frame = Some(trapframe_frame);
    vm::mappages(page_table, trapframe_va, trapframe_pa, PGSIZE, PTE_R | PTE_W | PTE_A | PTE_D);
    // Load payload
    proc.exec(payload);
    // Setup Kernel Stack
    let kstack = KernelStack::new(proc.pid);
    let kstack_top = kstack.top();
    proc.kstack = Some(kstack);

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
    tf.kernel_sp = kstack_top;
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
    if proc.pid == 1 {
        proc.context.ra = fs_init_wrapper as usize;
    } else {
        proc.context.ra = trap_user_return as usize;
    }
    // Set kernel stack pointer for context switch
    proc.context.sp = kstack_top;

    // Note: proc.state is already set to Runnable by alloc(), and bitmap is already updated
    // This line is redundant but kept for clarity
    proc.state = ProcState::Runnable;
    proc
}
