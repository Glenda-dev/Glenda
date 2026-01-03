use super::ProcContext;
use crate::cap::{CapType, Capability};
use crate::hart;
use crate::ipc::UTCB;
use crate::mem::pmem;
use crate::mem::{PGSIZE, PageTable, PhysAddr, VirtAddr};
use crate::trap::TrapFrame;
use crate::trap::user::trap_user_return;
use core::sync::atomic::AtomicUsize;
use riscv::register::{satp, sscratch};

pub const KSTACK_PAGES: usize = 4; // 16KB

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
    pub ipc_badge: usize,
    pub ipc_cap: Option<Capability>,

    // --- UTCB (User Thread Control Block) ---
    pub utcb_frame: Option<Capability>, // UTCB 所在的物理帧 (以 Capability 形式存储)

    // Priveleged Thread Indicator
    pub privileged: bool, // 是否为内核线程
}

impl TCB {
    pub const fn new() -> Self {
        Self {
            ref_count: AtomicUsize::new(1),
            context: ProcContext::new(),
            priority: 0,
            timeslice: 0,
            state: ThreadState::Inactive,
            affinity: 0,
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
            ipc_badge: 0,
            ipc_cap: None,
            utcb_frame: None,
            privileged: false,
        }
    }

    pub fn get_kstack_top(&self) -> VirtAddr {
        self.kstack.as_ref().expect("Kernel stack not configured").obj_ptr()
            + (KSTACK_PAGES * PGSIZE)
    }

    pub fn get_tf(&mut self) -> &mut TrapFrame {
        let tf_cap = self.trapframe.as_ref().expect("TrapFrame not configured");
        tf_cap.obj_ptr().as_mut::<TrapFrame>()
    }

    pub fn get_tf_va(&self) -> VirtAddr {
        let tf_cap = self.trapframe.as_ref().expect("TrapFrame not configured");
        tf_cap.obj_ptr()
    }

    pub fn get_pt(&self) -> &PageTable {
        let vspace_cap = self.vspace_root.as_ref().expect("VSpace root not configured");
        vspace_cap.obj_ptr().as_ref::<PageTable>()
    }

    pub fn get_satp(&self) -> usize {
        let pt_addr = self
            .vspace_root
            .as_ref()
            .expect("VSpace root not configured")
            .obj_ptr()
            .to_pa()
            .as_usize();
        let ppn = pt_addr >> 12;
        (8 << 60) | ppn // Sv39 mode
    }

    /// 创建一个内核线程
    /// 内核线程运行在 S-Mode，共享内核地址空间
    pub fn new_kthread(entry: usize) -> Self {
        let mut tcb = Self::new();
        tcb.privileged = true;
        tcb.kstack = pmem::alloc_frame_cap(KSTACK_PAGES);

        // 设置上下文以跳转到入口函数
        tcb.context.ra = entry;
        tcb.context.sp = tcb.get_kstack_top().as_usize();
        // s0 (fp) 设为 0，方便调试回溯终止
        tcb.context.s0 = 0;
        unimplemented!()
    }

    /// 配置线程的核心资源
    /// 这是 Capability 系统分发 VSpace 和 CSpace 的关键接口
    pub fn configure(
        &mut self,
        cspace: Option<&Capability>,
        vspace: Option<&Capability>,
        utcb_frame: Option<&Capability>,
        trapframe: Option<&Capability>,
        kstack: Option<&Capability>,
    ) {
        if !cspace.is_none() {
            self.cspace_root = cspace.cloned();
        }
        if !vspace.is_none() {
            self.vspace_root = vspace.cloned();
        }
        if !utcb_frame.is_none() {
            self.utcb_frame = utcb_frame.cloned();
        }
        if !trapframe.is_none() {
            self.trapframe = trapframe.cloned();
        }
        if !kstack.is_none() {
            self.kstack = kstack.cloned();
        }
    }

    pub fn set_priority(&mut self, prio: u8) {
        self.priority = prio;
    }

    pub fn set_registers(&mut self, entry_point: usize, stack_top: usize) {
        // 1. 获取内核栈顶
        let kstack_top = self.get_kstack_top().as_usize();

        // 2. 获取 TrapFrame
        let tf = self.get_tf();
        let tf_va = tf as *mut TrapFrame as usize;

        // 3. 设置用户态初始状态
        tf.sp = stack_top; // 用户栈顶
        tf.kernel_epc = entry_point; // sepc
        tf.kernel_satp = satp::read().bits(); // sstatus
        tf.kernel_hartid = hart::get().id; // hartid
        tf.kernel_sp = kstack_top; // 内核栈顶
        tf.a0 = tf_va; // sscratch 指向 TrapFrame 的虚拟地址
        unsafe {
            sscratch::write(tf_va);
        }

        // 4. 设置内核上下文，使其在被调度时跳转到 trap_return_wrapper
        // 由于 switch_context 只恢复 Callee-Saved 寄存器，无法直接传递参数 a0
        // 我们利用 s0 寄存器来传递 TrapFrame 指针，并使用一个 wrapper 函数
        self.context.ra = trap_user_return as usize;
        self.context.sp = kstack_top;
    }

    pub fn set_fault_handler(&mut self, ep: Capability) {
        self.fault_handler = Some(ep);
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
            Some(vaddr.as_mut::<UTCB>())
        } else {
            None
        }
    }

    pub fn cap_lookup(&self, cptr: usize) -> Option<Capability> {
        self.cap_lookup_slot(cptr).map(|(cap, _)| cap)
    }

    pub fn cap_lookup_slot(&self, cptr: usize) -> Option<(Capability, PhysAddr)> {
        if cptr == 0 {
            return None;
        }
        // 1. 获取 Root CNode
        if let CapType::CNode { paddr, bits } =
            self.cspace_root.as_ref().expect("CSpace root not configured").object
        {
            let cnode = crate::cap::CNode::from_addr(paddr, bits);
            // 2. 在 CNode 中查找
            cnode.lookup_cap(cptr).map(|cap| (cap, cnode.get_slot_addr(cptr)))
        } else {
            None
        }
    }
}

unsafe impl Send for TCB {}
unsafe impl Sync for TCB {}
