use crate::proc::thread::TCB;
use alloc::collections::VecDeque;
use core::sync::atomic::AtomicUsize;

/// IPC 通信端点
/// 用于线程间同步消息传递
pub struct Endpoint {
    /// 引用计数
    pub ref_count: AtomicUsize,

    /// 等待发送的线程队列
    /// 存储元组: (线程指针, Badge, 可选的待传递能力)
    pub send_queue: VecDeque<(*mut TCB, usize, Option<crate::cap::Capability>)>,

    /// 等待接收的线程队列
    pub recv_queue: VecDeque<*mut TCB>,

    /// 内核层面的 pending 通知队列（用于 IRQ 通知等，无消息体，仅 badge）
    pub pending_notifs: VecDeque<usize>,
}

impl Endpoint {
    pub const fn new() -> Self {
        Self {
            ref_count: AtomicUsize::new(1), // 初始引用计数为 1 (创建者持有)
            send_queue: VecDeque::new(),
            recv_queue: VecDeque::new(),
            pending_notifs: VecDeque::new(),
        }
    }
}
