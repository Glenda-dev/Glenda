use super::context::switch_context;
use super::thread::{TCB, ThreadState};
use crate::hart;
use crate::hart::MAX_HARTS;
use crate::sbi;
use riscv::register::sstatus;
use spin::Mutex;

// 最大优先级数量 (0-255)
const MAX_PRIORITY: usize = 256;

struct TcbQueue {
    head: Option<*mut TCB>,
    tail: Option<*mut TCB>,
}

unsafe impl Send for TcbQueue {}

impl TcbQueue {
    const fn new() -> Self {
        Self { head: None, tail: None }
    }

    fn push_back(&mut self, tcb: *mut TCB) {
        unsafe {
            (*tcb).prev = self.tail;
            (*tcb).next = None;
            if let Some(tail) = self.tail {
                (*tail).next = Some(tcb);
            } else {
                self.head = Some(tcb);
            }
            self.tail = Some(tcb);
        }
    }

    fn pop_front(&mut self) -> Option<*mut TCB> {
        if let Some(head) = self.head {
            unsafe {
                let next = (*head).next;
                if let Some(next_ptr) = next {
                    (*next_ptr).prev = None;
                } else {
                    self.tail = None;
                }
                self.head = next;
                (*head).next = None;
                (*head).prev = None;
            }
            Some(head)
        } else {
            None
        }
    }
}

// 全局调度队列：每个优先级一个队列
// 注意：在 SMP 环境下，这应该是一个 Per-CPU 的结构，或者加全局锁
// 这里为了简化，使用全局锁保护所有队列
static READY_QUEUES: Mutex<[TcbQueue; MAX_PRIORITY]> =
    Mutex::new([const { TcbQueue::new() }; MAX_PRIORITY]);

static mut CURRENT_TCB: [Option<*mut TCB>; MAX_HARTS] = [None; MAX_HARTS];

fn kick_harts() {
    // 发送 IPI 给所有其他核心，唤醒它们或触发抢占
    // 这里的 mask 应该根据实际启用的 hart 计算，暂时广播给所有
    // 忽略错误
    let _ = sbi::send_ipi(0, 0); // mask=0, base=0 usually means all? No.
    // SBI v0.2: hart_mask, hart_mask_base.
    // To send to all harts: mask pointer? No, it's a bitmask if < XLEN.
    // But sbi_send_ipi takes a pointer in newer spec?
    // The implementation in sbi.rs uses legacy/v0.1 style or v0.2 direct value?
    // sbi.rs: sbi_call(SBI_EXT_IPI, 0, hart_mask, hart_mask_base, 0)
    // If we want to broadcast, we usually need to know the topology.
    // For now, let's assume a small number of harts and use a mask.
    // Assuming MAX_HARTS <= 64.
    let mask = (1 << MAX_HARTS) - 1;
    let _ = sbi::send_ipi(mask, 0);
}

/// 将线程加入调度队列
pub fn add_thread(tcb: &mut TCB) {
    let mut queues = READY_QUEUES.lock();
    let prio = tcb.priority as usize;

    // 确保状态正确
    if tcb.state == ThreadState::Ready {
        queues[prio].push_back(tcb as *mut _);
    }

    // 唤醒其他核心
    drop(queues);
    kick_harts();
}

