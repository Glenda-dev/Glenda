use crate::printk;
use crate::proc::thread::TCB;
use crate::trap::interrupt;
use core::sync::atomic::AtomicUsize;
use riscv::register::sstatus;
use spin::Mutex;

/// IPC 通信端点
/// 用于线程间同步消息传递
pub struct Endpoint {
    /// 引用计数
    pub ref_count: AtomicUsize,
    inner: Mutex<EndpointInner>,
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

impl Endpoint {
    pub const fn new() -> Self {
        Self {
            ref_count: AtomicUsize::new(1), // 初始引用计数为 1 (创建者持有)
            inner: Mutex::new(EndpointInner {
                send_queue_head: None,
                send_queue_tail: None,
                recv_queue_head: None,
                recv_queue_tail: None,
                notification_word: 0,
            }),
        }
    }

    pub fn enqueue_send(&self, tcb: *mut TCB) {
        let sie = interrupt::is_enabled();
        interrupt::disable();
        {
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
        }
        if sie {
            interrupt::enable();
        }
    }

    pub fn dequeue_send(&self) -> Option<*mut TCB> {
        let sie = interrupt::is_enabled();
        interrupt::disable();
        let ret = {
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
        };
        if sie {
            interrupt::enable();
        }
        ret
    }

    pub fn enqueue_recv(&self, tcb: *mut TCB) {
        let sie = interrupt::is_enabled();
        interrupt::disable();
        {
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
        if sie {
            interrupt::enable();
        }
    }

    pub fn dequeue_recv(&self) -> Option<*mut TCB> {
        let sie = interrupt::is_enabled();
        interrupt::disable();
        let ret = {
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
        };
        if sie {
            interrupt::enable();
        }
        ret
    }

    pub fn notify(&self, badge: usize) {
        let sie = interrupt::is_enabled();
        interrupt::disable();
        {
            let mut inner = self.inner.lock();
            inner.notification_word |= badge;
        }
        if sie {
            interrupt::enable();
        }
    }

    pub fn poll_notification(&self) -> usize {
        let sie = interrupt::is_enabled();
        interrupt::disable();
        let ret = {
            let mut inner = self.inner.lock();
            let word = inner.notification_word;
            inner.notification_word = 0;
            word
        };
        if sie {
            interrupt::enable();
        }
        ret
    }
}
