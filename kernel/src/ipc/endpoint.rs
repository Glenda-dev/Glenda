use crate::cap::Badge;
use crate::cap::Capability;
use crate::proc::thread::TCB;
use crate::proc::thread::ThreadState;
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
}

pub enum SendQueueResult {
    MatchedReceiver(*mut TCB, Option<Capability>),
    Queued,
}

pub enum RecvQueueResult {
    MatchedNotification(Badge),
    MatchedSender(*mut TCB),
    Queued,
}

pub enum NotifyResult {
    MatchedReceiver(*mut TCB),
    Pending,
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

    pub fn match_recv_or_queue_send(
        &self,
        sender: *mut TCB,
        state: ThreadState,
        badge: Badge,
        cap: Option<Capability>,
    ) -> SendQueueResult {
        let mut inner = self.inner.lock();
        if let Some(receiver) = inner.pop_recv() {
            return SendQueueResult::MatchedReceiver(receiver, cap);
        }

        unsafe {
            let tcb = &mut *sender;
            let _guard = tcb.lock.lock();
            tcb.wake_pending = false;
            tcb.state = state;
            tcb.ipc_badge = badge;
            tcb.ipc_cap = cap;
        }
        unsafe { inner.push_send(sender) };
        SendQueueResult::Queued
    }

    pub fn match_send_or_queue_recv(&self, receiver: *mut TCB) -> RecvQueueResult {
        let mut inner = self.inner.lock();
        if inner.notification_word != 0 {
            let word = inner.notification_word;
            inner.notification_word = 0;
            return RecvQueueResult::MatchedNotification(Badge::from(word));
        }

        if let Some(sender) = inner.pop_send() {
            return RecvQueueResult::MatchedSender(sender);
        }

        unsafe {
            let tcb = &mut *receiver;
            let _guard = tcb.lock.lock();
            tcb.ipc_active_ep = None;
            tcb.ipc_caller = None;
            tcb.wake_pending = false;
            tcb.state = ThreadState::BlockedRecv;
        }
        unsafe { inner.push_recv(receiver) };
        RecvQueueResult::Queued
    }

    pub fn notify(&self, badge: Badge) {
        let mut inner = self.inner.lock();
        inner.notification_word |= badge.get();
    }

    pub fn notify_or_dequeue_recv(&self, badge: Badge) -> NotifyResult {
        let mut inner = self.inner.lock();
        if let Some(receiver) = inner.pop_recv() {
            return NotifyResult::MatchedReceiver(receiver);
        }
        inner.notification_word |= badge.get();
        NotifyResult::Pending
    }

    pub fn poll_notification(&self) -> Badge {
        let mut inner = self.inner.lock();
        let word = inner.notification_word;
        inner.notification_word = 0;
        Badge::from(word)
    }

    pub fn destroy(&self) {
        use crate::proc::scheduler;
        // Unblock all senders
        while let Some(tcb_ptr) = self.dequeue_send() {
            unsafe {
                let tcb = &mut *tcb_ptr;
                scheduler::wake_up(tcb);
            }
        }
        // Unblock all receivers
        while let Some(tcb_ptr) = self.dequeue_recv() {
            unsafe {
                let tcb = &mut *tcb_ptr;
                scheduler::wake_up(tcb);
            }
        }
    }
}

impl EndpointInner {
    unsafe fn push_send(&mut self, tcb: *mut TCB) {
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

    fn pop_send(&mut self) -> Option<*mut TCB> {
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

    unsafe fn push_recv(&mut self, tcb: *mut TCB) {
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

    fn pop_recv(&mut self) -> Option<*mut TCB> {
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
