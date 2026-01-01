use super::MsgTag;
use crate::mem::{PGSIZE, VirtAddr};

/// 用户线程控制块 (UTCB)
/// 映射到用户地址空间，用于内核与用户态之间的高效数据交换
#[repr(C)]
#[derive(Debug, Clone, Copy)]
pub struct UTCB {
    /// 消息标签 (MR0)
    pub msg_tag: MsgTag,
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
pub struct IPCBuffer(pub [u8; IPC_BUFFER_SIZE]);

impl IPCBuffer {
    pub fn from_utcb(utcb: &UTCB) -> &mut Self {
        let buf_addr = (utcb as *const UTCB as usize) + UTCB_SIZE;
        unsafe { &mut *(buf_addr as *mut IPCBuffer) }
    }

    pub fn get_str(&self, offset: usize, len: usize) -> Option<&str> {
        if offset.checked_add(len)? > IPC_BUFFER_SIZE {
            return None;
        }
        core::str::from_utf8(&self.0[offset..offset + len]).ok()
    }
}
