use crate::proc::process::Pid;
use alloc::collections::BTreeMap;
use alloc::collections::VecDeque;
use spin::Mutex;

/// IPC 端点
pub struct Endpoint {
    /// 等待发送的进程 PID 队列
    pub send_queue: VecDeque<Pid>,
    /// 等待接收的进程 PID 队列
    pub recv_queue: VecDeque<Pid>,
}

impl Endpoint {
    pub fn new() -> Self {
        Self { send_queue: VecDeque::new(), recv_queue: VecDeque::new() }
    }
}

/// 全局 Endpoint 管理器 (简化版)
/// 在实际系统中，Endpoint 对象可能由内核堆分配器管理
pub static ENDPOINT_MANAGER: Mutex<BTreeMap<Pid, Endpoint>> = Mutex::new(BTreeMap::new());
