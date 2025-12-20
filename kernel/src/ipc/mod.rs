pub mod endpoint;
pub mod message;
pub mod utcb;

pub use message::{MsgTag, label};
pub use utcb::{IPCBuffer, UTCB, UTCB_SIZE, UTCB_VA};

use crate::mem::addr;
use crate::proc::scheduler;
use crate::proc::thread::{TCB, ThreadState};

pub use endpoint::Endpoint;

/// 执行消息拷贝 (Sender UTCB -> Receiver UTCB)
/// 同时传递 Badge 到接收者的上下文，并可选地传递一个 Capability
unsafe fn copy_msg(
    sender: &TCB,
    receiver: &mut TCB,
    badge: usize,
    cap: Option<crate::cap::Capability>,
) {
    let src = sender.get_utcb().expect("ipc: Sender has no UTCB");
    let dst = receiver.get_utcb().expect("ipc: Receiver has no UTCB");

    // 1. 拷贝消息头和寄存器
    dst.msg_tag = src.msg_tag;
    dst.mrs_regs = src.mrs_regs;
    dst.ipc_buffer_size = src.ipc_buffer_size;

    // 拷贝 IPC 缓冲区内容
    if dst.ipc_buffer_size > 0 {
        let src_buf_ptr = (sender.ipc_buffer) as *const u8;
        let dst_buf_ptr = (receiver.ipc_buffer) as *mut u8;
        unsafe {
            core::ptr::copy_nonoverlapping(src_buf_ptr, dst_buf_ptr, dst.ipc_buffer_size);
        }
    }

    // 2. 传递 Badge
    receiver.get_trapframe().expect("ipc: Receiver has no TrapFrame").t0 = badge;

    // 3. 传递 Capability (如果提供且接收者准备好了接收窗口)
    if let Some(c) = cap {
        let recv_window = dst.recv_window;
        if recv_window != 0 {
            if let Some((_, slot_addr)) = receiver.cap_lookup_slot(recv_window) {
                let slot = unsafe { &mut *(slot_addr as *mut crate::cap::cnode::Slot) };
                slot.cap = c;
            }
        }
    }
}

/// 发送操作 (sys_send)
///
/// * `current`: 当前正在执行的线程 (发送者)
/// * `ep`: 目标 Endpoint 对象
/// * `badge`: 发送 Capability 携带的身份标识
/// * `cap`: 可选的要传递的能力
pub fn send(
    current: &mut TCB,
    ep: &mut Endpoint,
    badge: usize,
    cap: Option<crate::cap::Capability>,
) {
    // 1. 检查是否有接收者在等待 (Rendezvous)
    if let Some(receiver_ptr) = ep.recv_queue.pop_front() {
        let receiver = unsafe { &mut *receiver_ptr };

        // --- 快速路径: 匹配成功 ---
        unsafe { copy_msg(current, receiver, badge, cap) };

        // 唤醒接收者
        scheduler::wake_up(receiver);
    } else {
        // --- 慢速路径: 阻塞 ---
        current.state = ThreadState::BlockedSend;

        // 将自己加入 Endpoint 的发送队列，同时保存 Badge 和要传递的能力
        ep.send_queue.push_back((current as *mut _, badge, cap));

        // 让出 CPU，触发调度
        scheduler::block_current_thread();
    }
}

/// 内核层面的通知（用于 IRQ 等），仅传递 badge
pub fn notify(ep: &mut Endpoint, badge: usize) {
    // 如果有接收者在等，直接交付并唤醒
    if let Some(receiver_ptr) = ep.recv_queue.pop_front() {
        let receiver = unsafe { &mut *receiver_ptr };
        receiver.get_trapframe().expect("ipc: Receiver has no TrapFrame").t0 = badge;
        scheduler::wake_up(receiver);
    } else {
        // 否则把通知放入 pending 队列，等待将来 recv
        ep.pending_notifs.push_back(badge);
    }
}

/// 接收操作 (sys_recv)
///
/// * `current`: 当前正在执行的线程 (接收者)
/// * `ep`: 目标 Endpoint 对象
pub fn recv(current: &mut TCB, ep: &mut Endpoint) {
    // 0. 检查是否有内核 pending 通知（例如 IRQ）
    if let Some(badge) = ep.pending_notifs.pop_front() {
        // 将 badge 放到接收者上下文并返回（无数据拷贝）
        current.get_trapframe().expect("ipc: Receiver has no TrapFrame").t0 = badge;
        return;
    }

    // 1. 检查是否有发送者在等待
    if let Some((sender_ptr, badge, cap)) = ep.send_queue.pop_front() {
        let sender = unsafe { &mut *sender_ptr };

        // --- 快速路径: 匹配成功 ---
        // 从等待的发送者那里拷贝数据
        unsafe { copy_msg(sender, current, badge, cap) };

        // 唤醒发送者
        scheduler::wake_up(sender);

        // 接收者收到数据，继续运行 (不阻塞)
    } else {
        // --- 慢速路径: 阻塞 ---
        current.state = ThreadState::BlockedRecv;

        // 将自己加入 Endpoint 的接收队列
        ep.recv_queue.push_back(current as *mut _);

        // 让出 CPU，触发调度
        scheduler::block_current_thread();
    }
}
