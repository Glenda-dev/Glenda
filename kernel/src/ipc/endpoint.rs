use crate::proc::thread::TCB;
use core::sync::atomic::AtomicUsize;

/// IPC 通信端点
/// 用于线程间同步消息传递
pub struct Endpoint {
    /// 引用计数
    pub ref_count: AtomicUsize,

    /// 等待发送的线程队列
    pub send_queue_head: Option<*mut TCB>,
    pub send_queue_tail: Option<*mut TCB>,

    /// 等待接收的线程队列
    pub recv_queue_head: Option<*mut TCB>,
    pub recv_queue_tail: Option<*mut TCB>,

    /// 内核层面的 pending 通知 (Bitwise OR of badges)
    pub notification_word: usize,
}

impl Endpoint {
    pub const fn new() -> Self {
        Self {
            ref_count: AtomicUsize::new(1), // 初始引用计数为 1 (创建者持有)
            send_queue_head: None,
            send_queue_tail: None,
            recv_queue_head: None,
            recv_queue_tail: None,
            notification_word: 0,
        }
    }

    pub fn enqueue_send(&mut self, tcb: *mut TCB) {
        unsafe {
            (*tcb).prev = self.send_queue_tail;
            (*tcb).next = None;
            if let Some(tail) = self.send_queue_tail {
                (*tail).next = Some(tcb);
            } else {
                self.send_queue_head = Some(tcb);
            }
            self.send_queue_tail = Some(tcb);
        }
    }

    pub fn dequeue_send(&mut self) -> Option<*mut TCB> {
        if let Some(head) = self.send_queue_head {
            unsafe {
                let next = (*head).next;
                if let Some(next_ptr) = next {
                    (*next_ptr).prev = None;
                } else {
                    self.send_queue_tail = None;
                }
                self.send_queue_head = next;
                (*head).next = None;
                (*head).prev = None;
            }
            Some(head)
        } else {
            None
        }
    }

    pub fn enqueue_recv(&mut self, tcb: *mut TCB) {
        unsafe {
            (*tcb).prev = self.recv_queue_tail;
            (*tcb).next = None;
            if let Some(tail) = self.recv_queue_tail {
                (*tail).next = Some(tcb);
            } else {
                self.recv_queue_head = Some(tcb);
            }
            self.recv_queue_tail = Some(tcb);
        }
    }

    pub fn dequeue_recv(&mut self) -> Option<*mut TCB> {
        if let Some(head) = self.recv_queue_head {
            unsafe {
                let next = (*head).next;
                if let Some(next_ptr) = next {
                    (*next_ptr).prev = None;
                } else {
                    self.recv_queue_tail = None;
                }
                self.recv_queue_head = next;
                (*head).next = None;
                (*head).prev = None;
            }
            Some(head)
        } else {
            None
        }
    }
}
