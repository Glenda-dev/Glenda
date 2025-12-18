use crate::proc::thread::TCB;
use alloc::collections::VecDeque;

/// IPC 通信端点
/// 用于线程间同步消息传递
pub struct Endpoint {
    /// 等待发送的线程队列
    /// 存储元组: (线程指针, Badge)
    /// Badge 是发送者使用的 Capability 携带的身份标识
    pub send_queue: VecDeque<(*mut TCB, usize)>,

    /// 等待接收的线程队列
    pub recv_queue: VecDeque<*mut TCB>,
}

impl Endpoint {
    pub const fn new() -> Self {
        Self { send_queue: VecDeque::new(), recv_queue: VecDeque::new() }
    }
}
