pub mod endpoint;

use crate::mem::addr;
use crate::proc::scheduler;
use crate::proc::thread::{TCB, ThreadState, UTCB};

pub use endpoint::Endpoint;

/// 获取 TCB 对应的 UTCB 内核指针
/// 这是一个辅助函数，用于在内核态访问用户的 UTCB
unsafe fn get_utcb(tcb: &TCB) -> &mut UTCB {
    let frame = tcb.utcb_frame.as_ref().expect("Thread has no UTCB");
    // 将物理地址转换为内核虚拟地址 (HHDM)
    let vaddr = addr::phys_to_virt(frame.addr());
    &mut *(vaddr as *mut UTCB)
}

/// 执行消息拷贝 (Sender UTCB -> Receiver UTCB)
/// 同时传递 Badge 到接收者的上下文
unsafe fn copy_msg(sender: &TCB, receiver: &mut TCB, badge: usize) {
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
        core::ptr::copy_nonoverlapping(src_buf_ptr, dst_buf_ptr, dst.ipc_buffer_size);
    }

    // 2. 传递 Badge
    // 根据微内核 ABI，Badge 通常放入接收者的 t0 寄存器，或者 UTCB 的特定字段
    // 这里我们将其放入接收者的上下文寄存器 t0 中
    receiver.get_trapframe().expect("ipc: Receiver has no TrapFrame").t0 = badge;
}

/// 发送操作 (sys_send)
///
/// * `current`: 当前正在执行的线程 (发送者)
/// * `ep`: 目标 Endpoint 对象
/// * `badge`: 发送 Capability 携带的身份标识
pub fn send(current: &mut TCB, ep: &mut Endpoint, badge: usize) {
    // 1. 检查是否有接收者在等待 (Rendezvous)
    if let Some(receiver_ptr) = ep.recv_queue.pop_front() {
        let receiver = unsafe { &mut *receiver_ptr };

        // --- 快速路径: 匹配成功 ---
        // 既然接收者在等，直接拷贝数据
        unsafe { copy_msg(current, receiver, badge) };

        // 唤醒接收者
        // 接收者将从 BlockedRecv 变为 Ready，并进入调度队列
        scheduler::wake_up(receiver);

        // 发送者继续运行 (Non-blocking send if partner ready)
        // 注意：如果是 Call 操作，发送者这里应该转为 BlockedRecv 等待回复
        // 但在简化的 Send/Recv 模型中，发送完成即返回
    } else {
        // --- 慢速路径: 阻塞 ---
        // 没有接收者，发送者必须阻塞等待
        current.state = ThreadState::BlockedSend;

        // 将自己加入 Endpoint 的发送队列，同时保存 Badge
        ep.send_queue.push_back((current as *mut _, badge));

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
    if let Some((sender_ptr, badge)) = ep.send_queue.pop_front() {
        let sender = unsafe { &mut *sender_ptr };

        // --- 快速路径: 匹配成功 ---
        // 从等待的发送者那里拷贝数据
        unsafe { copy_msg(sender, current, badge) };

        // 唤醒发送者
        // 发送者将从 BlockedSend 变为 Ready
        scheduler::wake_up(sender);

        // 接收者收到数据，继续运行 (不阻塞)
    } else {
        // --- 慢速路径: 阻塞 ---
        // 没有发送者，接收者阻塞等待
        current.state = ThreadState::BlockedRecv;

        // 将自己加入 Endpoint 的接收队列
        ep.recv_queue.push_back(current as *mut _);

        // 让出 CPU，触发调度
        scheduler::block_current_thread();
    }
}
