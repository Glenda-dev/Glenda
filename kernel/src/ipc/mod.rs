pub mod endpoint;
pub mod msg;
pub mod protocol;
pub mod utcb;

use crate::cap::{Badge, CapType, Capability, Rights};
use crate::error::Error;
use crate::proc::scheduler;
use crate::proc::thread::{TCB, ThreadState};
pub use endpoint::Endpoint;
pub use msg::{MsgFlags, MsgTag};
pub use utcb::{MsgArgs, UTCB};

pub fn transfer_cap(tcb: &TCB) -> Option<Capability> {
    let utcb = match get_utcb_ptr(tcb) {
        Some(ptr) => unsafe { &*ptr },
        None => return None,
    };

    let tag = utcb.msg_tag;
    if tag.flags().contains(MsgFlags::HAS_CAP) {
        if let Some(cap) = tcb.cap_lookup(utcb.cap_transfer) {
            if cap.has_rights(Rights::GRANT) {
                log!("ipc: Transfer cap {:?}", utcb.cap_transfer);
                return Some(cap);
            } else {
                warn!("ipc: Warning: cannot grant cap {:?}", utcb.cap_transfer);
            }
        } else {
            warn!("ipc: Warning: cap to transfer not found {:?}", utcb.cap_transfer);
        }
    }
    None
}

pub fn get_utcb_ptr(tcb: &TCB) -> Option<*mut UTCB> {
    if let Some(cap) = &tcb.utcb_frame {
        if cap.cap_type() == CapType::Frame {
            return Some(cap.obj_ptr().as_mut_ptr::<UTCB>());
        }
    }
    None
}

/// 检查 IPC 调用是否会导致循环等待死锁
fn check_deadlock(sender: &TCB, receiver: &TCB) -> bool {
    let mut curr = receiver;
    loop {
        if curr as *const TCB == sender as *const TCB {
            return true;
        }
        if let Some(partner_ptr) = curr.ipc_partner {
            curr = unsafe { &*partner_ptr };
        } else {
            break;
        }
    }
    false
}

/// 传播优先级，实现完整的优先级继承链
fn propagate_priority(priority: u8, target_ptr: *mut TCB) {
    let mut curr_ptr = target_ptr;
    loop {
        let curr = unsafe { &mut *curr_ptr };
        if priority > curr.priority {
            curr.priority = priority;
            if let Some(partner_ptr) = curr.ipc_partner {
                let partner = unsafe { &mut *partner_ptr };
                // 递归加锁可能会死锁，这里使用 try_lock
                if let Some(_guard) = partner.lock.try_lock() {
                    curr_ptr = partner_ptr;
                    continue;
                }
            }
        }
        break;
    }
}

