use super::{Badge, MsgTag};
use crate::cap::CapPtr;
use crate::ipc::MsgFlags;

pub const MAX_MRS: usize = 8; // 最大消息寄存器数量

pub type MsgArgs = [usize; MAX_MRS];

/// 用户线程控制块 (UTCB)
/// 映射到用户地址空间，用于内核与用户态之间的高效数据交换
#[repr(C)]
#[derive(Clone, Copy)]
pub struct UTCB {
    /// 消息标签 (MR0)
    pub msg_tag: MsgTag,
    /// 消息寄存器 (MR1-MR7) - 对应 CPU 寄存器
    pub mrs_regs: MsgArgs,
    /// Capability 传递描述符 (CPTR)
    pub cap_transfer: CapPtr,
    /// 接收窗口描述符 (CNode CPTR)
    pub recv_window: CapPtr,
    /// Badge 标识
    pub badge: Badge,
    /// ipc缓冲区
    pub size: usize,
    pub ipc_buffer: [u8; BUFFER_MAX_SIZE],
}

pub const BUFFER_MAX_SIZE: usize = 3 * 1024; // 3KB

impl UTCB {
    pub fn copy_to(&mut self, dest: &mut UTCB) {
        // 只复制消息相关的字段
        dest.msg_tag = self.msg_tag;
        dest.mrs_regs = self.mrs_regs;

        if self.msg_tag.flags().contains(MsgFlags::HAS_BUFFER) {
            let len = core::cmp::min(self.size, BUFFER_MAX_SIZE);
            dest.ipc_buffer[..len].copy_from_slice(&self.ipc_buffer[..len]);
            dest.size = len;
        }
    }

    pub fn available_data(&self) -> usize {
        self.size
    }

    pub fn available_space(&self) -> usize {
        BUFFER_MAX_SIZE - self.available_data()
    }
}
