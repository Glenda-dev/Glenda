use crate::cap::Badge;
use crate::ipc::MsgTag;
use crate::proc::thread::TCB;
use crate::sync::SpinLock;
use core::sync::atomic::AtomicUsize;

/// IPC 通信端点
/// 用于线程间同步消息传递
pub struct Endpoint {
    /// 引用计数
    pub ref_count: AtomicUsize,
    inner: SpinLock<EndpointInner>,
}

struct EndpointInner {
    /// 等待发送的线程队列
    send_queue_head: Option<*mut TCB>,
    send_queue_tail: Option<*mut TCB>,

    /// 等待接收的线程队列
    recv_queue_head: Option<*mut TCB>,
    recv_queue_tail: Option<*mut TCB>,

    /// 内核层面的 pending 通知 (Bitwise OR of badges)
    notification_word: usize,
    /// Pending 通知的标签
    notification_tag: Option<MsgTag>,
}

impl Endpoint {
    pub const fn new() -> Self {
        Self {
            ref_count: AtomicUsize::new(0), // 初始引用计数由 create_endpoint 增加到 1
            inner: SpinLock::new(EndpointInner {
                send_queue_head: None,
                send_queue_tail: None,
                recv_queue_head: None,
                recv_queue_tail: None,
                notification_word: 0,
                notification_tag: None,
            }),
        }
    }

    pub fn enqueue_send(&self, tcb: *mut TCB) {
        let mut inner = self.inner.lock();
        unsafe {
            (*tcb).prev = inner.send_queue_tail;
            (*tcb).next = None;
            if let Some(tail) = inner.send_queue_tail {
                (*tail).next = Some(tcb);
            } else {
                inner.send_queue_head = Some(tcb);
            }
            inner.send_queue_tail = Some(tcb);
        }
        // lock drop will pop_off
    }

    pub fn dequeue_send(&self) -> Option<*mut TCB> {
        let mut inner = self.inner.lock();
        if let Some(head) = inner.send_queue_head {
            unsafe {
                let next = (*head).next;
                if let Some(next_ptr) = next {
                    (*next_ptr).prev = None;
                } else {
                    inner.send_queue_tail = None;
                }
                inner.send_queue_head = next;
                (*head).next = None;
                (*head).prev = None;
            }
            Some(head)
        } else {
            None
        }
    }

    pub fn enqueue_recv(&self, tcb: *mut TCB) {
        let mut inner = self.inner.lock();
        unsafe {
            (*tcb).prev = inner.recv_queue_tail;
            (*tcb).next = None;
            if let Some(tail) = inner.recv_queue_tail {
                (*tail).next = Some(tcb);
            } else {
                inner.recv_queue_head = Some(tcb);
            }
            inner.recv_queue_tail = Some(tcb);
        }
    }

    pub fn dequeue_recv(&self) -> Option<*mut TCB> {
        let mut inner = self.inner.lock();
        if let Some(head) = inner.recv_queue_head {
            unsafe {
                let next = (*head).next;
                if let Some(next_ptr) = next {
                    (*next_ptr).prev = None;
                } else {
                    inner.recv_queue_tail = None;
                }
                inner.recv_queue_head = next;
                (*head).next = None;
                (*head).prev = None;
            }
            Some(head)
        } else {
            None
        }
    }

    pub fn notify(&self, badge: Badge, tag: Option<MsgTag>) {
        let mut inner = self.inner.lock();
        inner.notification_word |= badge.get();
        if tag.is_some() {
            inner.notification_tag = tag;
        }
    }

    pub fn poll_notification(&self) -> (Badge, Option<MsgTag>) {
        let mut inner = self.inner.lock();
        let word = inner.notification_word;
        let tag = inner.notification_tag.take();
        inner.notification_word = 0;
        (Badge::from(word), tag)
    }

    pub fn destroy(&self) {
        use crate::proc::scheduler;
        use crate::proc::thread::ThreadState;
        // Unblock all senders
        while let Some(tcb_ptr) = self.dequeue_send() {
            unsafe {
                let tcb = &mut *tcb_ptr;
                tcb.state = ThreadState::Ready;
                scheduler::add_thread(tcb);
            }
        }
        // Unblock all receivers
        while let Some(tcb_ptr) = self.dequeue_recv() {
            unsafe {
                let tcb = &mut *tcb_ptr;
                tcb.state = ThreadState::Ready;
                scheduler::add_thread(tcb);
            }
        }
    }
}
