use super::ProcContext;
use crate::cap::{CapType, Capability};
use crate::ipc::{UTCB, UTCB_SIZE};
use crate::mem::{KernelStack, PhysAddr, PhysFrame, VSpace, VirtAddr};
use crate::trap::TrapFrame;
use alloc::collections::VecDeque;
use core::mem::size_of;
use core::sync::atomic::AtomicUsize;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ThreadState {
    Inactive,
    Ready,
    Running,
    BlockedSend,
    BlockedRecv,
}

#[repr(C)]
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
    pub vspace: Option<VSpace>,

    // --- IPC State ---
    pub fault_handler: Option<Capability>, // 异常处理 Endpoint
    pub ipc_buffer: VirtAddr,              // IPC 消息缓冲区 (UTCB的一部分)

    // IPC 等待队列：当此线程处于 BlockedRecv 状态时，
    // 试图向此线程发送消息的其他线程会挂入此队列
    pub send_queue: VecDeque<*mut TCB>,

    // 正在与之通信的目标线程 (用于 Send/Recv 握手)
    pub ipc_partner: Option<*mut TCB>,

    // --- UTCB (User Thread Control Block) ---
    pub utcb_frame: Option<PhysFrame>, // UTCB 所在的物理帧
    pub utcb_base: VirtAddr,           // UTCB 在用户地址空间中的基址
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
            vspace: None,
            fault_handler: None,
            ipc_buffer: VirtAddr::null(),
            send_queue: VecDeque::new(),
            ipc_partner: None,
            utcb_frame: None,
            utcb_base: VirtAddr::null(),
        }
    }

    /// 配置线程的核心资源
    /// 这是 Capability 系统分发 VSpace 和 CSpace 的关键接口
    pub fn configure(
        &mut self,
        cspace: Capability,
        vspace: Capability,
        utcb_frame: PhysFrame,
        utcb_vaddr: VirtAddr,
        fault_ep: Option<Capability>,
    ) {
        self.cspace_root = cspace;
        self.vspace_root = vspace;
        self.utcb_frame = Some(utcb_frame);
        self.fault_handler = fault_ep;
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
        if let Some(utcb_frame) = &self.utcb_frame {
            let vaddr = utcb_frame.va();
            Some(vaddr.as_mut::<UTCB>())
        } else {
            None
        }
    }

    pub fn get_satp(&self) -> Option<VirtAddr> {
        // 假设 Capability 中存储了页表的物理地址
        // 这里需要转换为虚拟地址
        if let CapType::PageTable { paddr, .. } = self.vspace_root.object {
            Some(paddr.to_va())
        } else {
            None
        }
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
