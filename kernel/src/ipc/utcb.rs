use super::MsgTag;
use crate::cap::CapPtr;
use crate::mem::VirtAddr;

pub const MAX_MRS: usize = 7; // 最大消息寄存器数量

/// 用户线程控制块 (UTCB)
/// 映射到用户地址空间，用于内核与用户态之间的高效数据交换
#[repr(C)]
#[derive(Debug, Clone, Copy)]
pub struct UTCB {
    /// 消息标签 (MR0)
    pub msg_tag: MsgTag,
    /// 消息寄存器 (MR1-MR7) - 对应 CPU 寄存器
    pub mrs_regs: [usize; MAX_MRS],
    /// Capability 传递描述符 (CPTR)
    pub cap_transfer: CapPtr,
    /// 接收窗口描述符 (CNode CPTR + Index)
    pub recv_window: CapPtr,
    /// 线程本地存储指针
    pub tls: VirtAddr,
    pub cursor: usize,
    /// ipc缓冲区大小
    pub buffer_size: usize,
    /// ipc缓冲区
    pub ipc_buffer: [u8; BUFFER_MAX_SIZE],
}

pub const BUFFER_MAX_SIZE: usize = 3 * 1024; // 3KB

impl UTCB {
    pub fn copy_to(&self, dest: &mut UTCB) {
        dest.msg_tag = self.msg_tag;
        dest.mrs_regs = self.mrs_regs;
        dest.cap_transfer = self.cap_transfer;
        dest.recv_window = self.recv_window;
        dest.tls = self.tls;
        dest.buffer_size = self.buffer_size;
        unsafe {
            core::ptr::copy_nonoverlapping(
                &self.ipc_buffer as *const [u8; BUFFER_MAX_SIZE],
                &mut dest.ipc_buffer as *mut [u8; BUFFER_MAX_SIZE],
                self.buffer_size,
            );
        }
    }
    pub fn get_str(&self, offset: usize, len: usize) -> Option<&str> {
        if offset + len > self.buffer_size {
            return None;
        }
        let slice = &self.ipc_buffer[offset..offset + len];
        core::str::from_utf8(slice).ok()
    }
}
