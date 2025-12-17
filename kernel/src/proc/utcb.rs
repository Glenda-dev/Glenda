use crate::mem::VirtAddr;

/// 用户线程控制块 (UTCB)
/// 映射到用户地址空间，用于内核与用户态之间的高效数据交换
#[repr(C)]
#[derive(Debug, Clone, Copy)]
pub struct Utcb {
    /// 消息标签 (MR0) - 通常在寄存器中，但这里保留位置
    pub msg_tag: usize,
    /// 消息寄存器 (MR1-MR7) - 通常在寄存器中
    pub mrs_regs: [usize; 7],
    /// 扩展消息寄存器 (MR8-MR63) - 用于传递超过寄存器数量的数据
    pub mrs: [usize; 56],
    
    /// Capability 传递描述符 (CPTR)
    pub cap_transfer: usize,
    /// 接收窗口描述符 (CNode CPTR + Index)
    pub recv_window: usize,
    
    /// 线程本地存储指针
    pub tls: VirtAddr,
}

impl Utcb {
    pub const fn new() -> Self {
        Self {
            msg_tag: 0,
            mrs_regs: [0; 7],
            mrs: [0; 56],
            cap_transfer: 0,
            recv_window: 0,
            tls: 0,
        }
    }
}