/// 执行消息拷贝 (Sender UTCB -> Receiver UTCB)
/// 同时传递 Badge 到接收者的上下文，并可选地传递一个 Capability
unsafe fn copy_msg(
    sender: &mut TCB,
    receiver: &mut TCB,
    badge: Badge,
    cap: Option<Capability>,
    reply_cap: Option<Capability>,
) -> Result<(), Error> {
    log!("ipc: Copy_msg sender={:p} receiver={:p} badge={:?}", sender, receiver, badge);

    // 优先级继承与时间片捐赠 (避免 ABBA 死锁，按地址顺序加锁)
    {
        let src_addr = sender as *const TCB as usize;
        let dst_addr = receiver as *const TCB as usize;
        let receiver_ptr = receiver as *mut TCB;
        if src_addr < dst_addr {
            let _g1 = sender.lock.lock();
            let _g2 = receiver.lock.lock();
            propagate_priority(sender.priority, receiver_ptr);
            receiver.timeslice += sender.timeslice;
            sender.timeslice = 0; // Donated
        } else {
            let _g1 = receiver.lock.lock();
            let _g2 = sender.lock.lock();
            propagate_priority(sender.priority, receiver_ptr);
            receiver.timeslice += sender.timeslice;
            sender.timeslice = 0; // Donated
        }
    }
    let src_ptr = get_utcb_ptr(sender).ok_or(Error::MappingFailed)?;
    let dst_ptr = get_utcb_ptr(receiver).ok_or(Error::MappingFailed)?;
    let src = unsafe { &mut *src_ptr };
    let dst = unsafe { &mut *dst_ptr };
    // 1. 传递消息内容
    src.copy_to(dst);

    // 2. 传递 Badge
    dst.badge = badge;

    // 3. 传递 Capability (如果提供且接收者准备好了接收窗口)

    if let Some(c) = cap {
        let recv_window = dst.recv_window;
        let cspace = receiver.get_cspace();
        match cspace.insert(recv_window, &c) {
            Err(e) => {
                error!(
                    "ipc: Failed to transfer capability to receiver at {}: {:?}",
                    recv_window, e
                );
                return Err(e);
            }
            Ok(_) => {
                log!("ipc: Transferred capability to receiver at {}", recv_window);
            }
        }
    }

    // 4. 如果是 Call，还需要传递 Reply Cap
    if let Some(rc) = reply_cap {
        let reply_window = dst.reply_window;
        let cspace = receiver.get_cspace();
        match cspace.insert(reply_window, &rc) {
            Err(e) => {
                error!(
                    "ipc: Failed to transfer reply capability to receiver at {}: {:?}",
                    reply_window, e
                );
                return Err(e);
            }
            Ok(_) => {
                log!("ipc: Transferred reply capability to receiver at {}", reply_window);
            }
        }
    }
    Ok(())
}

/// 发送操作
///
/// * `current`: 当前正在执行的线程 (发送者)
/// * `ep`: 目标 Endpoint 对象
/// * `badge`: 发送 Capability 携带的身份标识
/// * `cap`: 可选的要传递的能力
pub fn send(
    current: &mut TCB,
    ep: &Endpoint,
    badge: Badge,
    cap: Option<Capability>,
) -> Result<(), Error> {
    // 1. 检查是否有接收者在等待 (Rendezvous)
    if let Some(receiver_ptr) = ep.dequeue_recv() {
        log!(
            "ipc: Send current={:p} ep={:p} badge={:?} matched receiver={:p}",
            current,
            ep as *const _,
            badge,
            receiver_ptr
        );
        let receiver = unsafe { &mut *receiver_ptr };

        // --- 快速路径: 匹配成功 ---
        unsafe { copy_msg(current, receiver, badge, cap, None)? };

        // 唤醒接收者
        scheduler::wake_up(receiver);
    } else {
        log!("ipc: Send current={:p} ep={:p} badge={:?} blocking", current, ep as *const _, badge);
        // --- 慢速路径: 阻塞 ---
        current.state = ThreadState::BlockedSend;
        current.ipc_badge = badge;
        current.ipc_cap = cap;

        // 将自己加入 Endpoint 的发送队列，同时保存 Badge 和要传递的能力
        ep.enqueue_send(current as *mut _);

        // 让出 CPU，触发调度
        scheduler::block_current_thread();
    }
    Ok(())
}

