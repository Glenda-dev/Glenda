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
    pub head: usize,
    pub tail: usize,
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
        dest.head = self.head;
        dest.tail = self.tail;
        unsafe {
            core::ptr::copy_nonoverlapping(
                &self.ipc_buffer as *const [u8; BUFFER_MAX_SIZE],
                &mut dest.ipc_buffer as *mut [u8; BUFFER_MAX_SIZE],
                BUFFER_MAX_SIZE,
            );
        }
    }

    pub fn available_data(&self) -> usize {
        if self.tail >= self.head {
            self.tail - self.head
        } else {
            BUFFER_MAX_SIZE - self.head + self.tail
        }
    }

    pub fn available_space(&self) -> usize {
        BUFFER_MAX_SIZE - self.available_data() - 1
    }

    pub fn read_bytes(&mut self, data: &mut [u8]) -> usize {
        let len = core::cmp::min(data.len(), self.available_data());
        for i in 0..len {
            data[i] = self.ipc_buffer[self.head];
            self.head = (self.head + 1) % BUFFER_MAX_SIZE;
        }
        len
    }

    /// 从指定偏移量读取字符串，处理环形缓冲区绕回
    pub fn with_str<F, R>(&self, offset: usize, len: usize, f: F) -> Option<R>
    where
        F: FnOnce(&str) -> R,
    {
        if len > BUFFER_MAX_SIZE || offset >= BUFFER_MAX_SIZE {
            return None;
        }

        if offset + len <= BUFFER_MAX_SIZE {
            // 连续内存
            let slice = &self.ipc_buffer[offset..offset + len];
            core::str::from_utf8(slice).ok().map(f)
        } else {
            // 绕回内存，需要临时缓冲区
            let mut buf = [0u8; 512];
            let actual_len = core::cmp::min(len, buf.len());
            let part1_len = BUFFER_MAX_SIZE - offset;
            let part2_len = actual_len - part1_len;
            buf[..part1_len].copy_from_slice(&self.ipc_buffer[offset..]);
            buf[part1_len..actual_len].copy_from_slice(&self.ipc_buffer[..part2_len]);
            core::str::from_utf8(&buf[..actual_len]).ok().map(f)
        }
    }
}
