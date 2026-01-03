pub mod endpoint;
pub mod message;
pub mod utcb;

pub use message::{MsgTag, label};
pub use utcb::UTCB;

use crate::cap::{CapType, Capability, Slot, rights};
use crate::mem::VirtAddr;
use crate::proc::scheduler;
use crate::proc::thread::{TCB, ThreadState};

pub use endpoint::Endpoint;
pub use utcb::MAX_MRS;

fn get_utcb_ptr(tcb: &TCB) -> Option<*mut UTCB> {
    if let Some(cap) = &tcb.utcb_frame {
        if let CapType::Frame { paddr, .. } = cap.object {
            return Some(paddr.to_va().as_mut_ptr::<UTCB>());
        }
    }
    None
}

/// 执行消息拷贝 (Sender UTCB -> Receiver UTCB)
/// 同时传递 Badge 到接收者的上下文，并可选地传递一个 Capability
unsafe fn copy_msg(
    sender: &TCB,
    receiver: &mut TCB,
    badge: usize,
    cap: Option<Capability>,
    reply_cap: Option<Capability>,
) {
    let src_ptr = get_utcb_ptr(sender).expect("ipc: Sender has no UTCB");
    let dst_ptr = get_utcb_ptr(receiver).expect("ipc: Receiver has no UTCB");
    let src = unsafe { &*src_ptr };
    let dst = unsafe { &mut *dst_ptr };
    // 1. 传递消息内容
    src.copy_to(dst);
    // 2. 传递 Badge
    set_badge(receiver, badge);

    // 3. 传递 Capability (如果提供且接收者准备好了接收窗口)
    // 优先传递用户指定的 cap，如果没有则传递内核生成的 reply_cap
    let final_cap = cap.or(reply_cap);

    if let Some(c) = final_cap {
        let recv_window = dst.recv_window;
        if recv_window != 0 {
            if let Some((_, slot_addr)) = receiver.cap_lookup_slot(recv_window) {
                let slot = slot_addr.as_mut::<Slot>();
                slot.cap = c;
            }
        }
    }
}

fn set_badge(tcb: &mut TCB, badge: usize) {
    let tf = tcb.get_tf();
    tf.a1 = badge;
}

/// 发送操作
///
/// * `current`: 当前正在执行的线程 (发送者)
/// * `ep`: 目标 Endpoint 对象
/// * `badge`: 发送 Capability 携带的身份标识
/// * `cap`: 可选的要传递的能力
pub fn send(current: &mut TCB, ep: &Endpoint, badge: usize, cap: Option<Capability>) {
    // 1. 检查是否有接收者在等待 (Rendezvous)
    if let Some(receiver_ptr) = ep.dequeue_recv() {
        let receiver = unsafe { &mut *receiver_ptr };

        // --- 快速路径: 匹配成功 ---
        unsafe { copy_msg(current, receiver, badge, cap, None) };

        // 唤醒接收者
        scheduler::wake_up(receiver);
    } else {
        // --- 慢速路径: 阻塞 ---
        current.state = ThreadState::BlockedSend;
        current.ipc_badge = badge;
        current.ipc_cap = cap;

        // 将自己加入 Endpoint 的发送队列，同时保存 Badge 和要传递的能力
        ep.enqueue_send(current as *mut _);

        // 让出 CPU，触发调度
        scheduler::block_current_thread();
    }
}

