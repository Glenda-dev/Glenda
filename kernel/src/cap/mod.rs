// kernel/src/cap/mod.rs

pub mod capability;
pub mod cspace;

pub use capability::Capability;
pub use cspace::CSpace;

use crate::mem::PhysAddr;
use crate::proc::process::Pid;

pub type CapPtr = usize;

/// 能力权限位
pub mod rights {
    pub const READ: u8 = 1 << 0;
    pub const WRITE: u8 = 1 << 1;
    pub const GRANT: u8 = 1 << 2; // 允许传递此 Cap
    pub const CALL: u8 = 1 << 3; // 允许 Invoke
}

/// 内核对象类型
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum CapType {
    Empty,
    /// 未类型化内存，可用于 Retype 生成其他对象
    Untyped {
        start: PhysAddr,
        size: usize,
    },
    /// 线程控制块 (Thread Control Block)，这里暂时关联到 PID
    TCB {
        pid: Pid,
    },
    /// IPC 通信端点
    Endpoint {
        id: usize,
    },
    /// 物理页帧，可映射到地址空间
    Frame {
        start: PhysAddr,
    },
    /// 页表
    PageTable {
        start: PhysAddr,
    },
    /// 中断处理权限
    IrqHandler {
        irq: usize,
    },
    /// CSpace 节点 (用于构建层级 CSpace，这里简化处理)
    CNode {
        start: PhysAddr,
        size: usize,
    },
}