/// 核心调度循环
/// 永远不会返回
pub fn scheduler() -> ! {
    loop {
        // 1. 关闭中断以保护调度逻辑
        // 在 RISC-V 中，这通常在进入异常处理时自动完成，但在 idle loop 中需要手动管理
        unsafe { sstatus::clear_sie() };

        let mut next_thread: Option<*mut TCB> = None;

        // 2. 寻找最高优先级的 Ready 线程
        {
            let mut queues = READY_QUEUES.lock();
            // 从最高优先级 (255) 向下遍历
            for prio in (0..MAX_PRIORITY).rev() {
                if let Some(tcb_ptr) = queues[prio].pop_front() {
                    next_thread = Some(tcb_ptr);
                    break;
                }
            }
        }

        if let Some(tcb_ptr) = next_thread {
            let tcb = unsafe { &mut *tcb_ptr };

            // 更新状态
            tcb.state = ThreadState::Running;

            // 获取当前 CPU 的 Hart 结构
            let hart = hart::get();
            // 设置当前运行的线程
            set_current(tcb_ptr);

            // 执行上下文切换：从当前 CPU 的 idle context 切换到线程 context
            unsafe {
                switch_context(&mut hart.context, &mut tcb.context);
            }
            set_current(core::ptr::null_mut());

            // --- 线程返回 ---
            // 当线程被抢占或主动 yield 后，会回到这里
            set_current(tcb_ptr);
        } else {
            // 没有可运行的线程，进入低功耗等待
            unsafe {
                riscv::register::sstatus::set_sie();
                riscv::asm::wfi();
            }
        }
    }
}

/// 主动放弃 CPU (Yield)
/// 将当前线程放回 Ready 队列末尾，并触发调度
pub fn yield_proc() {
    let tcb_ptr = match current() {
        Some(ptr) => ptr,
        None => return,
    };
    let mut context = hart::get().context;

    let tcb = unsafe { &mut *tcb_ptr };

    // 只有 Running 状态的线程才能 yield
    if tcb.state == ThreadState::Running {
        tcb.state = ThreadState::Ready;
        add_thread(tcb); // 放回队列末尾
    }

    // 切换回调度器 (context)
    unsafe {
        switch_context(&mut tcb.context, &mut context);
    }
}

/// 阻塞当前线程
/// 线程状态必须在调用此函数前被设置为 BlockedSend / BlockedRecv / Inactive
pub fn block_current_thread() {
    let tcb_ptr = match current() {
        Some(ptr) => ptr,
        None => return,
    };
    let mut context = hart::get().context;

    let tcb = unsafe { &mut *tcb_ptr };

    // 确保线程不再是 Running 状态
    assert!(
        tcb.state != ThreadState::Running,
        "Thread must set block state before calling block()"
    );

    // 直接切换回调度器，不加入 Ready 队列
    unsafe {
        switch_context(&mut tcb.context, &mut context);
    }
}

/// 唤醒指定线程
/// 将线程状态设置为 Ready 并加入调度队列
pub fn wake_up(tcb: &mut TCB) {
    if tcb.state != ThreadState::Ready && tcb.state != ThreadState::Running {
        tcb.state = ThreadState::Ready;
        add_thread(tcb);

        // 如果被唤醒线程优先级高于当前线程，触发抢占 (reschedule)
        // 目前 add_thread 已经发送了 IPI，会触发其他核心的调度检查
        // TODO: 对于当前核心，如果需要抢占，应该在这里检查并 yield
        // 但为了简化，暂时依赖 IPI 或时间片轮转
    }
}

/// 触发重新调度
/// 抢占当前线程，进入调度器
/// 通常在修改线程优先级后调用
pub fn reschedule() {
    let tcb_ptr = match current() {
        Some(ptr) => ptr,
        None => return,
    };
    let mut context = hart::get().context;
    let tcb = unsafe { &mut *tcb_ptr };
    // 将当前线程状态设置为 Ready 并加入队列
    if tcb.state == ThreadState::Running {
        tcb.state = ThreadState::Ready;
        add_thread(tcb);
    }
    // 切换回调度器
    unsafe {
        switch_context(&mut tcb.context, &mut context);
    }
}

pub fn current() -> Option<*mut TCB> {
    let hart = hart::getid();
    let tcb_ptr = unsafe { CURRENT_TCB[hart] };
    if let Some(ptr) = tcb_ptr { Some(ptr) } else { None }
}

fn set_current(tcb_ptr: *mut TCB) {
    let hart = hart::getid();
    unsafe {
        CURRENT_TCB[hart] = Some(tcb_ptr);
    }
}
