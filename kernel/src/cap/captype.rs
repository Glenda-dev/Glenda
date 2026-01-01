use crate::mem::{PhysAddr, VirtAddr};

/// 内核对象类型
/// 这里存储的是对象的“身份信息”，通常是物理地址或内核虚拟地址
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum CapType {
    Empty,

    /// 未类型化内存 (Untyped Memory)
    /// 系统启动时，所有空闲物理内存都以此形式存在
    /// 可通过 Retype 操作分裂或转换为其他内核对象
    Untyped {
        start_paddr: PhysAddr,
        size: usize,
    },

    /// 线程控制块 (TCB)
    /// 指向内核空间中实际的 TCB 对象
    /// 拥有此 Cap 可以操作线程 (Suspend, Resume, SetRegs)
    Thread {
        tcb_ptr: VirtAddr, // TCB 在内核堆中的虚拟地址
    },

    /// IPC 通信端点 (Endpoint)
    /// 指向内核空间中的 Endpoint 对象
    /// 用于线程间同步 IPC (Send/Recv)
    Endpoint {
        ep_ptr: VirtAddr, // Endpoint 在内核堆中的虚拟地址
    },

    /// 回复对象 (ReplyObject)
    /// 这是一种特殊的 Cap，通常在 Recv 成功后由内核临时授予
    /// 指向正在等待回复的发送方 TCB
    /// 只能使用一次 (One-shot)
    Reply {
        tcb_ptr: VirtAddr, // 等待回复的线程 TCB 地址
    },

    /// 物理页帧 (Frame)
    /// 代表一块物理内存，可以被 Map 到 VSpace 中
    Frame {
        paddr: PhysAddr,
    },

    /// 页表对象 (PageTable)
    /// 代表一个页表节点，可以作为 VSpace 的根或中间节点
    PageTable {
        paddr: PhysAddr, // 页表的物理基地址 (用于写入 satp 或 PTE)
        level: usize,    // 页表层级 (例如 RISC-V 的 0, 1, 2)
    },

    /// 能力节点 (CNode)
    /// CSpace 的组成部分，本质上是一个 Capability 数组
    CNode {
        paddr: PhysAddr, // CNode 占用的物理页地址
        bits: u8,        // CNode 大小 = 2^bits 个 Slot
    },

    /// 中断处理权限
    /// 允许用户态驱动程序 Ack 中断或绑定 Notification
    IrqHandler {
        irq: usize,
    },

    /// 控制台 (Console)
    /// 允许向内核控制台输出日志
    Console,
}

pub mod types {
    pub const CNODE: usize = 1;
    pub const TCB: usize = 2;
    pub const ENDPOINT: usize = 3;
    pub const FRAME: usize = 4;
    pub const PAGETABLE: usize = 5;
}

impl CapType {
    /// 判断 Cap 是否指向有效的内核对象
    pub fn is_valid(&self) -> bool {
        !matches!(self, CapType::Empty)
    }

    /// 判断是否为可调度的对象 (TCB)
    pub fn is_schedulable(&self) -> bool {
        matches!(self, CapType::Thread { .. })
    }

    /// 判断是否为 IPC 端点
    pub fn is_ipc_endpoint(&self) -> bool {
        matches!(self, CapType::Endpoint { .. })
    }

    /// 判断是否为物理内存帧
    pub fn is_frame(&self) -> bool {
        matches!(self, CapType::Frame { .. })
    }

    /// 判断是否为页表对象
    pub fn is_pagetable(&self) -> bool {
        matches!(self, CapType::PageTable { .. })
    }

    /// 判断是否为 CNode 对象
    pub fn is_cnode(&self) -> bool {
        matches!(self, CapType::CNode { .. })
    }

    /// 判断是否为未类型化内存
    pub fn is_untyped(&self) -> bool {
        matches!(self, CapType::Untyped { .. })
    }
}
