use super::asid;
use crate::cap::{Badge, CNode, CapPtr, CapType, Capability, Slot};
use crate::hal;
use crate::hal::mem::{KSTACK_PAGES, PGSIZE};
use crate::hal::proc::ProcContext;
use crate::hal::trap::TrapFrame;
use crate::hal::trap::{trap_user_handler, trap_user_return};
use crate::ipc::{MsgArgs, UTCB};
use crate::mem::PageTable;
use crate::mem::VirtAddr;
use crate::mem::pmem;
use core::fmt::Display;
use core::sync::atomic::AtomicUsize;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ThreadState {
    Inactive,
    Ready,
    Running,
    BlockedSend,
    BlockedRecv,
    BlockedCall,
}

#[repr(C)]
#[derive(Debug)]
pub struct TCB {
    /// 引用计数
    pub ref_count: AtomicUsize,

    // --- Core Execution State ---
    pub context: ProcContext, // 架构相关寄存器 (IP, SP, etc.)
    pub priority: u8,         // 调度优先级 (0-255)
    pub timeslice: usize,     // 剩余时间片
    pub state: ThreadState,   // 当前状态
    pub affinity: usize,      // CPU 亲和性

    // --- Kernel Stack ---
    pub kstack: Option<Capability>, // 内核栈的物理帧 (以 Capability 形式存储)

    // TrapFrame
    pub trapframe: Option<Capability>, // 用户态上下文所在的物理帧 (以 Capability 形式存储)

    // --- Resource Containers (Capabilities) ---
    pub cspace_root: Option<Capability>, // Root CNode (CSpace)
    pub vspace_root: Option<Capability>, // Root PageTable (VSpace)

    // --- IPC State ---
    pub fault_handler: Option<Capability>, // 异常处理 Endpoint

    // IPC 等待队列：当此线程处于 BlockedRecv 状态时，
    // 试图向此线程发送消息的其他线程会挂入此队列
    pub send_queue_head: Option<*mut TCB>,
    pub send_queue_tail: Option<*mut TCB>,

    // Intrusive list node (for Ready Queue or other's Send Queue)
    pub prev: Option<*mut TCB>,
    pub next: Option<*mut TCB>,

    // 正在与之通信的目标线程 (用于 Send/Recv 握手)
    pub ipc_partner: Option<*mut TCB>,

    // IPC state when blocked
    pub ipc_badge: Badge,
    pub ipc_cap: Option<Capability>,

    // --- UTCB (User Thread Control Block) ---
    pub utcb_frame: Option<Capability>, // UTCB 所在的物理帧 (以 Capability 形式存储)

    // Priveleged Thread Indicator
    pub privileged: bool, // 是否为内核线程

    // 是否为原生线程
    pub native: bool,

    // Global thread list for debugging
    pub global_prev: Option<*mut TCB>,
    pub global_next: Option<*mut TCB>,
}

pub static mut ALL_THREADS: Option<*mut TCB> = None;

impl TCB {
    pub const fn new() -> Self {
        Self {
            ref_count: AtomicUsize::new(1),
            context: ProcContext::new(),
            priority: 0,
            timeslice: 0,
            state: ThreadState::Inactive,
            affinity: usize::MAX,
            kstack: None,
            trapframe: None,
            cspace_root: None,
            vspace_root: None,
            fault_handler: None,
            send_queue_head: None,
            send_queue_tail: None,
            prev: None,
            next: None,
            ipc_partner: None,
            ipc_badge: Badge::null(),
            ipc_cap: None,
            utcb_frame: None,
            privileged: false,
            native: true,
            global_prev: None,
            global_next: None,
        }
    }

    pub fn register(tcb: *mut TCB) {
        unsafe {
            if let Some(head) = ALL_THREADS {
                (*head).global_prev = Some(tcb);
                (*tcb).global_next = Some(head);
            }
            ALL_THREADS = Some(tcb);
        }
    }

    pub fn get_kstack_top(&self) -> VirtAddr {
        self.kstack.as_ref().expect("Kernel stack not configured").obj_ptr()
            + (KSTACK_PAGES * PGSIZE)
    }

    pub fn get_tf(&mut self) -> &mut TrapFrame {
        let tf_cap = self.trapframe.as_ref().expect("TrapFrame not configured");
        unsafe { tf_cap.obj_ptr().as_mut::<TrapFrame>() }
    }

    pub fn get_tf_va(&self) -> VirtAddr {
        let tf_cap = self.trapframe.as_ref().expect("TrapFrame not configured");
        tf_cap.obj_ptr()
    }

    pub fn get_pt(&self) -> &PageTable {
        let vspace_cap = self.vspace_root.as_ref().expect("VSpace root not configured");
        unsafe { vspace_cap.obj_ptr().as_ref::<PageTable>() }
    }

    pub fn get_cspace(&self) -> &CNode {
        let cspace_cap = self.cspace_root.as_ref().expect("CSpace root not configured");
        unsafe { cspace_cap.obj_ptr().as_ref::<CNode>() }
    }

    pub fn mmu_register(&self) -> usize {
        let (paddr, mut id) =
            self.vspace_root.as_ref().expect("VSpace root not configured").vspace_info();
        if asid::check(id) {
            id = asid::alloc();
        }
        hal::mem::get_mmu_register(paddr, id.id as usize)
    }

