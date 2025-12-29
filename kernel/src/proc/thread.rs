use super::ProcContext;
use crate::cap::{CapType, Capability};
use crate::ipc::{UTCB, UTCB_SIZE};
use crate::mem::{KernelStack, PhysAddr, VSpace, VirtAddr};
use crate::trap::TrapFrame;
use core::mem::size_of;
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
    // 线程必须拥有内核栈才能在内核态运行
    // 使用 Option 是为了处理分配失败的情况，但在 Ready 状态前必须为 Some
    pub kstack: Option<KernelStack>,

    // --- Resource Containers (Capabilities) ---
    pub cspace_root: Capability, // Root CNode (CSpace)
    pub vspace_root: Capability, // Root PageTable (VSpace)

    // 地址空间
    pub vspace: VSpace,

    // --- IPC State ---
    pub fault_handler: Option<Capability>, // 异常处理 Endpoint
    pub ipc_buffer: VirtAddr,              // IPC 消息缓冲区 (UTCB的一部分)

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
    pub utcb_base: VirtAddr,            // UTCB 在用户地址空间中的基址

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
            cspace_root: Capability::empty(),
            vspace_root: Capability::empty(),
            vspace: VSpace::empty(),
            fault_handler: None,
            ipc_buffer: VirtAddr::null(),
            send_queue_head: None,
            send_queue_tail: None,
            prev: None,
            next: None,
            ipc_partner: None,
            ipc_badge: 0,
            ipc_cap: None,
            utcb_frame: None,
            utcb_base: VirtAddr::null(),
            privileged: false,
        }
    }

    /// 创建一个内核线程
    /// 内核线程运行在 S-Mode，共享内核地址空间
    pub fn new_kthread(entry: usize) -> Self {
        let mut tcb = Self::new();
        tcb.privileged = true;
        tcb.kstack = Some(KernelStack::alloc().expect("Failed to alloc kstack for kernel thread"));

        // 设置上下文以跳转到入口函数
        tcb.context.ra = entry;
        tcb.context.sp = tcb.kstack.as_ref().unwrap().top().as_usize();
        // s0 (fp) 设为 0，方便调试回溯终止
        tcb.context.s0 = 0;

        tcb
    }

    /// 配置线程的核心资源
    /// 这是 Capability 系统分发 VSpace 和 CSpace 的关键接口
    pub fn configure(
        &mut self,
        cspace: &Capability,
        vspace: &Capability,
        utcb_frame: Option<&Capability>,
        utcb_vaddr: VirtAddr,
        fault_ep: Option<&Capability>,
    ) {
        self.cspace_root = cspace.clone();
        self.vspace_root = vspace.clone();
        // 初始化 VSpace 对象
        self.vspace.configure(vspace);
        self.utcb_frame = utcb_frame.cloned();
        self.fault_handler = fault_ep.cloned();
        self.utcb_base = utcb_vaddr;
        // UTCB 通常包含 IPC Buffer
        self.ipc_buffer = utcb_vaddr + UTCB_SIZE;
    }

    pub fn set_priority(&mut self, prio: u8) {
        self.priority = prio;
    }

    pub fn set_registers(&mut self, ra: usize, sp: usize) {
        self.context.ra = ra;
        self.context.sp = sp;
    }

    pub fn resume(&mut self) {
        if self.state == ThreadState::Inactive {
            self.state = ThreadState::Ready;
        }
    }

    pub fn suspend(&mut self) {
        self.state = ThreadState::Inactive;
    }

    /// 获取当前线程的 TrapFrame (用户态上下文)
    /// TrapFrame 总是位于内核栈的顶部
    pub fn get_trapframe(&self) -> Option<&mut TrapFrame> {
        if self.privileged {
            return None;
        }
        if let Some(kstack) = &self.kstack {
            let top = kstack.top();
            // TrapFrame 位于栈顶下方
            let tf_addr = top - size_of::<TrapFrame>();
            Some(tf_addr.as_mut::<TrapFrame>())
        } else {
            None
        }
    }

    pub fn get_trapframe_va(&self) -> Option<VirtAddr> {
        if let Some(kstack) = &self.kstack {
            let top = kstack.top();
            // TrapFrame 位于栈顶下方
            let tf_addr = top - size_of::<TrapFrame>();
            Some(tf_addr)
        } else {
            None
        }
    }

    pub fn get_utcb(&self) -> Option<&mut UTCB> {
        if let Some(utcb_cap) = &self.utcb_frame {
            let vaddr = utcb_cap.obj_ptr();
            Some(vaddr.as_mut::<UTCB>())
        } else {
            None
        }
    }

    pub fn get_satp(&self) -> usize {
        self.vspace.get_satp()
    }

    pub fn cap_lookup(&self, cptr: usize) -> Option<Capability> {
        self.cap_lookup_slot(cptr).map(|(cap, _)| cap)
    }

    pub fn cap_lookup_slot(&self, cptr: usize) -> Option<(Capability, PhysAddr)> {
        // 1. 获取 Root CNode
        if let CapType::CNode { paddr, bits } = self.cspace_root.object {
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