/// Call 操作 (sys_call)
/// 发送消息并等待回复，是原子的 Send + Recv
pub fn call(current: &mut TCB, ep: &Endpoint, badge: usize, cap: Option<Capability>) {
    // 1. 检查是否有接收者在等待
    if let Some(receiver_ptr) = ep.dequeue_recv() {
        let receiver = unsafe { &mut *receiver_ptr };

        // 生成 Reply Capability 指向当前线程
        let reply_cap = Capability::new(
            CapType::Reply { tcb_ptr: VirtAddr::from(current as *const TCB as usize) },
            rights::ALL,
        );

        // --- 快速路径: 匹配成功 ---
        unsafe { copy_msg(current, receiver, badge, cap, Some(reply_cap)) };

        // 唤醒接收者
        scheduler::wake_up(receiver);

        // 当前线程进入 BlockedCall 状态，等待回复
        current.state = ThreadState::BlockedCall;
        scheduler::block_current_thread();
    } else {
        // --- 慢速路径: 阻塞在发送队列 ---
        current.state = ThreadState::BlockedCall;
        current.ipc_badge = badge;
        current.ipc_cap = cap;

        ep.enqueue_send(current as *mut _);
        scheduler::block_current_thread();
    }
}

/// Reply 操作
/// 向指定的 TCB 发送回复消息
pub fn reply(current: &mut TCB, target: &mut TCB) {
    // 只有处于 BlockedCall 状态的线程才能接收 Reply
    if target.state == ThreadState::BlockedCall {
        // Reply 不产生新的 Reply Cap
        unsafe { copy_msg(current, target, 0, None, None) };

        // 唤醒目标线程
        scheduler::wake_up(target);
    }
}

/// 内核层面的通知（用于 IRQ 等），仅传递 badge
pub fn notify(ep: &Endpoint, badge: usize) {
    if let Some(receiver_ptr) = ep.dequeue_recv() {
        let receiver = unsafe { &mut *receiver_ptr };

        // 修复：设置 Badge 的同时，必须更新 MsgTag 告知接收者这是通知
        if let Some(utcb_ptr) = get_utcb_ptr(receiver) {
            unsafe { (*utcb_ptr).msg_tag = MsgTag::new(label::NOTIFY, 0) };
        }

        set_badge(receiver, badge);
        scheduler::wake_up(receiver);
    } else {
        ep.notify(badge);
    }
}

/// 接收操作 (sys_recv)
///
/// * `current`: 当前正在执行的线程 (接收者)
/// * `ep`: 目标 Endpoint 对象
pub fn recv(current: &mut TCB, ep: &Endpoint) {
    // 0. 检查是否有内核 pending 通知（例如 IRQ）
    let pending = ep.poll_notification();
    if pending != 0 {
        // 修复：主动检查时也要设置 MsgTag
        if let Some(utcb_ptr) = get_utcb_ptr(current) {
            unsafe { (*utcb_ptr).msg_tag = MsgTag::new(label::NOTIFY, 0) };
        }

        set_badge(current, pending);
        return;
    }

    // 1. 检查是否有发送者在等待
    if let Some(sender_ptr) = ep.dequeue_send() {
        let sender = unsafe { &mut *sender_ptr };
        let badge = sender.ipc_badge;
        let cap = sender.ipc_cap.take();

        // 如果发送者是在执行 Call，我们需要为接收者生成一个 Reply Cap
        let reply_cap = if sender.state == ThreadState::BlockedCall {
            Some(Capability::new(
                CapType::Reply { tcb_ptr: VirtAddr::from(sender as *const TCB as usize) },
                rights::ALL,
            ))
        } else {
            None
        };

        // --- 快速路径: 匹配成功 ---
        // 从等待的发送者那里拷贝数据
        unsafe { copy_msg(sender, current, badge, cap, reply_cap) };

        // 唤醒发送者
        // 如果是 Call，发送者已经处于 BlockedCall，不需要在这里唤醒？
        // 不对，如果是 Call，发送者在等待 Reply，所以不应该在这里唤醒。
        // 如果是 Send，发送者在等待消息被接收，所以应该在这里唤醒。
        if sender.state != ThreadState::BlockedCall {
            scheduler::wake_up(sender);
        }

        // 接收者收到数据，继续运行 (不阻塞)
    } else {
        // --- 慢速路径: 阻塞 ---
        current.state = ThreadState::BlockedRecv;

        // 将自己加入 Endpoint 的接收队列
        ep.enqueue_recv(current as *mut _);

        // 让出 CPU，触发调度
        scheduler::block_current_thread();
    }
}