    /// 创建一个内核线程
    /// 内核线程运行在 S-Mode，共享内核地址空间
    pub fn new_kthread(entry: usize) -> Self {
        let mut tcb = Self::new();
        tcb.privileged = true;
        tcb.kstack = pmem::alloc_frame_cap(KSTACK_PAGES);
        let sp = tcb.get_kstack_top().as_usize();
        // 设置上下文以跳转到入口函数
        tcb.context.configure(entry, sp);
        // s0 (fp) 设为 0，方便调试回溯终止
        tcb.context.set_fp(0);
        tcb.native = true;
        unimplemented!();
    }

    /// 配置线程的核心资源
    /// 这是 Capability 系统分发 VSpace 和 CSpace 的关键接口
    pub fn configure(
        &mut self,
        cspace: &Capability,
        vspace: &Capability,
        utcb_frame: &Capability,
        trapframe: &Capability,
        kstack: &Capability,
    ) {
        self.cspace_root = Some(cspace.clone());
        self.vspace_root = Some(vspace.clone());
        self.utcb_frame = Some(utcb_frame.clone());
        self.trapframe = Some(trapframe.clone());
        self.kstack = Some(kstack.clone());
    }

    pub fn set_priority(&mut self, prio: u8) {
        self.priority = prio;
    }

    pub fn set_entrypoint(&mut self, entry_point: usize, stack_top: usize, thread_pointer: usize) {
        // 1. 获取内核栈顶和 mmu
        let kstack_top = self.get_kstack_top().as_usize();
        let mmu = self.mmu_register();

        // 2. 获取 TrapFrame
        let tf = self.get_tf();

        // 3. 设置用户态初始状态
        tf.configure(entry_point, stack_top, thread_pointer);
        tf.configure_kernel(
            mmu,
            hal::cpu::cpu_id(),
            kstack_top,
            trap_user_handler as *const () as usize,
        );

        // 4. 设置内核上下文，使其在被调度时跳转到 trap_user_return
        let ra = trap_user_return as *const () as usize;
        self.context.configure(ra, kstack_top);
    }

    pub fn set_fault_handler(&mut self, ep: Capability, native: bool) {
        self.fault_handler = Some(ep);
        self.native = native;
    }

    pub fn set_affinity(&mut self, hart_id: usize) {
        self.affinity = hart_id;
    }

    pub fn set_registers(&mut self, regs: &MsgArgs) {
        let tf = self.get_tf();
        tf.set_registers(regs);
    }

    pub fn resume(&mut self) {
        if self.state == ThreadState::Inactive {
            self.state = ThreadState::Ready;
        }
    }

    pub fn suspend(&mut self) {
        self.state = ThreadState::Inactive;
    }

    pub fn get_utcb(&self) -> Option<&mut UTCB> {
        if let Some(utcb_cap) = &self.utcb_frame {
            let vaddr = utcb_cap.obj_ptr();
            Some(unsafe { vaddr.as_mut::<UTCB>() })
        } else {
            None
        }
    }

    pub fn get_cspace_mut(&self) -> Option<&mut CNode> {
        let root_cap = self.cspace_root.as_ref()?;
        if root_cap.cap_type() == CapType::CNode {
            unsafe { Some(root_cap.obj_ptr().as_mut::<CNode>()) }
        } else {
            None
        }
    }

    pub fn cap_lookup(&self, cptr: CapPtr) -> Option<Capability> {
        // 1. 获取 Root CNode
        let root_cap = self.cspace_root.as_ref().expect("CSpace root not configured");
        if root_cap.cap_type() == CapType::CNode {
            let cnode = unsafe { root_cap.obj_ptr().as_mut::<CNode>() };
            // 2. 在 CNode 中查找
            cnode.lookup(cptr)
        } else {
            None
        }
    }

    pub fn lookup_slot(&self, cptr: CapPtr) -> Option<*mut Slot> {
        let root_cap = self.cspace_root.as_ref().expect("CSpace root not configured");
        if root_cap.cap_type() == CapType::CNode {
            let cnode = unsafe { root_cap.obj_ptr().as_mut::<CNode>() };
            unsafe { cnode.lookup_slot_ptr(cptr) }
        } else {
            None
        }
    }

    pub fn debug_print(&self) {
        // use crate::printk;
        printk!("TCB @ {:#x}:\n", self as *const _ as usize);
        printk!("  state:          {:?}\n", self.state);
        printk!("  priority:       {}\n", self.priority);
        printk!("  affinity:       {}\n", self.affinity);
        printk!("  timeslice:      {}\n", self.timeslice);
        printk!("  context:        {:?}\n", self.context);

        if let Some(cap) = &self.cspace_root {
            printk!("  cspace_root:    {}\n", cap);
        }
        if let Some(cap) = &self.vspace_root {
            printk!("  vspace_root:    {}\n", cap);
        }
        if let Some(cap) = &self.utcb_frame {
            printk!("  utcb_frame:     {}\n", cap);
        }
        if let Some(cap) = &self.trapframe {
            printk!("  trapframe:      {}\n", cap);
        }
        if let Some(cap) = &self.kstack {
            printk!("  kstack:         {}\n", cap);
        }

        if let Some(handler) = &self.fault_handler {
            printk!("  fault_handler:  {}\n", handler);
        }

        if let Some(partner) = self.ipc_partner {
            printk!("  ipc_partner:    {:#x}\n", partner as usize);
        }

        printk!("  privileged:     {}\n", self.privileged);
        printk!("  native:         {}\n", self.native);
    }
}

unsafe impl Send for TCB {}
unsafe impl Sync for TCB {}

impl Display for TCB {
    fn fmt(&self, f: &mut core::fmt::Formatter<'_>) -> core::fmt::Result {
        write!(
            f,
            "TCB {{ state: {:?}, priority: {}, affinity: {} }}",
            self.state, self.priority, self.affinity
        )
    }
}
