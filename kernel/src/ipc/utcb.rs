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
    /// 缓冲区起始偏移 (用于消耗读取)
    pub head: usize,
    /// 缓冲区数据总长度
    pub size: usize,
    pub ipc_buffer: [u8; BUFFER_MAX_SIZE],
}

pub const BUFFER_MAX_SIZE: usize = 3 * 1024; // 3KB

impl UTCB {
    pub fn copy_to(&mut self, dest: &mut UTCB) {
        // 只复制消息相关的字段
        dest.msg_tag = self.msg_tag;
        dest.mrs_regs = self.mrs_regs;
        dest.head = self.head;

        if self.msg_tag.flags().contains(MsgFlags::HAS_BUFFER) {
            let len = core::cmp::min(self.size, BUFFER_MAX_SIZE);
            dest.ipc_buffer[..len].copy_from_slice(&self.ipc_buffer[..len]);
            dest.size = len;
        }
    }

    pub fn available_data(&self) -> usize {
        self.size - self.head
    }

    pub fn available_space(&self) -> usize {
        BUFFER_MAX_SIZE - self.size
    }

    pub fn write(&mut self, data: &[u8]) -> usize {
        let to_write = core::cmp::min(BUFFER_MAX_SIZE, data.len());
        self.ipc_buffer[..to_write].copy_from_slice(&data[..to_write]);
        self.size = to_write;
        self.head = 0;
        to_write
    }

    pub fn append(&mut self, data: &[u8]) -> usize {
        let space = self.available_space();
        let to_write = core::cmp::min(space, data.len());
        self.ipc_buffer[self.size..self.size + to_write].copy_from_slice(&data[..to_write]);
        self.size += to_write;
        to_write
    }

    pub fn read(&mut self, buf: &mut [u8]) -> usize {
        let data_len = self.available_data();
        let to_read = core::cmp::min(data_len, buf.len());
        if to_read > 0 {
            buf[..to_read].copy_from_slice(&self.ipc_buffer[self.head..self.head + to_read]);
            self.head += to_read;
        }
        to_read
    }

    pub fn write_byte(&mut self, byte: u8) -> bool {
        if self.available_space() > 0 {
            self.ipc_buffer[self.size] = byte;
            self.size += 1;
            true
        } else {
            false
        }
    }
}
