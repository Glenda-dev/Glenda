use super::ProcContext;
use crate::cap::Capability;
use crate::irq::TrapFrame;
use crate::mem::PGSIZE;
use crate::mem::{KernelStack, PhysFrame, VSpace, VirtAddr};
use alloc::collections::VecDeque;

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
            ipc_buffer: 0,
            send_queue: VecDeque::new(),
            ipc_partner: None,
            utcb_frame: None,
            utcb_base: 0,
        }
    }

    pub fn alloc() -> Option<Self> {
        let mut tcb = TCB::new();

        // 分配内核栈
        if let Some(kstack) = KernelStack::alloc() {
            tcb.kstack = Some(kstack);
            Some(tcb)
        } else {
            None
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
            unsafe { Some(&mut *(tf_addr as *mut TrapFrame)) }
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
            let vaddr = self.utcb_base;
            unsafe { Some(&mut *(vaddr as *mut UTCB)) }
        } else {
            None
        }
    }

    pub fn get_satp(&self) -> VirtAddr {
        // 假设 Capability 中存储了页表的物理地址
        // 这里需要转换为虚拟地址
        unimplemented!()
    }

    pub fn cap_lookup(&self, _cptr: usize) -> Option<Capability> {
        // TODO: 实现 Capability 查找逻辑
        unimplemented!()
    }
}

unsafe impl Send for TCB {}
unsafe impl Sync for TCB {}

pub const UTCB_VA: VirtAddr = 0x8000_0000;

/// 用户线程控制块 (UTCB)
/// 映射到用户地址空间，用于内核与用户态之间的高效数据交换
#[repr(C)]
#[derive(Debug, Clone, Copy)]
pub struct UTCB {
    /// 消息标签 (MR0)
    pub msg_tag: usize,
    /// 消息寄存器 (MR1-MR7) - 对应 CPU 寄存器
    pub mrs_regs: [usize; 7],

    /// Capability 传递描述符 (CPTR)
    pub cap_transfer: usize,
    /// 接收窗口描述符 (CNode CPTR + Index)
    pub recv_window: usize,

    /// 线程本地存储指针
    pub tls: VirtAddr,

    /// ipc缓冲区大小
    pub ipc_buffer_size: usize,
}

pub const UTCB_SIZE: usize = core::mem::size_of::<UTCB>();

pub const IPC_BUFFER_SIZE: usize = PGSIZE - UTCB_SIZE;

#[repr(C)]
pub struct IPCBuffer {
    pub data: [u8; IPC_BUFFER_SIZE],
}

impl IPCBuffer {
    pub fn from_utcb(utcb: &UTCB) -> &mut Self {
        let buf_addr = (utcb as *const UTCB as usize) + UTCB_SIZE;
        unsafe { &mut *(buf_addr as *mut IPCBuffer) }
    }
}