/// Call 操作 (sys_call)
/// 发送消息并等待回复，是原子的 Send + Recv
pub fn call(
    current: &mut TCB,
    ep: &Endpoint,
    badge: Badge,
    cap: Option<Capability>,
) -> Result<(), Error> {
    log!("",);
    // 1. 检查是否有接收者在等待
    if let Some(receiver_ptr) = ep.dequeue_recv() {
        let receiver = unsafe { &mut *receiver_ptr };

        // 检查死锁
        if check_deadlock(current, receiver) {
            error!("ipc: Deadlock detected in call!");
            ep.enqueue_recv(receiver_ptr); // 放回队列
            return Err(Error::InvalidCapability);
        }

        log!(
            "ipc: Call current={:p} ep={:p} badge={:?} matched receiver={:p}",
            current,
            ep as *const _,
            badge,
            receiver_ptr
        );

        // 生成 Reply Capability 指向当前线程
        let reply_cap = Capability::create_reply(current, Rights::ALL);

        // --- 快速路径: 匹配成功 ---
        unsafe { copy_msg(current, receiver, badge, cap, Some(reply_cap))? };

        // 设置通信伙伴，用于死锁检查
        current.ipc_partner = Some(receiver as *mut _);

        // 当前线程进入 BlockedCall 状态，等待回复
        // 必须在 wake_up 之前设置，否则如果 wake_up 导致抢占，当前线程会被错误地置为 Ready
        current.state = ThreadState::BlockedCall;

        // 唤醒接收者
        scheduler::wake_up(receiver);

        // 如果 wake_up 没有导致抢占（或者抢占后又回来了），我们需要检查是否还需要阻塞
        // 如果已经被 Reply 唤醒，状态会变成 Running，就不需要再阻塞了
        if current.state == ThreadState::BlockedCall {
            scheduler::block_current_thread();
        }

        // 返回后清空通信伙伴
        current.ipc_partner = None;
    } else {
        log!("ipc: Call current={:p} ep={:p} badge={:?} blocking", current, ep as *const _, badge);
        // --- 慢速路径: 阻塞在发送队列 ---
        current.state = ThreadState::BlockedCall;
        current.ipc_badge = badge;
        current.ipc_cap = cap;

        ep.enqueue_send(current as *mut _);
        scheduler::block_current_thread();
    }
    Ok(())
}

/// Reply 操作
/// 向指定的 TCB 发送回复消息
pub fn reply(current: &mut TCB, target: &mut TCB, cap: Option<Capability>) -> Result<(), Error> {
    // 只有处于 BlockedCall 状态的线程才能接收 Reply
    if target.state == ThreadState::BlockedCall {
        log!("ipc: Reply current={:p} target={:p} success", current, target);
        // Reply 不产生新的 Reply Cap
        unsafe { copy_msg(current, target, Badge::null(), cap, None)? };

        // 任务完成，当前线程恢复基础优先级
        {
            let _guard = current.lock.lock();
            current.priority = current.base_priority;
        }

        // 唤醒目标线程
        scheduler::wake_up(target);
        Ok(())
    } else {
        error!(
            "ipc: Reply current={:p} target={:p} failed target state {:?}",
            current, target, target.state
        );
        Err(Error::InvalidCapability)
    }
}

/// 内核层面的通知（用于 IRQ 等），传递 badge
pub fn notify(ep: &Endpoint, badge: Badge) -> Result<(), Error> {
    if badge.is_null() {
        warn!("ipc: Notify with null badge on ep={:p}", ep as *const _);
    }
    if let Some(receiver_ptr) = ep.dequeue_recv() {
        log!(
            "ipc: Notify ep={:p} badge={:?} matched receiver={:p}",
            ep as *const _,
            badge,
            receiver_ptr
        );
        let receiver = unsafe { &mut *receiver_ptr };
        let utcb = receiver.get_utcb().ok_or(Error::MappingFailed)?;
        utcb.msg_tag = MsgTag::new(protocol::KERNEL_PROTO, protocol::NOTIFY, MsgFlags::NONE);
        utcb.badge = badge;
        scheduler::wake_up(receiver);
    } else {
        log!("ipc: Notify ep={:p} badge={:?} pending", ep as *const _, badge,);
        ep.notify(badge);
    }
    Ok(())
}

