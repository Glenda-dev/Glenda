pub mod endpoint;

use crate::hart;
use crate::ipc::endpoint::ENDPOINT_MANAGER;
use crate::proc::process::{Pid, ProcState, Process};
use crate::proc::scheduler::scheduler;
use crate::proc::table::{NPROC, PROC_TABLE};

pub type Args = [usize; 8];

/// 初始化 IPC 子系统
pub fn init() {
    // 可以在这里预分配一些 Endpoint
}

/// 发送消息
/// endpoint_id: 端点 ID
/// badge: 发送者的身份标识 (由 Cap 提供)
/// data: 要发送的数据 (寄存器内容)
pub fn send(endpoint_id: usize, badge: usize, data: Args) {
    let mut endpoints = ENDPOINT_MANAGER.lock();
    let ep = endpoints.entry(endpoint_id).or_insert(endpoint::Endpoint::new());

    // 1. 检查是否有接收者在等待
    if let Some(receiver_pid) = ep.recv_queue.pop_front() {
        // --- 快速路径 (Fastpath) ---
        // 找到接收者，直接拷贝数据并唤醒它
        let mut p_table = PROC_TABLE.lock();

        // 查找接收者
        // TODO: 更高效的 PID -> Process 映射
        // 简单起见，这里假设 PID 对应数组索引，或者遍历查找
        let mut receiver_idx = None;
        for i in 0..NPROC {
            if p_table[i].pid == receiver_pid {
                receiver_idx = Some(i);
                break;
            }
        }

        if let Some(idx) = receiver_idx {
            let receiver = &mut p_table[idx];
            // 获取接收者的 TrapFrame
            unsafe {
                if !receiver.trapframe.is_null() {
                    let tf = &mut *receiver.trapframe;
                    tf.a0 = data[0]; // MR0: Tag
                    tf.a1 = data[1]; // MR1
                    tf.a2 = data[2];
                    tf.a3 = data[3];
                    tf.a4 = data[4];
                    tf.a5 = data[5];
                    tf.a6 = data[6];
                    tf.a7 = data[7];

                    tf.t0 = badge; // 传递 Badge
                }
            }

            // 唤醒接收者
            receiver.state = ProcState::Ready;
            // TODO: 如果支持优先级调度，这里可以检查是否需要抢占
        }
    } else {
        // --- 慢速路径 (Slowpath) ---
        // 没有接收者，当前进程阻塞
        let current_proc_ptr = hart::get().proc;
        let current_pid = unsafe { (*current_proc_ptr).pid };

        ep.send_queue.push_back(current_pid);

        // 将当前进程设为阻塞并让出 CPU
        unsafe {
            (*current_proc_ptr).state = ProcState::Blocked;
        }

        // 释放锁，避免死锁
        drop(endpoints);

        scheduler();
    }
}

/// 接收消息
pub fn recv(endpoint_id: usize) {
    let mut endpoints = ENDPOINT_MANAGER.lock();
    let ep = endpoints.entry(endpoint_id).or_insert(endpoint::Endpoint::new());

    // 1. 检查是否有发送者在等待
    if let Some(sender_pid) = ep.send_queue.pop_front() {
        // --- 快速路径 ---
        // 找到发送者，从发送者拷贝数据到当前进程
        let mut p_table = PROC_TABLE.lock();

        let mut sender_idx = None;
        for i in 0..NPROC {
            if p_table[i].pid == sender_pid {
                sender_idx = Some(i);
                break;
            }
        }

        let mut data = [0usize; 8];
        // TODO: badge需要从 Sender 的 Cap 中获取，这里简化

        if let Some(idx) = sender_idx {
            let sender = &mut p_table[idx];
            unsafe {
                if !sender.trapframe.is_null() {
                    let tf = &*sender.trapframe;
                    data[0] = tf.a0;
                    data[1] = tf.a1;
                    data[2] = tf.a2;
                    data[3] = tf.a3;
                    data[4] = tf.a4;
                    data[5] = tf.a5;
                    data[6] = tf.a6;
                    data[7] = tf.a7;
                }
            }
            sender.state = ProcState::Ready;
        }

        // 写入 Current (Receiver)
        let current_proc_ptr = hart::get().proc;
        unsafe {
            if !(*current_proc_ptr).trapframe.is_null() {
                let tf = &mut *(*current_proc_ptr).trapframe;
                tf.a0 = data[0];
                tf.a1 = data[1];
                tf.a2 = data[2];
                tf.a3 = data[3];
                tf.a4 = data[4];
                tf.a5 = data[5];
                tf.a6 = data[6];
                tf.a7 = data[7];
                // tf.t0 = badge;
            }
        }
    } else {
        // --- 慢速路径 ---
        // 没有发送者，阻塞当前进程
        let current_proc_ptr = hart::get().proc;
        let current_pid = unsafe { (*current_proc_ptr).pid };

        ep.recv_queue.push_back(current_pid);

        unsafe {
            (*current_proc_ptr).state = ProcState::Blocked;
        }

        drop(endpoints);

        scheduler();
    }
}

pub fn reply_recv(endpoint_id: usize, badge: usize, data: Args) {
    let mut endpoints = ENDPOINT_MANAGER.lock();
    let ep = endpoints.entry(endpoint_id).or_insert(endpoint::Endpoint::new());

    // 处理 ReplyRecv 操作类似于 Send + Recv 的组合
    // 1. 发送数据给等待的接收者（如果有）
    if let Some(receiver_pid) = ep.recv_queue.pop_front() {
        let mut p_table = PROC_TABLE.lock();

        let mut receiver_idx = None;
        for i in 0..NPROC {
            if p_table[i].pid == receiver_pid {
                receiver_idx = Some(i);
                break;
            }
        }

        if let Some(idx) = receiver_idx {
            let receiver = &mut p_table[idx];
            unsafe {
                if !receiver.trapframe.is_null() {
                    let tf = &mut *receiver.trapframe;
                    tf.a0 = data[0];
                    tf.a1 = data[1];
                    tf.a2 = data[2];
                    tf.a3 = data[3];
                    tf.a4 = data[4];
                    tf.a5 = data[5];
                    tf.a6 = data[6];
                    tf.a7 = data[7];

                    tf.t0 = badge;
                }
            }
            receiver.state = ProcState::Ready;
        }
    }

    // 2. 接收数据（阻塞当前进程）
    let current_proc_ptr = hart::get().proc;
    let current_pid = unsafe { (*current_proc_ptr).pid };

    ep.recv_queue.push_back(current_pid);

    unsafe {
        (*current_proc_ptr).state = ProcState::Blocked;
    }

    drop(endpoints);

    scheduler();
}