/// 接收操作 (sys_recv)
///
/// * `current`: 当前正在执行的线程 (接收者)
/// * `ep`: 目标 Endpoint 对象
pub fn recv(current: &mut TCB, ep: &Endpoint) -> Result<(), Error> {
    // 0. 检查是否有内核 pending 通知（例如 IRQ）
    let pending = ep.poll_notification();
    if !pending.is_null() {
        log!("ipc: Recv current={:p} ep={:p} matched notification", current, ep as *const _);
        // 修复：主动检查时也要设置 MsgTag
        if let Some(utcb_ptr) = get_utcb_ptr(current) {
            unsafe {
                (*utcb_ptr).msg_tag =
                    MsgTag::new(protocol::KERNEL_PROTO, protocol::NOTIFY, MsgFlags::NONE);
                (*utcb_ptr).badge = pending;
            };
        }
        return Ok(());
    }

    // 1. 检查是否有发送者在等待
    if let Some(sender_ptr) = ep.dequeue_send() {
        log!(
            "ipc: Recv current={:p} ep={:p} matched sender={:p}",
            current,
            ep as *const _,
            sender_ptr
        );
        let sender = unsafe { &mut *sender_ptr };
        let badge = sender.ipc_badge;
        let cap = sender.ipc_cap.take();

        // 如果发送者是在执行 Call，我们需要为接收者生成一个 Reply Cap
        let reply_cap = if sender.state == ThreadState::BlockedCall {
            Some(Capability::create_reply(sender, Rights::ALL))
        } else {
            None
        };

        // --- 快速路径: 匹配成功 ---
        // 从等待的发送者那里拷贝数据
        unsafe { copy_msg(sender, current, badge, cap, reply_cap)? };

        // 唤醒发送者
        // 如果是 Call，发送者已经处于 BlockedCall，不需要在这里唤醒？
        // 不对，如果是 Call，发送者在等待 Reply，所以不应该在这里唤醒。
        // 如果是 Send，发送者在等待消息被接收，所以应该在这里唤醒。
        if sender.state != ThreadState::BlockedCall {
            scheduler::wake_up(sender);
        }

        // 接收者收到数据，继续运行 (不阻塞)
    } else {
        log!("ipc: Recv current={:p} ep={:p} blocking", current, ep as *const _);
        // --- 慢速路径: 阻塞 ---
        current.state = ThreadState::BlockedRecv;

        // 将自己加入 Endpoint 的接收队列
        ep.enqueue_recv(current as *mut _);

        // 让出 CPU，触发调度
        scheduler::block_current_thread();
    }
    Ok(())
}

/// Proxy 操作
/// 类似于 Call，但使用当前 UTCB 中的 Badge。
/// 这允许服务在调用其他服务时保留原始调用者的身份（Badge）。
///
/// 场景：Client (Badge A) -> Proxy -> Server (看到 Badge A)
pub fn proxy(current: &mut TCB, ep: &Endpoint, cap: Option<Capability>) -> Result<(), Error> {
    let utcb = current.get_utcb().ok_or(Error::MappingFailed)?;
    // 1. 从当前 UTCB 获取 Badge (通常是上一条接收到的消息的 Badge)
    let badge = utcb.badge;
    let reply_window = utcb.reply_window;

    if let Some(slot_ptr) = current.lookup_slot(reply_window) {
        log!(
            "ipc: Proxy current={:p} ep={:p} badge={:?} reply_window={:?} tail-calling",
            current,
            ep as *const _,
            badge,
            reply_window
        );
        let slot = unsafe { &mut *slot_ptr };
        let _guard = unsafe { slot.lock_cnode() };
        let reply_cap = slot.cap.clone();

        // 3. 校验并在此刻转移出原有的回复权
        // 如果不是 Reply 类型，说明没有可供转发的调用者
        if reply_cap.cap_type() != CapType::Reply {
            warn!("ipc: Proxy failed: No reply cap in reply_window!");
            return Err(Error::InvalidCapability);
        }
        slot.cap = Capability::empty(); // 转移出 Reply Cap，防止被多次转发
        // 获取原始调用者 TCB
        let original_caller = unsafe { reply_cap.obj_ptr().as_mut::<TCB>() };
        call(original_caller, ep, badge, cap)?;
    } else {
        log!("ipc: Proxy current={:p} ep={:p} badge={:?}", current, ep as *const _, badge);
        call(current, ep, badge, cap)?;
    }
    // Proxy 进程直接返回 Ok，无需等待 Server 回复，实现了真正的尾调用
    Ok(())
}
